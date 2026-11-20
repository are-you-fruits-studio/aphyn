#include "aphyn/cluster_compute.wgsl"

// pass 1 of 3: the light count of every cluster
@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let cluster = id.x;

    if (cluster >= clusters_count()) {
        return;
    }

    let aabb = cluster_aabb(global_data, cluster);
    var count = 0u;

    for (var i = global_data.directional_lights_count; i < global_data.lights_count; i++) {
        count += u32(cluster_intersects_light(aabb, i));
    }

    cluster_grid[cluster] = ClusterCell(0u, count);
}
