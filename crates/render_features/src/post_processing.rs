use aphyn_ecs::WorldDataMut;
use aphyn_math::{Vec2, Vec4};
use aphyn_render::{CurrentRenderingResource, FilterMode, RENDER_TARGET_FORMAT, write_buffer_elements_with};
use aphyn_render_core::{
    GpuBinding, GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuPassOps, GpuSampler, GpuShader, GpuTexture,
    GpuTextureFormat, GpuUniformStorageBuffer, GpuViewport, RenderApi,
};
use aphyn_utils::mem::AllBitsInit;

use crate::{
    BloomResource, ColorGradingResource, ExposureResource, RENDER_ORDER_BLOOM, RENDER_ORDER_COLOR_GRADING, RENDER_ORDER_EXPOSURE,
    RenderFeature, RenderPassCtx, RenderPassWriter, create_engine_shader, draw_fullscreen, fullscreen_shader_desc, texture_binding,
};

pub const BLUR_LAYERS_COUNT: usize = 4;

fn current_target(ctx: &RenderPassCtx) -> Option<GpuTexture> {
    Some(
        ctx.world
            .resources()
            .get::<CurrentRenderingResource>()?
            .target
            .texture()
            .clone(),
    )
}

struct ExposureGpu {
    buffer_uniform: GpuUniformStorageBuffer,
    texture: GpuTexture,
    sampler: GpuSampler,
    shader: GpuShader,
}

#[derive(Default)]
pub struct ExposureRenderFeature {
    gpu: Option<ExposureGpu>,
}

impl RenderFeature for ExposureRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        let Some(exposure) = world
            .resources()
            .get::<ExposureResource>()
            .filter(|exposure| exposure.is_enabled)
        else {
            return;
        };

        let multiplier = 2.0_f32.powf(exposure.exposure);

        passes.add(RENDER_ORDER_EXPOSURE, move |this, ctx| this.draw(multiplier, ctx));
    }
}

