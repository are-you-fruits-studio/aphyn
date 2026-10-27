use std::marker::PhantomData;

#[repr(C)]
pub struct FfiSliceMut<'a, T> {
    ptr: *mut T,
    len: u64,
    _phantom: PhantomData<&'a mut T>,
}

impl<'a, T> FfiSliceMut<'a, T> {
    pub fn into_slice_mut(self) -> &'a mut [T] {
        // todo
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len as usize) }
    }

    pub fn into_slice(self) -> &'a [T] {
        // todo
        unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) }
    }

    pub fn from_slice(slice: &'a mut [T]) -> Self {
        Self {
            len: slice.len() as u64,
            ptr: (&raw mut *slice) as *mut T,
            _phantom: PhantomData,
        }
    }
}

#[repr(C)]
pub struct FfiSliceRef<'a, T> {
    ptr: *const T,
    len: u64,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T> FfiSliceRef<'a, T> {
    pub fn into_slice(self) -> &'a [T] {
        // todo
        unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) }
    }

    pub fn from_slice(slice: &'a [T]) -> Self {
        Self {
            len: slice.len() as u64,
            ptr: (&raw const *slice) as *const T,
            _phantom: PhantomData,
        }
    }
}

//

#[repr(C)]
pub struct FfiStrSliceMut<'a> {
    bytes: FfiSliceMut<'a, u8>,
}

impl<'a> FfiStrSliceMut<'a> {
    pub fn into_slice_mut(self) -> &'a mut str {
        // todo
        unsafe { std::str::from_utf8_unchecked_mut(self.bytes.into_slice_mut()) }
    }

    pub fn into_slice(self) -> &'a str {
        // todo
        unsafe { std::str::from_utf8_unchecked(self.bytes.into_slice()) }
    }

    pub fn from_slice(slice: &'a mut str) -> Self {
        unsafe {
            Self {
                bytes: FfiSliceMut::from_slice(slice.as_bytes_mut()),
            }
        }
    }
}

#[repr(C)]
pub struct FfiStrSliceRef<'a> {
    bytes: FfiSliceRef<'a, u8>,
}

impl<'a> FfiStrSliceRef<'a> {
    pub fn into_slice(self) -> &'a str {
        // todo
        unsafe { std::str::from_utf8_unchecked(self.bytes.into_slice()) }
    }

    pub fn from_slice(slice: &'a str) -> Self {
        Self {
            bytes: FfiSliceRef::from_slice(slice.as_bytes()),
        }
    }
}

// todo: From, Send, Sync