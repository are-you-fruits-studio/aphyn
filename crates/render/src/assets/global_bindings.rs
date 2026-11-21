use aphyn_math::{Mat4, Vec3, Vec4};
use aphyn_reflection::*;
use aphyn_render_core::{GpuUniformStorageBuffer, RenderApi};
use aphyn_serialization::*;
use aphyn_utils::mem::{AllBitVariationsValid, AllBitsInit};

pub const LIGHTS_COUNT_MAX: usize = 4096;
pub const SHADOW_VIEWS_MAX: usize = 64;

pub const CLUSTER_DIMS: [u32; 3] = [16, 9, 24];
pub const CLUSTERS_COUNT: usize = (CLUSTER_DIMS[0] * CLUSTER_DIMS[1] * CLUSTER_DIMS[2]) as usize;
pub const CLUSTER_PAIRS_CAPACITY_DEFAULT: usize = CLUSTERS_COUNT * 64;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct StandardClusterCell {
    pub index: u32,
    pub count: u32,
}

unsafe impl AllBitVariationsValid for StandardClusterCell {}
unsafe impl AllBitsInit for StandardClusterCell {}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serializable, Reflectable)]
pub enum RenderSpace {
    Clip,
    Window,
    #[default]
    World,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardGenericLight {
    pub color: Vec3<f32>,
    pub light_type: u32,
    pub center: Vec3<f32>,
    pub range: f32,
    pub direction_dst: Vec3<f32>,
    pub fov: f32,
    pub shadow_start: u32,
    pub shadow_count: u32,
    pub _padding: [u32; 2],
}

unsafe impl AllBitVariationsValid for StandardGenericLight {}
unsafe impl AllBitsInit for StandardGenericLight {}

impl Default for StandardGenericLight {
    fn default() -> Self {
        Self {
            color: Vec3::splat(0.0),
            light_type: 0,
            center: Vec3::splat(0.0),
            range: 0.0,
            direction_dst: Vec3::new(0.0, -1.0, 0.0),
            fov: 0.0,
            shadow_start: 0,
            shadow_count: 0,
            _padding: [0; 2],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardUniformGlobal {
    pub world_to_clip: Mat4<f32>,
    pub window_to_clip: Mat4<f32>,
    pub world_to_view: Mat4<f32>,
    pub clip_to_view: Mat4<f32>,
    pub camera_position_world: Vec3<f32>,
    pub lights_count: u32,
    pub ambient_color: Vec3<f32>,
    pub shadow_bias: f32,
    pub camera_forward_world: Vec3<f32>,
    pub shadow_normal_offset: f32,
    pub cluster_dims: [u32; 3],
    pub directional_lights_count: u32,
    pub target_size: [f32; 2],
    pub cluster_near: f32,
    pub cluster_far: f32,
    pub cluster_index_capacity: u32,
    pub _padding: [u32; 3],
}

unsafe impl AllBitVariationsValid for StandardUniformGlobal {}
unsafe impl AllBitsInit for StandardUniformGlobal {}

impl Default for StandardUniformGlobal {
    fn default() -> Self {
        Self {
            world_to_clip: Mat4::IDENTITY,
            window_to_clip: Mat4::IDENTITY,
            world_to_view: Mat4::IDENTITY,
            clip_to_view: Mat4::IDENTITY,
            camera_position_world: Vec3::splat(0.0),
            lights_count: 0,
            ambient_color: Vec3::splat(0.0),
            shadow_bias: 0.0,
            camera_forward_world: Vec3::new(0.0, 0.0, 1.0),
            shadow_normal_offset: 0.0,
            cluster_dims: CLUSTER_DIMS,
            directional_lights_count: 0,
            target_size: [1.0, 1.0],
            cluster_near: 0.1,
            cluster_far: 1000.0,
            cluster_index_capacity: 0,
            _padding: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StandardShadowView {
    pub world_to_clip: Mat4<f32>,
    // uv offset in xy, uv scale in zw
    pub atlas_rect: Vec4<f32>,
    pub split_far: f32,
    pub _padding: [f32; 3],
}

unsafe impl AllBitVariationsValid for StandardShadowView {}
unsafe impl AllBitsInit for StandardShadowView {}

impl Default for StandardShadowView {
    fn default() -> Self {
        Self {
            world_to_clip: Mat4::IDENTITY,
            atlas_rect: Vec4::new(0.0, 0.0, 1.0, 1.0),
            split_far: 0.0,
            _padding: [0.0; 3],
        }
    }
}

impl From<StandardLight> for StandardGenericLight {
    fn from(value: StandardLight) -> Self {
        match value {
            StandardLight::Point {
                color, center, range, ..
            } => Self {
                light_type: 0,
                color,
                center,
                range,
                ..Default::default()
            },
            StandardLight::Spot {
                color,
                center,
                range,
                direction_dst,
                fov,
                ..
            } => Self {
                light_type: 1,
                color,
                center,
                range,
                direction_dst,
                fov,
                ..Default::default()
            },
            StandardLight::Directional {
                color, direction_dst, ..
            } => Self {
                light_type: 2,
                color,
                direction_dst,
                ..Default::default()
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serializable)]
pub enum StandardLight {
    Point {
        color: Vec3<f32>,
        center: Vec3<f32>,
        range: f32,
        shadows: bool,
    },
    Spot {
        color: Vec3<f32>,
        center: Vec3<f32>,
        range: f32,
        direction_dst: Vec3<f32>,
        fov: f32,
        shadows: bool,
    },
    Directional {
        color: Vec3<f32>,
        direction_dst: Vec3<f32>,
        shadows: bool,
    },
}

impl StandardLight {
    pub fn shadows(&self) -> bool {
        match *self {
            Self::Point { shadows, .. } | Self::Spot { shadows, .. } | Self::Directional { shadows, .. } => shadows,
        }
    }
}

impl Default for StandardLight {
    fn default() -> Self {
        Self::Point {
            color: Vec3::splat(0.0),
            center: Vec3::splat(0.0),
            range: 0.0,
            shadows: false,
        }
    }
}

#[repr(C)]
pub struct StandardGlobalBindings {
    uniform_buffer: GpuUniformStorageBuffer,
    lights_buffer: GpuUniformStorageBuffer,
    shadow_views_buffer: GpuUniformStorageBuffer,
    cluster_grid_buffer: GpuUniformStorageBuffer,
    cluster_indices_buffer: GpuUniformStorageBuffer,
    cluster_dropped_pairs_buffer: GpuUniformStorageBuffer,
    cluster_pairs_capacity: usize,
}

impl StandardGlobalBindings {
    pub(crate) fn new(core: &RenderApi) -> Self {
        let uniform_buffer = core.create_uniform_storage_buffer(Some("standard global uniform"), size_of::<StandardUniformGlobal>());
        let lights_buffer =
            core.create_uniform_storage_buffer(Some("standard lights"), LIGHTS_COUNT_MAX * size_of::<StandardGenericLight>());
        let shadow_views_buffer = core.create_uniform_storage_buffer(
            Some("standard shadow views"),
            SHADOW_VIEWS_MAX * size_of::<StandardShadowView>(),
        );
        let cluster_grid_buffer = core.create_uniform_storage_buffer(
            Some("standard cluster grid"),
            CLUSTERS_COUNT * size_of::<StandardClusterCell>(),
        );
        let cluster_indices_buffer = Self::create_cluster_indices_buffer(core, CLUSTER_PAIRS_CAPACITY_DEFAULT);
        let cluster_dropped_pairs_buffer =
            core.create_uniform_storage_buffer(Some("standard cluster dropped pairs"), size_of::<u32>());

        let this = Self {
            uniform_buffer,
            lights_buffer,
            shadow_views_buffer,
            cluster_grid_buffer,
            cluster_indices_buffer,
            cluster_dropped_pairs_buffer,
            cluster_pairs_capacity: CLUSTER_PAIRS_CAPACITY_DEFAULT,
        };
        this.write_uniform(core, StandardUniformGlobal::default());

        this
    }

    fn create_cluster_indices_buffer(core: &RenderApi, pairs_capacity: usize) -> GpuUniformStorageBuffer {
        core.create_uniform_storage_buffer(Some("standard cluster light indices"), pairs_capacity * size_of::<u32>())
    }

    pub(crate) fn set_cluster_pairs_capacity(&mut self, core: &RenderApi, pairs_capacity: usize) {
        let pairs_capacity = pairs_capacity.max(1);

        if pairs_capacity == self.cluster_pairs_capacity {
            return;
        }

        self.cluster_indices_buffer = Self::create_cluster_indices_buffer(core, pairs_capacity);
        self.cluster_pairs_capacity = pairs_capacity;
    }

    pub fn cluster_pairs_capacity(&self) -> usize {
        self.cluster_pairs_capacity
    }

    pub(crate) fn reset_cluster_dropped_pairs(&self, core: &RenderApi) {
        crate::write_buffer_elements_with(core, &self.cluster_dropped_pairs_buffer, 0, 1, |_| 0_u32);
    }

    pub(crate) fn read_cluster_dropped_pairs(&self, core: &RenderApi) -> Option<u32> {
        let bytes = core.read_uniform_storage_buffer(&self.cluster_dropped_pairs_buffer)?;

        Some(u32::from_ne_bytes(bytes.as_slice().get(..4)?.try_into().unwrap()))
    }

    pub(crate) fn write_uniform(&self, core: &RenderApi, uniform: StandardUniformGlobal) {
        crate::write_buffer_bytes(core, &self.uniform_buffer, 0, aphyn_utils::mem::as_bytes(&uniform));
    }

    pub(crate) fn write_lights_with(&self, core: &RenderApi, count: u32, f: impl FnMut(usize) -> StandardGenericLight) {
        let count = (count as usize).min(LIGHTS_COUNT_MAX);

        crate::write_buffer_elements_with(core, &self.lights_buffer, 0, count, f);
    }

    pub(crate) fn write_shadow_views(&self, core: &RenderApi, views: &[StandardShadowView]) {
        let count = views.len().min(SHADOW_VIEWS_MAX);

        crate::write_buffer_elements_with(core, &self.shadow_views_buffer, 0, count, |i| views[i]);
    }

    pub fn uniform_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.uniform_buffer
    }

    pub fn lights_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.lights_buffer
    }

    pub fn shadow_views_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.shadow_views_buffer
    }

    pub fn cluster_grid_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.cluster_grid_buffer
    }

    pub fn cluster_indices_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.cluster_indices_buffer
    }

    pub fn cluster_dropped_pairs_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.cluster_dropped_pairs_buffer
    }
}
