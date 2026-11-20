#include "aphyn/standard_types.wgsl"

struct ClusterAabb {
    min: vec3<f32>,
    max: vec3<f32>,
}

struct ClusterLight {
    center: vec3<f32>,
    range: f32,
    direction: vec3<f32>,
    is_spot: bool,
    half_angle_cos: f32,
    half_angle_sin: f32,
}

fn cluster_slice_depth(g: GlobalData, slice: u32) -> f32 {
    return g.cluster_near * pow(g.cluster_far / g.cluster_near, f32(slice) / f32(g.cluster_dims.z));
}

fn cluster_index(g: GlobalData, frag_coord: vec2<f32>, view_z: f32) -> u32 {
    let dims = g.cluster_dims;
    let tile = min(vec2<u32>(max(frag_coord / g.target_size * vec2<f32>(dims.xy), vec2<f32>(0.0))), dims.xy - vec2<u32>(1u));
    let slice_f = log(max(view_z, g.cluster_near) / g.cluster_near) * f32(dims.z) / log(g.cluster_far / g.cluster_near);
    let slice = min(u32(max(slice_f, 0.0)), dims.z - 1u);

    return tile.x + tile.y * dims.x + slice * dims.x * dims.y;
}

fn cluster_index_world(g: GlobalData, frag_coord: vec2<f32>, position_world: vec3<f32>) -> u32 {
    let view_z = (g.matrix_world_to_view * vec4<f32>(position_world, 1.0)).z;

    return cluster_index(g, frag_coord, view_z);
}

fn cluster_view_point(g: GlobalData, ndc: vec2<f32>, ndc_z: f32) -> vec3<f32> {
    let p = g.matrix_clip_to_view * vec4<f32>(ndc, ndc_z, 1.0);

    return p.xyz / p.w;
}

// the first and last slices reach the camera's own near and far planes, which clamped fragments need
fn cluster_aabb(g: GlobalData, cluster: u32) -> ClusterAabb {
    let dims = g.cluster_dims;
    let x = cluster % dims.x;
    let y = (cluster / dims.x) % dims.y;
    let z = cluster / (dims.x * dims.y);

    let ndc_x0 = f32(x) / f32(dims.x) * 2.0 - 1.0;
    let ndc_x1 = f32(x + 1u) / f32(dims.x) * 2.0 - 1.0;
    // tile rows go down the screen, NDC goes up
    let ndc_y0 = 1.0 - f32(y + 1u) / f32(dims.y) * 2.0;
    let ndc_y1 = 1.0 - f32(y) / f32(dims.y) * 2.0;

    let corners = array<vec2<f32>, 4>(
        vec2<f32>(ndc_x0, ndc_y0),
        vec2<f32>(ndc_x1, ndc_y0),
        vec2<f32>(ndc_x0, ndc_y1),
        vec2<f32>(ndc_x1, ndc_y1),
    );

    let near_z = cluster_view_point(g, vec2<f32>(0.0), 1.0).z;
    let far_z = cluster_view_point(g, vec2<f32>(0.0), 0.0).z;
    let t_lo = select((cluster_slice_depth(g, z) - near_z) / (far_z - near_z), 0.0, z == 0u);
    let t_hi = select((cluster_slice_depth(g, z + 1u) - near_z) / (far_z - near_z), 1.0, z + 1u >= dims.z);

    var aabb: ClusterAabb;
    aabb.min = vec3<f32>(3.4e38);
    aabb.max = vec3<f32>(-3.4e38);

    for (var i = 0u; i < 4u; i++) {
        let p0 = cluster_view_point(g, corners[i], 1.0);
        let p1 = cluster_view_point(g, corners[i], 0.0);
        let a = mix(p0, p1, t_lo);
        let b = mix(p0, p1, t_hi);

        aabb.min = min(aabb.min, min(a, b));
        aabb.max = max(aabb.max, max(a, b));
    }

    return aabb;
}

fn cluster_light(g: GlobalData, light: GenericLight) -> ClusterLight {
    var out: ClusterLight;

    out.center = (g.matrix_world_to_view * vec4<f32>(light.center, 1.0)).xyz;
    // view space keeps the camera's scale
    out.range = light.range * length((g.matrix_world_to_view * vec4<f32>(1.0, 0.0, 0.0, 0.0)).xyz);

    let half_angle = light.fov * 0.5;
    // a cone of 180 degrees or more culls like a sphere
    out.is_spot = light.light_type == 1u && half_angle < 1.57;

    if (out.is_spot) {
        out.direction = normalize((g.matrix_world_to_view * vec4<f32>(light.direction_dst, 0.0)).xyz);
        out.half_angle_cos = cos(half_angle);
        out.half_angle_sin = sin(half_angle);
    }

    return out;
}

fn cluster_light_intersects(light: ClusterLight, aabb: ClusterAabb) -> bool {
    let closest = clamp(light.center, aabb.min, aabb.max);
    let to_closest = closest - light.center;

    if (dot(to_closest, to_closest) > light.range * light.range) {
        return false;
    }

    if (!light.is_spot) {
        return true;
    }

    // cone vs the box's bounding sphere
    let sphere_center = (aabb.min + aabb.max) * 0.5;
    let sphere_radius = length(aabb.max - aabb.min) * 0.5;
    let v = sphere_center - light.center;
    let v_len_sq = dot(v, v);
    let v1_len = dot(v, light.direction);
    let distance_closest = light.half_angle_cos * sqrt(max(v_len_sq - v1_len * v1_len, 0.0)) - v1_len * light.half_angle_sin;

    let angle_cull = distance_closest > sphere_radius;
    let front_cull = v1_len > sphere_radius + light.range;
    let back_cull = v1_len < -sphere_radius;

    return !(angle_cull || front_cull || back_cull);
}
