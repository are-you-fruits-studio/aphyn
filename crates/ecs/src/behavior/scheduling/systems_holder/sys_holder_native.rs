use std::sync::{Arc, Mutex};

use aphyn_ffi::FfiVec;
use aphyn_utils::{AssumeSend, thread_pool::ThreadPool};

use crate::*;

pub struct SystemsHolderNative {
    systems: FfiVec<SystemFfi>,
    system_datas: Arc<[Mutex<SystemResourcesHolderUnsafeFfi>]>,
    execution_graph: Arc<OrderGraph>,
    thread_pool: ThreadPool,
}

impl SystemsHolderNative {
    pub fn new(systems: FfiVec<SystemFfi>, execution_graph: OrderGraph, types: TypesRegistryAccessFfi) -> Self {
        Self {
            system_datas: systems
                .iter()
                .map(|_| Mutex::new(SystemResourcesHolderUnsafeFfi::new(types.clone())))
                .collect::<Arc<_>>(),
            systems,
            execution_graph: Arc::new(execution_graph),
            thread_pool: ThreadPool::new(Self::non_main_threads_count()),
        }
    }

    fn non_main_threads_count() -> usize {
        match std::thread::available_parallelism() {
            Ok(count) => (count.get() - 1).max(1),
            Err(_) => 3,
        }
    }

    // todo: support an external runner that decides whether systems run single-threaded or multithreaded.
    pub fn execute_iteration(&self, data: *mut WorldDataUnsafeFfi) {
        let iter = Arc::new(Mutex::new(self.execution_graph.iter()));

        self.thread_pool.scope(|scope| {
            loop {
                let system_index = {
                    let mut iter = iter.lock().unwrap();

                    if iter.all_ended() {
                        break;
                    }

                    iter.start_next()
                };

                if let Some(system_index) = system_index {
                    let iter = Arc::clone(&iter);
                    let systems = &self.systems;
                    let system_datas = Arc::clone(&self.system_datas);
                    // todo
                    let data = unsafe { AssumeSend::new(data) };

                    let job = move || {
                        let data = data.into_inner();
                        let system = &systems[system_index];
                        let system_data = &mut *system_datas[system_index as usize].try_lock().ok().unwrap();

                        {
                            unsafe {
                                let system_ctx = SystemCtxFfi {
                                    world_mut: data,
                                    system_data: &mut *system_data,
                                };

                                // Safety. Access is managed by OrderGraph and data usage.
                                system.execute(system_ctx);
                            }
                        }

                        {
                            iter.lock().unwrap().end(system_index);
                        }
                    };

                    let job: Box<dyn FnOnce() + Send> = Box::new(job);

                    scope.push_job_unhandled(job);
                } else {
                    scope.panic_if_err();
                }
            }
        });
    }
}
