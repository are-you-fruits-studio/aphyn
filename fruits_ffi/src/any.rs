use std::{alloc::Layout, ffi::c_void, marker::PhantomData, mem::ManuallyDrop};

use crate::{FfiBox, FfiBoxVtable, FfiExtendedTypeInfo};

#[repr(C)]
struct FfiAnyMetadata {
    pub type_info: FfiExtendedTypeInfo,
    pub box_vtable: FfiBoxVtable,
}

impl FfiAnyMetadata {
    pub const fn new<T: 'static>() -> Self {
        Self {
            type_info: FfiExtendedTypeInfo::of::<T>(),
            box_vtable: FfiBoxVtable::new::<T>(),
        }
    }
}

#[repr(C)]
pub struct FfiAny {
    ptr: *mut c_void,
    meta: &'static FfiAnyMetadata,
}

impl FfiAny {
    pub fn new<T: 'static>(value: T) -> Self {
        unsafe {
            let ptr = std::alloc::alloc(Layout::new::<T>());

            (ptr as *mut T).write(value);
           
            Self {
                ptr: ptr as *mut c_void,
                meta: const { &FfiAnyMetadata::new::<T>() },
            }
        }
    }

    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        &self.meta.type_info
    }

    pub fn as_any_ref<'r>(&'r self) -> FfiAnyRef<'r> {
        FfiAnyRef {
            ptr: self.ptr,
            type_info: &self.meta.type_info,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_mut<'r>(&'r mut self) -> FfiAnyMut<'r> {
        FfiAnyMut {
            ptr: self.ptr,
            type_info: &self.meta.type_info,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_ptr(&self) -> FfiAnyPtr {
        FfiAnyPtr {
            ptr: self.ptr,
            type_info: &self.meta.type_info,
        }
    }

    pub fn downcast<T: 'static>(self) -> Option<FfiBox<T>> {
        unsafe { self.meta.type_info.short().does_match::<T>().then(|| {
            let this = ManuallyDrop::new(self);
            FfiBox::from_raw(this.ptr as *mut T, &this.meta.box_vtable)
        }) }
    }
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        unsafe { self.meta.type_info.short().does_match::<T>().then(|| &*(self.ptr as *const T)) }
    }
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        unsafe { self.meta.type_info.short().does_match::<T>().then(|| &mut *(self.ptr as *mut T)) }
    }
}

impl Drop for FfiAny {
    fn drop(&mut self) {
        struct FfiAnyMemoryToDealloc {
            ptr: *mut c_void,
            meta: &'static FfiAnyMetadata,
        }

        impl Drop for FfiAnyMemoryToDealloc {
            fn drop(&mut self) {
                unsafe { (self.meta.box_vtable.dealloc_fn)(self.ptr) };
            }
        }

        unsafe {
            let to_dealloc = FfiAnyMemoryToDealloc {
                ptr: self.ptr,
                meta: self.meta,
            };

            (self.meta.box_vtable.drop_in_place_fn)(self.ptr);

            drop(to_dealloc);
        }
    }
}

#[repr(C)]
pub struct FfiAnyRef<'a> {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FfiAnyRef<'a> {
    pub fn new<T: 'static>(value: &'a T) -> Self {
        Self {
            ptr: value as *const T as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            _phantom: PhantomData,
        }
    }

    pub const fn ptr(&self) -> *const c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub fn as_any_ptr(&self) -> FfiAnyPtr {
        FfiAnyPtr {
            ptr: self.ptr,
            type_info: &self.type_info,
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        unsafe { self.type_info.short().does_match::<T>().then(|| &*(self.ptr as *const T)) }
    }
}

#[repr(C)]
pub struct FfiAnyMut<'a> {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> FfiAnyMut<'a> {
    pub fn new<T: 'static>(value: &'a mut T) -> Self {
        Self {
            ptr: value as *mut T as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
            _phantom: PhantomData,
        }
    }

    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }

    pub fn as_any_ref<'r>(&'r self) -> FfiAnyRef<'r>
        where 'a: 'r
    {
        FfiAnyRef {
            ptr: self.ptr,
            type_info: &self.type_info,
            _phantom: PhantomData,
        }
    }
    pub fn as_any_ptr(&self) -> FfiAnyPtr {
        FfiAnyPtr {
            ptr: self.ptr,
            type_info: &self.type_info,
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        unsafe { self.type_info.short().does_match::<T>().then(|| &*(self.ptr as *const T)) }
    }
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        unsafe { self.type_info.short().does_match::<T>().then(|| &mut *(self.ptr as *mut T)) }
    }
}

#[repr(C)]
pub struct FfiAnyPtr {
    ptr: *mut c_void,
    type_info: &'static FfiExtendedTypeInfo,
}

impl FfiAnyPtr {
    pub fn new<T: 'static>(value: *mut T) -> Self {
        Self {
            ptr: value as *mut c_void,
            type_info: const { &FfiExtendedTypeInfo::of::<T>() },
        }
    }

    pub fn ptr(&self) -> *const c_void {
        self.ptr
    }

    pub const fn type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.type_info
    }
   

    pub unsafe fn as_any_ref<'r>(&self) -> FfiAnyRef<'r> {
        FfiAnyRef {
            ptr: self.ptr,
            type_info: &self.type_info,
            _phantom: PhantomData,
        }
    }
    pub unsafe fn as_any_mut<'r>(&self) -> FfiAnyMut<'r> {
        FfiAnyMut {
            ptr: self.ptr,
            type_info: &self.type_info,
            _phantom: PhantomData,
        }
    }

    pub fn downcast_ptr<T: 'static>(&self) -> Option<*mut T> {
        self.type_info.short().does_match::<T>().then(|| self.ptr as *mut T)
    }
}