#include "aphyn/fullscreen.wgsl"
#include "aphyn/standard_types.wgsl"

struct SsaoUniform {
    view_to_clip: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    world_to_view: mat4x4<f32>,
    target_size: vec2<f32>,
    radius: f32,
    intensity: f32,
    bias: f32,
    sample_count: u32,
}

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) instance_index: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.position = vec4f(fullscreen_triangle_position(in.vertex_index), 0.0, 1.0);
    out.instance_index = in.instance_index;

    return out;
}

fn ssao_pixel_to_ndc(pixel: vec2<f32>, target_size: vec2<f32>) -> vec2<f32> {
    let uv = pixel / target_size;

    return vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
}

fn ssao_ndc_to_pixel(ndc: vec2<f32>, target_size: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * target_size;
}

// depth is reversed: 0 is the far plane, which is also what pixels nothing was drawn to hold
fn ssao_view_position(ssao: SsaoUniform, pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let view = ssao.clip_to_view * vec4<f32>(ssao_pixel_to_ndc(pixel, ssao.target_size), depth, 1.0);

    return view.xyz / view.w;
}
