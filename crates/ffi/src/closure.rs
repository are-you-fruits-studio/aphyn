use std::{ffi::c_void, marker::PhantomData};

use crate::{FfiAllocator, FfiOption, FfiVec};

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

//

#[repr(C)]
struct FfiFnOnceVtable<I, O> {
    call_once: unsafe extern "C-unwind" fn(this: *mut c_void, input: I) -> O,
    drop_in_place: unsafe extern "C-unwind" fn(this: *mut c_void),
}

#[repr(C)]
struct FfiFnOnceEntry<I, O> {
    offset: u64,
    fns: FfiOption<FfiFnOnceVtable<I, O>>,
}

#[repr(C)]
pub struct FfiFnOnceVec<'a, I, O> {
    alloc_ptr: *mut u8,
    alloc_size: u64,
    data_start: u64,
    bytes_len: u64,
    max_align: u64,
    allocator: FfiAllocator,
    entries: FfiVec<FfiFnOnceEntry<I, O>>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, I, O> FfiFnOnceVec<'a, I, O> {
    pub const fn new() -> Self {
        Self {
            alloc_ptr: std::ptr::null_mut(),
            alloc_size: 0,
            data_start: 0,
            bytes_len: 0,
            max_align: 1,
            allocator: FfiAllocator::from_global(),
            entries: FfiVec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }

    pub fn push<F: FnOnce(I) -> O + 'a>(&mut self, f: F) -> usize {
        unsafe extern "C-unwind" fn ffi_call_once<I, O, F: FnOnce(I) -> O>(this: *mut c_void, input: I) -> O {
            let f = unsafe { (this as *mut F).read() };
            f(input)
        }
        unsafe extern "C-unwind" fn ffi_drop_in_place<F>(this: *mut c_void) {
            unsafe { std::ptr::drop_in_place(this as *mut F) }
        }

        let align = align_of::<F>() as u64;
        let offset = self.bytes_len.next_multiple_of(align);
        let needed = offset + size_of::<F>() as u64;
        self.max_align = self.max_align.max(align);

        if self.data_start + needed > self.alloc_size || !(self.base() as usize).is_multiple_of(self.max_align as usize) {
            self.realloc(needed);
        }

        unsafe { (self.base().add(offset as usize) as *mut F).write(f) };
        self.bytes_len = needed;

        self.entries.push(FfiFnOnceEntry {
            offset,
            fns: FfiOption::Some(FfiFnOnceVtable {
                call_once: ffi_call_once::<I, O, F>,
                drop_in_place: ffi_drop_in_place::<F>,
            }),
        });

        self.len() - 1
    }

    pub fn call(&mut self, idx: usize, input: I) -> Option<O> {
        let base = self.base();
        let entry = self.entries.get_mut(idx as u64)?;
        let fns = entry.fns.take()?;

        unsafe { Some((fns.call_once)(base.add(entry.offset as usize) as *mut c_void, input)) }
    }

    pub fn clear(&mut self) {
        let base = self.base();
        for entry in self.entries.as_mut_slice() {
            if let Some(fns) = entry.fns.take() {
                unsafe { (fns.drop_in_place)(base.add(entry.offset as usize) as *mut c_void) };
            }
        }

        self.entries.clear();
        self.bytes_len = 0;
    }

    fn base(&self) -> *mut u8 {
        if self.alloc_ptr.is_null() {
            std::ptr::without_provenance_mut(self.max_align as usize)
        } else {
            unsafe { self.alloc_ptr.add(self.data_start as usize) }
        }
    }

    fn realloc(&mut self, needed: u64) {
        let old_cap = self.alloc_size - self.data_start;
        let new_cap = needed.max(old_cap * 2).max(64);
        let new_size = new_cap + self.max_align - 1;

        unsafe {
            let new_ptr = self.allocator.alloc(new_size, 1);
            let new_start = new_ptr.align_offset(self.max_align as usize) as u64;

            if !self.alloc_ptr.is_null() {
                std::ptr::copy_nonoverlapping(self.base(), new_ptr.add(new_start as usize), self.bytes_len as usize);
                self.allocator.dealloc(self.alloc_ptr, self.alloc_size, 1);
            }

            self.alloc_ptr = new_ptr;
            self.alloc_size = new_size;
            self.data_start = new_start;
        }
    }
}

impl<'a, I, O> Default for FfiFnOnceVec<'a, I, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, I, O> Drop for FfiFnOnceVec<'a, I, O> {
    fn drop(&mut self) {
        self.clear();

        if !self.alloc_ptr.is_null() {
            unsafe { self.allocator.dealloc(self.alloc_ptr, self.alloc_size, 1) };
        }
    }
}
