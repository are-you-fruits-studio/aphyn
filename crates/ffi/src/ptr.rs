use std::{ffi::c_void, marker::PhantomData};

#[repr(transparent)]
pub struct FfiOpaqueRef<'a>{
    ptr: *const c_void,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FfiOpaqueRef<'a> {
    pub const fn new<T: 'a>(v: &'a T) -> Self {
        Self {
            ptr: v as *const T as *const c_void,
            _phantom: PhantomData,
        }
    }

    pub const unsafe fn from_raw(ptr: *const c_void) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub const fn ptr(&self) -> *const c_void {
        self.ptr
    }

    pub unsafe fn as_ref<T>(self) -> &'a T {
        unsafe {
            &*(self.ptr as *const T)
        }
    }
}

#[repr(transparent)]
pub struct FfiOpaqueMut<'a>{
    ptr: *mut c_void,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FfiOpaqueMut<'a> {
    pub const fn new<T: 'a>(v: &'a mut T) -> Self {
        Self {
            ptr: v as *mut T as *mut c_void,
            _phantom: PhantomData,
        }
    }

    pub const unsafe fn from_raw(ptr: *mut c_void) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    pub const fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub unsafe fn as_ref<T>(self) -> &'a T {
        unsafe {
            &*(self.ptr as *mut T)
        }
    }

    pub unsafe fn as_mut<T>(self) -> &'a mut T {
        unsafe {
            &mut *(self.ptr as *mut T)
        }
    }
}