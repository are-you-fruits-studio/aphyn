use std::ffi::c_void;

use aphyn_ecs::{TypesRegistryAccessFfi, TypesRegistryCache, WorldDataMut, WorldDataUnsafeFfi};
use aphyn_ffi::{FfiDroppable, FfiVec};

use crate::{RenderPassEntry, RenderPassWriter, RenderPasses};

pub trait RenderFeature: Send + Sync + 'static {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>);
}

#[repr(C)]
pub(crate) struct RenderFeatureFfiVtable {
    pub(crate) add_passes: unsafe extern "C-unwind" fn(
        this: *mut c_void,
        slot: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        passes: *mut RenderPasses,
        entries: *mut FfiVec<RenderPassEntry>,
    ),
}

#[repr(C)]
pub(crate) struct RenderFeatureFfi {
    pub(crate) data: FfiDroppable,
    pub(crate) vtable: &'static RenderFeatureFfiVtable,
}

pub(crate) struct SlotState<T> {
    pub(crate) value: T,
    // todo: don't store TypesRegistryCache on RenderFeature behalf. Check how SystemFfi and SystemsHolderBuilderMut handle types cache.
    // Builder passes the same cache to all the systems registered in the same lib.
    // The same way the RenderFeature register ctx/builder should pass the types cache to all the RenderFeatures that were registered in the same lib.
    pub(crate) types: Option<TypesRegistryCache>,
}

impl<T> SlotState<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, types: None }
    }
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

impl RenderFeatureFfi {
    pub(crate) fn from_feature<F: RenderFeature>(feature: F) -> Self {
        unsafe extern "C-unwind" fn ffi_add_passes<F: RenderFeature>(
            this: *mut c_void,
            slot: u32,
            world: *mut WorldDataUnsafeFfi,
            types: *const TypesRegistryAccessFfi,
            passes: *mut RenderPasses,
            entries: *mut FfiVec<RenderPassEntry>,
        ) {
            unsafe {
                let SlotState { value, types: cache } = &mut *(this as *mut SlotState<F>);
                let world = slot_world(cache, world, types);
                let mut writer = RenderPassWriter::new(slot, &mut *passes, &mut *entries);

                value.add_passes(world, &mut writer);
            }
        }

        Self {
            data: FfiDroppable::new(SlotState::new(feature)),
            vtable: &RenderFeatureFfiVtable {
                add_passes: ffi_add_passes::<F>,
            },
        }
    }

    pub(crate) unsafe fn add_passes(
        &mut self,
        slot: u32,
        world: *mut WorldDataUnsafeFfi,
        types: *const TypesRegistryAccessFfi,
        passes: &mut RenderPasses,
        entries: &mut FfiVec<RenderPassEntry>,
    ) {
        unsafe { (self.vtable.add_passes)(self.data.get(), slot, world, types, passes, entries) }
    }

    pub(crate) fn state(&self) -> *mut c_void {
        self.data.get()
    }
}

// Safety. Slots are only built from `RenderFeature` state, which is `Send + Sync`.
unsafe impl Send for RenderFeatureFfi {}
unsafe impl Sync for RenderFeatureFfi {}
