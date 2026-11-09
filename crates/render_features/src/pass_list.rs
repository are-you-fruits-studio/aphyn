use std::{ffi::c_void, marker::PhantomData};

use aphyn_ecs::{TypesRegistryAccessFfi, WorldDataMut, WorldDataUnsafeFfi};
use aphyn_ffi::{FfiFnOnceVec, FfiVec};
use aphyn_render::RenderApiResource;

use crate::{SlotState, slot_world};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RenderPassEntry {
    pub order: u64,
    pub slot: u32,
    pub index: u32,
}

#[repr(C)]
pub(crate) struct RenderPassInput {
    pub(crate) this: *mut c_void,
    pub(crate) world: *mut WorldDataUnsafeFfi,
    pub(crate) types: *const TypesRegistryAccessFfi,
    pub(crate) render_api: *const RenderApiResource,
}

pub(crate) type RenderPasses = FfiFnOnceVec<'static, RenderPassInput, ()>;

pub struct RenderPassWriter<'a, T: ?Sized> {
    slot: u32,
    passes: &'a mut RenderPasses,
    entries: &'a mut FfiVec<RenderPassEntry>,
    _phantom: PhantomData<fn(&mut T)>,
}

impl<'a, T: 'static> RenderPassWriter<'a, T> {
    pub(crate) fn new(slot: u32, passes: &'a mut RenderPasses, entries: &'a mut FfiVec<RenderPassEntry>) -> Self {
        Self {
            slot,
            passes,
            entries,
            _phantom: PhantomData,
        }
    }

    pub fn add<P: FnOnce(&mut T, &mut RenderPassCtx) + Send + 'static>(&mut self, order: u64, pass: P) {
        self.push(order, move |input: RenderPassInput| unsafe {
            let SlotState { value, types } = &mut *(input.this as *mut SlotState<T>);
            let mut ctx = RenderPassCtx {
                render_api: &*input.render_api,
                world: slot_world(types, input.world, input.types),
            };

            pass(value, &mut ctx)
        });
    }
}

impl<'a, T: ?Sized> RenderPassWriter<'a, T> {
    pub(crate) fn push<P: FnOnce(RenderPassInput) + Send + 'static>(&mut self, order: u64, pass: P) {
        let index = self.passes.push(pass);
        let index = u32::try_from(index).expect("too many render passes");

        self.entries.push(RenderPassEntry {
            order,
            slot: self.slot,
            index,
        });
    }
}

pub struct RenderPassCtx<'a> {
    pub render_api: &'a RenderApiResource,
    pub world: WorldDataMut<'a>,
}
