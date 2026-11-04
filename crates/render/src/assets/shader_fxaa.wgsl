#include "aphyn/fullscreen.wgsl"

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

const LUMA_WEIGHTS = vec3f(0.299, 0.587, 0.114);
const CONTRAST_THRESHOLD_ABSOLUTE = 0.0312;
const CONTRAST_THRESHOLD_RELATIVE = 0.125;
const SUBPIXEL_BLENDING = 0.75;
const EDGE_STEPS_COUNT = 10u;
const EDGE_STEP_SIZES = array<f32, EDGE_STEPS_COUNT>(1.0, 1.5, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 4.0);
const EDGE_END_GUESS = 8.0;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EdgeEnd {
    travelled: f32,
    luma_delta: f32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let p = fullscreen_triangle_position(in.vertex_index);

    out.position = vec4f(p, 0.0, 1.0);
    out.uv = fullscreen_uv(p);

    return out;
}

// the target is linear at this point, sqrt approximates the perceptual curve contrast is judged on
fn luma_of(color: vec3f) -> f32 {
    return dot(sqrt(saturate(color)), LUMA_WEIGHTS);
}

fn luma_at(uv: vec2f) -> f32 {
    return luma_of(textureSampleLevel(src_texture, src_sampler, uv, 0.0).xyz);
}

fn find_edge_end(edge_uv: vec2f, along_edge: vec2f, edge_luma: f32, gradient_threshold: f32) -> EdgeEnd {
    var step_sizes = EDGE_STEP_SIZES;
    var travelled = 0.0;
    var luma_delta = 0.0;

    for (var i = 0u; i < EDGE_STEPS_COUNT; i++) {
        travelled += step_sizes[i];
        luma_delta = luma_at(edge_uv + along_edge * travelled) - edge_luma;

        if (abs(luma_delta) >= gradient_threshold) {
            return EdgeEnd(travelled, luma_delta);
        }
    }

    return EdgeEnd(travelled + EDGE_END_GUESS, luma_delta);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let texel = 1.0 / vec2f(textureDimensions(src_texture));
    let center = textureSampleLevel(src_texture, src_sampler, in.uv, 0.0);

    let m = luma_of(center.xyz);
    let n = luma_at(in.uv + vec2f(0.0, -texel.y));
    let s = luma_at(in.uv + vec2f(0.0, texel.y));
    let w = luma_at(in.uv + vec2f(-texel.x, 0.0));
    let e = luma_at(in.uv + vec2f(texel.x, 0.0));

    let luma_max = max(m, max(max(n, s), max(w, e)));
    let luma_min = min(m, min(min(n, s), min(w, e)));
    let contrast = luma_max - luma_min;

    if (contrast < max(CONTRAST_THRESHOLD_ABSOLUTE, CONTRAST_THRESHOLD_RELATIVE * luma_max)) {
        return center;
    }

    let nw = luma_at(in.uv + vec2f(-texel.x, -texel.y));
    let ne = luma_at(in.uv + vec2f(texel.x, -texel.y));
    let sw = luma_at(in.uv + vec2f(-texel.x, texel.y));
    let se = luma_at(in.uv + vec2f(texel.x, texel.y));

    let neighbourhood = (2.0 * (n + s + w + e) + nw + ne + sw + se) / 12.0;
    let subpixel_contrast = smoothstep(0.0, 1.0, saturate(abs(neighbourhood - m) / contrast));
    let subpixel_blend = subpixel_contrast * subpixel_contrast * SUBPIXEL_BLENDING;

    let gradient_across_y = 2.0 * abs(n + s - 2.0 * m) + abs(ne + se - 2.0 * e) + abs(nw + sw - 2.0 * w);
    let gradient_across_x = 2.0 * abs(w + e - 2.0 * m) + abs(nw + sw - 2.0 * n) + abs(ne + se - 2.0 * s);
    let is_edge_along_x = gradient_across_y >= gradient_across_x;

    let luma_positive = select(e, s, is_edge_along_x);
    let luma_negative = select(w, n, is_edge_along_x);
    let gradient_positive = abs(luma_positive - m);
    let gradient_negative = abs(luma_negative - m);
    let is_positive = gradient_positive >= gradient_negative;

    let along_edge = select(vec2f(0.0, texel.y), vec2f(texel.x, 0.0), is_edge_along_x);
    let across_edge = select(vec2f(texel.x, 0.0), vec2f(0.0, texel.y), is_edge_along_x) * select(-1.0, 1.0, is_positive);

    let edge_uv = in.uv + across_edge * 0.5;
    let edge_luma = 0.5 * (m + select(luma_negative, luma_positive, is_positive));
    let gradient_threshold = 0.25 * max(gradient_positive, gradient_negative);

    let end_positive = find_edge_end(edge_uv, along_edge, edge_luma, gradient_threshold);
    let end_negative = find_edge_end(edge_uv, -along_edge, edge_luma, gradient_threshold);

    let nearest_travelled = min(end_positive.travelled, end_negative.travelled);
    let nearest_delta = select(end_negative.luma_delta, end_positive.luma_delta, end_positive.travelled <= end_negative.travelled);

    var edge_blend = 0.5 - nearest_travelled / (end_positive.travelled + end_negative.travelled);

    if ((nearest_delta >= 0.0) == (m - edge_luma >= 0.0)) {
        edge_blend = 0.0;
    }

    let color = textureSampleLevel(src_texture, src_sampler, in.uv + across_edge * max(subpixel_blend, edge_blend), 0.0);

    return vec4f(color.xyz, center.w);
}
