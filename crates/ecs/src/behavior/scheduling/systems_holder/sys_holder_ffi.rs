use std::ffi::c_void;

use aphyn_ffi::{FfiDroppable, FfiOption, FfiVec};

use crate::*;

#[repr(C)]
pub struct SystemsHolderFfi {
    data: FfiDroppable,
    execute_iteration_fn: unsafe extern "C-unwind" fn(
        *const c_void,
        data: *mut WorldDataUnsafeFfi,
        ThreadPoolAccess,
        FfiOption<&ProfileRecorder>,
        profile_tag: u64,
    ),
}

impl SystemsHolderFfi {
    pub fn new(systems: FfiVec<SystemFfi>, execution_graph: OrderGraph, types: TypesRegistryAccessFfi) -> Self {
        Self::from_native(SystemsHolderNative::new(systems, execution_graph, types))
    }

    pub(crate) fn from_native(systems_holder: SystemsHolderNative) -> Self {
        unsafe extern "C-unwind" fn ffi_execute_iteration(
            this: *const c_void,
            data: *mut WorldDataUnsafeFfi,
            thread_pool: ThreadPoolAccess,
            profiler: FfiOption<&ProfileRecorder>,
            profile_tag: u64,
        ) {
            unsafe {
                (&*(this as *const SystemsHolderNative)).execute_iteration(data, thread_pool, profiler.into_option(), profile_tag);
            }
        }

        Self {
            data: FfiDroppable::new(systems_holder),
            execute_iteration_fn: ffi_execute_iteration,
        }
    }

    pub fn execute_iteration(
        &self,
        data: *mut WorldDataUnsafeFfi,
        thread_pool: ThreadPoolAccess,
        profiler: Option<&ProfileRecorder>,
        profile_tag: u64,
    ) {
        unsafe { (self.execute_iteration_fn)(self.data.get(), data, thread_pool, profiler.into(), profile_tag) }
    }
}

unsafe impl Send for SystemsHolderFfi where SystemsHolderNative: Send {}
unsafe impl Sync for SystemsHolderFfi where SystemsHolderNative: Sync {}
