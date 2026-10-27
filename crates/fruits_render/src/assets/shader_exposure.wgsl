#include "fruits/fullscreen.wgsl"

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> multiplier: f32;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let p = fullscreen_triangle_position(in.vertex_index);

    out.position = vec4f(p, 0.0, 1.0);
    out.uv = fullscreen_uv(p);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let src_color = textureSampleLevel(src_texture, src_sampler, in.uv, 0.0);

    return vec4f(src_color.xyz * multiplier, 1.0);
}
