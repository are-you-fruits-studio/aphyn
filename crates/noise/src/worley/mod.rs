use aphyn_math::*;

mod closest;
mod edge_distance;
mod enclosed_quadrangle;

pub use closest::*;
pub use edge_distance::*;
pub use enclosed_quadrangle::*;

macro_rules! worley_noise {
    ($Name:ident, $V:ty, $N:literal => $Out:ty, $sample:ident) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $Name {
            seed: u32,
        }

        impl $Name {
            pub fn new(seed: u32) -> Self {
                Self { seed }
            }

            pub fn sample(&self, p: $V) -> $Out {
                $sample::<_, $N>(self.seed, p)
            }
        }
    };
}
pub(crate) use worley_noise;

pub(crate) trait Space<const N: usize>: Copy {
    // x fastest, so FIRST_PASS[first_pass_index(o)] == o
    const FIRST_PASS: &'static [[i32; N]];
    // inner 3^N first, then the rest of the 5^N, so the best edge shrinks early
    const SECOND_PASS: &'static [[i32; N]];

    fn from_coords(coords: [f32; N]) -> Self;
    fn coords(self) -> [f32; N];
}

impl Space<2> for Vec2<f32> {
    const FIRST_PASS: &'static [[i32; 2]] = &FIRST_PASS_2D;
    const SECOND_PASS: &'static [[i32; 2]] = &SECOND_PASS_2D;

    fn from_coords(coords: [f32; 2]) -> Self {
        Self::from_array(coords)
    }

    fn coords(self) -> [f32; 2] {
        self.into_array()
    }
}

impl Space<3> for Vec3<f32> {
    const FIRST_PASS: &'static [[i32; 3]] = &FIRST_PASS_3D;
    const SECOND_PASS: &'static [[i32; 3]] = &SECOND_PASS_3D;

    fn from_coords(coords: [f32; 3]) -> Self {
        Self::from_array(coords)
    }

    fn coords(self) -> [f32; 3] {
        self.into_array()
    }
}

pub(crate) const FIRST_PASS_2D: [[i32; 2]; 9] = offsets(1, 0);
const FIRST_PASS_3D: [[i32; 3]; 27] = offsets(1, 0);
const SECOND_PASS_2D: [[i32; 2]; 25] = concat(offsets::<2, 9>(1, 0), offsets::<2, 16>(2, 2));
const SECOND_PASS_3D: [[i32; 3]; 125] = concat(offsets::<3, 27>(1, 0), offsets::<3, 98>(2, 2));

pub(crate) struct FirstPass<const N: usize> {
    pub base: [i32; N],
    pub local: [f32; N],
    // vectors from the sample to the features of FIRST_PASS cells; 27 fits both dimensions
    pub to_feature: [[f32; N]; 27],
    pub nearest: usize,
    pub second: usize,
    pub distance_sq: [f32; 2],
}

impl<const N: usize> FirstPass<N> {
    pub fn new<V: Space<N>>(seed: u32, p: V) -> Self {
        let p = p.coords();
        let floor = p.map(f32::floor);
        let base = floor.map(|f| f as i32);
        let local = sub(p, floor);

        let mut pass = Self {
            base,
            local,
            to_feature: [[0.0; N]; 27],
            nearest: 0,
            second: 0,
            distance_sq: [f32::MAX; 2],
        };
        for (idx, &offset) in V::FIRST_PASS.iter().enumerate() {
            let to_feature = vector_to_feature(seed, base, offset, local);
            pass.to_feature[idx] = to_feature;
            let d = length_sq(to_feature);
            if d < pass.distance_sq[0] {
                pass.second = pass.nearest;
                pass.distance_sq[1] = pass.distance_sq[0];
                pass.nearest = idx;
                pass.distance_sq[0] = d;
            } else if d < pass.distance_sq[1] {
                pass.second = idx;
                pass.distance_sq[1] = d;
            }
        }
        pass
    }

    pub fn cell<V: Space<N>>(&self, idx: usize) -> [i32; N] {
        add(self.base, V::FIRST_PASS[idx])
    }

    // rank 0 is the nearest feature, rank 1 the second nearest
    pub fn closest<V: Space<N>>(&self, seed: u32, rank: usize) -> ClosestPoint<V> {
        let idx = [self.nearest, self.second][rank];
        ClosestPoint {
            distance_sq: self.distance_sq[rank],
            point: V::from_coords(feature_point(seed, self.cell::<V>(idx))),
        }
    }
}

pub(crate) fn jitter<const N: usize>(seed: u32, cell: [i32; N]) -> [f32; N] {
    std::array::from_fn(|i| unit(hash(cell, seed, i as u32)))
}

pub(crate) fn feature_point<const N: usize>(seed: u32, cell: [i32; N]) -> [f32; N] {
    let jitter = jitter(seed, cell);
    std::array::from_fn(|i| cell[i] as f32 + jitter[i])
}

