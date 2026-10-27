#include "aphyn/fullscreen.wgsl"

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    return vec4f(fullscreen_triangle_position(vertex_index), 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let dim = textureDimensions(src_texture);
    let uv = pos.xy / vec2f(f32(dim.x), f32(dim.y));
    let src_color = textureSampleLevel(src_texture, src_sampler, uv, 0.0);

    let alpha = saturate(src_color.w);
    let rgb = select(src_color.xyz / src_color.w, vec3f(0.0, 0.0, 0.0), alpha == 0.0);

    return vec4f(rgb, alpha);
}
