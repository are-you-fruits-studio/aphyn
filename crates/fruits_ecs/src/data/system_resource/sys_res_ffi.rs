use std::ffi::c_void;

use fruits_ffi::{FfiDroppable, FfiStaticRef};

use crate::*;

#[repr(C)]
struct SystemResourcesHolderUnsafeFfiVTable {
    get_or_insert_fn: unsafe extern "C-unwind" fn(*const c_void, u64) -> SystemResourceGetOrInsertResult,
}

#[repr(C)]
pub struct SystemResourcesHolderUnsafeFfi {
    data: FfiDroppable,
    vtable: FfiStaticRef<SystemResourcesHolderUnsafeFfiVTable>,
}

impl SystemResourcesHolderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        pub unsafe extern "C-unwind" fn ffi_get_or_insert(this_ref: *const c_void, id: u64) -> SystemResourceGetOrInsertResult {
            // todo
            unsafe { (&*(this_ref as *const SystemResourcesHolderNative)).get_or_insert(id) }
        }

        Self {
            data: FfiDroppable::new(SystemResourcesHolderNative::new(types)),
            vtable: FfiStaticRef::new(&SystemResourcesHolderUnsafeFfiVTable {
                get_or_insert_fn: ffi_get_or_insert,
            }),
        }
    }

    pub unsafe fn get_or_insert(&self, id: u64) -> SystemResourceGetOrInsertResult {
        // todo
        unsafe { (self.vtable.get_or_insert_fn)(self.data.get(), id) }
    }
}

unsafe impl Send for SystemResourcesHolderUnsafeFfi {}
unsafe impl Sync for SystemResourcesHolderUnsafeFfi {}