pub(crate) fn vector_to_feature<const N: usize>(seed: u32, base: [i32; N], offset: [i32; N], local: [f32; N]) -> [f32; N] {
    let jitter = jitter(seed, add(base, offset));
    std::array::from_fn(|i| offset[i] as f32 + jitter[i] - local[i])
}

pub(crate) fn first_pass_index<const N: usize>(offset: [i32; N]) -> usize {
    let mut idx = 0;
    let mut stride = 1;
    for c in offset {
        idx += (c + 1) as usize * stride;
        stride *= 3;
    }
    idx
}

// offsets within `radius` (Chebyshev) whose largest component is at least `min_ring`, x fastest
const fn offsets<const N: usize, const M: usize>(radius: i32, min_ring: i32) -> [[i32; N]; M] {
    let side = 2 * radius + 1;
    let mut total = 1;
    let mut i = 0;
    while i < N {
        total *= side;
        i += 1;
    }

    let mut out = [[0; N]; M];
    let mut n = 0;
    let mut idx = 0;
    while idx < total {
        let mut offset = [0; N];
        let mut ring = 0;
        let mut rest = idx;
        let mut axis = 0;
        while axis < N {
            offset[axis] = rest % side - radius;
            ring = max(ring, offset[axis].abs());
            rest /= side;
            axis += 1;
        }
        if ring >= min_ring {
            out[n] = offset;
            n += 1;
        }
        idx += 1;
    }
    assert!(n == M);
    out
}

const fn concat<const N: usize, const A: usize, const B: usize, const C: usize>(
    a: [[i32; N]; A],
    b: [[i32; N]; B],
) -> [[i32; N]; C] {
    assert!(A + B == C);
    let mut out = [[0; N]; C];
    let mut i = 0;
    while i < C {
        out[i] = if i < A { a[i] } else { b[i - A] };
        i += 1;
    }
    out
}

const fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

pub(crate) fn add<const N: usize>(a: [i32; N], b: [i32; N]) -> [i32; N] {
    std::array::from_fn(|i| a[i].wrapping_add(b[i]))
}

pub(crate) fn sub<const N: usize>(a: [f32; N], b: [f32; N]) -> [f32; N] {
    std::array::from_fn(|i| a[i] - b[i])
}

pub(crate) fn dot<const N: usize>(a: [f32; N], b: [f32; N]) -> f32 {
    (0..N).map(|i| a[i] * b[i]).sum()
}

pub(crate) fn length_sq<const N: usize>(v: [f32; N]) -> f32 {
    dot(v, v)
}

pub(crate) fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

pub(crate) fn hash<const N: usize>(cell: [i32; N], seed: u32, salt: u32) -> u32 {
    let mut h = seed.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    for c in cell {
        h = (h ^ c as u32).wrapping_mul(0x27D4_EB2F);
        h ^= h >> 15;
    }
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^ (h >> 16)
}

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;

    pub fn points<const N: usize>(count: i32, extent: f32) -> impl Iterator<Item = [f32; N]> {
        (0..count).map(move |i| std::array::from_fn(|axis| unit(hash([i, N as i32], 7, axis as u32)) * 2.0 * extent - extent))
    }

    pub fn neighbours<const N: usize>(radius: i32) -> impl Iterator<Item = [i32; N]> {
        let side = 2 * radius + 1;
        (0..side.pow(N as u32)).map(move |idx| {
            let mut rest = idx;
            std::array::from_fn(|_| {
                let c = rest % side - radius;
                rest /= side;
                c
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;
    use super::*;

    #[test]
    fn first_pass_tables_are_x_fastest() {
        for (i, &o) in FIRST_PASS_2D.iter().enumerate() {
            assert_eq!(first_pass_index(o), i);
        }
        for (i, &o) in FIRST_PASS_3D.iter().enumerate() {
            assert_eq!(first_pass_index(o), i);
        }
        assert_eq!(&SECOND_PASS_2D[..9], &FIRST_PASS_2D[..]);
        assert_eq!(&SECOND_PASS_3D[..27], &FIRST_PASS_3D[..]);
        assert_eq!(neighbours::<3>(1).collect::<Vec<_>>(), FIRST_PASS_3D.to_vec());
    }

    #[test]
    fn first_pass_finds_two_nearest() {
        fn check<V: Space<N>, const N: usize>(p: [f32; N]) {
            let pass = FirstPass::new(5, V::from_coords(p));
            let mut all: Vec<f32> = (0..V::FIRST_PASS.len()).map(|i| length_sq(pass.to_feature[i])).collect();
            all.sort_by(f32::total_cmp);
            assert_eq!(pass.distance_sq, [all[0], all[1]]);
            assert_ne!(pass.nearest, pass.second);
        }
        for p in points::<2>(2000, 20.0) {
            check::<Vec2<f32>, 2>(p);
        }
        for p in points::<3>(2000, 20.0) {
            check::<Vec3<f32>, 3>(p);
        }
    }
}
