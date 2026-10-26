use fruits_ffi::FfiDroppable;
use wgpu::{RenderPipeline, ShaderModule, ShaderModuleDescriptor, ShaderSource};

use crate::{DEPTH_TEXTURE_FORMAT, RENDER_TARGET_FORMAT, RenderState, StandardInstance, StandardVertex};

pub struct StandardShaderNative {
    pub module: ShaderModule,
    pub render_pipeline: RenderPipeline,
}

#[repr(C)]
pub struct StandardShader {
    native: FfiDroppable,
}

impl StandardShader {
    pub(crate) fn new(render_state: &RenderState, name: &str, source: &str) -> Option<Self> {
        render_state.device().push_error_scope(wgpu::ErrorFilter::Validation);

        let module = render_state.device().create_shader_module(ShaderModuleDescriptor {
            label: Some(name),
            source: ShaderSource::Wgsl(source.into()),
        });

        let render_pipeline = crate::create_render_pipeline(
            render_state.device(),
            Some(name),
            &render_state.render_data().pipeline_layout_standard,
            &module,
            &[StandardVertex::desc(), StandardInstance::desc()],
            wgpu::ColorTargetState {
                format: RENDER_TARGET_FORMAT,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            },
            Some(wgpu::DepthStencilState {
                bias: Default::default(),
                depth_compare: wgpu::CompareFunction::LessEqual,
                depth_write_enabled: true,
                format: DEPTH_TEXTURE_FORMAT,
                stencil: Default::default(),
            }),
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
        );

        if let Some(err) = pollster::block_on(render_state.device().pop_error_scope()) {
            println!("failed to create shader {name}: {err}");
            return None;
        }

        Some(Self {
            native: FfiDroppable::new(StandardShaderNative { module, render_pipeline }),
        })
    }

    pub unsafe fn native(&self) -> &StandardShaderNative {
        unsafe { &*(self.native.get() as *const StandardShaderNative) }
    }
}

unsafe impl Send for StandardShader where StandardShaderNative: Send {}
unsafe impl Sync for StandardShader where StandardShaderNative: Sync {}
