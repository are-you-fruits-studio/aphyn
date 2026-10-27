use std::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiAllocator {
    alloc_fn: unsafe extern "C-unwind" fn(u64, u64) -> *mut c_void,
    dealloc_fn: unsafe extern "C-unwind" fn(*mut c_void, u64, u64),
}

impl FfiAllocator {
    pub const fn from_system() -> Self {
        Self {
            alloc_fn: Self::c_alloc_system,
            dealloc_fn: Self::c_dealloc_system,
        }
    }

    pub const fn from_global() -> Self {
        Self {
            alloc_fn: Self::c_alloc_global,
            dealloc_fn: Self::c_dealloc_global,
        }
    }

    // todo
    pub unsafe fn alloc(&self, size: u64, align: u64) -> *mut u8 {
        // todo
        unsafe { (self.alloc_fn)(size, align) as *mut u8 }
    }

    // todo
    pub unsafe fn dealloc(&self, ptr: *mut u8, size: u64, align: u64) {
        // todo
        unsafe {
            (self.dealloc_fn)(ptr as *mut c_void, size, align);
        }
    }

    unsafe extern "C-unwind" fn c_alloc_system(size: u64, align: u64) -> *mut c_void {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size as usize, align as usize);

            std::alloc::System.alloc(layout) as *mut c_void
        }
    }

    unsafe extern "C-unwind" fn c_dealloc_system(ptr: *mut c_void, size: u64, align: u64) {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size as usize, align as usize);

            std::alloc::System.dealloc(ptr as *mut u8, layout);
        }
    }

    unsafe extern "C-unwind" fn c_alloc_global(size: u64, align: u64) -> *mut c_void {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size as usize, align as usize);

            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            ptr as *mut c_void
        }
    }

    unsafe extern "C-unwind" fn c_dealloc_global(ptr: *mut c_void, size: u64, align: u64) {
        unsafe {
            let layout = Layout::from_size_align_unchecked(size as usize, align as usize);

            std::alloc::dealloc(ptr as *mut u8, layout);
        }
    }
}

//

#[repr(C)]
pub struct FfiTypedMemory<T> {
    data: *mut T,
    dealloc_fn: unsafe extern "C-unwind" fn(*mut c_void),
}

impl<T> FfiTypedMemory<T> {
    pub fn new() -> Self {
        unsafe extern "C-unwind" fn ffi_dealloc<D>(data: *mut c_void) {
            unsafe { std::alloc::dealloc(data as *mut u8, Layout::new::<D>()) };
        }

        unsafe {
            let ptr = std::alloc::alloc(Layout::new::<T>());
            if ptr.is_null() {
                std::alloc::handle_alloc_error(Layout::new::<T>());
            }
            Self {
                data: ptr as *mut T,
                dealloc_fn: ffi_dealloc::<T>,
            }
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.data
    }

    pub fn into_opaque(self) -> FfiOpaqueMemory {
        unsafe { std::mem::transmute::<Self, FfiOpaqueMemory>(self) }
    }

    pub fn as_opaque(&self) -> &FfiOpaqueMemory {
        unsafe { std::mem::transmute::<&Self, &FfiOpaqueMemory>(self) }
    }

    pub fn as_opaque_mut(&mut self) -> &mut FfiOpaqueMemory {
        unsafe { std::mem::transmute::<&mut Self, &mut FfiOpaqueMemory>(self) }
    }
}

impl<T> Drop for FfiTypedMemory<T> {
    fn drop(&mut self) {
        unsafe {
            (self.dealloc_fn)(self.data as *mut c_void);
        }
    }
}

//

#[repr(C)]
pub struct FfiOpaqueMemory {
    data: *mut c_void,
    dealloc_fn: unsafe extern "C-unwind" fn(*mut c_void),
}

impl FfiOpaqueMemory {
    pub fn new<T>() -> Self {
        FfiTypedMemory::<T>::new().into_opaque()
    }

    pub unsafe fn into_typed<T>(self) -> FfiTypedMemory<T> {
        unsafe { std::mem::transmute::<Self, FfiTypedMemory<T>>(self) }
    }

    pub unsafe fn as_typed<T>(&self) -> &FfiTypedMemory<T> {
        unsafe { std::mem::transmute::<&Self, &FfiTypedMemory<T>>(self) }
    }

    pub unsafe fn as_typed_mut<T>(&mut self) -> &mut FfiTypedMemory<T> {
        unsafe { std::mem::transmute::<&mut Self, &mut FfiTypedMemory<T>>(self) }
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.data
    }
}

impl Drop for FfiOpaqueMemory {
    fn drop(&mut self) {
        unsafe {
            (self.dealloc_fn)(self.data);
        }
    }
}
