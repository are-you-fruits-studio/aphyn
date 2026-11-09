@group(0) @binding(0) var<storage, read> vertex: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> color: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> transform: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var out: VertexOutput;

    out.pos = transform * vertex[index];
    out.color = color[index / 2];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}