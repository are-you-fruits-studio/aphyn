use std::{ffi::c_void, fmt::Debug};

use crate::{FfiBoxVtable, FfiStrSliceRef, boxed::alloc_for};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiShortTypeInfo {
    size: u64,
    align: u64,
    fn_name: unsafe extern "C-unwind" fn() -> FfiStrSliceRef<'static>,
}

impl FfiShortTypeInfo {
    pub const fn of<T>() -> Self {
        unsafe extern "C-unwind" fn ffi_name<T>() -> FfiStrSliceRef<'static> {
            FfiStrSliceRef::from_slice(std::any::type_name::<T>())
        }

        Self {
            size: std::mem::size_of::<T>() as u64,
            align: std::mem::align_of::<T>() as u64,
            fn_name: ffi_name::<T>,
        }
    }

    pub fn does_match<T>(&self) -> bool {
        std::mem::size_of::<T>() as u64 == self.size
            && std::mem::align_of::<T>() as u64 == self.align
            && std::any::type_name::<T>() == unsafe { (self.fn_name)() }.into_slice()
    }

    pub fn matches(&self, other: &FfiShortTypeInfo) -> bool {
        self.size == other.size && self.align == other.align && self.name() == other.name()
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn align(&self) -> u64 {
        self.align
    }

    pub fn name(&self) -> &'static str {
        unsafe { (self.fn_name)().into_slice() }
    }
}

impl Debug for FfiShortTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FfiShortTypeInfo")
            .field("size", &self.size())
            .field("align", &self.align())
            .field("name", &self.name())
            .finish()
    }
}

unsafe impl Send for FfiShortTypeInfo {}
unsafe impl Sync for FfiShortTypeInfo {}

//

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiExtendedTypeInfo {
    short: FfiShortTypeInfo,
    fn_drop: unsafe extern "C-unwind" fn(*mut c_void),
    fn_move_to_box: unsafe extern "C-unwind" fn(*const c_void) -> *mut c_void,
    box_vtable: FfiBoxVtable,
}

impl FfiExtendedTypeInfo {
    pub const fn of<T: 'static>() -> Self {
        unsafe extern "C-unwind" fn ffi_drop<T>(p: *mut c_void) {
            unsafe { std::ptr::drop_in_place(p as *mut T) }
        }
        unsafe extern "C-unwind" fn ffi_move_to_box<T>(p: *const c_void) -> *mut c_void {
            unsafe {
                let ptr = alloc_for::<T>();
                ptr.write((p as *const T).read());
                ptr as *mut c_void
            }
        }

        Self {
            short: FfiShortTypeInfo::of::<T>(),
            fn_drop: ffi_drop::<T>,
            fn_move_to_box: ffi_move_to_box::<T>,
            box_vtable: FfiBoxVtable::new::<T>(),
        }
    }

    pub const fn short(&self) -> &FfiShortTypeInfo {
        &self.short
    }

    pub unsafe fn drop(&self, ptr: *mut c_void) {
        unsafe { (self.fn_drop)(ptr) }
    }

    pub unsafe fn raw_drop_fn(&self) -> unsafe extern "C-unwind" fn(*mut c_void) {
        self.fn_drop
    }

    // moves the value out of `ptr` into memory allocated by the binary that built this info
    pub unsafe fn move_to_box(&self, ptr: *const c_void) -> *mut c_void {
        unsafe { (self.fn_move_to_box)(ptr) }
    }

    // frees memory from `move_to_box` or `FfiBox::new` of this type, without dropping the value
    pub unsafe fn dealloc_box(&self, ptr: *mut c_void) {
        unsafe { (self.box_vtable.dealloc_fn)(ptr) }
    }

    pub const fn box_vtable(&self) -> &FfiBoxVtable {
        &self.box_vtable
    }
}

// todo
impl Debug for FfiExtendedTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FfiExtendedTypeInfo").field("short", &self.short()).finish()
    }
}

unsafe impl Send for FfiExtendedTypeInfo {}
unsafe impl Sync for FfiExtendedTypeInfo {}
