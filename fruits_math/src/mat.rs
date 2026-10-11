use std::{marker::PhantomData, mem::ManuallyDrop, ops::{Index, IndexMut, Mul}};

use fruits_utils::{mem::{AllBitVariationsValid, AllBitsInit}, stack_vec::StackVec};
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeSeq};

use crate::{Primitive, num::Number};

/// Column-major
#[derive(Copy, Clone, Debug, PartialEq, Hash)]
#[repr(transparent)]
pub struct Mat<const N: usize, T> {
    data: [[T; N]; N],
}

impl<const N: usize, T> Default for Mat<N, T>
    where [[T; N]; N]: Default
{
    fn default() -> Self {
        Self { data: Default::default() }
    }
}

impl<const N: usize, T> Mat<N, T> {
    pub const fn from_array(data: [[T; N]; N]) -> Self {
        Self { data }
    }
    pub const fn as_array(&self) -> &[[T; N]; N] {
        &self.data
    }
    pub const fn as_array_mut(&mut self) -> &mut [[T; N]; N] {
        &mut self.data
    }

    pub const fn try_col(&self, x: usize) -> Option<&[T; N]> {
        if x >= N {
            return None;
        }

        Some(&self.data[x])
    }

    pub const fn col(&self, x: usize) -> &[T; N] {
        &self.data[x]
    }

    pub const fn try_col_mut(&mut self, x: usize) -> Option<&mut [T; N]> {
        if x >= N {
            return None;
        }

        Some(&mut self.data[x])
    }

    pub const fn col_mut(&mut self, x: usize) -> &mut [T; N] {
        &mut self.data[x]
    }
}

impl<const N: usize, T: Primitive> Mat<N, T> {
    pub const fn into_array(self) -> [[T; N]; N] {
        self.data
    }

    pub const fn try_row(&self, y: usize) -> Option<[T; N]> {
        if y >= N {
            return None;
        }

        let mut array = [T::ZERO; N];

        let mut i = 0;

        while i < N {
            array[i] = self.data[i][y];
            i += 1;
        }

        Some(array)
    }

    pub const fn row(&self, y: usize) -> [T; N] {
        let mut array = [T::ZERO; N];

        let mut i = 0;

        while i < N {
            array[i] = self.data[i][y];
            i += 1;
        }

        array
    }

    pub const fn get(&self, x: usize, y: usize) -> Option<&T> {
        if x >= N || y >= N {
            return None;
        }

        Some(&self.data[x][y])
    }

    pub const fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        if x >= N || y >= N {
            return None;
        }

        Some(&mut self.data[x][y])
    }

    pub const fn splat(v: T) -> Self {
        Self::from_array([[v; N]; N])
    }

    pub const fn transpose(&mut self) {
        let mut i = 0;

        while i < N {
            let mut j = 0;

            while j < i {
                (self.data[i][j], self.data[j][i]) = (self.data[j][i], self.data[i][j]);

                j += 1;
            }

            i += 1;
        }
    }

    pub const IDENTITY: Self = {
        let mut mat = Mat::<N, T>::splat(T::ZERO);

        let mut i = 0;
        while i < N {
            mat.data[i][i] = T::ONE;
            i += 1;
        }

        mat
    };

    pub(crate) const fn ignored_element(&self, ignored_x: usize, ignored_y: usize, index_x: usize, index_y: usize) -> T {
        let data = self.as_array();

        data[index_x + (ignored_x <= index_x) as usize][index_y + (ignored_y <= index_y) as usize]
    }
}

impl<const N: usize, T: Primitive> From<Mat<N, T>> for [[T; N]; N] {
    fn from(data: Mat<N, T>) -> Self {
        data.into_array()
    }
}

impl<const N: usize, T: Primitive> From<[[T; N]; N]> for Mat<N, T> {
    fn from(data: [[T; N]; N]) -> Self {
        Self::from_array(data)
    }
}

impl<const N: usize, T: Primitive> Index<(usize, usize)> for Mat<N, T> {
    type Output = T;

    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        &self.data[x][y]
    }
}

impl<const N: usize, T: Primitive> IndexMut<(usize, usize)> for Mat<N, T> {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        &mut self.data[x][y]
    }
}

impl<const N: usize, T: Primitive> Index<usize> for Mat<N, T> {
    type Output = [T; N];

    fn index(&self, i: usize) -> &Self::Output {
        &self.data[i]
    }
}

impl<const N: usize, T: Primitive> IndexMut<usize> for Mat<N, T> {
    fn index_mut(&mut self, i: usize) -> &mut Self::Output {
        &mut self.data[i]
    }
}

impl<const N: usize, T: Number> Mul<T> for Mat<N, T> {
    type Output = Mat<N, T>;

    fn mul(mut self, rhs: T) -> Self::Output {
        let mut i = 0;

        while i < N {
            let mut j = 0;

            while j < N {
                self.data[i][j] *= rhs;

                j += 1;
            }

            i += 1;
        }

        self
    }
}

unsafe impl<const N: usize, T: AllBitVariationsValid> AllBitVariationsValid for Mat<N, T> {}
unsafe impl<const N: usize, T: AllBitsInit> AllBitsInit for Mat<N, T> {}

struct SerializableMatColumn<'a, const N: usize, T> {
    data: &'a [T; N],
}

impl<'a, const N: usize, T: Serialize> Serialize for SerializableMatColumn<'a, N, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(N))?;

        for i in self.data {
            seq.serialize_element(i)?
        }

        seq.end()
    }
}

impl<const N: usize, T: Serialize> Serialize for Mat<N, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(N))?;

        for i in &self.data {
            let column = SerializableMatColumn { data: i };
            seq.serialize_element(&column)?
        }

        seq.end()
    }
}

#[repr(transparent)]
struct DeserializableMatColumn<const N: usize, T> {
    data: [T; N],
}

struct ArrayVisitor<const N: usize, T> {
    _phantom: PhantomData<fn([T; N]) -> [T; N]>,
}

impl<'de, const N: usize, T: Deserialize<'de>> Visitor<'de> for ArrayVisitor<N, T> {
    type Value = [T; N];

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("[T; N]")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut array = StackVec::<T, N>::new();

        for _ in 0..N {
            let Some(element) = seq.next_element::<T>()? else {
                // todo: return error instead of panic.
                panic!("failed to deserialize array");
            };

            array.push(element).ok().unwrap();
        }

        Ok(array.try_into_array().ok().unwrap())
    }
}

impl<'de, const N: usize, T: Deserialize<'de>> Deserialize<'de> for DeserializableMatColumn<N, T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(DeserializableMatColumn {
            data: deserializer.deserialize_seq(ArrayVisitor::<N, T> { _phantom: PhantomData })?
        })
    }
}

impl<'de, const N: usize, T: Deserialize<'de>> Deserialize<'de> for Mat<N, T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let columns_array = deserializer.deserialize_seq(ArrayVisitor::<N, DeserializableMatColumn<N, T>> { _phantom: PhantomData })?;

        let array = unsafe {
            let columns_array = ManuallyDrop::new(columns_array);

            let array_ptr = &raw const columns_array;
            let array_ptr = array_ptr as *const [[T; N]; N];

            array_ptr.read()
        };
   
        Ok(Mat::from_array(array))
    }
}
