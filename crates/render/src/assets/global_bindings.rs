use aphyn_render_core::{GpuUniformStorageBuffer, RenderApi};

use crate::{StandardGenericLight, StandardUniformGlobal};

pub const LIGHTS_COUNT_MAX: usize = 1024;

#[repr(C)]
pub struct StandardGlobalBindings {
    uniform_buffer: GpuUniformStorageBuffer,
    lights_buffer: GpuUniformStorageBuffer,
}

impl StandardGlobalBindings {
    pub(crate) fn new(core: &RenderApi) -> Self {
        let uniform_buffer = core.create_uniform_storage_buffer(Some("standard global uniform"), size_of::<StandardUniformGlobal>());
        let lights_buffer =
            core.create_uniform_storage_buffer(Some("standard lights"), LIGHTS_COUNT_MAX * size_of::<StandardGenericLight>());

        let this = Self {
            uniform_buffer,
            lights_buffer,
        };
        this.write_uniform(core, StandardUniformGlobal::default());

        this
    }

    pub(crate) fn write_uniform(&self, core: &RenderApi, uniform: StandardUniformGlobal) {
        crate::write_buffer_bytes(core, &self.uniform_buffer, 0, aphyn_utils::mem::as_bytes(&uniform));
    }

    pub(crate) fn write_lights_with(&self, core: &RenderApi, count: u32, f: impl FnMut(usize) -> StandardGenericLight) {
        let count = (count as usize).min(LIGHTS_COUNT_MAX);

        crate::write_buffer_elements_with(core, &self.lights_buffer, 0, count, f);
    }

    pub fn uniform_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.uniform_buffer
    }

    pub fn lights_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.lights_buffer
    }
}
