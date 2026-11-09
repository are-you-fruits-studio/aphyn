#include "aphyn/standard_types.wgsl"

struct UnlitMaterial {
    color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> global_data: GlobalData;
@group(1) @binding(0) var<uniform> material: UnlitMaterial;

@vertex
fn vs_main(@location(0) position: vec3<f32>, instance_raw: InstanceRawAttributes) -> @builtin(position) vec4<f32> {
    let instance = map_instance_data(instance_raw);

    return global_data.matrix_world_to_clip * instance.local_to_world * vec4<f32>(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return material.color;
}
