use super::GizmoLine;
use aphyn_ecs::Resource;
use aphyn_ffi::*;
use aphyn_math::{Mat4, Vec2, Vec3, Vec4};
use aphyn_render::{RenderSpace, StandardGlobalBindings, StandardInstanceBuffer, StandardTexture};
use aphyn_render_core::{GpuSampler, GpuShader, GpuTexture, GpuUniformStorageBuffer, GpuVertexBuffer};
use aphyn_utils::mem::AllBitsInit;

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

// todo: support ffi
#[derive(Resource)]
pub struct GizmosRenderResource {
    pub vertex_buffer: GpuUniformStorageBuffer,
    pub color_buffer: GpuUniformStorageBuffer,
    pub transform_buffer: GpuUniformStorageBuffer,
    pub shader: GpuShader,
    pub frame_lines: Vec<(RenderSpace, Vec<GizmoLine>)>,
}

// todo: support ffi
#[derive(Resource)]
pub struct DepthTextureResource {
    pub texture: StandardTexture,
}

// todo: support ffi
#[derive(Resource)]
pub struct TransparentTargetTextureResource {
    pub texture: StandardTexture,
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

// todo: support ffi
#[derive(Resource)]
pub struct ExposureRenderResource {
    pub buffer_uniform: GpuUniformStorageBuffer,
    pub texture: GpuTexture,
    pub sampler: GpuSampler,
    pub shader: GpuShader,
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

pub const BLUR_LAYERS_COUNT: usize = 4;

#[repr(C)]
#[derive(Default)]
pub struct BloomUniform {
    pub threshold: Vec2<f32>,
    pub intensity: f32,
    pub _padding0: f32,
}

unsafe impl AllBitsInit for BloomUniform {}

#[repr(C)]
pub struct BloomStorageLayer {
    pub uv_scale_offset: Vec4<f32>,
    pub direction: Vec2<f32>,
    pub intensity: f32,
    pub _padding0: f32,
}

unsafe impl AllBitsInit for BloomStorageLayer {}

// todo: support ffi
#[derive(Resource)]
pub struct BloomRenderResource {
    pub buffer_storage_layers: GpuUniformStorageBuffer,
    pub buffer_uniform: GpuUniformStorageBuffer,
    pub textures: [GpuTexture; 3],
    pub sampler_linear: GpuSampler,
    pub sampler_nearest: GpuSampler,
    pub shader_gather: GpuShader,
    pub shader_blur: GpuShader,
    pub shader_downscale: GpuShader,
    pub shader_apply: GpuShader,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum ColorGradingType {
    Aces,
}

#[repr(C)]
#[derive(Resource)]
pub struct ColorGradingResource {
    pub ty: Option<ColorGradingType>,
}

impl Default for ColorGradingResource {
    fn default() -> Self {
        Self { ty: None }
    }
}

// todo: support ffi
#[derive(Resource)]
pub struct ColorGradingRenderResource {
    pub texture: GpuTexture,
    pub sampler: GpuSampler,
    pub shader: GpuShader,
}

// todo: support ffi
#[derive(Resource)]
pub struct StandardRenderResource {
    pub globals: StandardGlobalBindings,
    pub instances: StandardInstanceBuffer,
    pub identity_instance: StandardInstanceBuffer,
    pub batched_vertex_buffer: GpuVertexBuffer,
    pub lights_count: u32,
    pub camera_pos: Vec3<f32>,
    pub camera_proj_matrix: Mat4<f32>,
    pub shader_transparent_final: GpuShader,
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

//
