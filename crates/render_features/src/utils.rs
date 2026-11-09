use std::ops::Range;

use aphyn_math::{Mat4, Vec3};
use aphyn_render::{StandardLight, WgslComposer};
use aphyn_render_core::{
    GpuBinding, GpuBlendState, GpuPassOps, GpuRenderPass, GpuRenderPassTarget, GpuShader, GpuShaderDesc, GpuTexture, GpuViewport,
    RenderApi,
};
use aphyn_transform::GlobalTransform;

use crate::StandardLightComponent;

pub const GIZMO_LINES_PER_DRAW_MAX: usize = 1024 * 16;

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

pub fn render_wgsl_composer() -> WgslComposer {
    let mut composer = WgslComposer::with_engine_libraries();
    composer.add_library("aphyn/bloom.wgsl", include_str!("./assets/bloom.wgsl"));
    composer
}

pub(crate) fn fullscreen_shader_desc(blend: GpuBlendState) -> GpuShaderDesc {
    GpuShaderDesc {
        blend,
        depth_write: false,
        ..Default::default()
    }
}

pub(crate) fn create_engine_shader(core: &RenderApi, name: &str, source: &str, desc: GpuShaderDesc) -> GpuShader {
    let code = render_wgsl_composer().compose(name, source, &[]).unwrap();

    core.create_shader(Some(name), &code, desc).unwrap()
}

pub(crate) fn texture_binding(texture: &GpuTexture) -> GpuBinding<'_> {
    GpuBinding::Texture {
        texture,
        mip_level: None,
    }
}

pub(crate) fn draw_fullscreen(
    core: &RenderApi,
    shader: &GpuShader,
    target: &GpuTexture,
    ops: GpuPassOps<[f32; 4]>,
    bindings: &[(usize, GpuBinding)],
    viewport: Option<GpuViewport>,
    instances: Range<usize>,
) -> bool {
    core.draw(
        &GpuRenderPass {
            shader,
            target: Some(GpuRenderPassTarget {
                view: target,
                mip_level: 0,
                ops,
            }),
            depth: None,
            bind_groups: &[bindings],
            vertex_buffers: &[],
            viewport,
            scissor: None,
        },
        0..3,
        instances,
    )
}

#[cfg(test)]
mod tests {
    use super::render_wgsl_composer;

    #[test]
    fn render_shaders_are_valid() {
        let composer = render_wgsl_composer();

        let shaders: &[(&str, &str)] = &[
            (
                "shader_transparent_final.wgsl",
                include_str!("./assets/shader_transparent_final.wgsl"),
            ),
            ("shader_exposure.wgsl", include_str!("./assets/shader_exposure.wgsl")),
            (
                "shader_bloom_gather_threshold.wgsl",
                include_str!("./assets/shader_bloom_gather_threshold.wgsl"),
            ),
            ("shader_bloom_blur.wgsl", include_str!("./assets/shader_bloom_blur.wgsl")),
            (
                "shader_bloom_downscale.wgsl",
                include_str!("./assets/shader_bloom_downscale.wgsl"),
            ),
            ("shader_bloom_apply.wgsl", include_str!("./assets/shader_bloom_apply.wgsl")),
            (
                "shader_color_grading_aces.wgsl",
                include_str!("./assets/shader_color_grading_aces.wgsl"),
            ),
            ("shader_gizmo.wgsl", include_str!("./assets/shader_gizmo.wgsl")),
        ];

        for (name, source) in shaders {
            let code = composer.compose(name, source, &[]).unwrap();
            let module =
                naga::front::wgsl::parse_str(&code).unwrap_or_else(|error| panic!("{name}\n{}", error.emit_to_string(&code)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{name}\n{}", error.emit_to_string(&code)));
        }
    }
}
