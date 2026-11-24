use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_math::{Mat4, Vec2, Vec4};
use aphyn_render::{CurrentRenderingResource, RenderApiResource, write_buffer_elements_with};
use aphyn_render_core::{
    GpuBinding, GpuBlendState, GpuPassOps, GpuShader, GpuTexture, GpuTextureFormat, GpuUniformStorageBuffer, RenderApi,
};
use aphyn_utils::mem::AllBitsInit;

use crate::{
    AmbientOcclusionTextureResource, DepthTextureResource, RENDER_ORDER_AMBIENT_OCCLUSION, RenderFeature, RenderPassCtx,
    RenderPassWriter, SSAO_SAMPLES_MAX, SceneNormalsTextureResource, SsaoResource, create_engine_shader, draw_fullscreen,
    fullscreen_shader_desc, texture_binding,
};

const AMBIENT_OCCLUSION_FORMAT: GpuTextureFormat = GpuTextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Copy, Clone)]
struct SsaoUniform {
    view_to_clip: Mat4<f32>,
    clip_to_view: Mat4<f32>,
    world_to_view: Mat4<f32>,
    target_size: Vec2<f32>,
    radius: f32,
    intensity: f32,
    bias: f32,
    sample_count: u32,
    _padding: [u32; 2],
}

unsafe impl AllBitsInit for SsaoUniform {}

struct SsaoGpu {
    buffer_uniform: GpuUniformStorageBuffer,
    buffer_kernel: GpuUniformStorageBuffer,
    texture_blur: GpuTexture,
    shader_occlusion: GpuShader,
    shader_blur: GpuShader,
}

impl SsaoGpu {
    fn new(core: &RenderApi, size: [usize; 2]) -> Self {
        let kernel = hemisphere_kernel();
        let buffer_kernel = core.create_uniform_storage_buffer(Some("ssao kernel"), size_of_val(kernel.as_slice()));
        write_buffer_elements_with(core, &buffer_kernel, 0, kernel.len(), |i| kernel[i]);

        let fullscreen =
            |name: &str, source: &str| create_engine_shader(core, name, source, fullscreen_shader_desc(GpuBlendState::REPLACE));

        Self {
            buffer_uniform: core.create_uniform_storage_buffer(Some("ssao uniform"), size_of::<SsaoUniform>()),
            buffer_kernel,
            texture_blur: core.create_texture(Some("ssao blur"), size, AMBIENT_OCCLUSION_FORMAT, 1),
            shader_occlusion: fullscreen("shader_ssao.wgsl", include_str!("./assets/shader_ssao.wgsl")),
            shader_blur: fullscreen("shader_ssao_blur.wgsl", include_str!("./assets/shader_ssao_blur.wgsl")),
        }
    }
}

// tangent-space samples in the +z hemisphere, denser near the center, from a fixed seed
fn hemisphere_kernel() -> Vec<Vec4<f32>> {
    let mut state = 0x9e37_79b9_u32;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };

    (0..SSAO_SAMPLES_MAX)
        .map(|i| {
            loop {
                let x = random() * 2.0 - 1.0;
                let y = random() * 2.0 - 1.0;
                let z = random();
                let length_squared = x * x + y * y + z * z;

                if !(1e-4..=1.0).contains(&length_squared) {
                    continue;
                }

                let t = i as f32 / SSAO_SAMPLES_MAX as f32;
                let scale = (0.1 + 0.9 * t * t) / length_squared.sqrt();

                break Vec4::new(x * scale, y * scale, z * scale, 0.0);
            }
        })
        .collect()
}

#[derive(Default)]
pub struct SsaoRenderFeature {
    texture_white: Option<GpuTexture>,
    texture_occlusion: Option<GpuTexture>,
    gpu: Option<SsaoGpu>,
}

