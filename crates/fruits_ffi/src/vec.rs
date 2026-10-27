use std::{
    ffi::c_void, fmt::Debug, marker::PhantomData, ops::{Deref, DerefMut, Index, IndexMut},
};

use crate::FfiAllocator;

#[repr(transparent)]
pub struct FfiRawVec<T> {
    inner: FfiRawVecInner,
    _phantom: PhantomData<T>,
}

impl<T> FfiRawVec<T> {
    pub const fn new() -> Self {
        Self {
            inner: Self::empty_inner(),
            _phantom: PhantomData,
        }
    }

    pub unsafe fn from_raw_parts(vec: FfiRawVecInner) -> Self {
        Self {
            inner: vec,
            _phantom: PhantomData,
        }
    }

    pub fn with_capacity(mut cap: u64) -> Self {
        let allocator = FfiAllocator::from_global();

        let ptr;

        if std::mem::size_of::<T>() == 0 {
            ptr = std::ptr::null_mut();
            cap = ZERO_SIZED_CAP;
        } else {
            ptr = unsafe { allocator.alloc(std::mem::size_of::<T>() as u64 * cap, std::mem::align_of::<T>() as u64) };
        }

        Self {
            inner: FfiRawVecInner {
                allocator,
                cap,
                len: 0,
                ptr,
            },
            _phantom: PhantomData,
        }
    }

    pub const fn cap(&self) -> u64 {
        self.inner.cap
    }

    pub const fn len(&self) -> u64 {
        self.inner.len
    }

    pub const fn len_mut(&mut self) -> &mut u64 {
        &mut self.inner.len
    }

    pub const fn ptr(&self) -> *mut T {
        self.inner.ptr as *mut T
    }

    pub fn grow(&mut self, data_mover: impl FnOnce(*const T, *mut T, u64)) {
        if std::mem::size_of::<T>() == 0 {
            return;
        }

        let new_cap = if self.inner.cap == 0 { 4 } else { self.inner.cap * 2 };

        let size_of_t = std::mem::size_of::<T>() as u64;
        let align_of_t = std::mem::align_of::<T>() as u64;

        unsafe {
            let new_ptr = self.inner.allocator.alloc(size_of_t * new_cap, align_of_t);
            
            if self.inner.cap != 0 {
                data_mover(self.inner.ptr as *const T, new_ptr as *mut T, self.inner.len);

                self.inner.allocator.dealloc(self.inner.ptr, size_of_t * self.inner.cap, align_of_t);
            }
            
            self.inner.ptr = new_ptr;
            self.inner.cap = new_cap;
        }
    }

    const fn empty_inner() -> FfiRawVecInner {
        let cap = if std::mem::size_of::<T>() == 0 { ZERO_SIZED_CAP } else { 0 };

        FfiRawVecInner {
            allocator: FfiAllocator::from_global(),
            cap,
            len: 0,
            ptr: std::ptr::null_mut(),
        }
    }

    pub fn as_inner(&self) -> &FfiRawVecInner {
        &self.inner
    }
    pub fn as_inner_mut(&mut self) -> &mut FfiRawVecInner {
        &mut self.inner
    }
    pub fn into_inner(mut self) -> FfiRawVecInner {
        std::mem::replace(&mut self.inner, Self::empty_inner())
    }
}

impl<T> Drop for FfiRawVec<T> {
    fn drop(&mut self) {
        if self.inner.ptr.is_null() {
            return;
        }
        
        // todo
        unsafe {
            self.inner.allocator.dealloc(
                self.inner.ptr,
                std::mem::size_of::<T>() as u64 * self.inner.cap,
                std::mem::align_of::<T>() as u64,
            );
        }
    }
}

//

#[repr(C)]
pub struct FfiRawVecInner {
    pub ptr: *mut u8,
    pub cap: u64,
    pub len: u64,
    pub allocator: FfiAllocator,
}

//

