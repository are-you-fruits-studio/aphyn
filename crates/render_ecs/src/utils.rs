use aphyn_asset_storage::AssetStorageResource;
use aphyn_math::Mat4;
use aphyn_math::Vec3;
use aphyn_render::StandardLight;
use aphyn_render::StandardMaterialShader;
use aphyn_render::StandardShader;
use aphyn_transform::GlobalTransform;

use crate::StandardLightComponent;
use crate::StandardMaterial;

pub const GIZMO_LINES_PER_DRAW_MAX: usize = 1024 * 16;
pub const INSTANCES_PER_DRAW_MAX: usize = 1024 * 8;
pub const TRIANGLES_PER_BATCHED_DRAW_MAX: usize = 1024 * 32;

pub fn create_window_to_clip_matrix(width: f32, height: f32, near: f32, far: f32) -> Mat4<f32> {
    let z_near_to_far_inv = 1.0 / (far - near);

    // todo: make window space bottom-to-top (not inverted like it is right now compared to the clip and world space)

    Mat4::from_array([
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, z_near_to_far_inv, 0.0],
        [-1.0, 1.0, -near * z_near_to_far_inv, 1.0],
    ])
}

pub(crate) fn custom_shader<'a>(material: &StandardMaterial, shaders: &'a AssetStorageResource<StandardShader>) -> Option<&'a StandardShader> {
    match &material.meta().shader {
        StandardMaterialShader::Engine(_) => None,
        StandardMaterialShader::Custom(shader) => shaders.get(shader),
    }
}

pub fn light_from_components(light: &StandardLightComponent, transform: &GlobalTransform) -> StandardLight {
    match light {
        StandardLightComponent::Point { color, range } => StandardLight::Point {
            color: *color,
            range: *range,
            center: transform.position,
        },
        StandardLightComponent::Spot { color, range, fov } => StandardLight::Spot {
            color: *color,
            range: *range,
            fov: *fov,
            center: transform.position,
            direction_dst: (transform.scale_rotation * Vec3::new(0.0, -1.0, 0.0)).normalized(),
        },
        StandardLightComponent::Directional { color } => StandardLight::Directional {
            color: *color,
            direction_dst: (transform.scale_rotation * Vec3::new(0.0, -1.0, 0.0)).normalized(),
        },
    }
}