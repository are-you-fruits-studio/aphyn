#include "aphyn/standard_bindings.wgsl"

fn light_shadow(light: GenericLight, position_world: vec3<f32>, normal_world: vec3<f32>) -> f32 {
    if (light.shadow_count == 0u) {
        return 1.0;
    }

    let view_depth = dot(position_world - global_data.camera_position_world, global_data.camera_forward_world);

    var index = light.shadow_start;
    let end = light.shadow_start + light.shadow_count;
    while (index < end && shadow_views[index].split_far < view_depth) {
        index++;
    }

    if (index == end) {
        return 1.0;
    }

    let view = shadow_views[index];
    let position_offset = position_world + normalize(normal_world) * global_data.shadow_normal_offset;
    let clip = view.matrix_world_to_clip * vec4<f32>(position_offset, 1.0);
    let ndc = clip.xyz / clip.w;

    if (clip.w <= 0.0 || any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z < 0.0 || ndc.z > 1.0) {
        return 1.0;
    }

    // stay half a texel inside the tile so the 2x2 filter never reads a neighbor tile
    let half_texel = 0.5 / vec2<f32>(textureDimensions(shadow_atlas));
    let tile_uv = ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    let uv = clamp(view.atlas_rect.xy + tile_uv * view.atlas_rect.zw, view.atlas_rect.xy + half_texel, view.atlas_rect.xy + view.atlas_rect.zw - half_texel);

    return textureSampleCompareLevel(shadow_atlas, shadow_sampler, uv, ndc.z - global_data.shadow_bias);
}
