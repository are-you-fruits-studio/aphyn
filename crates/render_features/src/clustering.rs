use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_ffi::FfiOption;
use aphyn_render::{CLUSTERS_COUNT, RenderApiResource};
use aphyn_render_core::{GpuBinding, GpuComputePass, GpuComputeShader, RenderApi};

use crate::{GlobalBindingsResource, RENDER_ORDER_LIGHT_CULLING, RenderFeature, RenderPassWriter, render_wgsl_composer};

const DROPPED_PAIRS_CHECK_PERIOD: u32 = 120;
const CLUSTER_WORKGROUP_SIZE: usize = 64;

#[repr(C)]
#[derive(Default)]
pub struct ClusteringRenderFeature {
    shaders: FfiOption<ClusterShaders>,
    frame: u32,
    logged_dropped_pairs: bool,
}

#[repr(C)]
struct ClusterShaders {
    count: GpuComputeShader,
    scan: GpuComputeShader,
    fill: GpuComputeShader,
}

impl ClusterShaders {
    fn new(core: &RenderApi) -> Option<Self> {
        let create = |name: &str, source: &str| {
            let code = render_wgsl_composer().compose(name, source, &[]).ok()?;
            core.create_compute_shader(Some(name), &code)
        };

        Some(Self {
            count: create(
                "cluster_prefix_count.wgsl",
                include_str!("./assets/cluster_prefix_count.wgsl"),
            )?,
            scan: create("cluster_prefix_scan.wgsl", include_str!("./assets/cluster_prefix_scan.wgsl"))?,
            fill: create("cluster_prefix_fill.wgsl", include_str!("./assets/cluster_prefix_fill.wgsl"))?,
        })
    }
}

impl RenderFeature for ClusteringRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let Some(render_api) = world.resources().get::<RenderApiResource>().map(RenderApiResource::clone) else {
            return;
        };

        if self.shaders.is_none() {
            match ClusterShaders::new(render_api.core()) {
                Some(shaders) => self.shaders = FfiOption::Some(shaders),
                None => {
                    eprintln!("clustered lighting: failed to create the light culling shaders");
                    return;
                }
            }
        }

        self.frame = self.frame.wrapping_add(1);
        let check_dropped_pairs = !self.logged_dropped_pairs && self.frame % DROPPED_PAIRS_CHECK_PERIOD == 0;

        passes.add(RENDER_ORDER_LIGHT_CULLING, move |this, ctx| {
            let Some(shaders) = this.shaders.as_ref() else {
                return;
            };
            let resources = ctx.world.resources();
            let Some(globals) = resources.get::<GlobalBindingsResource>() else {
                return;
            };
            let bindings = &globals.bindings;

            ctx.render_api.reset_cluster_dropped_pairs(bindings);

            let group = [
                (0, GpuBinding::Buffer(bindings.uniform_buffer())),
                (1, GpuBinding::Buffer(bindings.lights_buffer())),
                (5, GpuBinding::Buffer(bindings.cluster_grid_buffer())),
                (6, GpuBinding::Buffer(bindings.cluster_indices_buffer())),
                (7, GpuBinding::Buffer(bindings.cluster_dropped_pairs_buffer())),
            ];
            let per_cluster = [CLUSTERS_COUNT.div_ceil(CLUSTER_WORKGROUP_SIZE), 1, 1];
            let dispatch = |label: &str, shader: &GpuComputeShader, workgroups: [usize; 3]| {
                ctx.render_api.core().dispatch(&GpuComputePass {
                    label: Some(label),
                    shader,
                    bind_groups: &[&group],
                    workgroups,
                });
            };

            dispatch("cluster count", &shaders.count, per_cluster);
            dispatch("cluster scan", &shaders.scan, [1, 1, 1]);
            dispatch("cluster fill", &shaders.fill, per_cluster);

            if check_dropped_pairs
                && let Some(dropped) = ctx.render_api.read_cluster_dropped_pairs(bindings)
                && dropped > 0
            {
                this.logged_dropped_pairs = true;
                eprintln!(
                    "clustered lighting: {dropped} light-cluster pairs dropped, raise ClusteredLightingResource::max_light_cluster_pairs"
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_shaders_are_valid() {
        let composer = render_wgsl_composer();
        let shaders = [
            (
                "cluster_prefix_count.wgsl",
                include_str!("./assets/cluster_prefix_count.wgsl"),
            ),
            ("cluster_prefix_scan.wgsl", include_str!("./assets/cluster_prefix_scan.wgsl")),
            ("cluster_prefix_fill.wgsl", include_str!("./assets/cluster_prefix_fill.wgsl")),
        ];

        for (name, source) in shaders {
            let code = composer.compose(name, source, &[]).unwrap();
            let module = naga::front::wgsl::parse_str(&code).unwrap_or_else(|error| panic!("{}", error.emit_to_string(&code)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(&code)));
        }
    }
}
