use aphyn_render::{
    FilterMode, RenderApiResource, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat, WgslComposer,
    clear_color_target,
};
use aphyn_render_core::{
    GpuBinding, GpuBlendState, GpuCullMode, GpuPassOps, GpuPrimitiveTopology, GpuRenderPass, GpuSampler, GpuShader, GpuShaderDesc,
};

pub(crate) struct RenderTarget {
    pub texture: StandardTexture,
    sampler: GpuSampler,
    shader: GpuShader,
}

impl RenderTarget {
    pub fn new(render_api: &RenderApiResource) -> Self {
        let core = render_api.core();

        let texture = render_api.create_texture(
            FilterMode::Nearest,
            render_api.size(),
            &[],
            StandardTextureAssetMetadata {
                format: StandardTextureFormat::Rgba16Float,
                ..Default::default()
            },
        );

        let sampler = core.create_sampler(Some("Render Surface Sampler"), FilterMode::Nearest);

        let code = WgslComposer::with_engine_libraries()
            .compose("copy_texture.wgsl", include_str!("./assets/copy_texture.wgsl"), &[])
            .unwrap();

        let shader = core
            .create_shader(
                Some("copy_texture.wgsl"),
                &code,
                GpuShaderDesc {
                    topology: GpuPrimitiveTopology::TriangleList,
                    cull_mode: GpuCullMode::Back,
                    blend: GpuBlendState::REPLACE,
                    ..Default::default()
                },
            )
            .unwrap();

        Self {
            texture,
            sampler,
            shader,
        }
    }

    pub fn size(&self) -> [u32; 2] {
        self.texture.size()
    }

    pub fn clear(&self, render_api: &RenderApiResource) {
        clear_color_target(render_api, &self.texture, [0.0; 4]);
    }

    pub fn render_to_surface(&self, render_api: &RenderApiResource) {
        render_api.core().draw_to_surface(
            &GpuRenderPass {
                shader: &self.shader,
                target: None,
                depth: None,
                bind_groups: &[&[
                    (
                        0,
                        GpuBinding::Texture {
                            texture: self.texture.texture(),
                            mip_level: None,
                        },
                    ),
                    (1, GpuBinding::Sampler(&self.sampler)),
                ]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            GpuPassOps::Clear([0.0; 4]),
            0..3,
            0..1,
        );
    }
}
