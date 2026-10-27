use fruits_asset_storage::AssetStorageResource;
use fruits_math::Mat4;
use fruits_math::Vec3;
use fruits_render_core::StandardEngineShader;
use fruits_render_core::StandardLight;
use fruits_render_core::StandardMaterialShader;
use fruits_render_core::StandardShader;
use fruits_transform::GlobalTransform;
use wgpu::*;

use crate::StandardLightComponent;
use crate::{
    StandardMaterial, StandardRenderResource,
};

pub const GIZMO_LINES_PER_DRAW_MAX: usize = 1024 * 16;
pub const INSTANCES_PER_DRAW_MAX: usize = 1024 * 8;
pub const TRIANGLES_PER_BATCHED_DRAW_MAX: usize = 1024 * 32;
pub const LIGHTS_COUNT_MAX: usize = 1024;

pub const TRANSPARENT_TARGET_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

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

// todo
pub(crate) fn get_render_data<'a, 'b>(
    material: &'b StandardMaterial,
    standard_render_res: &'a StandardRenderResource,
    shaders: &'a AssetStorageResource<StandardShader>,
) -> Option<(&'a RenderPipeline, &'b BindGroup)> {
    let material_native = unsafe { material.native() };

    let render_pipeline = match &material.meta().shader {
        StandardMaterialShader::Engine(shader) => match shader {
            StandardEngineShader::OpaqueLit => &standard_render_res.render_pipeline_opaque_lit,
            StandardEngineShader::OpaqueUnlit => &standard_render_res.render_pipeline_opaque_unlit,
            StandardEngineShader::TransparentLit => &standard_render_res.render_pipeline_transparent_lit,
            StandardEngineShader::TransparentUnlit => &standard_render_res.render_pipeline_transparent_unlit,
            StandardEngineShader::UiOpaqueUnlit => &standard_render_res.render_pipeline_ui_opaque_unlit,
            StandardEngineShader::UiTransparentUnlit => &standard_render_res.render_pipeline_ui_transparent_unlit,
        }
        StandardMaterialShader::Custom(shader) => unsafe { &shaders.get(shader)?.native().render_pipeline },
    };

    Some((render_pipeline, &material_native.bind_group))
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