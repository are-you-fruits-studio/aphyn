use aphyn_math::Mat4;
use aphyn_render_core::{GpuVertexBuffer, GpuVertexBufferLayout, GpuVertexBufferLayoutEntryTy, GpuVertexStepMode, RenderApi};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct StandardInstance {
    pub local_to_world: [[f32; 4]; 4],
}

impl StandardInstance {
    pub fn desc() -> GpuVertexBufferLayout {
        GpuVertexBufferLayout::new(
            GpuVertexStepMode::Instance,
            [
                (5, GpuVertexBufferLayoutEntryTy::Float4),
                (6, GpuVertexBufferLayoutEntryTy::Float4),
                (7, GpuVertexBufferLayoutEntryTy::Float4),
                (8, GpuVertexBufferLayoutEntryTy::Float4),
            ],
        )
        .unwrap()
    }
}

#[repr(C)]
pub struct StandardInstanceBuffer {
    buffer: GpuVertexBuffer,
    len: u32,
}

impl StandardInstanceBuffer {
    pub(crate) fn new(core: &RenderApi, len: u32) -> Self {
        Self {
            buffer: core.create_vertex_buffer(Some("standard instances"), len as usize, StandardInstance::desc()),
            len,
        }
    }

    pub(crate) fn write(&self, core: &RenderApi, start: u32, matrices: &[Mat4<f32>]) {
        let [start, count] = self.clamp_range(start, matrices.len().min(u32::MAX as usize) as u32);
        let matrices = &matrices[..count as usize];

        let offset = start as usize * size_of::<Mat4<f32>>();
        crate::write_buffer_bytes(core, &self.buffer, offset, aphyn_utils::mem::as_bytes_slice(matrices));
    }

    pub(crate) fn write_with(&self, core: &RenderApi, start: u32, count: u32, mut f: impl FnMut(u32) -> Mat4<f32>) {
        let [start, count] = self.clamp_range(start, count);

        crate::write_buffer_elements_with(core, &self.buffer, start as usize, count as usize, |i| f(i as u32));
    }

    pub fn buffer(&self) -> &GpuVertexBuffer {
        &self.buffer
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn clamp_range(&self, start: u32, count: u32) -> [u32; 2] {
        let start = start.min(self.len);
        [start, count.min(self.len - start)]
    }
}
