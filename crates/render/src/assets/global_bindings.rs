use aphyn_math::{Mat4, Vec3};
use aphyn_render_core::{GpuUniformStorageBuffer, RenderApi};
use aphyn_serialization::*;
use aphyn_utils::mem::{AllBitVariationsValid, AllBitsInit};

pub const LIGHTS_COUNT_MAX: usize = 1024;

#[repr(C)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serializable)]
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
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct StandardUniformGlobal {
    pub world_to_clip: Mat4<f32>,
    pub window_to_clip: Mat4<f32>,
    pub camera_position_world: Vec3<f32>,
    pub lights_count: u32,
}

unsafe impl AllBitVariationsValid for StandardUniformGlobal {}
unsafe impl AllBitsInit for StandardUniformGlobal {}

impl Default for StandardUniformGlobal {
    fn default() -> Self {
        Self {
            world_to_clip: Mat4::IDENTITY,
            window_to_clip: Mat4::IDENTITY,
            camera_position_world: Vec3::splat(0.0),
            lights_count: 0,
        }
    }
}

impl From<StandardLight> for StandardGenericLight {
    fn from(value: StandardLight) -> Self {
        match value {
            StandardLight::Point { color, center, range } => Self {
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
            } => Self {
                light_type: 1,
                color,
                center,
                range,
                direction_dst,
                fov,
            },
            StandardLight::Directional { color, direction_dst } => Self {
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
    },
    Spot {
        color: Vec3<f32>,
        center: Vec3<f32>,
        range: f32,
        direction_dst: Vec3<f32>,
        fov: f32,
    },
    Directional {
        color: Vec3<f32>,
        direction_dst: Vec3<f32>,
    },
}

impl Default for StandardLight {
    fn default() -> Self {
        Self::Point {
            color: Vec3::splat(0.0),
            center: Vec3::splat(0.0),
            range: 0.0,
        }
    }
}

#[repr(C)]
pub struct StandardGlobalBindings {
    uniform_buffer: GpuUniformStorageBuffer,
    lights_buffer: GpuUniformStorageBuffer,
}

impl StandardGlobalBindings {
    pub(crate) fn new(core: &RenderApi) -> Self {
        let uniform_buffer = core.create_uniform_storage_buffer(Some("standard global uniform"), size_of::<StandardUniformGlobal>());
        let lights_buffer =
            core.create_uniform_storage_buffer(Some("standard lights"), LIGHTS_COUNT_MAX * size_of::<StandardGenericLight>());

        let this = Self {
            uniform_buffer,
            lights_buffer,
        };
        this.write_uniform(core, StandardUniformGlobal::default());

        this
    }

    pub(crate) fn write_uniform(&self, core: &RenderApi, uniform: StandardUniformGlobal) {
        crate::write_buffer_bytes(core, &self.uniform_buffer, 0, aphyn_utils::mem::as_bytes(&uniform));
    }

    pub(crate) fn write_lights_with(&self, core: &RenderApi, count: u32, f: impl FnMut(usize) -> StandardGenericLight) {
        let count = (count as usize).min(LIGHTS_COUNT_MAX);

        crate::write_buffer_elements_with(core, &self.lights_buffer, 0, count, f);
    }

    pub fn uniform_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.uniform_buffer
    }

    pub fn lights_buffer(&self) -> &GpuUniformStorageBuffer {
        &self.lights_buffer
    }
}
