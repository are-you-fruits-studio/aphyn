use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_math::{Mat4, Vec3};
use aphyn_render::{CurrentRenderingResource, FilterMode, RenderApiResource, Shader, write_buffer_elements_with};
use aphyn_render_core::{
    GpuBinding, GpuPassOps, GpuRenderPass, GpuRenderPassDepth, GpuRenderPassTarget, GpuSampler, GpuShader, GpuTexture,
    GpuTextureFormat, GpuUniformStorageBuffer, RenderApi,
};
use aphyn_utils::mem::AllBitsInit;

use crate::{
    AmbientLightResource, DepthTextureResource, EnvironmentTextureResource, RENDER_ORDER_ENVIRONMENT, RENDER_ORDER_SKYBOX,
    RenderFeature, RenderPassCtx, RenderPassWriter, SkyboxResource,
};

const ENVIRONMENT_FORMAT: GpuTextureFormat = GpuTextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Copy, Clone)]
struct SkyboxUniform {
    clip_to_world: Mat4<f32>,
    is_octahedral: u32,
    _padding: [u32; 3],
}

unsafe impl AllBitsInit for SkyboxUniform {}

struct SkyboxGpu {
    buffer_octahedral: GpuUniformStorageBuffer,
    buffer_screen: GpuUniformStorageBuffer,
    sampler: GpuSampler,
}

impl SkyboxGpu {
    fn new(core: &RenderApi) -> Self {
        let buffer = |label| core.create_uniform_storage_buffer(Some(label), size_of::<SkyboxUniform>());

        Self {
            buffer_octahedral: buffer("skybox octahedral uniform"),
            buffer_screen: buffer("skybox screen uniform"),
            sampler: core.create_sampler(Some("environment"), FilterMode::Linear),
        }
    }
}

#[derive(Default)]
pub struct SkyboxRenderFeature {
    texture_ambient: Option<GpuTexture>,
    texture_octahedral: Option<GpuTexture>,
    gpu: Option<SkyboxGpu>,
}

impl RenderFeature for SkyboxRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let mut res = world.into_tuple_mut().0;

        let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
            return;
        };
        let Some(current) = res.get::<CurrentRenderingResource>() else {
            return;
        };
        let core = render_api.core();
        let ambient_color = current
            .ambient_light
            .or_else(|| res.get::<AmbientLightResource>().map(|ambient| ambient.color))
            .unwrap_or(Vec3::splat(0.0));
        let clip_to_world = current.world_to_clip().inverse().unwrap_or(Mat4::IDENTITY);

        let skybox = res.get::<SkyboxResource>().cloned().unwrap_or_default();
        let shader = res
            .get::<AssetStorageResource<Shader>>()
            .and_then(|shaders| shaders.get(&skybox.shader))
            .map(|shader| shader.shader().clone());

        let gpu = self.gpu.get_or_insert_with(|| SkyboxGpu::new(core));
        let sampler = gpu.sampler.clone();

        let texture = match shader {
            None => {
                let texture = self
                    .texture_ambient
                    .get_or_insert_with(|| core.create_texture(Some("environment ambient"), [1, 1], ENVIRONMENT_FORMAT, 1));
                core.clear_color_texture(
                    Some("clear environment ambient"),
                    texture,
                    0,
                    [ambient_color.x, ambient_color.y, ambient_color.z, 1.0],
                );

                texture.clone()
            }
            Some(shader) => {
                let size = skybox.octahedral_size.max(1) as usize;

                if self
                    .texture_octahedral
                    .as_ref()
                    .is_none_or(|texture| texture.size() != [size, size])
                {
                    let mip_level_count = size.ilog2() as usize + 1;
                    self.texture_octahedral = Some(core.create_texture(
                        Some("environment octahedral"),
                        [size, size],
                        ENVIRONMENT_FORMAT,
                        mip_level_count,
                    ));
                }

                let shader_screen = shader.clone();
                passes.add(RENDER_ORDER_ENVIRONMENT, move |this, ctx| this.draw_octahedral(&shader, ctx));
                passes.add(RENDER_ORDER_SKYBOX, move |this, ctx| {
                    this.draw_screen(&shader_screen, clip_to_world, ctx)
                });

                self.texture_octahedral.clone().unwrap()
            }
        };

        if let Some(resource) = res.get_mut::<EnvironmentTextureResource>() {
            resource.texture = texture;
            resource.sampler = sampler;
        } else {
            res.insert(EnvironmentTextureResource { texture, sampler });
        }
    }
}

impl SkyboxRenderFeature {
    fn draw_octahedral(&mut self, shader: &GpuShader, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let (Some(gpu), Some(texture)) = (self.gpu.as_ref(), self.texture_octahedral.as_ref()) else {
            return;
        };

        let uniform = SkyboxUniform {
            clip_to_world: Mat4::IDENTITY,
            is_octahedral: 1,
            _padding: [0; 3],
        };
        write_buffer_elements_with(core, &gpu.buffer_octahedral, 0, 1, |_| uniform);

        core.draw(
            &GpuRenderPass {
                label: Some("skybox octahedral"),
                shader,
                target: Some(GpuRenderPassTarget {
                    view: texture,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
                }),
                depth: None,
                bind_groups: &[&[(0, GpuBinding::Buffer(&gpu.buffer_octahedral))]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            0..3,
            0..1,
        );

        ctx.render_api.generate_mipmaps(texture);
    }

    fn draw_screen(&mut self, shader: &GpuShader, clip_to_world: Mat4<f32>, ctx: &mut RenderPassCtx) {
        let core = ctx.render_api.core();
        let resources = ctx.world.resources();
        let (Some(gpu), Some(current), Some(depth)) = (
            self.gpu.as_ref(),
            resources.get::<CurrentRenderingResource>(),
            resources.get::<DepthTextureResource>(),
        ) else {
            return;
        };

        let uniform = SkyboxUniform {
            clip_to_world,
            is_octahedral: 0,
            _padding: [0; 3],
        };
        write_buffer_elements_with(core, &gpu.buffer_screen, 0, 1, |_| uniform);

        core.draw(
            &GpuRenderPass {
                label: Some("skybox"),
                shader,
                target: Some(GpuRenderPassTarget {
                    view: current.target.texture(),
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                depth: Some(GpuRenderPassDepth {
                    view: depth.texture.texture(),
                    mip_level: 0,
                    ops: GpuPassOps::Load,
                }),
                bind_groups: &[&[(0, GpuBinding::Buffer(&gpu.buffer_screen))]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            0..3,
            0..1,
        );
    }
}
