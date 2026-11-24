#include "aphyn/standard_types.wgsl"
#include "aphyn/fullscreen.wgsl"

struct SkyboxData {
    matrix_clip_to_world: mat4x4<f32>,
    is_octahedral: u32,
    _padding_0: u32,
    _padding_1: u32,
    _padding_2: u32,
}

@group(0) @binding(0) var<uniform> skybox_data: SkyboxData;

struct SkyboxVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

// depth 0 is the far plane: with the default GreaterEqual test only pixels nothing was drawn into pass
fn skybox_vertex(vertex_index: u32) -> SkyboxVertex {
    var out: SkyboxVertex;

    let p = fullscreen_triangle_position(vertex_index);

    out.position = vec4<f32>(p, 0.0, 1.0);
    out.ndc = p;

    return out;
}

fn skybox_direction(ndc: vec2<f32>) -> vec3<f32> {
    if (skybox_data.is_octahedral != 0u) {
        return octahedral_decode(ndc);
    }

    let near = skybox_data.matrix_clip_to_world * vec4<f32>(ndc, 1.0, 1.0);
    let far = skybox_data.matrix_clip_to_world * vec4<f32>(ndc, 0.5, 1.0);

    return normalize(far.xyz / far.w - near.xyz / near.w);
}
