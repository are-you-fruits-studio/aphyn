struct GlobalData {
    matrix_world_to_clip: mat4x4<f32>,
    matrix_window_to_clip: mat4x4<f32>,
    matrix_world_to_view: mat4x4<f32>,
    matrix_clip_to_view: mat4x4<f32>,
    camera_position_world: vec3<f32>,
    lights_count: u32,
    ambient_color: vec3<f32>,
    shadow_bias: f32,
    camera_forward_world: vec3<f32>,
    shadow_normal_offset: f32,
    cluster_dims: vec3<u32>,
    directional_lights_count: u32,
    target_size: vec2<f32>,
    cluster_near: f32,
    cluster_far: f32,
    cluster_index_capacity: u32,
}

struct ClusterCell {
    index: u32,
    count: u32,
}

struct MaterialData {
    color: vec4<f32>,
    emission_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    alpha_threshold: f32,
}

struct GenericLight {
    color: vec3<f32>,
    light_type: u32,
    center: vec3<f32>,
    range: f32,
    direction_dst: vec3<f32>,
    fov: f32,
    shadow_start: u32,
    shadow_count: u32,
}

struct ShadowView {
    matrix_world_to_clip: mat4x4<f32>,
    // uv offset in xy, uv scale in zw
    atlas_rect: vec4<f32>,
    split_far: f32,
}

struct VertexAttributes {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) uv: vec2<f32>,
};

struct InstanceRawAttributes {
    @location(5) local_to_world_c0: vec4<f32>,
    @location(6) local_to_world_c1: vec4<f32>,
    @location(7) local_to_world_c2: vec4<f32>,
    @location(8) local_to_world_c3: vec4<f32>,
};

struct InstanceAttributes {
    local_to_world: mat4x4<f32>,
};

fn map_instance_data(input: InstanceRawAttributes) -> InstanceAttributes {
    var output: InstanceAttributes;

    output.local_to_world = mat4x4<f32>(
        input.local_to_world_c0,
        input.local_to_world_c1,
        input.local_to_world_c2,
        input.local_to_world_c3
    );

    return output;
}

// octahedral mapping of a unit vector to [-1, 1]^2
fn octahedral_encode(normal: vec3<f32>) -> vec2<f32> {
    let n = normal / (abs(normal.x) + abs(normal.y) + abs(normal.z));
    let wrapped = (1.0 - abs(n.yx)) * select(vec2<f32>(-1.0), vec2<f32>(1.0), n.xy >= vec2<f32>(0.0));

    return select(wrapped, n.xy, n.z >= 0.0);
}

fn octahedral_decode(encoded: vec2<f32>) -> vec3<f32> {
    var n = vec3<f32>(encoded, 1.0 - abs(encoded.x) - abs(encoded.y));
    let t = saturate(-n.z);
    n = vec3<f32>(n.xy + select(vec2<f32>(t), vec2<f32>(-t), n.xy >= vec2<f32>(0.0)), n.z);

    return normalize(n);
}

// the uv of a direction in an octahedral environment map, matching the skybox's octahedral render
fn octahedral_environment_uv(direction: vec3<f32>) -> vec2<f32> {
    let encoded = octahedral_encode(direction);

    return vec2<f32>(encoded.x * 0.5 + 0.5, 0.5 - encoded.y * 0.5);
}
