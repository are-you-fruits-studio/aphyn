use std::{ffi::c_void, mem::ManuallyDrop};

use aphyn_ffi::FfiOption;

#[repr(C)]
pub struct FfiBatch {
    pub data: *const c_void,
    pub next_fn: unsafe extern "C-unwind" fn(*const c_void, FfiOption<u64>) -> FfiOption<u64>,
    pub execute_fn: unsafe extern "C-unwind" fn(*const c_void, u64),
}

impl FfiBatch {
    pub unsafe fn next(&self, finished: Option<u64>) -> Option<u64> {
        unsafe { (self.next_fn)(self.data, finished.into()).into_option() }
    }

    pub unsafe fn execute(&self, task: u64) {
        unsafe { (self.execute_fn)(self.data, task) }
    }
}

#[repr(C)]
pub struct FfiJob {
    data: *mut c_void,
    call_fn: unsafe extern "C-unwind" fn(*mut c_void),
    drop_fn: unsafe extern "C-unwind" fn(*mut c_void),
}

impl FfiJob {
    pub fn new<F: 'static + Send + FnOnce()>(f: F) -> Self {
        unsafe { Self::new_unchecked(f) }
    }

    // Safety: the job must be called or dropped before anything `f` borrows goes away.
    pub unsafe fn new_unchecked<F: Send + FnOnce()>(f: F) -> Self {
        unsafe extern "C-unwind" fn ffi_call<F: FnOnce()>(data: *mut c_void) {
            let f = unsafe { *Box::from_raw(data as *mut F) };
            f();
        }

        unsafe extern "C-unwind" fn ffi_drop<F>(data: *mut c_void) {
            unsafe { drop(Box::from_raw(data as *mut F)) }
        }

        Self {
            data: Box::into_raw(Box::new(f)) as *mut c_void,
            call_fn: ffi_call::<F>,
            drop_fn: ffi_drop::<F>,
        }
    }

    pub fn call(self) {
        let this = ManuallyDrop::new(self);

        unsafe { (this.call_fn)(this.data) }
    }
}

impl Drop for FfiJob {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.data) }
    }
}

unsafe impl Send for FfiJob {}
