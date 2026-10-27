use crate::FfiRawVec;

// todo

#[repr(C)]
pub struct FfiVecDeque<T> {
    vec: FfiRawVec<T>,
    head: u64,
}

impl<T> FfiVecDeque<T> {
    pub const fn new() -> Self {
        Self {
            vec: FfiRawVec::new(),
            head: 0,
        }
    }

    pub fn with_capacity(cap: u64) -> Self {
        Self {
            vec: FfiRawVec::with_capacity(cap),
            head: 0,
        }
    }

    // pub fn from_vec(mut vec: Vec<T>) -> Self {
    //     let ffi_vec = if std::mem::size_of::<T>() == 0 {
    //         Self {
    //             vec: FfiRawVecInner {
    //                 ptr: std::ptr::null_mut(),
    //                 cap: ZERO_SIZED_CAP,
    //                 len: vec.len() as u64,
    //                 allocator: FfiAllocator::from_global(),
    //             },
    //             _phantom: PhantomData,
    //         }
    //     } else {
    //         Self {
    //             vec: FfiRawVecInner {
    //                 ptr: vec.as_mut_ptr() as *mut u8,
    //                 cap: vec.capacity() as u64,
    //                 len: vec.len() as u64,
    //                 allocator: FfiAllocator::from_global(),
    //             },
    //             _phantom: PhantomData,
    //         }
    //     };

    //     std::mem::forget(vec);

    //     ffi_vec
    // }

    pub fn len(&self) -> u64 {
        self.vec.len()
    }

    pub fn capacity(&self) -> u64 {
        self.vec.cap()
    }

    // pub const fn as_slice(&self) -> &[T] {
    //     if self.vec.len == 0 {
    //         return &[];
    //     }

    //     let ptr = if std::mem::size_of::<T>() == 0 {
    //         std::ptr::NonNull::dangling().as_ptr()
    //     } else {
    //         self.vec.ptr
    //     };

    //     // todo
    //     unsafe { std::slice::from_raw_parts(ptr as *mut T, self.vec.len as usize) }
    // }

    unsafe fn get_element_ptr(&self, idx: u64) -> *mut T {
        unsafe { self.vec.ptr().add(((self.head + idx) % self.vec.cap()) as usize) }
    }

    // pub const fn as_slice_mut(&mut self) -> &mut [T] {
    //     if self.vec.len == 0 {
    //         return &mut [];
    //     }

    //     let ptr = if std::mem::size_of::<T>() == 0 {
    //         std::ptr::NonNull::dangling().as_ptr()
    //     } else {
    //         self.vec.ptr
    //     };

    //     // todo
    //     unsafe { std::slice::from_raw_parts_mut(ptr as *mut T, self.vec.len as usize) }
    // }

    pub fn push_back(&mut self, v: T) {
        if std::mem::size_of::<T>() == 0 {
            *self.vec.len_mut() += 1;
            return;
        }

        if self.vec.len() == self.vec.cap() {
            let last_head = self.head;
            let last_cap = self.capacity();

            self.vec.grow(|src, dst, len| unsafe {
                let elements_count_0 = (last_cap - last_head).min(len);
                let elements_count_1 = len - elements_count_0;
                
                std::ptr::copy_nonoverlapping(src.add(last_head as usize), dst, elements_count_0 as usize);
                std::ptr::copy_nonoverlapping(src, dst.add(elements_count_0 as usize), elements_count_1 as usize);
            });

            self.head = 0;
        }

        unsafe {
            self.get_element_ptr(self.vec.len()).write(v);
        }

        *self.vec.len_mut() += 1;
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.vec.len() == 0 {
            return None;
        }

        *self.vec.len_mut() -= 1;

        let item = if std::mem::size_of::<T>() == 0 {
            unsafe { std::mem::zeroed::<T>() }
        } else {
            unsafe { self.get_element_ptr(0).read() }
        };
        
        self.head = (self.head + 1) % self.capacity();

        Some(item)
    }