impl RenderFeature for SsaoRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let mut res = world.into_tuple_mut().0;

        let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
            return;
        };
        let Some(current) = res.get::<CurrentRenderingResource>() else {
            return;
        };
        let core = render_api.core();
        let settings = res.get::<SsaoResource>().copied().filter(|settings| settings.is_enabled);

        let texture = match settings {
            None => self
                .texture_white
                .get_or_insert_with(|| {
                    let texture = core.create_texture(Some("ssao off"), [1, 1], AMBIENT_OCCLUSION_FORMAT, 1);
                    core.clear_color_texture(&texture, 0, [1.0; 4]);
                    texture
                })
                .clone(),
            Some(settings) => {
                let size = current.target_size().map(|x| x as usize);
                let view_to_clip = current.view_to_clip();
                let uniform = SsaoUniform {
                    view_to_clip,
                    clip_to_view: view_to_clip.inverse().unwrap_or(Mat4::IDENTITY),
                    world_to_view: current.world_to_view(),
                    target_size: Vec2::new(size[0] as f32, size[1] as f32),
                    radius: settings.radius,
                    intensity: settings.intensity,
                    bias: settings.bias,
                    sample_count: settings.sample_count.min(SSAO_SAMPLES_MAX),
                    _padding: [0; 2],
                };

                if self.texture_occlusion.as_ref().is_none_or(|texture| texture.size() != size) {
                    self.texture_occlusion = Some(core.create_texture(Some("ssao"), size, AMBIENT_OCCLUSION_FORMAT, 1));
                }

                passes.add(RENDER_ORDER_AMBIENT_OCCLUSION, move |this, ctx| this.draw(uniform, ctx));

                self.texture_occlusion.clone().unwrap()
            }
        };

        if let Some(resource) = res.get_mut::<AmbientOcclusionTextureResource>() {
            resource.texture = texture;
        } else {
            res.insert(AmbientOcclusionTextureResource { texture });
        }
    }
}

impl SsaoRenderFeature {
    fn draw(&mut self, uniform: SsaoUniform, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let resources = ctx.world.resources();

        let (Some(depth), Some(normals), Some(texture_occlusion)) = (
            resources.get::<DepthTextureResource>(),
            resources.get::<SceneNormalsTextureResource>(),
            self.texture_occlusion.as_ref(),
        ) else {
            return;
        };
        let size = texture_occlusion.size();

        let gpu = self.gpu.get_or_insert_with(|| SsaoGpu::new(core, size));

        if gpu.texture_blur.size() != size {
            gpu.texture_blur = core.create_texture(Some("ssao blur"), size, AMBIENT_OCCLUSION_FORMAT, 1);
        }

        write_buffer_elements_with(core, &gpu.buffer_uniform, 0, 1, |_| uniform);

        let depth = depth.texture.texture();
        let clear_white = GpuPassOps::Clear([1.0; 4]);

        draw_fullscreen(
            core,
            &gpu.shader_occlusion,
            texture_occlusion,
            clear_white,
            &[
                (0, texture_binding(depth)),
                (1, texture_binding(normals.texture.texture())),
                (2, GpuBinding::Buffer(&gpu.buffer_uniform)),
                (3, GpuBinding::Buffer(&gpu.buffer_kernel)),
            ],
            None,
            0..1,
        );

        for (instance, source, target) in [
            (0, texture_occlusion, &gpu.texture_blur),
            (1, &gpu.texture_blur, texture_occlusion),
        ] {
            draw_fullscreen(
                core,
                &gpu.shader_blur,
                target,
                clear_white,
                &[
                    (0, texture_binding(depth)),
                    (1, texture_binding(source)),
                    (2, GpuBinding::Buffer(&gpu.buffer_uniform)),
                ],
                None,
                instance..(instance + 1),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_lies_in_the_unit_hemisphere() {
        let kernel = hemisphere_kernel();

        assert_eq!(kernel.len(), SSAO_SAMPLES_MAX as usize);
        for sample in kernel {
            let length = (sample.x * sample.x + sample.y * sample.y + sample.z * sample.z).sqrt();

            assert!(sample.z >= 0.0);
            assert!((0.09..=1.0001).contains(&length), "{length}");
        }
    }
}
