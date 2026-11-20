#include "aphyn/cluster_compute.wgsl"

const SCAN_THREADS: u32 = 256u;

var<workgroup> partial_sums: array<u32, SCAN_THREADS>;

// pass 2 of 3, one workgroup: every thread scans a contiguous chunk of clusters
@compute @workgroup_size(256)
fn cs_main(@builtin(local_invocation_index) thread: u32) {
    let n = clusters_count();
    let chunk = (n + SCAN_THREADS - 1u) / SCAN_THREADS;
    let start = min(thread * chunk, n);
    let end = min(start + chunk, n);

    var sum = 0u;
    for (var i = start; i < end; i++) {
        sum += cluster_grid[i].count;
    }

    partial_sums[thread] = sum;
    workgroupBarrier();

    // inclusive Hillis-Steele scan
    for (var stride = 1u; stride < SCAN_THREADS; stride *= 2u) {
        var value = 0u;
        if (thread >= stride) {
            value = partial_sums[thread - stride];
        }
        workgroupBarrier();
        partial_sums[thread] += value;
        workgroupBarrier();
    }

    var offset = partial_sums[thread] - sum;

    for (var i = start; i < end; i++) {
        let count = cluster_grid[i].count;
        cluster_grid[i] = ClusterCell(offset, cluster_indices_fit(offset, count));
        offset += count;
    }
}
