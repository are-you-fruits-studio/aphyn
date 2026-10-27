
// todo: ffi
/// N is the amount of bytes
pub struct BitArray<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> BitArray<N> {
    pub const fn new() -> Self {
        Self { data: [0; N] }
    }

    pub const fn splat(v: bool) -> Self {
        let v = if v { !0 } else { 0 };

        Self { data: [v; N] }
    }

    pub const fn len(&self) -> usize {
        N * 8
    }

    pub const fn get(&self, index: usize) -> bool {
        let array_index = index / 8;
        let bit_index = index % 8;

        get_bit(self.data[array_index], bit_index)
    }

    pub const fn set(&mut self, index: usize, v: bool) {
        let array_index = index / 8;
        let bit_index = index % 8;

        set_bit(&mut self.data[array_index], bit_index, v);
    }

    pub const fn count_zeros(&self) -> usize {
        let mut sum = 0;

        let mut i = 0;
        while i < N {
            sum += self.data[i].count_zeros() as usize;

            i += 1;
        }

        sum
    }

    pub const fn count_ones(&self) -> usize {
        let mut sum = 0;

        let mut i = 0;
        while i < N {
            sum += self.data[i].count_ones() as usize;

            i += 1;
        }

        sum
    }

    pub const fn into_bytes(self) -> [u8; N] {
        self.data
    }

    pub const fn as_bytes(&self) -> &[u8; N] {
        &self.data
    }

    pub const fn as_bytes_mut(&mut self) -> &mut [u8; N] {
        &mut self.data
    }
}

impl<const N: usize> Default for BitArray<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, const N: usize> IntoIterator for &'a BitArray<N> {
    type Item = <BitArrayIter<'a, N> as Iterator>::Item;

    type IntoIter = BitArrayIter<'a, N>;

    fn into_iter(self) -> Self::IntoIter {
        BitArrayIter::<'a, N>::new(self)
    }
}

impl<const N: usize> IntoIterator for BitArray<N> {
    type Item = <BitArrayIntoIter<N> as Iterator>::Item;

    type IntoIter = BitArrayIntoIter<N>;

    fn into_iter(self) -> Self::IntoIter {
        BitArrayIntoIter::<N>::new(self)
    }
}

pub struct BitArrayIter<'a, const N: usize> {
    array: &'a BitArray<N>,
    i: usize,
}

impl<'a, const N: usize> BitArrayIter<'a, N> {
    pub const fn new(array: &'a BitArray<N>) -> Self {
        Self { array, i: 0 }
    }
}

impl<'a, const N: usize> Iterator for BitArrayIter<'a, N> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.array.len() {
            return None;
        }

        let result = self.array.get(self.i);
        self.i += 1;

        Some(result)
    }
}

pub struct BitArrayIntoIter<const N: usize> {
    array: BitArray<N>,
    i: usize,
}

impl<const N: usize> BitArrayIntoIter<N> {
    pub const fn new(array: BitArray<N>) -> Self {
        Self { array, i: 0 }
    }
}

impl<const N: usize> Iterator for BitArrayIntoIter<N> {
    type Item = bool;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.array.len() {
            return None;
        }

        let result = self.array.get(self.i);
        self.i += 1;

        Some(result)
    }
}

//

// todo: BitVec

//

const fn get_bit(src: u8, bit: usize) -> bool {
    src & (1 << bit) != 0
}

const fn set_bit(src: &mut u8, bit: usize, v: bool) {
    if v {
        *src |= 1 << bit;
    } else {
        *src &= !(1 << bit);
    }
}
