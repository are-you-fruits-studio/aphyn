use std::ffi::c_void;

use fruits_ffi::{FfiDroppable, FfiStaticRef, FfiString};

use crate::*;

#[repr(C)]
struct SystemsHolderBuilderFfiVTable {
    insert_system_fn: unsafe extern "C-unwind" fn(*mut c_void, system: SystemFfi) -> bool,
    order_fn: unsafe extern "C-unwind" fn(*mut c_void, prev: OrderEntry, next: OrderEntry),
    insert_group_child_fn: unsafe extern "C-unwind" fn(*mut c_void, group: FfiString, child: OrderEntry),
    clear_fn: unsafe extern "C-unwind" fn(*mut c_void),
    build_fn: unsafe extern "C-unwind" fn(*mut c_void) -> SystemsHolderFfi,
}

#[repr(C)]
pub struct SystemsHolderBuilderFfi {
    data: FfiDroppable,
    vtable: FfiStaticRef<SystemsHolderBuilderFfiVTable>,
}

impl SystemsHolderBuilderFfi {
    pub fn new(types: TypesRegistryAccessFfi) -> Self {
        unsafe extern "C-unwind" fn ffi_insert_system(this: *mut c_void, system: SystemFfi) -> bool {
            unsafe {
                let this = &mut *(this as *mut SystemsHolderBuilderNative);

                this.insert_system(system)
            }
        }
        unsafe extern "C-unwind" fn ffi_order(this: *mut c_void, prev: OrderEntry, next: OrderEntry) {
            unsafe {
                let this = &mut *(this as *mut SystemsHolderBuilderNative);

                this.order(prev, next)
            }
        }
        unsafe extern "C-unwind" fn ffi_insert_group_child(this: *mut c_void, group: FfiString, child: OrderEntry) {
            unsafe {
                let this = &mut *(this as *mut SystemsHolderBuilderNative);

                this.insert_group_child(group, child)
            }
        }
        unsafe extern "C-unwind" fn ffi_clear(this: *mut c_void) {
            unsafe {
                let this = &mut *(this as *mut SystemsHolderBuilderNative);

                this.clear()
            }
        }
        unsafe extern "C-unwind" fn ffi_build(this: *mut c_void) -> SystemsHolderFfi {
            unsafe {
                let this = &mut *(this as *mut SystemsHolderBuilderNative);

                let result = this.build();

                SystemsHolderFfi::from_native(result)
            }
        }

        Self {
            data: FfiDroppable::new(SystemsHolderBuilderNative::new(types)),
            vtable: FfiStaticRef::new(&SystemsHolderBuilderFfiVTable {
                insert_system_fn: ffi_insert_system,
                order_fn: ffi_order,
                insert_group_child_fn: ffi_insert_group_child,
                clear_fn: ffi_clear,
                build_fn: ffi_build,
            }),
        }
    }

    pub fn insert_system(&mut self, system: SystemFfi) -> bool {
        unsafe {
            let this = self.data.get();

            (self.vtable.insert_system_fn)(this, system)
        }
    }

    pub fn order(&mut self, prev: OrderEntry, next: OrderEntry) {
        unsafe {
            let this = self.data.get();

            (self.vtable.order_fn)(this, prev, next)
        }
    }

    pub fn insert_group_child(&mut self, group: FfiString, child: OrderEntry) {
        unsafe {
            let this = self.data.get();

            (self.vtable.insert_group_child_fn)(this, group, child)
        }
    }

    pub fn clear(&mut self) {
        unsafe {
            let this = self.data.get();

            (self.vtable.clear_fn)(this)
        }
    }

    pub fn build(&mut self) -> SystemsHolderFfi {
        unsafe {
            let this = self.data.get();

            (self.vtable.build_fn)(this)
        }
    }
}
