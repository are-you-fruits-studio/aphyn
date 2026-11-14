struct Uniforms {
    world_to_clip: mat4x4<f32>,
    camera_position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.pos = uniforms.world_to_clip * vec4<f32>(vertex.position, 1.0);
    out.world_position = vertex.position;
    out.normal = vertex.normal;
    out.color = vertex.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_dir = normalize(uniforms.camera_position.xyz - in.world_position);
    let shade = 0.6 + 0.4 * abs(dot(normalize(in.normal), view_dir));

    return vec4<f32>(in.color.rgb * shade, 1.0);
}
