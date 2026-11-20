#include "aphyn/clusters.wgsl"

@group(0) @binding(0) var<uniform> global_data: GlobalData;
@group(0) @binding(1) var<storage, read> lights: array<GenericLight>;
@group(0) @binding(5) var<storage, read_write> cluster_grid: array<ClusterCell>;
@group(0) @binding(6) var<storage, read_write> cluster_light_indices: array<u32>;
@group(0) @binding(7) var<storage, read_write> cluster_dropped_pairs: atomic<u32>;

fn clusters_count() -> u32 {
    return global_data.cluster_dims.x * global_data.cluster_dims.y * global_data.cluster_dims.z;
}

fn cluster_intersects_light(aabb: ClusterAabb, light_index: u32) -> bool {
    return cluster_light_intersects(cluster_light(global_data, lights[light_index]), aabb);
}

// how many of `count` indices fit into the index list from `offset` on; counts the rest as dropped
fn cluster_indices_fit(offset: u32, count: u32) -> u32 {
    let capacity = global_data.cluster_index_capacity;
    let fit = min(count, capacity - min(offset, capacity));

    if (fit < count) {
        atomicAdd(&cluster_dropped_pairs, count - fit);
    }

    return fit;
}
