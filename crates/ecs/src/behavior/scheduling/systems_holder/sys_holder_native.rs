use std::sync::Mutex;

use aphyn_ffi::FfiVec;

use crate::*;

pub struct SystemsHolderNative {
    systems: FfiVec<SystemFfi>,
    // todo: move into FfiSystem and make system calling a mutable fn. Also upgrade graph to be generic and provide mutable references in mutable graph iteration.
    system_datas: Box<[Mutex<SystemResourcesHolderUnsafeFfi>]>,
    execution_graph: OrderGraph,
}

impl SystemsHolderNative {
    pub fn new(systems: FfiVec<SystemFfi>, execution_graph: OrderGraph, types: TypesRegistryAccessFfi) -> Self {
        Self {
            system_datas: systems
                .iter()
                .map(|_| Mutex::new(SystemResourcesHolderUnsafeFfi::new(types.clone())))
                .collect(),
            systems,
            execution_graph,
        }
    }

    pub fn execute_iteration(
        &self,
        data: *mut WorldDataUnsafeFfi,
        thread_pool: ThreadPoolAccess,
        profiler: Option<&ProfileRecorder>,
        profile_tag: u64,
    ) {
        let data = WorldDataPtr(data);

        // the systems themselves are recorded as systems, not as batch tasks
        let scheduler_pool = thread_pool.with_profile(None);

        scheduler_pool.execute_graph(OrderGraphTasks(self.execution_graph.iter()), |system_index: u64| {
            let system = &self.systems[system_index];
            let system_data = &mut *self.system_datas[system_index as usize].try_lock().ok().unwrap();

            let profile = profiler.map(|recorder| ProfileCtx {
                recorder,
                parent: system.name().into(),
            });

            let system_ctx = SystemCtxFfi {
                world_mut: data.get(),
                system_data: &mut *system_data,
                // Safety. The access and the profile context outlive this call; background jobs own their context.
                thread_pool: unsafe { scheduler_pool.with_profile(profile).extend_lifetime() },
            };

            let Some(profiler) = profiler else {
                // Safety. Access is managed by OrderGraph and data usage.
                unsafe { system.execute(system_ctx) };
                return;
            };

            let thread = scheduler_pool.current_worker_index();
            profiler.measure(ProfileSpanKind::System, system.name(), thread, profile_tag, || unsafe {
                system.execute(system_ctx)
            });
        });
    }
}

struct OrderGraphTasks(OrderGraphIterator);

impl GraphIter for OrderGraphTasks {
    type Item = u64;

    fn next(&mut self, finished: Option<u64>) -> Option<u64> {
        if let Some(finished) = finished {
            self.0.end(finished);
        }

        self.0.start_next()
    }
}

struct WorldDataPtr(*mut WorldDataUnsafeFfi);

impl WorldDataPtr {
    fn get(&self) -> *mut WorldDataUnsafeFfi {
        self.0
    }
}

// Safety. Concurrent access is managed by OrderGraph and data usage.
unsafe impl Sync for WorldDataPtr {}
