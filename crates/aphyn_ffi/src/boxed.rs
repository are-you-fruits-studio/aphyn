use std::{
    alloc::Layout, ffi::c_void, fmt::{Debug, Display}, mem::ManuallyDrop, ops::{Deref, DerefMut}
};

#[repr(C)]
pub struct FfiBoxVtable {
    pub drop_in_place_fn: unsafe extern "C-unwind" fn(*mut c_void),
    pub dealloc_fn: unsafe extern "C-unwind" fn(*mut c_void),
}

impl FfiBoxVtable {
    pub const fn new<T>() -> Self {
        unsafe extern "C-unwind" fn ffi_drop_in_place<T>(ptr: *mut c_void) {
            unsafe { std::ptr::drop_in_place(ptr as *mut T) }
        }
        unsafe extern "C-unwind" fn ffi_dealloc<T>(ptr: *mut c_void) {
            unsafe { std::alloc::dealloc(ptr as *mut u8, Layout::new::<T>()) }
        }

        Self {
            drop_in_place_fn: ffi_drop_in_place::<T>,
            dealloc_fn: ffi_dealloc::<T>,
        }
    }
}

#[repr(C)]
pub struct FfiBox<T> {
    ptr: *mut T,
    vtable: &'static FfiBoxVtable,
}

impl<T> FfiBox<T> {
    pub fn new(value: T) -> Self {
        unsafe {
            let ptr = std::alloc::alloc(Layout::new::<T>()) as *mut T;

            ptr.write(value);

            Self {
                ptr,
                vtable: &const { FfiBoxVtable::new::<T>() },
            }
        }
    }

    pub(crate) unsafe fn from_raw(ptr: *mut T, vtable: &'static FfiBoxVtable) -> Self {
        Self {
            ptr,
            vtable,
        }
    }

    pub fn into_inner(self) -> T {
        unsafe {
            let val = self.ptr.read();
           
            let this = ManuallyDrop::new(self);
            (this.vtable.dealloc_fn)(this.ptr as *mut c_void);

            val
        }
    }

    pub fn as_ref(&self) -> &T {
        unsafe { &*(self.ptr as *mut T) }
    }

    pub fn as_mut(&self) -> &mut T {
        unsafe { &mut *(self.ptr as *mut T) }
    }

    pub fn as_raw(&self) -> *mut T {
        self.ptr as *mut T
    }
}

impl<T> Drop for FfiBox<T> {
    fn drop(&mut self) {
        struct FfiMemoryToDealloc {
            ptr: *mut c_void,
            vtable: &'static FfiBoxVtable,
        }

        impl Drop for FfiMemoryToDealloc {
            fn drop(&mut self) {
                unsafe { (self.vtable.dealloc_fn)(self.ptr) };
            }
        }

        unsafe {
            let to_dealloc = FfiMemoryToDealloc {
                ptr: self.ptr as *mut c_void,
                vtable: self.vtable,
            };

            (self.vtable.drop_in_place_fn)(self.ptr as *mut c_void);

            drop(to_dealloc);
        }
    }
}

impl<T> Deref for FfiBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T> DerefMut for FfiBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T: Debug> Debug for FfiBox<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&**self, f)
    }
}

impl<T: Display> Display for FfiBox<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&**self, f)
    }
}

unsafe impl<T: Send> Send for FfiBox<T> { }
unsafe impl<T: Sync> Sync for FfiBox<T> { }