impl ExposureRenderFeature {
    fn draw(&mut self, multiplier: f32, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let Some(target) = current_target(ctx) else {
            return;
        };

        let gpu = self.gpu.get_or_insert_with(|| {
            let buffer_uniform = core.create_uniform_storage_buffer(Some("exposure uniform"), size_of::<f32>());

            ExposureGpu {
                buffer_uniform,
                texture: core.create_texture(Some("exposure temp"), target.size(), RENDER_TARGET_FORMAT, 1),
                sampler: core.create_sampler(Some("exposure"), FilterMode::Nearest),
                shader: create_engine_shader(
                    core,
                    "shader_exposure.wgsl",
                    include_str!("./assets/shader_exposure.wgsl"),
                    fullscreen_shader_desc(GpuBlendState::REPLACE),
                ),
            }
        });

        if gpu.texture.size() != target.size() {
            gpu.texture = core.create_texture(Some("exposure temp"), target.size(), RENDER_TARGET_FORMAT, 1);
        }

        write_buffer_elements_with(core, &gpu.buffer_uniform, 0, 1, |_| multiplier);

        draw_fullscreen(
            core,
            &gpu.shader,
            &gpu.texture,
            GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
            &[
                (0, texture_binding(&target)),
                (1, GpuBinding::Sampler(&gpu.sampler)),
                (2, GpuBinding::Buffer(&gpu.buffer_uniform)),
            ],
            None,
            0..1,
        );

        core.copy_texture(&gpu.texture, &target);
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct BloomUniform {
    threshold: Vec2<f32>,
    intensity: f32,
    _padding0: f32,
}

unsafe impl AllBitsInit for BloomUniform {}

#[repr(C)]
struct BloomStorageLayer {
    uv_scale_offset: Vec4<f32>,
    direction: Vec2<f32>,
    intensity: f32,
    _padding0: f32,
}

unsafe impl AllBitsInit for BloomStorageLayer {}

struct BloomGpu {
    buffer_storage_layers: GpuUniformStorageBuffer,
    buffer_uniform: GpuUniformStorageBuffer,
    textures: [GpuTexture; 3],
    sampler_linear: GpuSampler,
    sampler_nearest: GpuSampler,
    shader_gather: GpuShader,
    shader_blur: GpuShader,
    shader_downscale: GpuShader,
    shader_apply: GpuShader,
}

impl BloomGpu {
    fn create_textures(core: &RenderApi, size: [usize; 2]) -> [GpuTexture; 3] {
        std::array::from_fn(|i| core.create_texture(Some(&format!("bloom buffer {i}")), size, GpuTextureFormat::Rgba16Float, 1))
    }

    fn new(core: &RenderApi, size: [usize; 2]) -> Self {
        let mut blur_layers: [f32; BLUR_LAYERS_COUNT] = [8.0, 4.0, 2.0, 1.0];
        let layers_intensity_sum = blur_layers.into_iter().sum::<f32>();
        blur_layers.iter_mut().for_each(|i| *i /= layers_intensity_sum);

        let storage_layers = blur_layers
            .into_iter()
            .enumerate()
            .flat_map(|(blur_layer, intensity)| {
                let uv_scale = 1.0 / (1 << blur_layer) as f32;
                let uv_scale_offset = Vec4::<f32>::new(uv_scale, uv_scale, 0.0, 0.0);

                [Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)].map(|direction| BloomStorageLayer {
                    intensity,
                    uv_scale_offset,
                    direction,
                    _padding0: 0.0,
                })
            })
            .collect::<Vec<_>>();

        let buffer_storage_layers = core.create_uniform_storage_buffer(
            Some("bloom storage layers"),
            storage_layers.len() * size_of::<BloomStorageLayer>(),
        );
        core.write_uniform_storage_buffer_with(&buffer_storage_layers, 0, buffer_storage_layers.size(), |bytes| {
            bytes.copy_from_slice(aphyn_utils::mem::as_bytes_slice(&storage_layers))
        });

        let buffer_uniform = core.create_uniform_storage_buffer(Some("bloom uniform"), size_of::<BloomUniform>());

        let fullscreen = |name: &str, source: &str, blend| create_engine_shader(core, name, source, fullscreen_shader_desc(blend));

        let additive = GpuBlendComponent {
            src_factor: GpuBlendFactor::One,
            dst_factor: GpuBlendFactor::One,
            operation: GpuBlendOperation::Add,
        };

        Self {
            buffer_storage_layers,
            buffer_uniform,
            textures: Self::create_textures(core, size),
            sampler_linear: core.create_sampler(Some("bloom linear"), FilterMode::Linear),
            sampler_nearest: core.create_sampler(Some("bloom nearest"), FilterMode::Nearest),
            shader_gather: fullscreen(
                "shader_bloom_gather_threshold.wgsl",
                include_str!("./assets/shader_bloom_gather_threshold.wgsl"),
                GpuBlendState::REPLACE,
            ),
            shader_blur: fullscreen(
                "shader_bloom_blur.wgsl",
                include_str!("./assets/shader_bloom_blur.wgsl"),
                GpuBlendState::REPLACE,
            ),
            shader_downscale: fullscreen(
                "shader_bloom_downscale.wgsl",
                include_str!("./assets/shader_bloom_downscale.wgsl"),
                GpuBlendState::REPLACE,
            ),
            shader_apply: fullscreen(
                "shader_bloom_apply.wgsl",
                include_str!("./assets/shader_bloom_apply.wgsl"),
                GpuBlendState {
                    color: additive,
                    alpha: additive,
                },
            ),
        }
    }
}

#[derive(Default)]
pub struct BloomRenderFeature {
    gpu: Option<BloomGpu>,
}

// todo: fix pixelated bloom
impl RenderFeature for BloomRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        let Some(bloom) = world.resources().get::<BloomResource>().filter(|bloom| bloom.is_enabled) else {
            return;
        };

        let uniform = BloomUniform {
            threshold: Vec2::new(bloom.threshold, bloom.threshold_softening),
            intensity: bloom.intensity,
            _padding0: 0.0,
        };

        passes.add(RENDER_ORDER_BLOOM, move |this, ctx| this.draw(uniform, ctx));
    }
}

