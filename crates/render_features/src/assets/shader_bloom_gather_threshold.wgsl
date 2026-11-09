#include "aphyn/fullscreen.wgsl"
#include "aphyn/bloom.wgsl"

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) instance_index: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.position = vec4f(fullscreen_triangle_position(in.vertex_index), 0.0, 1.0);
    out.instance_index = in.instance_index;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dim = textureDimensions(src_texture);
    let uv = in.position.xy / vec2f(f32(dim.x), f32(dim.y));
    let src_color = textureSampleLevel(src_texture, src_sampler, uv, 0.0);

    let luminance = src_color.x + src_color.y + src_color.z;

    let threshold = bloom_uniform.threshold;

    let epsilon = abs(threshold.y * 0.5);

    let t = smoothstep(threshold.x - epsilon, threshold.x + epsilon, luminance);

    return vec4f(mix(vec3f(0.0, 0.0, 0.0), src_color.xyz, t), 1.0);
}
