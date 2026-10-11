@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) position: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var pos = array<vec2f, 3>(
        vec2f(-1.0, -3.0),
        vec2f(3.0, 1.0),
        vec2f(-1.0, 1.0)
    );

    var out: VertexOutput;
    out.clip_position = vec4f(pos[vertex_index], 0.0, 1.0);
    out.position = pos[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let uv = in.position * vec2f(0.5, -0.5) + vec2f(0.5, 0.5);
    let src_color = textureSampleLevel(src_texture, src_sampler, uv, 0.0);

    return vec4f(src_color.xyz, 1.0);
}
