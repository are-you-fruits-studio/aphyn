use aphyn_ecs::WorldDataMut;
use aphyn_ffi::FfiVec;
use aphyn_render::RenderApiResource;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RenderPassEntry {
    pub order: u64,
    pub slot: u32,
    pub index: u32,
}

pub struct RenderPassWriter<'a, P> {
    slot: u32,
    passes: &'a mut FfiVec<P>,
    entries: &'a mut FfiVec<RenderPassEntry>,
}

impl<'a, P> RenderPassWriter<'a, P> {
    pub(crate) fn new(slot: u32, passes: &'a mut FfiVec<P>, entries: &'a mut FfiVec<RenderPassEntry>) -> Self {
        Self { slot, passes, entries }
    }

    pub fn add(&mut self, order: u64, pass: P) {
        let index = u32::try_from(self.passes.len()).expect("too many render passes in one slot");

        self.passes.push(pass);
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
