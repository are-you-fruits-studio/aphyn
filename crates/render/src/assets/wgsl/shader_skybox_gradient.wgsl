#include "aphyn/skybox.wgsl"

const ZENITH_COLOR: vec3<f32> = vec3<f32>(0.15, 0.35, 0.8);
const HORIZON_COLOR: vec3<f32> = vec3<f32>(0.7, 0.8, 0.9);
const GROUND_COLOR: vec3<f32> = vec3<f32>(0.25, 0.22, 0.2);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> SkyboxVertex {
    return skybox_vertex(vertex_index);
}

@fragment
fn fs_main(in: SkyboxVertex) -> @location(0) vec4<f32> {
    let y = skybox_direction(in.ndc).y;

    let sky = mix(HORIZON_COLOR, ZENITH_COLOR, pow(saturate(y), 0.5));
    let ground = mix(HORIZON_COLOR, GROUND_COLOR, pow(saturate(-y), 0.3));

    return vec4<f32>(select(ground, sky, y >= 0.0), 1.0);
}
