#include "aphyn/standard_bindings.wgsl"
#include "aphyn/pbr.wgsl"

// todo: light structure. Needs to support lights:
// - Point (hdr-color, center, range)
// - Spot (hdr-color, center, range, direction_dst, fov)
// - Directional (hdr-color, direction_dst)
// - Area? (todo for the future)

@vertex
fn vs_main(vertex: VertexAttributes, instance_raw: InstanceRawAttributes) -> VertexOutput {
    var instance = map_instance_data(instance_raw);

    return customer_vertex(vertex, instance);
}

#ifdef LIT
struct VertexOutput {
    @builtin(position) position_clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal_world: vec3<f32>,
    @location(2) tangent_world: vec4<f32>,
    @location(3) position_world: vec3<f32>,
    @location(4) uv: vec2<f32>,
};

fn customer_vertex(vertex: VertexAttributes, instance: InstanceAttributes) -> VertexOutput {
    var out: VertexOutput;

    var position_world = instance.local_to_world * vec4<f32>(vertex.position, 1.0);

    out.position_clip = global_data.matrix_world_to_clip * position_world;
    out.color = vertex.color * material_data.color;
    out.normal_world = (instance.local_to_world * vec4<f32>(vertex.normal, 0.0)).xyz;
    out.tangent_world = vec4<f32>((instance.local_to_world * vec4<f32>(vertex.tangent.xyz, 0.0)).xyz, vertex.tangent.w);
    out.position_world = position_world.xyz;
    out.uv = vertex.uv;

    return out;
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

    let normal_tangent_space = textureSample(normal_texture, normal_sampler, in.uv).xyz * 2.0 - 1.0;
    let TBN = create_matrix_tbn(in.normal_world, in.tangent_world);
    let normal_world = normalize(TBN * normal_tangent_space);

    var view_dir = normalize(global_data.camera_position_world - in.position_world);
    var metallic = material_data.metallic * textureSample(metallic_texture, metallic_sampler, in.uv).x;
    var roughness = material_data.roughness * textureSample(roughness_texture, roughness_sampler, in.uv).x;

    for (var i: u32 = 0; i < global_data.lights_count; i++) {
        var light = light_generic_to_fragment(lights[i], in.position_world);

        var color_lit = cook_torrance_brdf(
            normal_world,
            view_dir,
            diffuse_color,
            metallic,
            roughness,
            light.direction_src,
            light.color
        );

        color = vec4<f32>(color.xyz + color_lit, color.w);
    }
#endif

    return color;
}
