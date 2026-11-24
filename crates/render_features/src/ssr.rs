use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_math::Mat4;
use aphyn_render::{CurrentRenderingResource, FilterMode, RenderApiResource, StandardSsrUniform};
use aphyn_render_core::{
    GpuCompareFunction, GpuPassOps, GpuRenderPass, GpuRenderPassDepth, GpuResourceId, GpuSampler, GpuShader, GpuShaderDesc,
    GpuTexture, GpuTextureFormat, RenderApi,
};

use crate::{
    DepthTextureResource, GlobalBindingsResource, RENDER_ORDER_REFLECTIONS_DEPTH, RENDER_ORDER_REFLECTIONS_HISTORY, RenderFeature,
    RenderPassCtx, RenderPassWriter, SsrResource, SsrTexturesResource, create_engine_shader, texture_binding,
};

// `add_passes` calls a history may go unused before it is dropped
const HISTORY_UNUSED_CALLS_MAX: u64 = 64;

struct SsrHistory {
    target: GpuResourceId,
    color: GpuTexture,
    world_to_clip_previous: Mat4<f32>,
    is_valid: bool,
    last_used_call: u64,
}

struct SsrGpu {
    shader_depth_copy: GpuShader,
    sampler: GpuSampler,
    texture_depth_off: GpuTexture,
    texture_history_off: GpuTexture,
}

impl SsrGpu {
    fn new(core: &RenderApi) -> Self {
        let texture_depth_off = core.create_texture(Some("ssr depth off"), [1, 1], GpuTextureFormat::Depth32Float, 1);
        core.clear_depth_texture(&texture_depth_off, 0, 0.0);

        let texture_history_off = core.create_texture(Some("ssr history off"), [1, 1], GpuTextureFormat::Rgba16Float, 1);
        core.clear_color_texture(&texture_history_off, 0, [0.0; 4]);

        Self {
            shader_depth_copy: create_engine_shader(
                core,
                "shader_ssr_depth_copy.wgsl",
                include_str!("./assets/shader_ssr_depth_copy.wgsl"),
                GpuShaderDesc {
                    depth_write: true,
                    depth_compare: GpuCompareFunction::Always,
                    ..Default::default()
                },
            ),
            sampler: core.create_sampler(Some("ssr history"), FilterMode::Linear),
            texture_depth_off,
            texture_history_off,
        }
    }
}

#[derive(Default)]
pub struct SsrRenderFeature {
    gpu: Option<SsrGpu>,
    texture_depth: Option<GpuTexture>,
    histories: Vec<SsrHistory>,
    calls: u64,
}

impl RenderFeature for SsrRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let mut res = world.into_tuple_mut().0;

        let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
            return;
        };
        let Some(current) = res.get::<CurrentRenderingResource>() else {
            return;
        };
        let core = render_api.core();
        let settings = res.get::<SsrResource>().copied().filter(|settings| settings.is_enabled);
        let gpu = self.gpu.get_or_insert_with(|| SsrGpu::new(core));

        self.calls += 1;

        let (uniform, textures) = match settings {
            None => {
                self.histories.clear();
                self.texture_depth = None;

                let textures = SsrTexturesResource {
                    depth: gpu.texture_depth_off.clone(),
                    history: gpu.texture_history_off.clone(),
                    sampler: gpu.sampler.clone(),
                };

                (StandardSsrUniform::default(), textures)
            }
            Some(settings) => {
                let target = current.target.texture();
                let size = target.size();
                let calls = self.calls;

                self.histories
                    .retain(|history| calls - history.last_used_call <= HISTORY_UNUSED_CALLS_MAX);

                let index = match self.histories.iter().position(|history| history.target == target.id()) {
                    Some(index) => index,
                    None => {
                        self.histories.push(SsrHistory {
                            target: target.id(),
                            color: create_history_texture(core, target),
                            world_to_clip_previous: Mat4::IDENTITY,
                            is_valid: false,
                            last_used_call: calls,
                        });
                        self.histories.len() - 1
                    }
                };

                let history = &mut self.histories[index];
                history.last_used_call = calls;

                if history.color.size() != size || history.color.format() != target.format() {
                    history.color = create_history_texture(core, target);
                    history.is_valid = false;
                }

                if self.texture_depth.as_ref().is_none_or(|texture| texture.size() != size) {
                    self.texture_depth = Some(core.create_texture(Some("ssr depth"), size, GpuTextureFormat::Depth32Float, 1));
                }

                let uniform = StandardSsrUniform {
                    world_to_clip_previous: history.world_to_clip_previous,
                    max_distance: settings.max_distance,
                    max_steps: settings.max_steps,
                    thickness: settings.thickness,
                    max_roughness: settings.max_roughness,
                    intensity: settings.intensity,
                    edge_fade: settings.edge_fade,
                    is_enabled: 1,
                    has_history: history.is_valid as u32,
                    depth_bias: settings.depth_bias,
                    _padding: [0; 3],
                };
                let textures = SsrTexturesResource {
                    depth: self.texture_depth.clone().unwrap(),
                    history: history.color.clone(),
                    sampler: gpu.sampler.clone(),
                };

                let world_to_clip = current.world_to_clip();

                passes.add(RENDER_ORDER_REFLECTIONS_DEPTH, |this, ctx| this.copy_depth(ctx));
                passes.add(RENDER_ORDER_REFLECTIONS_HISTORY, move |this, ctx| {
                    this.capture_history(index, world_to_clip, ctx)
                });

                (uniform, textures)
            }
        };

        if let Some(globals) = res.get::<GlobalBindingsResource>() {
            render_api.write_ssr_uniform(&globals.bindings, uniform);
        }

        if let Some(resource) = res.get_mut::<SsrTexturesResource>() {
            *resource = textures;
        } else {
            res.insert(textures);
        }
    }
}

fn create_history_texture(core: &RenderApi, target: &GpuTexture) -> GpuTexture {
    let [width, height] = target.size();
    let mip_level_count = width.max(height).max(1).ilog2() as usize + 1;

    core.create_texture(Some("ssr history"), target.size(), target.format(), mip_level_count)
}

impl SsrRenderFeature {
    fn copy_depth(&mut self, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let (Some(gpu), Some(texture_depth), Some(depth)) = (
            self.gpu.as_ref(),
            self.texture_depth.as_ref(),
            ctx.world.resources().get::<DepthTextureResource>(),
        ) else {
            return;
        };

        core.draw(
            &GpuRenderPass {
                shader: &gpu.shader_depth_copy,
                target: None,
                depth: Some(GpuRenderPassDepth {
                    view: texture_depth,
                    mip_level: 0,
                    ops: GpuPassOps::Clear(0.0),
                }),
                bind_groups: &[&[(0, texture_binding(depth.texture.texture()))]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            0..3,
            0..1,
        );
    }

    fn capture_history(&mut self, index: usize, world_to_clip: Mat4<f32>, ctx: &mut RenderPassCtx) {
        let (Some(history), Some(current)) = (
            self.histories.get_mut(index),
            ctx.world.resources().get::<CurrentRenderingResource>(),
        ) else {
            return;
        };

        if !ctx.render_api.core().copy_texture(current.target.texture(), &history.color) {
            return;
        }

        ctx.render_api.generate_mipmaps(&history.color);
        history.world_to_clip_previous = world_to_clip;
        history.is_valid = true;
    }
}
