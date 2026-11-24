#include "aphyn/standard_types.wgsl"

@group(0) @binding(0) var<uniform> global_data: GlobalData;
@group(0) @binding(1) var<storage, read> lights: array<GenericLight>;
@group(0) @binding(2) var<storage, read> shadow_views: array<ShadowView>;
@group(0) @binding(3) var shadow_atlas: texture_depth_2d;
@group(0) @binding(4) var shadow_sampler: sampler_comparison;
@group(0) @binding(5) var<storage, read> cluster_grid: array<ClusterCell>;
@group(0) @binding(6) var<storage, read> cluster_light_indices: array<u32>;
#ifndef DEPTH_NORMALS
@group(0) @binding(7) var scene_normals: texture_2d<f32>;
#endif
@group(0) @binding(8) var ambient_occlusion: texture_2d<f32>;

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
