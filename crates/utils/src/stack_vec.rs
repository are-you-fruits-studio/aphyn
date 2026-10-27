use std::{
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut, Index, IndexMut},
};

// todo: ffi?
pub struct StackVec<T, const C: usize> {
    buf: [MaybeUninit<T>; C],
    len: usize,
}

impl<T, const C: usize> StackVec<T, C> {
    pub const fn new() -> Self {
        Self {
            buf: [const { MaybeUninit::uninit() }; C],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, v: T) -> Result<(), T> {
        if self.len == C {
            return Err(v);
        }

        self.buf[self.len] = MaybeUninit::new(v);
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        // Safety. Init state is managed by len.
        let item = unsafe { std::ptr::read(self.buf[self.len - 1].as_mut_ptr()) };
        self.len -= 1;
        Some(item)
    }

    pub const fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len {
            return None;
        }

        // Safety. Init state is managed by len.
        unsafe { Some(self.buf[i].assume_init_ref()) }
    }

    pub const fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        if i >= self.len {
            return None;
        }

        // Safety. Init state is managed by len.
        unsafe { Some(self.buf[i].assume_init_mut()) }
    }

    pub const fn as_slice(&self) -> &[T] {
        // Safety. Init state is managed by len.
        unsafe { std::slice::from_raw_parts(self.buf.as_ptr() as *const T, self.len) }
    }

    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // Safety. Init state is managed by len.
        unsafe { std::slice::from_raw_parts_mut(self.buf.as_mut_ptr() as *mut T, self.len) }
    }

    pub fn try_into_array(self) -> Result<[T; C], Self> {
        Ok(unsafe {
            if self.len != C {
                return Err(self);
            }

            let this = ManuallyDrop::new(self);

            let buf_ptr = &raw const this.buf;
            let buf_ptr = buf_ptr as *const [T; C];

            buf_ptr.read()
        })
    }
}

impl<T, const C: usize> Default for StackVec<T, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const C: usize> Deref for StackVec<T, C> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const C: usize> DerefMut for StackVec<T, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, const C: usize> Index<usize> for StackVec<T, C> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!("Index out of range.");
        }

        // Safety. Init state is managed by len.
        unsafe { self.buf[index].assume_init_ref() }
    }
}

impl<T, const C: usize> IndexMut<usize> for StackVec<T, C> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            panic!("Index out of range.");
        }

        // Safety. Init state is managed by len.
        unsafe { self.buf[index].assume_init_mut() }
    }
}

impl<T, const C: usize> Drop for StackVec<T, C> {
    fn drop(&mut self) {
        // Safety. Init state is managed by len.
        unsafe {
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(self.buf.as_mut_ptr(), self.len));
        }
    }
}
