use std::{ffi::c_void, marker::PhantomData};

// todo
unsafe trait HasFfiSafeAlt {
    type FfiSafeAlt;

    fn into_ffi(self) -> Self::FfiSafeAlt;
    fn from_ffi(alt: Self::FfiSafeAlt) -> Self;
}
#[repr(C)]
struct FfiSafeFn<I: HasFfiSafeAlt, O: HasFfiSafeAlt> {
    data: *const c_void,
    caller: unsafe extern "C-unwind" fn(*const c_void, I::FfiSafeAlt) -> O::FfiSafeAlt,
}
impl<I: HasFfiSafeAlt, O: HasFfiSafeAlt> FfiSafeFn<I, O> {
    pub const fn new<F: 'static + Fn(I) -> O>(f: &'static F) -> Self {
        unsafe extern "C-unwind" fn fn_ffi_alt<I: HasFfiSafeAlt, O: HasFfiSafeAlt, F: 'static + Fn(I) -> O>(ptr: *const c_void, i: I::FfiSafeAlt) -> O::FfiSafeAlt {
            let real_fn = unsafe { &*(ptr as *const F) };

            let o = real_fn(I::from_ffi(i));

            o.into_ffi()
        }

        Self {
            data: f as *const F as *const c_void,
            caller: fn_ffi_alt::<I, O, F>,
        }
    }

    pub fn call(&self, i: I) -> O {
        let i = i.into_ffi();
        let o = unsafe { (self.caller)(self.data, i) };
        O::from_ffi(o)
    }
}

//

#[repr(C)]
pub struct FfiFnRef<'a, I, O> {
    data: *const c_void,
    fn_execute: unsafe extern "C-unwind" fn(this: *const c_void, input: I) -> O,
    _phantom: PhantomData<&'a ()>,
}
impl<'a, I, O> FfiFnRef<'a, I, O> {
    pub fn new<F: Fn(I) -> O>(f: &'a F) -> Self {
        unsafe extern "C-unwind" fn ffi_execute<I, O, F: Fn(I) -> O>(this: *const c_void, input: I) -> O {
            unsafe {
                let this = &*(this as *const F);

                this(input)
            }
        }

        Self {
            data: f as *const F as *const c_void,
            fn_execute: ffi_execute::<I, O, F>,
            _phantom: Default::default(),
        }
    }

    pub fn execute(&self, input: I) -> O {
        unsafe {
            (self.fn_execute)(self.data, input)
        }
    }
}

#[repr(C)]
pub struct FfiFnMutMut<'a, I, O> {
    data: *mut c_void,
    fn_execute: unsafe extern "C-unwind" fn(this: *mut c_void, input: I) -> O,
    _phantom: PhantomData<&'a mut ()>,
}
impl<'a, I, O> FfiFnMutMut<'a, I, O> {
    pub fn new<F: FnMut(I) -> O>(f: &'a mut F) -> Self {
        unsafe extern "C-unwind" fn ffi_execute<I, O, F: FnMut(I) -> O>(this: *mut c_void, input: I) -> O {
            unsafe {
                let this = &mut *(this as *mut F);

                this(input)
            }
        }

        Self {
            data: f as *mut F as *mut c_void,
            fn_execute: ffi_execute::<I, O, F>,
            _phantom: Default::default(),
        }
    }

    pub fn as_mut<'r>(&'r mut self) -> FfiFnMutMut<'r, I, O>
        where 'a: 'r
    {
        FfiFnMutMut {
            data: self.data,
            fn_execute: self.fn_execute,
            _phantom: PhantomData,
        }
    }

    pub fn execute(&mut self, input: I) -> O {
        unsafe {
            (self.fn_execute)(self.data, input)
        }
    }
}

impl<'a, I, O, F: FnMut(I) -> O> From<&'a mut F> for FfiFnMutMut<'a, I, O> {
    fn from(value: &'a mut F) -> Self {
        Self::new(value)
    }
}