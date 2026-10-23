use fruits_math::Mat4;
use fruits_math::Vec3;
use fruits_math::Vec4;
use fruits_render_core::RenderSpace;
use fruits_render_core::StandardLight;
use fruits_render_core::StandardUniformMaterial;
use fruits_transform::GlobalTransform;
use wgpu::*;
use fruits_render_core::RenderState;

use crate::StandardLightComponent;
use crate::{
    StandardMaterial, StandardRenderResource,
};

pub const GIZMO_LINES_PER_DRAW_MAX: usize = 1024 * 16;
pub const INSTANCES_PER_DRAW_MAX: usize = 1024 * 8;
pub const TRIANGLES_PER_BATCHED_DRAW_MAX: usize = 1024 * 32;
pub const LIGHTS_COUNT_MAX: usize = 1024;

pub const DEPTH_TEXTURE_FORMAT: TextureFormat = TextureFormat::Depth32Float;
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
    render_state: &RenderState,
    window_to_clip_mat: Mat4<f32>,
) -> (&'a RenderPipeline, &'b BindGroup) {
    let material_native = unsafe { material.native() };
    let material = material.meta();
    let world_to_clip = match material.space {
        RenderSpace::Clip => Mat4::IDENTITY,
        RenderSpace::Window => window_to_clip_mat,
        RenderSpace::World => standard_render_res.camera_proj_matrix,
    };

    let uniform = StandardUniformMaterial {
        color: srgb_to_linear(material.color),
        emission_color: srgb_to_linear(material.emission_color),
        metallic: material.metallic,
        roughness: material.roughness,
        alpha_threshold: material.alpha_threshold.into_option().unwrap_or(0.0),
        world_to_clip,
        _padding: Default::default(),
    };

    // optimize - only write when there are changes.
    render_state.queue().write_buffer(
        &material_native.buffer_uniform,
        0,
        fruits_utils::mem::as_bytes(&[uniform]),
    );

    let render_pipeline = match (material.is_lit, material.alpha_threshold.is_some()) {
        (true, true) => &standard_render_res.render_pipeline_opaque_lit,
        (false, true) => &standard_render_res.render_pipeline_opaque_unlit,
        (true, false) => &standard_render_res.render_pipeline_transparent_lit,
        (false, false) => &standard_render_res.render_pipeline_transparent_unlit,
    };

    (&render_pipeline, &material_native.bind_group)
}

fn srgb_to_linear(color: Vec4<f32>) -> Vec4<f32> {
    Vec4::new(color.x.powf(2.2), color.y.powf(2.2), color.z.powf(2.2), color.w)
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