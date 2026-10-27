use std::ffi::c_void;

use fruits_ffi::{FfiDroppable, FfiOpaqueVec, FfiStaticRef};

use crate::*;

#[repr(C)]
struct EventsHolderUnsafeFfiVTable {
    get_fn: unsafe extern "C-unwind" fn(*const c_void, type_id: u64) -> *mut FfiOpaqueVec,
    get_or_create_fn: unsafe extern "C-unwind" fn(*const c_void, type_id: u64) -> *mut FfiOpaqueVec,
    clear_fn: unsafe extern "C-unwind" fn(*const c_void),
}

#[repr(C)]
pub struct EventsHolderUnsafeFfi {
    data: FfiDroppable,
    vtable: FfiStaticRef<EventsHolderUnsafeFfiVTable>,
}

impl EventsHolderUnsafeFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        unsafe extern "C-unwind" fn ffi_get(this: *const c_void, type_id: u64) -> *mut FfiOpaqueVec {
            unsafe { (&*(this as *const EventsHolderNative)).get(type_id) }
        }

        unsafe extern "C-unwind" fn ffi_get_or_create(this: *const c_void, type_id: u64) -> *mut FfiOpaqueVec {
            unsafe { (&*(this as *const EventsHolderNative)).get_or_create(type_id) }
        }

        unsafe extern "C-unwind" fn ffi_clear(this: *const c_void) {
            unsafe { (&*(this as *const EventsHolderNative)).clear() }
        }

        Self {
            data: FfiDroppable::new(EventsHolderNative::new(types)),
            vtable: FfiStaticRef::new(&EventsHolderUnsafeFfiVTable {
                get_fn: ffi_get,
                get_or_create_fn: ffi_get_or_create,
                clear_fn: ffi_clear,
            }),
        }
    }

    pub fn get(&self, type_id: u64) -> *mut FfiOpaqueVec {
        unsafe { (self.vtable.get_fn)(self.data.get(), type_id) }
    }

    pub fn get_or_create(&self, type_id: u64) -> *mut FfiOpaqueVec {
        unsafe { (self.vtable.get_or_create_fn)(self.data.get(), type_id) }
    }

    pub fn clear(&self) {
        unsafe { (self.vtable.clear_fn)(self.data.get()) }
    }
}

unsafe impl Send for EventsHolderUnsafeFfi { }
unsafe impl Sync for EventsHolderUnsafeFfi { }