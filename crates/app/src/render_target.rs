use aphyn_render_core::{
    CreateBindGroupEntry, CreateBindGroupLayoutEntry, RenderState, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat,
    WgslComposer, clear_color_target, create_bind_group, create_bind_group_layout, create_render_pipeline,
};
use wgpu::*;

pub(crate) struct RenderTarget {
    pub texture: StandardTexture,
    bind_group: BindGroup,
    render_pipeline: RenderPipeline,
}

impl RenderTarget {
    pub fn new(render_state: &RenderState) -> Self {
        let size = render_state.size();

        let texture = render_state.create_texture(
            FilterMode::Nearest,
            size,
            &[],
            StandardTextureAssetMetadata {
                format: StandardTextureFormat::Rgba16Float,
                ..Default::default()
            },
        );

        let sampler = render_state.device().create_sampler(&SamplerDescriptor {
            label: Some("Render Surface Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            compare: None,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        let bind_group_layout = create_bind_group_layout(
            render_state.device(),
            Some("Render Surface Bind Group Layout"),
            &[
                CreateBindGroupLayoutEntry::Texture,
                CreateBindGroupLayoutEntry::SamplerNonFiltering,
            ],
        );

        let bind_group = create_bind_group(
            render_state.device(),
            Some("Render Surface Bind Group"),
            &bind_group_layout,
            &[
                CreateBindGroupEntry::Texture(unsafe { &texture.native().view }),
                CreateBindGroupEntry::Sampler(&sampler),
            ],
        );

        let pipeline_layout = render_state.device().create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Surface Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = WgslComposer::with_engine_libraries()
            .create_shader_module(
                render_state.device(),
                "copy_texture.wgsl",
                include_str!("./assets/copy_texture.wgsl"),
                &[],
            )
            .unwrap();

        let render_pipeline = create_render_pipeline(
            render_state.device(),
            Some("Surface Render Pipeline"),
            &pipeline_layout,
            &shader,
            &[],
            ColorTargetState {
                format: render_state.surface_config_format(),
                blend: Some(BlendState::REPLACE),
                write_mask: ColorWrites::ALL,
            },
            None,
            PrimitiveTopology::TriangleList,
            Some(Face::Back),
        );

        Self {
            texture,
            bind_group,
            render_pipeline,
        }
    }

    pub fn size(&self) -> [u32; 2] {
        let size = unsafe { self.texture.native() }.texture.size();
        [size.width, size.height]
    }

    pub fn clear(&self, render_state: &RenderState) {
        clear_color_target(render_state, unsafe { &self.texture.native().view }, [0.0; 4]);
    }

    pub fn render_to_surface(&self, render_state: &RenderState) {
        let Some(surface_texture) = render_state.surface().get_current_texture().ok() else {
            return;
        };

        let view = &surface_texture.texture.create_view(&Default::default());

        let mut encoder = render_state.device().create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Surface Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Surface Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        render_state.queue().submit(std::iter::once(encoder.finish()));

        surface_texture.present();
    }
}
