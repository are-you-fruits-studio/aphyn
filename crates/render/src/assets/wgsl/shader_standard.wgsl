#include "aphyn/standard_bindings.wgsl"
#include "aphyn/pbr.wgsl"
#include "aphyn/shadows.wgsl"
#include "aphyn/clusters.wgsl"

@vertex
fn vs_main(vertex: VertexAttributes, instance_raw: InstanceRawAttributes) -> VertexOutput {
    var instance = map_instance_data(instance_raw);

    return customer_vertex(vertex, instance);
}

#ifdef DEPTH_NORMALS
struct VertexOutput {
    @builtin(position) position_clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal_world: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

fn customer_vertex(vertex: VertexAttributes, instance: InstanceAttributes) -> VertexOutput {
    var out: VertexOutput;

    var position_world = instance.local_to_world * vec4<f32>(vertex.position, 1.0);

    out.position_clip = global_data.matrix_world_to_clip * position_world;
    out.color = vertex.color * material_data.color;
    out.normal_world = (instance.local_to_world * vec4<f32>(vertex.normal, 0.0)).xyz;
    out.uv = vertex.uv;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = in.color * textureSample(color_texture, color_sampler, in.uv);

    if (color.w < material_data.alpha_threshold) {
        discard;
    }

    return vec4<f32>(octahedral_encode(normalize(in.normal_world)), 0.0, 0.0);
}
#else
#ifdef LIT
struct VertexOutput {
    @builtin(position) position_clip: vec4<f32>,
    @location(0) color: vec4<f32>,
#ifdef TRANSPARENT
    @location(1) normal_world: vec3<f32>,
#endif
    @location(2) tangent_world: vec4<f32>,
    @location(3) position_world: vec3<f32>,
    @location(4) uv: vec2<f32>,
};

fn customer_vertex(vertex: VertexAttributes, instance: InstanceAttributes) -> VertexOutput {
    var out: VertexOutput;

    var position_world = instance.local_to_world * vec4<f32>(vertex.position, 1.0);

    out.position_clip = global_data.matrix_world_to_clip * position_world;
    out.color = vertex.color * material_data.color;
#ifdef TRANSPARENT
    out.normal_world = (instance.local_to_world * vec4<f32>(vertex.normal, 0.0)).xyz;
#endif
    out.tangent_world = vec4<f32>((instance.local_to_world * vec4<f32>(vertex.tangent.xyz, 0.0)).xyz, vertex.tangent.w);
    out.position_world = position_world.xyz;
    out.uv = vertex.uv;

    return out;
}

// opaque geometry reuses the normal of the depth-normals prepass
fn geometric_normal_world(in: VertexOutput) -> vec3<f32> {
#ifdef TRANSPARENT
    return normalize(in.normal_world);
#else
    return octahedral_decode(textureLoad(scene_normals, vec2<i32>(in.position_clip.xy), 0).xy);
#endif
}
#else
struct VertexOutput {
    @builtin(position) position_clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

fn customer_vertex(vertex: VertexAttributes, instance: InstanceAttributes) -> VertexOutput {
    var out: VertexOutput;

    var position_world = instance.local_to_world * vec4<f32>(vertex.position, 1.0);

#ifdef UI
    out.position_clip = global_data.matrix_window_to_clip * position_world;
#else
    out.position_clip = global_data.matrix_world_to_clip * position_world;
#endif
    out.color = vertex.color * material_data.color;
    out.uv = vertex.uv;

    return out;
}
#endif

#ifdef LIT
fn shade_light(
    light_index: u32,
    in: VertexOutput,
    geometric_normal_world: vec3<f32>,
    normal_world: vec3<f32>,
    view_dir: vec3<f32>,
    diffuse_color: vec3<f32>,
    metallic: f32,
    roughness: f32
) -> vec3<f32> {
    let light = light_generic_to_fragment(lights[light_index], in.position_world);

    let color_lit = cook_torrance_brdf(
        normal_world,
        view_dir,
        diffuse_color,
        metallic,
        roughness,
        light.direction_src,
        light.color
    );

    return color_lit * light_shadow(lights[light_index], in.position_world, geometric_normal_world);
}
#endif

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = in.color * textureSample(color_texture, color_sampler, in.uv);

#ifndef TRANSPARENT
    if (color.w < material_data.alpha_threshold) {
        discard;
    }

    color.w = 1.0;
#endif

#ifdef LIT
    var diffuse_color = color.xyz;

    var emission = material_data.emission_color.xyz * material_data.emission_color.w * textureSample(emission_texture, emission_sampler, in.uv).xyz;

    color = vec4<f32>(emission, color.w);

    let geometric_normal = geometric_normal_world(in);
    let normal_tangent_space = textureSample(normal_texture, normal_sampler, in.uv).xyz * 2.0 - 1.0;
    let TBN = create_matrix_tbn(geometric_normal, in.tangent_world);
    let normal_world = normalize(TBN * normal_tangent_space);

    var view_dir = normalize(global_data.camera_position_world - in.position_world);
    var metallic = material_data.metallic * textureSample(metallic_texture, metallic_sampler, in.uv).x;
    var roughness = material_data.roughness * textureSample(roughness_texture, roughness_sampler, in.uv).x;

    for (var i: u32 = 0; i < global_data.directional_lights_count; i++) {
        color = vec4<f32>(color.xyz + shade_light(i, in, geometric_normal, normal_world, view_dir, diffuse_color, metallic, roughness), color.w);
    }

    let cell = cluster_grid[cluster_index_world(global_data, in.position_clip.xy, in.position_world)];

    for (var i: u32 = 0; i < cell.count; i++) {
        let light_index = cluster_light_indices[cell.index + i];
        color = vec4<f32>(color.xyz + shade_light(light_index, in, geometric_normal, normal_world, view_dir, diffuse_color, metallic, roughness), color.w);
    }

#ifdef TRANSPARENT
    let ambient_occlusion_factor = 1.0;
#else
    // clamped: the texture is 1x1 when ambient occlusion is off
    let ambient_occlusion_pixel = min(vec2<u32>(in.position_clip.xy), textureDimensions(ambient_occlusion) - 1u);
    let ambient_occlusion_factor = textureLoad(ambient_occlusion, ambient_occlusion_pixel, 0).x;
#endif

    color = vec4<f32>(color.xyz + global_data.ambient_color * diffuse_color * (1.0 - metallic) * ambient_occlusion_factor, color.w);
#endif

    return color;
}
#endif
