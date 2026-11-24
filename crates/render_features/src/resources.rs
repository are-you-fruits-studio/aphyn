use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::Resource;
use aphyn_ffi::*;
use aphyn_math::{Vec3, Vec4};
use aphyn_render::{CLUSTER_PAIRS_CAPACITY_DEFAULT, RenderSpace, Shader, StandardGlobalBindings, StandardTexture};
use aphyn_render_core::{GpuSampler, GpuTexture};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GizmoLine {
    pub start: Vec3<f32>,
    pub end: Vec3<f32>,
    // todo: transparency.
    pub color: Vec4<f32>,
}

#[repr(C)]
#[derive(Resource)]
pub struct GizmosResource {
    lines: FfiIndexMap<RenderSpace, FfiVec<GizmoLine>>,
}

impl GizmosResource {
    pub fn space(&mut self, space: RenderSpace) -> &mut FfiVec<GizmoLine> {
        self.lines.get_mut(&space).unwrap()
    }

    pub fn spaces(&mut self) -> FfiIndexMapPairsMutIter<'_, RenderSpace, FfiVec<GizmoLine>> {
        self.lines.iter_mut()
    }
}

impl Default for GizmosResource {
    fn default() -> Self {
        let mut lines = FfiIndexMap::new();

        lines.insert(RenderSpace::Clip, FfiVec::new());
        lines.insert(RenderSpace::Window, FfiVec::new());
        lines.insert(RenderSpace::World, FfiVec::new());

        Self { lines }
    }
}

#[repr(C)]
#[derive(Resource, Default)]
pub struct GizmosFrameResource {
    pub lines: FfiVec<(RenderSpace, FfiVec<GizmoLine>)>,
}

#[repr(C)]
#[derive(Resource)]
pub struct GlobalBindingsResource {
    pub bindings: StandardGlobalBindings,
}

#[repr(C)]
#[derive(Resource)]
pub struct DepthTextureResource {
    pub texture: StandardTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct SceneNormalsTextureResource {
    pub texture: StandardTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct TransparentTargetResource {
    pub texture: StandardTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct AmbientOcclusionTextureResource {
    pub texture: GpuTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct EnvironmentTextureResource {
    pub texture: GpuTexture,
    pub sampler: GpuSampler,
}

#[repr(C)]
#[derive(Resource, Clone)]
pub struct SkyboxResource {
    pub shader: AssetHandle<Shader>,
    pub octahedral_size: u32,
}

impl Default for SkyboxResource {
    fn default() -> Self {
        Self {
            shader: AssetHandle::EMPTY,
            octahedral_size: 128,
        }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct ExposureResource {
    pub exposure: f32,
    pub is_enabled: bool,
}

impl Default for ExposureResource {
    fn default() -> Self {
        Self {
            exposure: 1.5,
            is_enabled: false,
        }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct AmbientLightResource {
    pub color: Vec3<f32>,
}

impl Default for AmbientLightResource {
    fn default() -> Self {
        Self { color: Vec3::splat(0.0) }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct BloomResource {
    pub is_enabled: bool,
    pub threshold: f32,
    pub threshold_softening: f32,
    pub intensity: f32,
}

impl Default for BloomResource {
    fn default() -> Self {
        Self {
            is_enabled: false,
            threshold: 3.0,
            threshold_softening: 2.0,
            intensity: 0.7,
        }
    }
}

pub const SSAO_SAMPLES_MAX: u32 = 64;

#[repr(C)]
#[derive(Resource, Clone, Copy, Debug)]
pub struct SsaoResource {
    pub is_enabled: bool,
    // view-space radius of the sampled hemisphere
    pub radius: f32,
    pub intensity: f32,
    // how far in view-space depth the scene must be in front of a sample to occlude it
    pub bias: f32,
    // clamped to SSAO_SAMPLES_MAX
    pub sample_count: u32,
}

impl Default for SsaoResource {
    fn default() -> Self {
        Self {
            is_enabled: false,
            radius: 0.5,
            intensity: 1.0,
            bias: 0.025,
            sample_count: 16,
        }
    }
}

#[repr(C)]
#[derive(Resource, Clone, Copy, Debug)]
pub struct SsrResource {
    pub is_enabled: bool,
    // view-space length of a reflected ray
    pub max_distance: f32,
    pub max_steps: u32,
    // how far behind the scene's depth a ray still counts as a hit, in view space
    pub thickness: f32,
    // how far behind the scene's depth a ray must be to count as a hit, in view space; keeps rays from hitting the surface they start on
    pub depth_bias: f32,
    // rougher surfaces keep the environment map's reflection
    pub max_roughness: f32,
    pub intensity: f32,
    // the part of the screen (in uv) over which reflections fade out towards its edges
    pub edge_fade: f32,
}

impl Default for SsrResource {
    fn default() -> Self {
        Self {
            is_enabled: false,
            max_distance: 50.0,
            max_steps: 48,
            thickness: 0.3,
            depth_bias: 0.05,
            max_roughness: 0.6,
            intensity: 1.0,
            edge_fade: 0.1,
        }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct SsrTexturesResource {
    pub depth: GpuTexture,
    pub history: GpuTexture,
    pub sampler: GpuSampler,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum ColorGradingType {
    Aces,
}

#[repr(C)]
#[derive(Resource, Default)]
pub struct ColorGradingResource {
    pub ty: Option<ColorGradingType>,
}

#[repr(C)]
#[derive(Resource)]
pub struct ScreenSpaceResource {
    pub near: f32,
    pub far: f32,
}

impl Default for ScreenSpaceResource {
    fn default() -> Self {
        Self {
            near: -10_000.0,
            far: 10_000.0,
        }
    }
}

#[repr(C)]
#[derive(Resource, Clone, Copy, Debug)]
pub struct ShadowSettingsResource {
    pub atlas_size: u32,
    pub tile_size: u32,
    pub cascade_count: u32,
    pub distance: f32,
    // 0 = uniform cascade splits, 1 = logarithmic
    pub split_lambda: f32,
    // how far behind a cascade's bounds directional casters are still captured
    pub caster_distance: f32,
    pub bias: f32,
    pub normal_offset: f32,
    pub spot_near: f32,
}

impl Default for ShadowSettingsResource {
    fn default() -> Self {
        Self {
            atlas_size: 4096,
            tile_size: 1024,
            cascade_count: 4,
            distance: 50.0,
            split_lambda: 0.75,
            caster_distance: 100.0,
            bias: 0.001,
            normal_offset: 0.1,
            spot_near: 0.05,
        }
    }
}

#[repr(C)]
#[derive(Resource, Clone, Copy, Debug)]
pub struct ClusteredLightingResource {
    // the size of the light index list all clusters share
    pub max_light_cluster_pairs: u32,
}

impl Default for ClusteredLightingResource {
    fn default() -> Self {
        Self {
            max_light_cluster_pairs: CLUSTER_PAIRS_CAPACITY_DEFAULT as u32,
        }
    }
}
