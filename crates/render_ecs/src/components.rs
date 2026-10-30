use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::Component;
use aphyn_ffi::FfiVec;
use aphyn_math::Vec3;
use aphyn_render::{Camera, StandardMaterial, StandardMesh, StandardVertex};
use aphyn_serialization::*;

#[repr(C)]
#[derive(Component, Default, Serializable)]
pub struct StandardRenderComponent {
    pub mesh: AssetHandle<StandardMesh>,
    pub material: AssetHandle<StandardMaterial>,
}

#[repr(C)]
#[derive(Default, Component, Clone, Serializable)]
pub struct StandardMeshComponent {
    pub mesh: AssetHandle<StandardMesh>,
}

#[repr(C)]
#[derive(Component, Default, Clone, Serializable)]
pub struct BatchedMeshComponent {
    pub vertices: FfiVec<StandardVertex>,
    pub indices: FfiVec<u16>,
}

#[repr(C)]
#[derive(Default, Component, Clone, Serializable)]
pub struct StandardMaterialComponent {
    pub material: AssetHandle<StandardMaterial>,
}

#[repr(C)]
#[derive(Component, Clone, Copy, Serializable)]
pub enum StandardLightComponent {
    Point {
        color: Vec3<f32>,
        range: f32,
    },
    Spot {
        color: Vec3<f32>,
        range: f32,
        fov: f32,
    },
    Directional {
        color: Vec3<f32>,
    },
}
impl Default for StandardLightComponent {
    fn default() -> Self {
        Self::Point {
            color: Vec3::splat(1.0),
            range: 10.0,
        }
    }
}

#[repr(C)]
#[derive(Component, Clone, Copy, Default, Serializable)]
pub struct CameraComponent(pub Camera);