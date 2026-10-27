use std::{ffi::c_void, marker::PhantomData};

use crate::FfiShortTypeInfo;

pub struct FfiReturnHandle<'a> {
    data: *mut c_void,
    fn_try_write: unsafe extern "C-unwind" fn(*mut c_void, src: *const u8, type_info: &FfiShortTypeInfo) -> bool,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FfiReturnHandle<'a> {
    pub fn new<T>(data: &mut Option<T>) -> Self {
        unsafe extern "C-unwind" fn ffi_try_write<T>(dst: *mut c_void, src: *const u8, type_info: &FfiShortTypeInfo) -> bool {
            unsafe {
                if !type_info.does_match::<T>() {
                    return false;
                }

                let dst = &mut *(dst as *mut Option<T>);

                *dst = Some(std::ptr::read(src as *const T));
                true
            }
        }

        Self {
            data: data as *mut Option<T> as *mut c_void,
            fn_try_write: ffi_try_write::<T>,
            _phantom: PhantomData,
        }
    }

    pub fn try_write<T>(&mut self, data: T) -> bool {
        unsafe {
            if (self.fn_try_write)(self.data, &data as *const T as *const u8, &FfiShortTypeInfo::of::<T>()) {
                std::mem::forget(data);
                true
            } else {
                false
            }
        }
    }
}