    // pub fn get(&self, idx: u64) -> Option<&T> {
    //     self.as_slice().get(idx as usize)
    // }

    // pub fn get_mut(&mut self, idx: u64) -> Option<&mut T> {
    //     self.as_slice_mut().get_mut(idx as usize)
    // }

    // pub fn swap_remove(&mut self, idx: u64) -> Option<T> {
    //     if idx >= self.vec.len {
    //         return None;
    //     }

    //     unsafe {
    //         let item = self.get_element_ptr(idx).read();

    //         self.vec.len -= 1;

    //         std::ptr::copy(self.get_element_ptr(self.vec.len), self.get_element_ptr(idx), 1);

    //         Some(item)
    //     }
    // }

    // pub fn remove(&mut self, idx: u64) -> Option<T> {
    //     if idx >= self.vec.len {
    //         return None;
    //     }

    //     unsafe {
    //         let item = self.get_element_ptr(idx).read();

    //         let moved_count = self.vec.len - 1 - idx;

    //         std::ptr::copy(self.get_element_ptr(idx + 1), self.get_element_ptr(idx), moved_count as usize);

    //         self.vec.len -= 1;

    //         Some(item)
    //     }
    // }

    // pub fn clear(&mut self) {
    //     if self.vec.len == 0 {
    //         return;
    //     }

    //     let ptr = if std::mem::size_of::<T>() == 0 {
    //         std::ptr::NonNull::dangling().as_ptr()
    //     } else {
    //         self.vec.ptr
    //     };

    //     unsafe {
    //         std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, self.vec.len as usize));
    //     }

    //     self.vec.len = 0;
    // }
}

// impl<T: Clone> FfiVecDeque<T> {
//     pub fn resize(&mut self, new_len: u64, value: T) {
//         // todo: optimize
//         while self.vec.len > new_len {
//             self.pop();
//         }

//         while self.vec.len < new_len {
//             self.push(value.clone());
//         }
//     }

//     pub fn extend_from_slice(&mut self, slice: &[T]) {
//         // todo: optimize
//         for i in slice {
//             self.push(i.clone());
//         }
//     }
// }

impl<T> Default for FfiVecDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

// impl<T: Clone> FfiVecDeque<T> {
//     pub fn clone_to_vec(&self) -> Vec<T> {
//         let mut vec = Vec::with_capacity(self.vec.len as usize);

//         for item in self {
//             vec.push(item.clone());
//         }

//         vec
//     }
// }

impl<T> Drop for FfiVecDeque<T> {
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
            let elements_count_0 = (self.vec.cap() - self.head).min(self.len());
            let elements_count_1 = self.len() - elements_count_0;

            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr.add(self.head as usize), elements_count_0 as usize));
            std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, elements_count_1 as usize));
        }
    }
}

// impl<T> From<Vec<T>> for FfiVec<T> {
//     fn from(value: Vec<T>) -> Self {
//         FfiVec::from_vec(value)
//     }
// }

// impl<'a, T> IntoIterator for &'a FfiVec<T> {
//     type Item = <&'a [T] as IntoIterator>::Item;

//     type IntoIter = <&'a [T] as IntoIterator>::IntoIter;

//     fn into_iter(self) -> Self::IntoIter {
//         self.as_slice().into_iter()
//     }
// }

// impl<'a, T> IntoIterator for &'a mut FfiVec<T> {
//     type Item = <&'a mut [T] as IntoIterator>::Item;

//     type IntoIter = <&'a mut [T] as IntoIterator>::IntoIter;

//     fn into_iter(self) -> Self::IntoIter {
//         self.as_slice_mut().into_iter()
//     }
// }

// impl<T: Debug> Debug for FfiVec<T> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         Debug::fmt(self.as_slice(), f)
//     }
// }

// impl<T> Deref for FfiVec<T> {
//     type Target = [T];

//     fn deref(&self) -> &Self::Target {
//         self.as_slice()
//     }
// }

