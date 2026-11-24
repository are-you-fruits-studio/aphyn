#include "aphyn/standard_bindings.wgsl"

fn ssr_interleaved_gradient_noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

fn ssr_clip_to_uv(clip: vec4<f32>) -> vec2<f32> {
    let ndc = clip.xy / clip.w;

    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}

fn ssr_is_on_screen(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
}

fn ssr_view_depth(uv: vec2<f32>, depth: f32) -> f32 {
    let view = global_data.matrix_clip_to_view * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);

    return view.z / view.w;
}

fn ssr_depth_pixel(uv: vec2<f32>) -> vec2<u32> {
    let size = textureDimensions(ssr_depth);

    return min(vec2<u32>(uv * vec2<f32>(size)), size - 1u);
}

struct SsrSample {
    uv: vec2<f32>,
    is_on_screen: bool,
    // the ray is farther than the scene by more than the depth bias; depth is reversed, and pixels nothing was drawn to (0) never count
    is_behind: bool,
    is_within_thickness: bool,
}

fn ssr_sample(start_world: vec3<f32>, end_world: vec3<f32>, t: f32) -> SsrSample {
    var out: SsrSample;

    let clip = global_data.matrix_world_to_clip * vec4<f32>(mix(start_world, end_world, t), 1.0);
    out.uv = ssr_clip_to_uv(clip);
    out.is_on_screen = ssr_is_on_screen(out.uv);

    if (!out.is_on_screen) {
        return out;
    }

    let scene_depth = textureLoad(ssr_depth, ssr_depth_pixel(out.uv), 0);
    let ray_depth = clip.z / clip.w;

    let distance_behind = abs(ssr_view_depth(out.uv, ray_depth) - ssr_view_depth(out.uv, scene_depth));

    out.is_behind = scene_depth > 0.0 && ray_depth < scene_depth && distance_behind > ssr_data.depth_bias;
    out.is_within_thickness = distance_behind < ssr_data.thickness;

    return out;
}

// the ray's parameter at a fraction `u` of its screen-space segment (perspective-correct)
fn ssr_ray_t(u: f32, w_start: f32, w_end: f32) -> f32 {
    return u * w_start / max((1.0 - u) * w_end + u * w_start, 1e-6);
}

// rgb: the reflected color, a: how much of it replaces the environment's specular radiance
fn ssr_trace(position_world: vec3<f32>, normal_world: vec3<f32>, view_dir: vec3<f32>, roughness: f32, pixel: vec2<f32>) -> vec4<f32> {
    if (ssr_data.is_enabled == 0u || material_data.reflections <= 0.0 || roughness >= ssr_data.max_roughness) {
        return vec4<f32>(0.0);
    }

#ifndef TRANSPARENT
    // transparents read the history this frame's opaques were just copied into
    if (ssr_data.has_history == 0u) {
        return vec4<f32>(0.0);
    }
#endif

    let direction = reflect(-view_dir, normal_world);
    let facing_fade = 1.0 - smoothstep(0.0, 0.5, dot(direction, view_dir));

    if (facing_fade <= 0.0) {
        return vec4<f32>(0.0);
    }

    let start = position_world;
    var end = start + direction * ssr_data.max_distance;
    let clip_start = global_data.matrix_world_to_clip * vec4<f32>(start, 1.0);
    var clip_end = global_data.matrix_world_to_clip * vec4<f32>(end, 1.0);

    // reversed depth: points in front of the near plane have z > w
    let near_start = clip_start.w - clip_start.z;
    let near_end = clip_end.w - clip_end.z;

    if (near_end < 0.0) {
        end = mix(start, end, near_start / (near_start - near_end) * 0.99);
        clip_end = global_data.matrix_world_to_clip * vec4<f32>(end, 1.0);
    }

    let steps = max(ssr_data.max_steps, 1u);
    let jitter = ssr_interleaved_gradient_noise(pixel);

    var u_front = 0.0;
    var u_hit = -1.0;

    for (var i = 1u; i <= steps; i++) {
        let u = (f32(i) - jitter) / f32(steps);
        let step_sample = ssr_sample(start, end, ssr_ray_t(u, clip_start.w, clip_end.w));

        if (!step_sample.is_on_screen) {
            break;
        }

        if (step_sample.is_behind) {
            if (step_sample.is_within_thickness) {
                u_hit = u;
                break;
            }
        } else {
            u_front = u;
        }
    }

    if (u_hit < 0.0) {
        return vec4<f32>(0.0);
    }

    for (var i = 0; i < 5; i++) {
        let u = (u_front + u_hit) * 0.5;

        if (ssr_sample(start, end, ssr_ray_t(u, clip_start.w, clip_end.w)).is_behind) {
            u_hit = u;
        } else {
            u_front = u;
        }
    }

    let t_hit = ssr_ray_t(u_hit, clip_start.w, clip_end.w);
    let hit = ssr_sample(start, end, t_hit);

    if (!hit.is_within_thickness) {
        return vec4<f32>(0.0);
    }

    // the ray hit the back of a surface
    let hit_normal = octahedral_decode(textureLoad(scene_normals, ssr_depth_pixel(hit.uv), 0).xy);

    if (dot(hit_normal, direction) > 0.0) {
        return vec4<f32>(0.0);
    }

    let hit_world = vec4<f32>(mix(start, end, t_hit), 1.0);
#ifdef TRANSPARENT
    let history_clip = global_data.matrix_world_to_clip * hit_world;
#else
    let history_clip = ssr_data.matrix_world_to_clip_previous * hit_world;
#endif

    if (history_clip.w <= 0.0) {
        return vec4<f32>(0.0);
    }

    let history_uv = ssr_clip_to_uv(history_clip);

    if (!ssr_is_on_screen(history_uv)) {
        return vec4<f32>(0.0);
    }

    let edge = min(min(hit.uv, 1.0 - hit.uv), min(history_uv, 1.0 - history_uv));
    let edge_fade = saturate(min(edge.x, edge.y) / max(ssr_data.edge_fade, 1e-4));
    let distance_fade = 1.0 - smoothstep(0.8, 1.0, t_hit * length(end - start) / max(ssr_data.max_distance, 1e-4));
    let roughness_fade = 1.0 - smoothstep(ssr_data.max_roughness * 0.7, ssr_data.max_roughness, roughness);

    let max_lod = f32(textureNumLevels(ssr_history) - 1u);
    let color = textureSampleLevel(ssr_history, ssr_sampler, history_uv, roughness * max_lod).xyz;
    let confidence = edge_fade * distance_fade * roughness_fade * facing_fade * ssr_data.intensity * material_data.reflections;

    return vec4<f32>(color, saturate(confidence));
}
