#include "aphyn/fullscreen.wgsl"

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let position = fullscreen_triangle_position(vertex_index);

    var out: VertexOutput;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = fullscreen_uv(position);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source_texture, source_sampler, in.uv);
    let distance_to_center = distance(in.uv, vec2<f32>(0.5, 0.5));
    let darkening = 1.0 - smoothstep(0.35, 0.75, distance_to_center);

    return vec4<f32>(color.rgb * darkening, color.a);
}