#[repr(C)]
pub struct FfiOpaqueVec {
    // vec field needs to be first + repr(C) for transmute
    vec: FfiRawVecInner,
    element_size: u64,
    element_align: u64,
    drop_fn: Option<unsafe extern "C-unwind" fn(*mut c_void)>,
}

impl FfiOpaqueVec {
    pub const unsafe fn new(element_size: u64, element_align: u64, drop_fn: Option<unsafe extern "C-unwind" fn(*mut c_void)>) -> Self {
        let cap = if element_size == 0 { ZERO_SIZED_CAP } else { 0 };

        Self {
            vec: FfiRawVecInner {
                allocator: FfiAllocator::from_global(),
                cap,
                len: 0,
                ptr: std::ptr::null_mut(),
            },
            element_size,
            element_align,
            drop_fn,
        }
    }

    pub fn clear(&mut self) {
        if self.vec.len == 0 {
            return;
        }

        if let Some(drop_fn) = self.drop_fn {
            let ptr = if self.element_size == 0 {
                std::ptr::NonNull::dangling().as_ptr()
            } else {
                self.vec.ptr
            };

            unsafe {
                for i in 0..self.vec.len {
                    drop_fn(ptr.add((i * self.element_size) as usize) as *mut c_void)
                }
            }
        }

        self.vec.len = 0;
    }

    pub unsafe fn as_vec<T>(&self) -> &FfiVec<T> {
        unsafe { std::mem::transmute::<&FfiRawVecInner, &FfiVec<T>>(&self.vec) }
    }

    pub unsafe fn as_vec_mut<T>(&mut self) -> &mut FfiVec<T> {
        unsafe { std::mem::transmute::<&mut FfiRawVecInner, &mut FfiVec<T>>(&mut self.vec) }
    }

    pub unsafe fn as_vec_ptr<T>(this: *mut Self) -> *mut FfiVec<T> {
        this as *mut FfiVec<T>
    }
}

impl Drop for FfiOpaqueVec {
    fn drop(&mut self) {
        if self.vec.cap == 0 {
            return;
        }

        let ptr = if self.element_size == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.ptr
        };

        // todo
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, self.vec.len as usize));

            // todo: exec on drop for guaranteed dealloc?
            if self.element_size != 0 {
                self.vec
                    .allocator
                    .dealloc(self.vec.ptr, self.element_size * self.vec.cap, self.element_align);
            }
        }
    }
}

//

// todo
#[repr(transparent)]
pub struct FfiVec<T> {
    vec: FfiRawVec<T>,
}

impl<T> FfiVec<T> {
    pub const fn new() -> Self {
        Self {
            vec: FfiRawVec::new(),
        }
    }

    pub fn with_capacity(cap: u64) -> Self {
        Self {
            vec: FfiRawVec::with_capacity(cap),
        }
    }

    pub fn from_vec(mut vec: Vec<T>) -> Self {
        let ffi_vec = if std::mem::size_of::<T>() == 0 {
            let mut new_raw = FfiRawVec::new();
            *new_raw.len_mut() = vec.len() as u64;
            Self {
                vec: new_raw,
            }
        } else {
            let mut raw_inner = FfiRawVecInner {
                ptr: vec.as_mut_ptr() as *mut u8,
                cap: vec.capacity() as u64,
                len: vec.len() as u64,
                allocator: FfiAllocator::from_global(),
            };

            // todo: repeat the behavior of the std::vec::Vec and use dangling pointer for empty vec instead of the nullptr.
            if raw_inner.cap == 0 {
                raw_inner.ptr = std::ptr::null_mut();
            }

            unsafe {
                Self {
                    vec: FfiRawVec::from_raw_parts(raw_inner),
                }
            }
        };

        std::mem::forget(vec);

        ffi_vec
    }

    pub fn len(&self) -> u64 {
        self.vec.len()
    }

