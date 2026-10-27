use std::{ffi::c_void, fmt::Debug};

use crate::{FfiAny, FfiAnyMut, FfiAnyRef, FfiOpaqueMut, FfiOpaqueRef, FfiStrSliceRef};

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
        unsafe extern "C-unwind" fn ffi_drop<T>(p: *mut c_void) {
            unsafe { std::ptr::drop_in_place(p as *mut T) }
        }

        unsafe extern "C-unwind" fn ffi_as_any<T: 'static>(r: FfiOpaqueRef) -> FfiAnyRef {
            unsafe { FfiAnyRef::new(r.as_ref::<T>()) }
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

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn align(&self) -> u64 {
        self.align
    }

    pub fn name(&self) -> &'static str {
        unsafe {
            (self.fn_name)().into_slice()
        }
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

unsafe impl Send for FfiShortTypeInfo { }
unsafe impl Sync for FfiShortTypeInfo { }

//

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiExtendedTypeInfo {
    short: FfiShortTypeInfo,
    fn_drop: unsafe extern "C-unwind" fn(*mut c_void),
    fn_as_any: unsafe extern "C-unwind" fn(FfiOpaqueRef) -> FfiAnyRef,
    fn_as_any_mut: unsafe extern "C-unwind" fn(FfiOpaqueMut) -> FfiAnyMut,
    fn_move_to_any: unsafe extern "C-unwind" fn(*const c_void) -> FfiAny,
}

impl FfiExtendedTypeInfo {
    pub const fn of<T: 'static>() -> Self {
        unsafe extern "C-unwind" fn ffi_drop<T>(p: *mut c_void) {
            unsafe { std::ptr::drop_in_place(p as *mut T) }
        }
        unsafe extern "C-unwind" fn ffi_as_any<T: 'static>(r: FfiOpaqueRef) -> FfiAnyRef {
            unsafe { FfiAnyRef::new(r.as_ref::<T>()) }
        }
        unsafe extern "C-unwind" fn ffi_as_any_mut<T: 'static>(r: FfiOpaqueMut) -> FfiAnyMut {
            unsafe { FfiAnyMut::new(r.as_mut::<T>()) }
        }
        unsafe extern "C-unwind" fn ffi_move_to_any<T: 'static>(p: *const c_void) -> FfiAny {
            unsafe { FfiAny::new((p as *const T).read()) }
        }

        Self {
            short: FfiShortTypeInfo::of::<T>(),
            fn_drop: ffi_drop::<T>,
            fn_as_any: ffi_as_any::<T>,
            fn_as_any_mut: ffi_as_any_mut::<T>,
            fn_move_to_any: ffi_move_to_any::<T>,
        }
    }

    pub const fn short(&self) -> &FfiShortTypeInfo {
        &self.short
    }

    pub unsafe fn drop(&self, ptr: *mut c_void) {
        unsafe {
            (self.fn_drop)(ptr)
        }
    }

    pub unsafe fn raw_drop_fn(&self) -> unsafe extern "C-unwind" fn(*mut c_void) {
        self.fn_drop
    }

    pub unsafe fn as_any<'a>(&self, ptr: FfiOpaqueRef<'a>) -> FfiAnyRef<'a> {
        unsafe {
            (self.fn_as_any)(ptr)
        }
    }

    pub unsafe fn as_any_mut<'a>(&self, ptr: FfiOpaqueMut<'a>) -> FfiAnyMut<'a> {
        unsafe {
            (self.fn_as_any_mut)(ptr)
        }
    }

    pub unsafe fn move_to_any(&self, ptr: *const c_void) -> FfiAny {
        unsafe {
            (self.fn_move_to_any)(ptr)
        }
    }
}

// todo
impl Debug for FfiExtendedTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FfiExtendedTypeInfo")
            .field("short", &self.short())
            .finish()
    }
}

unsafe impl Send for FfiExtendedTypeInfo { }
unsafe impl Sync for FfiExtendedTypeInfo { }