#include "fruits/fullscreen.wgsl"

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) position: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let p = fullscreen_triangle_position(vertex_index);

    var out: VertexOutput;
    out.clip_position = vec4f(p, 0.0, 1.0);
    out.position = p;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let uv = fullscreen_uv(in.position);
    let src_color = textureSampleLevel(src_texture, src_sampler, uv, 0.0);

    return vec4f(src_color.xyz, 1.0);
}
