struct GlobalData {
    matrix_world_to_clip: mat4x4<f32>,
    matrix_window_to_clip: mat4x4<f32>,
    camera_position_world: vec3<f32>,
    lights_count: u32,
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
