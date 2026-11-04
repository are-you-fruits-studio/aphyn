use aphyn_render_core::{GpuShader, RenderApi};

#[repr(C)]
#[derive(Clone)]
pub struct StandardShader {
    shader: GpuShader,
}

impl StandardShader {
    pub(crate) fn new(core: &RenderApi, name: &str, source: &str) -> Option<Self> {
        let shader = core.create_shader(Some(name), source, crate::standard_shader_desc(false))?;

        Some(Self { shader })
    }

    pub fn shader(&self) -> &GpuShader {
        &self.shader
    }
}
