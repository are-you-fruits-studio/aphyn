use aphyn_ecs::{ResMut, WorldDataMut};
use aphyn_ffi::FfiVec;
use aphyn_math::{Mat4, Vec4};
use aphyn_render::{CurrentRenderingResource, RenderSpace, write_buffer_elements_with};
use aphyn_render_core::{
    GpuBinding, GpuBlendState, GpuCullMode, GpuPassOps, GpuPrimitiveTopology, GpuRenderPass, GpuRenderPassTarget, GpuShader,
    GpuShaderDesc, GpuUniformStorageBuffer,
};

use crate::{
    GIZMO_LINES_PER_DRAW_MAX, GizmosFrameResource, GizmosResource, RENDER_ORDER_GIZMOS, RenderFeature, RenderPassCtx,
    RenderPassWriter, ScreenSpaceResource, create_engine_shader, create_window_to_clip_matrix, fullscreen_shader_desc,
};

pub fn prepare_gizmos(mut gizmos: ResMut<GizmosResource>, mut frame: ResMut<GizmosFrameResource>) {
    frame.lines.clear();

    for (space, lines) in gizmos.spaces() {
        frame.lines.push((*space, lines.clone()));
        lines.clear();
    }
}

struct GizmosGpu {
    vertex_buffer: GpuUniformStorageBuffer,
    color_buffer: GpuUniformStorageBuffer,
    transform_buffer: GpuUniformStorageBuffer,
    shader: GpuShader,
}

#[derive(Default)]
pub struct GizmosRenderFeature {
    gpu: Option<GizmosGpu>,
}

impl RenderFeature for GizmosRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        let has_lines = world
            .resources()
            .get::<GizmosFrameResource>()
            .is_some_and(|frame| frame.lines.iter().any(|(_, lines)| !lines.is_empty()));

        if has_lines {
            passes.add(RENDER_ORDER_GIZMOS, |this, ctx| this.draw(ctx));
        }
    }
}

impl GizmosRenderFeature {
    fn draw(&mut self, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let resources = ctx.world.resources();

        let (Some(frame), Some(current)) = (
            resources.get::<GizmosFrameResource>(),
            resources.get::<CurrentRenderingResource>(),
        ) else {
            return;
        };
        let (near, far) = resources
            .get::<ScreenSpaceResource>()
            .map_or((-10_000.0, 10_000.0), |screen_space| (screen_space.near, screen_space.far));

        let gpu = self.gpu.get_or_insert_with(|| {
            let transform_buffer = core.create_uniform_storage_buffer(Some("gizmos transform"), size_of::<Mat4<f32>>());

            GizmosGpu {
                vertex_buffer: core.create_uniform_storage_buffer(
                    Some("gizmos vertices"),
                    GIZMO_LINES_PER_DRAW_MAX * size_of::<[Vec4<f32>; 2]>(),
                ),
                color_buffer: core
                    .create_uniform_storage_buffer(Some("gizmos colors"), GIZMO_LINES_PER_DRAW_MAX * size_of::<Vec4<f32>>()),
                transform_buffer,
                shader: create_engine_shader(
                    core,
                    "shader_gizmo.wgsl",
                    include_str!("./assets/shader_gizmo.wgsl"),
                    GpuShaderDesc {
                        topology: GpuPrimitiveTopology::LineList,
                        cull_mode: GpuCullMode::None,
                        ..fullscreen_shader_desc(GpuBlendState::REPLACE)
                    },
                ),
            }
        });

        let target = current.target.texture();
        let target_size = current.target_size();

        for (space, lines) in frame.lines.iter() {
            if lines.is_empty() {
                continue;
            }

            let transform = match space {
                RenderSpace::Clip => Mat4::<f32>::IDENTITY,
                RenderSpace::Window => create_window_to_clip_matrix(target_size[0] as f32, target_size[1] as f32, near, far),
                RenderSpace::World => current.world_to_clip(),
            };

            write_buffer_elements_with(core, &gpu.transform_buffer, 0, 1, |_| *transform.as_array());

            let lines: &FfiVec<_> = lines;

            for lines in lines.chunks(GIZMO_LINES_PER_DRAW_MAX) {
                let count = lines.len();

                write_buffer_elements_with(core, &gpu.vertex_buffer, 0, count, |i| {
                    let line = &lines[i];
                    [
                        Vec4::new(line.start.x, line.start.y, line.start.z, 1.0),
                        Vec4::new(line.end.x, line.end.y, line.end.z, 1.0),
                    ]
                });
                write_buffer_elements_with(core, &gpu.color_buffer, 0, count, |i| lines[i].color);

                core.draw(
                    &GpuRenderPass {
                        shader: &gpu.shader,
                        target: Some(GpuRenderPassTarget {
                            view: target,
                            mip_level: 0,
                            ops: GpuPassOps::Load,
                        }),
                        depth: None,
                        bind_groups: &[&[
                            (0, GpuBinding::Buffer(&gpu.vertex_buffer)),
                            (1, GpuBinding::Buffer(&gpu.color_buffer)),
                            (2, GpuBinding::Buffer(&gpu.transform_buffer)),
                        ]],
                        vertex_buffers: &[],
                        viewport: None,
                        scissor: None,
                    },
                    0..(count * 2),
                    0..1,
                );
            }
        }
    }
}
