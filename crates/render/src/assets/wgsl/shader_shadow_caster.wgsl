#include "aphyn/standard_types.wgsl"

struct ShadowCasterData {
    matrix_world_to_clip: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> shadow_caster_data: ShadowCasterData;

@vertex
fn vs_main(vertex: VertexAttributes, instance_raw: InstanceRawAttributes) -> @builtin(position) vec4<f32> {
    let instance = map_instance_data(instance_raw);

    return shadow_caster_data.matrix_world_to_clip * instance.local_to_world * vec4<f32>(vertex.position, 1.0);
}

@fragment
fn fs_main() {}
