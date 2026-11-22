use aphyn_math::*;

#[derive(Clone, Copy, Debug)]
pub enum WorleyMode {
    F1,
    F2MinusF1,
    CellValue,
    EdgeDistance,
}

// one jittered feature point per integer cell; outputs are remapped to [0, 1]
#[derive(Clone, Copy)]
pub struct Worley {
    seed: u32,
    mode: WorleyMode,
}

impl Worley {
    pub fn new(seed: u32, mode: WorleyMode) -> Self {
        Self { seed, mode }
    }

    pub fn sample(&self, p: Vec3<f32>) -> f32 {
        let floor = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
        let base = [floor.x as i32, floor.y as i32, floor.z as i32];
        let local = p - floor;

        let mut f1 = f32::MAX;
        let mut f2 = f32::MAX;
        let mut nearest_offset = [0; 3];
        let mut nearest = Vec3::splat(0.0);
        let mut first_pass = [Vec3::splat(0.0); 27];

        for (idx, &offset) in FIRST_PASS.iter().enumerate() {
            let to_feature = self.vector_to_feature(base, offset, local);
            first_pass[idx] = to_feature;
            let d = to_feature.length_sq();
            if d < f1 {
                f2 = f1;
                f1 = d;
                nearest_offset = offset;
                nearest = to_feature;
            } else if d < f2 {
                f2 = d;
            }
        }

        match self.mode {
            WorleyMode::F1 => remap(f1.sqrt(), 1.0),
            WorleyMode::F2MinusF1 => remap(f2.sqrt() - f1.sqrt(), 1.0),
            WorleyMode::CellValue => unit(hash(add(base, nearest_offset), self.seed, 3)),
            WorleyMode::EdgeDistance => {
                // A 5³ pass around the nearest feature finds every neighbouring cell border. A border with a feature b
                // is at least (|b| − f1) / 2 away, and |b| is at least the distance to b's cell, so cells that can't
                // beat the current best are skipped before hashing. The result is clamped at 0.5, so that's the start.
                let f1 = f1.sqrt();
                let mut edge = 0.5_f32;
                for &offset in &SECOND_PASS {
                    let cell = add(nearest_offset, offset);
                    let axis = |i: usize, local: f32| {
                        let lo = cell[i] as f32 - local;
                        lo.max(-(lo + 1.0)).max(0.0)
                    };
                    let cell_distance_sq = Vec3::new(axis(0, local.x), axis(1, local.y), axis(2, local.z)).length_sq();
                    // the margin covers rounding in the bound
                    let reach = f1 + 2.0 * edge + 1e-4;
                    if cell_distance_sq > reach * reach {
                        continue;
                    }

                    let to_feature = match cell.iter().all(|c| c.abs() <= 1) {
                        true => first_pass[first_pass_index(cell)],
                        false => self.vector_to_feature(base, cell, local),
                    };
                    let between = to_feature - nearest;
                    if between.length_sq() > 1e-8 {
                        edge = edge.min(((nearest + to_feature) * 0.5).dot(between.normalized()));
                    }
                }
                remap(edge, 0.5)
            }
        }
    }

    fn jitter(&self, cell: [i32; 3]) -> Vec3<f32> {
        Vec3::new(
            unit(hash(cell, self.seed, 0)),
            unit(hash(cell, self.seed, 1)),
            unit(hash(cell, self.seed, 2)),
        )
    }

    fn vector_to_feature(&self, base: [i32; 3], offset: [i32; 3], local: Vec3<f32>) -> Vec3<f32> {
        let offset_f = Vec3::new(offset[0] as f32, offset[1] as f32, offset[2] as f32);
        offset_f + self.jitter(add(base, offset)) - local
    }
}

// x fastest, so FIRST_PASS[first_pass_index(o)] == o
const FIRST_PASS: [[i32; 3]; 27] = offsets::<27>(1, 0);
// inner 3³ first, then the rest of the 5³, so the best edge shrinks early
const SECOND_PASS: [[i32; 3]; 125] = {
    let inner = offsets::<27>(1, 0);
    let outer = offsets::<98>(2, 2);
    let mut all = [[0; 3]; 125];
    let mut i = 0;
    while i < 125 {
        all[i] = if i < 27 { inner[i] } else { outer[i - 27] };
        i += 1;
    }
    all
};

// offsets within `radius` (Chebyshev) whose largest component is at least `min_ring`
const fn offsets<const N: usize>(radius: i32, min_ring: i32) -> [[i32; 3]; N] {
    let mut out = [[0; 3]; N];
    let mut n = 0;
    let mut z = -radius;
    while z <= radius {
        let mut y = -radius;
        while y <= radius {
            let mut x = -radius;
            while x <= radius {
                let ring = max(x.abs(), max(y.abs(), z.abs()));
                if ring >= min_ring {
                    out[n] = [x, y, z];
                    n += 1;
                }
                x += 1;
            }
            y += 1;
        }
        z += 1;
    }
    assert!(n == N);
    out
}

const fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

#[cfg(test)]
fn neighbours(radius: i32) -> impl Iterator<Item = [i32; 3]> {
    let range = move || -radius..=radius;
    range().flat_map(move |z| range().flat_map(move |y| range().map(move |x| [x, y, z])))
}

fn first_pass_index(offset: [i32; 3]) -> usize {
    ((offset[0] + 1) + 3 * (offset[1] + 1) + 9 * (offset[2] + 1)) as usize
}

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1]), a[2].wrapping_add(b[2])]
}

fn remap(value: f32, max: f32) -> f32 {
    (value / max).clamp(0.0, 1.0)
}

fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn hash(cell: [i32; 3], seed: u32, salt: u32) -> u32 {
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
mod tests {
    use super::*;

    const MODES: [WorleyMode; 4] = [
        WorleyMode::F1,
        WorleyMode::F2MinusF1,
        WorleyMode::CellValue,
        WorleyMode::EdgeDistance,
    ];

    fn points() -> impl Iterator<Item = Vec3<f32>> {
        (0..2000).map(|i| {
            let i = i as u32;
            Vec3::new(
                unit(hash([i as i32, 0, 0], 7, 0)) * 40.0 - 20.0,
                unit(hash([i as i32, 0, 0], 7, 1)) * 40.0 - 20.0,
                unit(hash([i as i32, 0, 0], 7, 2)) * 40.0 - 20.0,
            )
        })
    }

    #[test]
    fn outputs_in_range_and_deterministic() {
        for mode in MODES {
            let a = Worley::new(5, mode);
            let b = Worley::new(5, mode);
            for p in points() {
                let v = a.sample(p);
                assert!((-1.0..=1.0).contains(&v), "{mode:?} gave {v} at {p:?}");
                assert_eq!(v, b.sample(p));
            }
        }
    }

    #[test]
    fn f1_is_zero_at_feature_point() {
        let worley = Worley::new(11, WorleyMode::F1);
        let cell = [3, -2, 5];
        let feature = Vec3::new(3.0, -2.0, 5.0) + worley.jitter(cell);
        assert!(worley.sample(feature) < 0.001);
    }

    #[test]
    fn edge_distance_is_at_least_half_f2_minus_f1() {
        // remapped: edge maps 2·e and F2−F1 maps d through the same clamp, so e >= d/2 keeps the order
        let edge = Worley::new(11, WorleyMode::EdgeDistance);
        let diff = Worley::new(11, WorleyMode::F2MinusF1);
        for p in points() {
            assert!(edge.sample(p) >= diff.sample(p) - 1e-4, "at {p:?}");
        }
    }

    // the unpruned 5³ second pass
    fn edge_distance_brute_force(worley: &Worley, p: Vec3<f32>) -> f32 {
        let floor = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
        let base = [floor.x as i32, floor.y as i32, floor.z as i32];
        let local = p - floor;

        let mut f1 = f32::MAX;
        let mut nearest_offset = [0; 3];
        let mut nearest = Vec3::splat(0.0);
        for offset in neighbours(1) {
            let to_feature = worley.vector_to_feature(base, offset, local);
            let d = to_feature.length_sq();
            if d < f1 {
                f1 = d;
                nearest_offset = offset;
                nearest = to_feature;
            }
        }

        let mut edge = f32::MAX;
        for offset in neighbours(2) {
            let to_feature = worley.vector_to_feature(base, add(nearest_offset, offset), local);
            let between = to_feature - nearest;
            if between.length_sq() > 1e-8 {
                edge = edge.min(((nearest + to_feature) * 0.5).dot(between.normalized()));
            }
        }
        remap(edge, 0.5)
    }

    #[test]
    fn edge_distance_matches_brute_force() {
        for seed in [1, 3, 11] {
            let worley = Worley::new(seed, WorleyMode::EdgeDistance);
            for i in 0..50_000 {
                let h = |salt| unit(hash([i, seed as i32, 0], 13, salt)) * 16.0 - 8.0;
                let p = Vec3::new(h(0), h(1), h(2));
                assert_eq!(worley.sample(p), edge_distance_brute_force(&worley, p), "at {p:?}");
            }
        }
    }

    #[test]
    fn cell_value_is_constant_near_feature_point() {
        let worley = Worley::new(11, WorleyMode::CellValue);
        let feature = Vec3::new(3.0, -2.0, 5.0) + worley.jitter([3, -2, 5]);
        let v = worley.sample(feature);
        assert_eq!(v, worley.sample(feature + Vec3::splat(0.01)));
    }

    #[test]
    fn worley_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<Worley>();
    }
}