    pub fn capacity(&self) -> u64 {
        self.vec.cap()
    }

    pub const fn as_slice(&self) -> &[T] {
        if self.vec.len() == 0 {
            return &[];
        }

        let ptr = if std::mem::size_of::<T>() == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.ptr()
        };

        // todo
        unsafe { std::slice::from_raw_parts(ptr as *mut T, self.vec.len() as usize) }
    }

    unsafe fn get_element_ptr(&self, idx: u64) -> *mut T {
        unsafe { self.vec.ptr().add(idx as usize) }
    }

    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        if self.vec.len() == 0 {
            return &mut [];
        }

        let ptr = if std::mem::size_of::<T>() == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.ptr()
        };

        // todo
        unsafe { std::slice::from_raw_parts_mut(ptr as *mut T, self.vec.len() as usize) }
    }

    pub fn push(&mut self, v: T) {
        if std::mem::size_of::<T>() == 0 {
            *self.vec.len_mut() += 1;
            return;
        }

        if self.vec.len() == self.vec.cap() {
            self.vec.grow(|src, dst, len| unsafe {
                std::ptr::copy_nonoverlapping(src, dst, len as usize)
            });
        }

        unsafe {
            self.get_element_ptr(self.vec.len()).write(v);
        }

        *self.vec.len_mut() += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.vec.len() == 0 {
            return None;
        }

        *self.vec.len_mut() -= 1;

        if std::mem::size_of::<T>() == 0 {
            Some(unsafe { std::mem::zeroed::<T>() })
        } else {
            Some(unsafe { self.get_element_ptr(self.vec.len()).read() })
        }
    }

    pub fn get(&self, idx: u64) -> Option<&T> {
        self.as_slice().get(idx as usize)
    }

    pub fn get_mut(&mut self, idx: u64) -> Option<&mut T> {
        self.as_mut_slice().get_mut(idx as usize)
    }

    pub fn swap_remove(&mut self, idx: u64) -> Option<T> {
        if idx >= self.vec.len() {
            return None;
        }

        unsafe {
            let item = self.get_element_ptr(idx).read();

            *self.vec.len_mut() -= 1;

            std::ptr::copy(self.get_element_ptr(self.vec.len()), self.get_element_ptr(idx), 1);

            Some(item)
        }
    }

    pub fn remove(&mut self, idx: u64) -> Option<T> {
        if idx >= self.vec.len() {
            return None;
        }

        unsafe {
            let item = self.get_element_ptr(idx).read();

            let moved_count = self.vec.len() - 1 - idx;

            std::ptr::copy(self.get_element_ptr(idx + 1), self.get_element_ptr(idx), moved_count as usize);

            *self.vec.len_mut() -= 1;

            Some(item)
        }
    }

    pub fn clear(&mut self) {
        if self.vec.len() == 0 {
            return;
        }

        let ptr = if std::mem::size_of::<T>() == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.ptr()
        };

        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, self.vec.len() as usize));
        }

        *self.vec.len_mut() = 0;
    }
}

impl<T: Clone> FfiVec<T> {
    pub fn resize(&mut self, new_len: u64, value: T) {
        // todo: optimize
        while self.vec.len() > new_len {
            self.pop();
        }

        while self.vec.len() < new_len {
            self.push(value.clone());
        }
    }

    pub fn extend_from_slice(&mut self, slice: &[T]) {
        // todo: optimize
        for i in slice {
            self.push(i.clone());
        }
    }
}

impl<T> Default for FfiVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> FfiVec<T> {
    pub fn clone_to_vec(&self) -> Vec<T> {
        let mut vec = Vec::with_capacity(self.vec.len() as usize);

        for item in self {
            vec.push(item.clone());
        }

        vec
    }
}

impl<T> Drop for FfiVec<T> {
    fn drop(&mut self) {
        if self.vec.cap() == 0 {
            return;
        }

        let ptr = if std::mem::size_of::<T>() == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.ptr()
        };

