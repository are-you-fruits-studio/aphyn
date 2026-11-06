use std::ffi::c_void;

use aphyn_ecs::{TypesRegistryAccessFfi, TypesRegistryCache, WorldDataMut, WorldDataUnsafeFfi};
use aphyn_ffi::{FfiDroppable, FfiVec};
use aphyn_render::{MaterialData, RenderApiResource};

use crate::{MaterialBatch, RenderPassCtx, RenderPassEntry, RenderPassWriter};

pub trait RenderFeature: Send + Sync + 'static {
    type Pass: Send + Sync + 'static;

    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self::Pass>);
    fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx);
}

#[repr(C)]
pub(crate) struct RenderSlotVtable {
    pub(crate) add_passes: unsafe extern "C-unwind" fn(
        this: *mut c_void,
        slot: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        entries: *mut FfiVec<RenderPassEntry>,
        batches: *const MaterialBatch,
        batches_len: u64,
    ),
    pub(crate) run_pass: unsafe extern "C-unwind" fn(
        this: *mut c_void,
        index: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        render_api: *const RenderApiResource,
    ),
    pub(crate) clear_passes: unsafe extern "C-unwind" fn(this: *mut c_void),
    // Set only for material templates.
    pub(crate) default_data: Option<unsafe extern "C-unwind" fn(this: *mut c_void, out: *mut MaterialData)>,
}

#[repr(C)]
pub(crate) struct RenderSlot {
    pub(crate) data: FfiDroppable,
    pub(crate) vtable: &'static RenderSlotVtable,
}

struct FeatureSlotState<F: RenderFeature> {
    feature: F,
    passes: FfiVec<F::Pass>,
    types: Option<TypesRegistryCache>,
}

pub(crate) unsafe fn slot_world<'a>(
    cache: &'a mut Option<TypesRegistryCache>,
    world: *mut WorldDataUnsafeFfi,
    types: *const TypesRegistryAccessFfi,
) -> WorldDataMut<'a> {
    unsafe {
        let cache = cache.get_or_insert_with(|| TypesRegistryCache::new((*types).clone()));

        WorldDataMut::new(world, cache)
    }
}

impl RenderSlot {
    pub(crate) fn from_feature<F: RenderFeature>(feature: F) -> Self {
        unsafe extern "C-unwind" fn ffi_add_passes<F: RenderFeature>(
            this: *mut c_void,
            slot: u32,
            world: *mut WorldDataUnsafeFfi,
            types: *const TypesRegistryAccessFfi,
            entries: *mut FfiVec<RenderPassEntry>,
            _batches: *const MaterialBatch,
            _batches_len: u64,
        ) {
            unsafe {
                let FeatureSlotState {
                    feature,
                    passes,
                    types: cache,
                } = &mut *(this as *mut FeatureSlotState<F>);
                let world = slot_world(cache, world, types);
                let mut writer = RenderPassWriter::new(slot, passes, &mut *entries);

                feature.add_passes(world, &mut writer);
            }
        }
        unsafe extern "C-unwind" fn ffi_run_pass<F: RenderFeature>(
            this: *mut c_void,
            index: u32,
            world: *mut WorldDataUnsafeFfi,
            types: *const TypesRegistryAccessFfi,
            render_api: *const RenderApiResource,
        ) {
            unsafe {
                let FeatureSlotState {
                    feature,
                    passes,
                    types: cache,
                } = &mut *(this as *mut FeatureSlotState<F>);
                let mut ctx = RenderPassCtx {
                    render_api: &*render_api,
                    world: slot_world(cache, world, types),
                };

                feature.run_pass(&passes[index as u64], &mut ctx);
            }
        }
        unsafe extern "C-unwind" fn ffi_clear_passes<F: RenderFeature>(this: *mut c_void) {
            unsafe {
                (*(this as *mut FeatureSlotState<F>)).passes.clear();
            }
        }

        Self {
            data: FfiDroppable::new(FeatureSlotState::<F> {
                feature,
                passes: FfiVec::new(),
                types: None,
            }),
            vtable: &RenderSlotVtable {
                add_passes: ffi_add_passes::<F>,
                run_pass: ffi_run_pass::<F>,
                clear_passes: ffi_clear_passes::<F>,
                default_data: None,
            },
        }
    }

    pub(crate) unsafe fn add_passes(
        &mut self,
        slot: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        entries: &mut FfiVec<RenderPassEntry>,
        batches: &[MaterialBatch],
    ) {
        unsafe {
            (self.vtable.add_passes)(
                self.data.get(),
                slot,
                world,
                types,
                entries,
                batches.as_ptr(),
                batches.len() as u64,
            )
        }
    }

    pub(crate) fn is_template(&self) -> bool {
        self.vtable.default_data.is_some()
    }

    pub(crate) fn default_data(&self) -> Option<MaterialData> {
        let default_data = self.vtable.default_data?;
        let mut data = MaterialData::new();
        unsafe { default_data(self.data.get(), &mut data) };
        Some(data)
    }

    pub(crate) unsafe fn run_pass(
        &mut self,
        index: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        render_api: &RenderApiResource,
    ) {
        unsafe { (self.vtable.run_pass)(self.data.get(), index, world, types, render_api) }
    }

    pub(crate) fn clear_passes(&mut self) {
        unsafe { (self.vtable.clear_passes)(self.data.get()) }
    }

    #[cfg(test)]
    pub(crate) unsafe fn passes<F: RenderFeature>(&self) -> &FfiVec<F::Pass> {
        unsafe { &(*(self.data.get() as *const FeatureSlotState<F>)).passes }
    }
}

// Safety. Slots are only built from `RenderFeature` state, which is `Send + Sync`.
unsafe impl Send for RenderSlot {}
unsafe impl Sync for RenderSlot {}
