#include "aphyn/cluster_compute.wgsl"

// pass 3 of 3: repeats the tests of pass 1 and writes the lights at the cluster's offset
@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cluster = id.x;

    if (cluster >= clusters_count()) {
        return;
    }

    let aabb = cluster_aabb(global_data, cluster);
    let cell = cluster_grid[cluster];
    var count = 0u;

    for (var i = global_data.directional_lights_count; i < global_data.lights_count && count < cell.count; i++) {
        if (cluster_intersects_light(aabb, i)) {
            cluster_light_indices[cell.index + count] = i;
            count++;
        }
    }
}