        // todo
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, self.vec.len() as usize));
        }
    }
}

impl<T> From<Vec<T>> for FfiVec<T> {
    fn from(value: Vec<T>) -> Self {
        FfiVec::from_vec(value)
    }
}

impl<'a, T> IntoIterator for &'a FfiVec<T> {
    type Item = <&'a [T] as IntoIterator>::Item;

    type IntoIter = <&'a [T] as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().into_iter()
    }
}

impl<'a, T> IntoIterator for &'a mut FfiVec<T> {
    type Item = <&'a mut [T] as IntoIterator>::Item;

    type IntoIter = <&'a mut [T] as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().into_iter()
    }
}

impl<T: Debug> Debug for FfiVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}

impl<T> Deref for FfiVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for FfiVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T: Clone> Clone for FfiVec<T> {
    fn clone(&self) -> Self {
        if std::mem::size_of::<T>() == 0 {
            unsafe {
                for _ in 0..self.vec.len() {
                    std::mem::forget((&*std::ptr::NonNull::<T>::dangling().as_ptr()).clone());
                }

                let mut new_raw = FfiRawVec::new();
                *new_raw.len_mut() = self.vec.len();

                return Self {
                    vec: new_raw,
                };
            }
        }

        unsafe {
            let mut new_raw = FfiRawVec::<T>::with_capacity(self.vec.len());

            for i in 0..self.vec.len() {
                let cloned = (&*self.get_element_ptr(i)).clone();
                new_raw.ptr().add(i as usize).write(cloned)
            }

            *new_raw.len_mut() = self.vec.len();

            return Self {
                vec: new_raw,
            };
        }
    }
}

impl<T: PartialEq> PartialEq for FfiVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq> Eq for FfiVec<T> {}

impl<T> FromIterator<T> for FfiVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        // todo: optimize
        let mut vec = FfiVec::new();

        for item in iter {
            vec.push(item);
        }

        vec
    }
}

impl<T> IntoIterator for FfiVec<T> {
    type Item = T;

    type IntoIter = FfiVecIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        FfiVecIntoIter { vec: self, idx: 0 }
    }
}

#[repr(C)]
pub struct FfiVecIntoIter<T> {
    vec: FfiVec<T>,
    idx: u64,
}

impl<T> Iterator for FfiVecIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.vec.len() {
            return None;
        }

        unsafe {
            let item = self.vec.get_element_ptr(self.idx).read();

            self.idx += 1;

            Some(item)
        }
    }
}

impl<T> Drop for FfiVecIntoIter<T> {
    fn drop(&mut self) {
        let ptr = if std::mem::size_of::<T>() == 0 {
            std::ptr::NonNull::dangling().as_ptr()
        } else {
            self.vec.vec.ptr()
        };

        let count = self.vec.len() - self.idx;

        // todo
        unsafe {
            let ptr = ptr.add(self.idx as usize);

            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, count as usize));

            *self.vec.vec.len_mut() = 0;
        }
    }
}

impl<T> Index<u64> for FfiVec<T> {
    type Output = T;

    fn index(&self, idx: u64) -> &Self::Output {
        self.get(idx).expect("index out of range")
    }
}

impl<T> IndexMut<u64> for FfiVec<T> {
    fn index_mut(&mut self, idx: u64) -> &mut Self::Output {
        self.get_mut(idx).expect("index out of range")
    }
}

unsafe impl<T: Send> Send for FfiVec<T> {}
unsafe impl<T: Sync> Sync for FfiVec<T> {}

pub const ZERO_SIZED_CAP: u64 = {
    let max_64 = u64::MAX as u64;
    let max_size = usize::MAX as u64;

    if max_64 < max_size { max_64 } else { max_size }
};

// todo:
// - consuming IntoIterator
// - (reallocating) into_vec
// - docs
// - safe drops in case of exotic panics.
