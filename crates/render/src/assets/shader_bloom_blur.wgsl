#include "aphyn/fullscreen.wgsl"
#include "aphyn/bloom.wgsl"

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) instance_index: u32,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let p = fullscreen_triangle_position(in.vertex_index);

    out.position = vec4f(p, 0.0, 1.0);
    out.instance_index = in.instance_index;
    out.uv = fullscreen_uv(p);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let layer = storage_layers[in.instance_index];

    let uv = in.uv * layer.uv_scale_offset.xy + layer.uv_scale_offset.zw;

    let texel = 1.0 / (vec2<f32>(textureDimensions(src_texture).xy));

    let offset = texel * layer.direction;

    let weights = array<f32, 3>(
        0.1225813374135857,
        0.2330624034988859,
        0.2887125181750568,
    );

    var sample = vec3<f32>(0.0);
    sample += textureSample(src_texture, src_sampler, uv - 2.0 * offset).xyz * weights[0];
    sample += textureSample(src_texture, src_sampler, uv - 1.0 * offset).xyz * weights[1];
    sample += textureSample(src_texture, src_sampler, uv + 0.0 * offset).xyz * weights[2];
    sample += textureSample(src_texture, src_sampler, uv + 1.0 * offset).xyz * weights[1];
    sample += textureSample(src_texture, src_sampler, uv + 2.0 * offset).xyz * weights[0];

    return vec4f(sample, 1.0);
}
