#include "aphyn/fullscreen.wgsl"

@group(0) @binding(0) var depth_texture: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    return vec4f(fullscreen_triangle_position(vertex_index), 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(depth_texture, vec2<i32>(position.xy), 0);
}
