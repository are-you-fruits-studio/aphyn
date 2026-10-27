#include "aphyn/standard_types.wgsl"

@group(0) @binding(0) var<uniform> global_data: GlobalData;
@group(0) @binding(1) var<storage, read> lights: array<GenericLight>;

@group(1) @binding(0) var<uniform> material_data: MaterialData;
@group(1) @binding(1) var color_texture: texture_2d<f32>;
@group(1) @binding(2) var color_sampler: sampler;
@group(1) @binding(3) var roughness_texture: texture_2d<f32>;
@group(1) @binding(4) var roughness_sampler: sampler;
@group(1) @binding(5) var metallic_texture: texture_2d<f32>;
@group(1) @binding(6) var metallic_sampler: sampler;
@group(1) @binding(7) var normal_texture: texture_2d<f32>;
@group(1) @binding(8) var normal_sampler: sampler;
@group(1) @binding(9) var emission_texture: texture_2d<f32>;
@group(1) @binding(10) var emission_sampler: sampler;
