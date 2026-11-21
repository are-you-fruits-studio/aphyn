use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::Component;
use aphyn_ffi::FfiVec;
use aphyn_math::Vec3;
use aphyn_reflection::*;
use aphyn_render::{Camera, Material, StandardMesh, StandardVertex};
use aphyn_serialization::*;

#[repr(C)]
#[derive(Default, Component, Clone, Serializable, Reflectable)]
pub struct MaterialComponent {
    pub material: AssetHandle<Material>,
}

#[repr(C)]
#[derive(Default, Component, Clone, Serializable, Reflectable)]
pub struct MeshComponent {
    pub mesh: AssetHandle<StandardMesh>,
}

#[repr(C)]
#[derive(Component, Default, Clone, Serializable, Reflectable)]
pub struct BatchedMeshComponent {
    pub vertices: FfiVec<StandardVertex>,
    pub indices: FfiVec<u32>,
}

#[repr(C)]
#[derive(Component, Clone, Copy, Serializable, Reflectable)]
pub enum StandardLightComponent {
    Point {
        color: Vec3<f32>,
        range: f32,
        shadows: bool,
    },
    Spot {
        color: Vec3<f32>,
        range: f32,
        fov: f32,
        shadows: bool,
    },
    Directional {
        color: Vec3<f32>,
        shadows: bool,
    },
}

impl Default for StandardLightComponent {
    fn default() -> Self {
        Self::Point {
            color: Vec3::splat(1.0),
            range: 10.0,
            shadows: false,
        }
    }
}

#[repr(C)]
#[derive(Component, Clone, Copy, Default, Serializable, Reflectable)]
pub struct CameraComponent(pub Camera);