impl BloomRenderFeature {
    fn draw(&mut self, uniform: BloomUniform, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let Some(target) = current_target(ctx) else {
            return;
        };

        let gpu = self.gpu.get_or_insert_with(|| BloomGpu::new(core, target.size()));

        if gpu.textures.iter().any(|texture| texture.size() != target.size()) {
            gpu.textures = BloomGpu::create_textures(core, target.size());
        }

        write_buffer_elements_with(core, &gpu.buffer_uniform, 0, 1, |_| uniform);

        let BloomGpu {
            buffer_storage_layers,
            buffer_uniform,
            textures,
            sampler_linear,
            sampler_nearest,
            shader_gather,
            shader_blur,
            shader_downscale,
            shader_apply,
        } = gpu;

        let screen_size = target.size().map(|i| i as f32);

        let bindings = |texture, sampler| {
            [
                (0, texture_binding(texture)),
                (1, GpuBinding::Sampler(sampler)),
                (2, GpuBinding::Buffer(&*buffer_uniform)),
                (3, GpuBinding::Buffer(&*buffer_storage_layers)),
            ]
        };

        let clear_black = GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]);

        draw_fullscreen(
            core,
            shader_gather,
            &textures[0],
            clear_black,
            &bindings(&target, sampler_nearest),
            None,
            0..1,
        );

        let instance_id_to_range = |i| i..(i + 1);

        let viewport_by_layer = |blur_layer: usize| {
            let downscale_uv_output = 1.0 / (1 << blur_layer) as f32;

            GpuViewport {
                x: 0.0,
                y: 0.0,
                width: screen_size[0] * downscale_uv_output,
                height: screen_size[1] * downscale_uv_output,
                min_depth: 0.0,
                max_depth: 1.0,
            }
        };

        let instance_id_by_layer = |blur_layer: usize| blur_layer * 2;

        for blur_layer in 0..BLUR_LAYERS_COUNT {
            let next_blur_layer = blur_layer + 1;

            if next_blur_layer < BLUR_LAYERS_COUNT {
                let (downscale_target, downscale_source) = if next_blur_layer % 2 == 0 {
                    (&textures[0], &textures[1])
                } else {
                    (&textures[1], &textures[0])
                };

                draw_fullscreen(
                    core,
                    shader_downscale,
                    downscale_target,
                    clear_black,
                    &bindings(downscale_source, sampler_linear),
                    Some(viewport_by_layer(next_blur_layer)),
                    instance_id_to_range(instance_id_by_layer(next_blur_layer) - 2),
                );
            }

            let blur_init_texture = &textures[blur_layer % 2];
            let blur_instance_id = instance_id_by_layer(blur_layer);
            let blur_viewport = viewport_by_layer(blur_layer);

            for (dir_i, (source, target)) in [(blur_init_texture, &textures[2]), (&textures[2], blur_init_texture)]
                .into_iter()
                .enumerate()
            {
                draw_fullscreen(
                    core,
                    shader_blur,
                    target,
                    clear_black,
                    &bindings(source, sampler_nearest),
                    Some(blur_viewport),
                    instance_id_to_range(blur_instance_id + dir_i),
                );
            }

            draw_fullscreen(
                core,
                shader_apply,
                &target,
                GpuPassOps::Load,
                &bindings(blur_init_texture, sampler_linear),
                None,
                instance_id_to_range(blur_instance_id),
            );
        }
    }
}

struct ColorGradingGpu {
    texture: GpuTexture,
    sampler: GpuSampler,
    shader: GpuShader,
}

#[derive(Default)]
pub struct ColorGradingRenderFeature {
    gpu: Option<ColorGradingGpu>,
}

impl RenderFeature for ColorGradingRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        if world
            .resources()
            .get::<ColorGradingResource>()
            .is_none_or(|grading| grading.ty.is_none())
        {
            return;
        }

        passes.add(RENDER_ORDER_COLOR_GRADING, |this, ctx| this.draw(ctx));
    }
}

impl ColorGradingRenderFeature {
    fn draw(&mut self, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let Some(target) = current_target(ctx) else {
            return;
        };

        let gpu = self.gpu.get_or_insert_with(|| ColorGradingGpu {
            texture: core.create_texture(Some("color grading temp"), target.size(), RENDER_TARGET_FORMAT, 1),
            sampler: core.create_sampler(Some("color grading"), FilterMode::Nearest),
            shader: create_engine_shader(
                core,
                "shader_color_grading_aces.wgsl",
                include_str!("./assets/shader_color_grading_aces.wgsl"),
                fullscreen_shader_desc(GpuBlendState::REPLACE),
            ),
        });

        if gpu.texture.size() != target.size() {
            gpu.texture = core.create_texture(Some("color grading temp"), target.size(), RENDER_TARGET_FORMAT, 1);
        }

        draw_fullscreen(
            core,
            &gpu.shader,
            &gpu.texture,
            GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
            &[(0, texture_binding(&target)), (1, GpuBinding::Sampler(&gpu.sampler))],
            None,
            0..1,
        );

        core.copy_texture(&gpu.texture, &target);
    }
}
