use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

// todo: ffi?
pub struct MorphVec<T> {
    buf: *mut u8,
    cap_bytes: usize,
    len: usize,
    _phantom: PhantomData<T>,
}
impl<T> MorphVec<T> {
    pub fn new() -> Self {
        Self {
            buf: std::ptr::null_mut(),
            cap_bytes: 0,
            len: 0,
            _phantom: Default::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, v: T) {
        let cap = (self.cap_bytes - std::mem::align_of::<T>().max(1) - 1) / std::mem::size_of::<T>();

        if self.buf.is_null() {
            let size = std::mem::size_of::<T>() * 4 + std::mem::align_of::<T>().max(1) - 1;

            // Safety. Memory is freed in Drip and realloc.
            self.buf = unsafe {
                let layout = Layout::from_size_align(size, 1).unwrap();
                let ptr = std::alloc::alloc(layout);
                if ptr.is_null() {
                    std::alloc::handle_alloc_error(layout);
                }
                ptr
            };
            self.cap_bytes = size;
        } else if self.len == cap {
            let size = self.cap_bytes * 2;

            // Safety. Memory is freed in Drip and realloc.
            let new_buf = unsafe {
                let layout = Layout::from_size_align(size, 1).unwrap();
                let ptr = std::alloc::alloc(layout);
                if ptr.is_null() {
                    std::alloc::handle_alloc_error(layout);
                }
                ptr
            };

            // Safety. Alignment is added to the buffer size on realloc.
            let offset_new_buf = unsafe { Self::buf_start(new_buf) };

            // Safety. Alignment is added to the buffer size on realloc.
            let offset_old_buf = unsafe { Self::buf_start(self.buf) };

            // Safety. Memory is alligned and allocated.
            unsafe {
                std::ptr::copy_nonoverlapping(offset_old_buf, offset_new_buf, self.len);
            }

            // Safety. Memory was allocated on realloc.
            unsafe {
                std::alloc::dealloc(self.buf, Layout::from_size_align(self.cap_bytes, 1).unwrap());
            }

            // todo
            self.buf = new_buf;
            self.cap_bytes = size;
        }

        // Safety. Alignment is added to the buffer size on realloc.
        let buf = unsafe { Self::buf_start(self.buf) };

        // Safety. Valid for writes - part of the allocation, aligned earlier in the function.
        unsafe {
            (buf.add(self.len)).write(v);
        }

        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // Safety. Memory is aligned and exists, because of len.
        unsafe { Some(Self::buf_start(self.buf).add(self.len).read()) }
    }
    pub fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len {
            return None;
        }

        // Safety. Memory is aligned and exists, because of len.
        unsafe { Some(&*Self::buf_start(self.buf).add(self.len)) }
    }
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        if i >= self.len {
            return None;
        }

        // Safety. Memory is aligned and exists, because of len.
        unsafe { Some(&mut *Self::buf_start(self.buf).add(self.len)) }
    }

    pub fn clear(&mut self) {
        if self.len == 0 {
            return;
        }

        // Safety. Same as Vec. Drops values in place as a slice.
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(Self::buf_start(self.buf), self.len));
        }

        self.len = 0;
    }

    /// Drops elements if any. Reuses the same memory.
    pub fn morph_into<U>(mut self) -> MorphVec<U> {
        self.clear();

        let old = ManuallyDrop::new(self);

        MorphVec::<U> {
            buf: old.buf,
            cap_bytes: old.cap_bytes,
            len: 0,
            _phantom: Default::default(),
        }
    }

    pub fn as_slice(&self) -> &[T] {
        // Safety. Aligned, length-checked, lifetimes-checked.
        unsafe { std::slice::from_raw_parts(Self::buf_start(self.buf), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // Safety. Aligned, length-checked, lifetimes-checked.
        unsafe { std::slice::from_raw_parts_mut(Self::buf_start(self.buf), self.len) }
    }

    /// # Safety
    ///
    /// Managed by caller.
    unsafe fn buf_start(ptr: *mut u8) -> *mut T {
        // Safety. Managed by caller.
        unsafe { ptr.add(ptr.align_offset(std::mem::align_of::<T>())) as *mut T }
    }
}
impl<T> Drop for MorphVec<T> {
    fn drop(&mut self) {
        // Safety. Same as Vec. Drops values in place as a slice and deallocates memory.
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(Self::buf_start(self.buf), self.len));

            if !self.buf.is_null() {
                std::alloc::dealloc(self.buf, Layout::from_size_align(self.cap_bytes, 1).unwrap());
            }
        }
    }
}
impl<T> Default for MorphVec<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Deref for MorphVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl<T> DerefMut for MorphVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
