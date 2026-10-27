// todo: use trait to control what type support such transmute.

use std::ops::Deref;

pub fn as_bytes_slice<T: AllBitsInit>(v: &[T]) -> &[u8] {
    unsafe { v.align_to::<u8>().1 }
}

pub fn as_bytes_slice_mut<T: AllBitVariationsValid>(v: &mut [T]) -> &mut [u8] {
    unsafe { v.align_to_mut::<u8>().1 }
}

pub fn as_bytes<T: AllBitsInit>(v: &T) -> &[u8] {
    as_bytes_slice(std::slice::from_ref(v))
}

pub fn as_bytes_mut<T: AllBitVariationsValid>(v: &mut T) -> &mut [u8] {
    as_bytes_slice_mut(std::slice::from_mut(v))
}

pub unsafe trait AllBitsInit {}
pub unsafe trait AllBitVariationsValid: AllBitsInit {}

macro_rules! all_bit_variations_valid_impl {
    ($($i: ident),+) => {
        $(
            unsafe impl AllBitsInit for $i { }
            unsafe impl AllBitVariationsValid for $i { }
        )+
    };
}

all_bit_variations_valid_impl! { u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64 }

unsafe impl<T: AllBitsInit> AllBitsInit for [T] {}
unsafe impl<T: AllBitVariationsValid> AllBitVariationsValid for [T] {}

unsafe impl<const N: usize, T: AllBitsInit> AllBitsInit for [T; N] {}
unsafe impl<const N: usize, T: AllBitVariationsValid> AllBitVariationsValid for [T; N] {}

//

// todo: useless?
#[repr(transparent)]
pub struct ReadOnly<T>(T);

impl<T> ReadOnly<T> {
    pub fn new(v: T) -> Self {
        Self(v)
    }

    pub fn as_ref(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for ReadOnly<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
