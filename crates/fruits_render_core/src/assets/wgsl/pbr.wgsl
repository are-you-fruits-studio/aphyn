#include "fruits/standard_types.wgsl"

const PI = radians(180.0);

struct FragmentLight {
    direction_src: vec3<f32>,
    color: vec3<f32>,
}

fn light_generic_to_fragment(light: GenericLight, fragment_pos_world: vec3<f32>) -> FragmentLight {
    var output: FragmentLight;

    output.color = light.color;

    if (light.light_type == 0) {
        // point
        output.direction_src = normalize(light.center - fragment_pos_world);
        var distance_vec = light.center - fragment_pos_world;
        var distance_sqr = dot(distance_vec, distance_vec);
        output.color /= distance_sqr;
    } else if (light.light_type == 1) {
        // spot
        output.direction_src = normalize(light.center - fragment_pos_world);
        var distance_vec = light.center - fragment_pos_world;
        var distance_sqr = dot(distance_vec, distance_vec);
        output.color /= distance_sqr;
        var projected_fragment_pos = light.center + light.direction_dst * dot(fragment_pos_world - light.center, light.direction_dst);
        var angle = acos(distance(light.center, projected_fragment_pos) / distance(light.center, fragment_pos_world)) * 2.0;
        output.color *= step(angle, light.fov);
    } else if (light.light_type == 2) {
        // directional
        output.direction_src = -light.direction_dst;
    } else {
        output.direction_src = vec3<f32>(0.0, 1.0, 0.0);
    }

    return output;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - cos_theta, 5.0);
}

fn ggx_distribution(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let nh = max(dot(n, h), 0.0);
    let denom = (nh * nh) * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denom * denom);
}

fn geometry_schlick_ggx(n: vec3<f32>, v: vec3<f32>, roughness: f32) -> f32 {
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let nv = max(dot(n, v), 0.0);
    return nv / (nv * (1.0 - k) + k);
}

fn cook_torrance_brdf(
    n: vec3<f32>,
    v: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    light_direction_src: vec3<f32>,
    light_color: vec3<f32>
) -> vec3<f32> {
    let h = normalize(v + light_direction_src);

    let nv = max(dot(n, v), 0.0);
    let nl = max(dot(n, light_direction_src), 0.0);

    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let fresnel = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let d = ggx_distribution(n, h, roughness);
    let g = geometry_schlick_ggx(n, v, roughness) * geometry_schlick_ggx(n, light_direction_src, roughness);

    let specular = (d * g * fresnel) / max(4.0 * nv * nl, 0.001);

    let k_d = (1.0 - fresnel) * (1.0 - metallic);
    let diffuse = (k_d * albedo) / PI;

    return (diffuse + specular) * light_color * nl;
}

fn create_matrix_tbn(normal: vec3<f32>, tangent_handedness: vec4<f32>) -> mat3x3<f32> {
    let N = normalize(normal);
    var T = normalize(tangent_handedness.xyz);
    T = normalize(T - N * dot(N, T));
    let B = cross(N, T) * tangent_handedness.w;
    return mat3x3<f32>(T, B, N);
}
