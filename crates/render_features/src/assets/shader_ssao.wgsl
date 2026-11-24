#include "aphyn/ssao.wgsl"

@group(0) @binding(0) var depth_texture: texture_depth_2d;
@group(0) @binding(1) var scene_normals: texture_2d<f32>;
@group(0) @binding(2) var<uniform> ssao: SsaoUniform;
@group(0) @binding(3) var<storage, read> kernel: array<vec4<f32>>;

fn interleaved_gradient_noise(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_texture, pixel, 0);

    if (depth <= 0.0) {
        return vec4f(1.0);
    }

    let position = ssao_view_position(ssao, in.position.xy, depth);
    let normal_world = octahedral_decode(textureLoad(scene_normals, pixel, 0).xy);
    let normal = normalize((ssao.world_to_view * vec4<f32>(normal_world, 0.0)).xyz);

    // a per-pixel rotation of the kernel around the normal, smoothed away by the blur
    let angle = 6.2831853 * interleaved_gradient_noise(in.position.xy);
    let random = vec3<f32>(cos(angle), sin(angle), 0.0);
    var tangent = random - normal * dot(random, normal);
    if (dot(tangent, tangent) < 1e-6) {
        tangent = cross(normal, vec3<f32>(0.0, 0.0, 1.0));
    }
    tangent = normalize(tangent);
    let tbn = mat3x3<f32>(tangent, cross(normal, tangent), normal);

    let count = min(ssao.sample_count, arrayLength(&kernel));
    var occlusion = 0.0;

    for (var i: u32 = 0; i < count; i++) {
        let sample = position + tbn * kernel[i].xyz * ssao.radius;
        let clip = ssao.view_to_clip * vec4<f32>(sample, 1.0);

        if (clip.w <= 0.0) {
            continue;
        }

        let sample_pixel = ssao_ndc_to_pixel(clip.xy / clip.w, ssao.target_size);

        if (any(sample_pixel < vec2<f32>(0.0)) || any(sample_pixel >= ssao.target_size)) {
            continue;
        }

        let scene_depth = textureLoad(depth_texture, vec2<i32>(sample_pixel), 0);
        let scene = ssao_view_position(ssao, floor(sample_pixel) + 0.5, scene_depth);

        // view space looks down +z: the scene occludes the sample when it is closer to the camera
        let is_occluded = scene.z <= sample.z - ssao.bias;
        let range = smoothstep(0.0, 1.0, ssao.radius / max(abs(position.z - scene.z), 1e-4));

        occlusion += select(0.0, range, is_occluded);
    }

    let ambient_occlusion = saturate(1.0 - occlusion / f32(max(count, 1u)) * ssao.intensity);

    return vec4f(ambient_occlusion, ambient_occlusion, ambient_occlusion, 1.0);
}