// impl<T> DerefMut for FfiVec<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.as_slice_mut()
//     }
// }

// impl<T: Clone> Clone for FfiVec<T> {
//     fn clone(&self) -> Self {
//         if std::mem::size_of::<T>() == 0 {
//             unsafe {
//                 for _ in 0..self.vec.len {
//                     std::mem::forget((&*std::ptr::NonNull::<T>::dangling().as_ptr()).clone());
//                 }

//                 return Self {
//                     vec: FfiRawVec {
//                         ptr: self.vec.ptr,
//                         cap: self.vec.len,
//                         len: self.vec.len,
//                         allocator: self.vec.allocator,
//                     },
//                     _phantom: PhantomData,
//                 };
//             }
//         }

//         unsafe {
//             let new_ptr = self.vec.allocator.alloc(
//                 std::mem::size_of::<T>() as u64 * self.vec.len,
//                 std::mem::align_of::<T>() as u64,
//             ) as *mut T;

//             for i in 0..self.vec.len {
//                 let cloned = (&*self.get_element_ptr(i)).clone();
//                 new_ptr.add(i as usize).write(cloned)
//             }

//             return Self {
//                 vec: FfiRawVec {
//                     ptr: new_ptr as *mut u8,
//                     cap: self.vec.len,
//                     len: self.vec.len,
//                     allocator: self.vec.allocator,
//                 },
//                 _phantom: PhantomData,
//             };
//         }
//     }
// }

// impl<T: PartialEq> PartialEq for FfiVec<T> {
//     fn eq(&self, other: &Self) -> bool {
//         self.as_slice() == other.as_slice()
//     }
// }

// impl<T: Eq> Eq for FfiVec<T> {}

// impl<T> FromIterator<T> for FfiVec<T> {
//     fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
//         // todo: optimize
//         let mut vec = FfiVec::new();

//         for item in iter {
//             vec.push(item);
//         }

//         vec
//     }
// }

// impl<T> IntoIterator for FfiVec<T> {
//     type Item = T;

//     type IntoIter = FfiVecIntoIter<T>;

//     fn into_iter(self) -> Self::IntoIter {
//         FfiVecIntoIter { vec: self, idx: 0 }
//     }
// }

// #[repr(C)]
// pub struct FfiVecIntoIter<T> {
//     vec: FfiVec<T>,
//     idx: u64,
// }

// impl<T> Iterator for FfiVecIntoIter<T> {
//     type Item = T;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.idx >= self.vec.vec.len {
//             return None;
//         }

//         unsafe {
//             let item = self.vec.get_element_ptr(self.idx).read();

//             self.idx += 1;

//             Some(item)
//         }
//     }
// }

// impl<T> Drop for FfiVecIntoIter<T> {
//     fn drop(&mut self) {
//         let ptr = if std::mem::size_of::<T>() == 0 {
//             std::ptr::NonNull::dangling().as_ptr()
//         } else {
//             self.vec.vec.ptr
//         };

//         let count = self.vec.vec.len - self.idx;

//         // todo
//         unsafe {
//             let ptr = ptr.add(self.idx as usize);

//             std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(ptr, count as usize));

//             self.vec.vec.len = 0;
//         }
//     }
// }

// impl<T> Index<u64> for FfiVec<T> {
//     type Output = T;

//     fn index(&self, idx: u64) -> &Self::Output {
//         self.get(idx).expect("index out of range")
//     }
// }

// impl<T> IndexMut<u64> for FfiVec<T> {
//     fn index_mut(&mut self, idx: u64) -> &mut Self::Output {
//         self.get_mut(idx).expect("index out of range")
//     }
// }

unsafe impl<T: Send> Send for FfiVecDeque<T> {}
unsafe impl<T: Sync> Sync for FfiVecDeque<T> {}

// todo:
// - consuming IntoIterator
// - (reallocating) into_vec
// - docs
// - safe drops in case of exotic panics.
