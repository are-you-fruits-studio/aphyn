#include "aphyn/ssao.wgsl"

@group(0) @binding(0) var depth_texture: texture_depth_2d;
@group(0) @binding(1) var src_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> ssao: SsaoUniform;

const BLUR_RADIUS: i32 = 4;

fn view_depth(pixel: vec2<i32>) -> f32 {
    return ssao_view_position(ssao, vec2<f32>(pixel) + 0.5, textureLoad(depth_texture, pixel, 0)).z;
}

// instance 0 blurs horizontally, instance 1 vertically
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let pixel = vec2<i32>(in.position.xy);
    let direction = select(vec2<i32>(1, 0), vec2<i32>(0, 1), in.instance_index == 1u);
    let max_pixel = vec2<i32>(ssao.target_size) - 1;
    let center_depth = view_depth(pixel);

    var sum = 0.0;
    var weight_sum = 0.0;

    for (var i: i32 = -BLUR_RADIUS; i <= BLUR_RADIUS; i++) {
        let tap = clamp(pixel + direction * i, vec2<i32>(0), max_pixel);
        let spatial = exp(-f32(i * i) / (2.0 * 2.5 * 2.5));
        // taps across a depth edge barely count, so occlusion doesn't bleed onto the other surface
        let depth_difference = abs(view_depth(tap) - center_depth) / max(ssao.radius, 1e-4);
        let weight = spatial * exp(-depth_difference * depth_difference * 16.0);

        sum += textureLoad(src_texture, tap, 0).x * weight;
        weight_sum += weight;
    }

    let ambient_occlusion = sum / max(weight_sum, 1e-4);

    return vec4f(ambient_occlusion, ambient_occlusion, ambient_occlusion, 1.0);
}
