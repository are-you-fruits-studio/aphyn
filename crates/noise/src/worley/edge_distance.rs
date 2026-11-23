use aphyn_math::*;

use super::{FirstPass, Space, add, dot, feature_point, first_pass_index, length_sq, sub, vector_to_feature, worley_noise};

pub(crate) const MAX_EDGE_DISTANCE: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeDistance<V> {
    pub distance: f32,
    pub points: [V; 2],
}

worley_noise!(WorleyEdgeDistance2D, Vec2<f32>, 2 => EdgeDistance<Vec2<f32>>, edge_distance);
worley_noise!(WorleyEdgeDistance3D, Vec3<f32>, 3 => EdgeDistance<Vec3<f32>>, edge_distance);

fn edge_distance<V: Space<N>, const N: usize>(seed: u32, p: V) -> EdgeDistance<V> {
    // A 5^N pass around the nearest feature finds every neighbouring cell border. A border with a feature b is at least
    // (|b| − f1) / 2 away, and |b| is at least the distance to b's cell, so cells that can't beat the current bound are
    // skipped before hashing. The result is capped, so the cap is the starting bound.
    let pass = FirstPass::new(seed, p);
    let nearest_offset = V::FIRST_PASS[pass.nearest];
    let nearest = pass.to_feature[pass.nearest];
    let f1 = pass.distance_sq[0].sqrt();

    let mut best = f32::MAX;
    let mut neighbour_offset = nearest_offset;
    for &offset in V::SECOND_PASS {
        let cell = add(nearest_offset, offset);
        let cell_distance_sq: f32 = (0..N)
            .map(|i| {
                let lo = cell[i] as f32 - pass.local[i];
                let d = lo.max(-(lo + 1.0)).max(0.0);
                d * d
            })
            .sum();
        // the margin covers rounding in the bound
        let reach = f1 + 2.0 * best.min(MAX_EDGE_DISTANCE) + 1e-4;
        if cell_distance_sq > reach * reach {
            continue;
        }

        let to_feature = match cell.iter().all(|c| c.abs() <= 1) {
            true => pass.to_feature[first_pass_index(cell)],
            false => vector_to_feature(seed, pass.base, cell, pass.local),
        };
        if let Some(d) = bisector_distance(nearest, to_feature)
            && d < best
        {
            best = d;
            neighbour_offset = cell;
        }
    }

    EdgeDistance {
        distance: best.min(MAX_EDGE_DISTANCE),
        points: [nearest_offset, neighbour_offset].map(|o| V::from_coords(feature_point(seed, add(pass.base, o)))),
    }
}

// distance from the sample (origin) to the bisector between two features given relative to it
fn bisector_distance<const N: usize>(nearest: [f32; N], other: [f32; N]) -> Option<f32> {
    let between = sub(other, nearest);
    let len_sq = length_sq(between);
    (len_sq > 1e-8).then(|| {
        let mid: [f32; N] = std::array::from_fn(|i| (nearest[i] + other[i]) * 0.5);
        dot(mid, between) / len_sq.sqrt()
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::super::{WorleyClosest2PointsDiff2D, WorleyClosest2PointsDiff3D, hash, unit};
    use super::*;

    // the unpruned 5^N second pass
    fn brute_force<V: Space<N>, const N: usize>(seed: u32, p: V) -> (f32, V) {
        let pass = FirstPass::new(seed, p);
        let nearest_offset = V::FIRST_PASS[pass.nearest];
        let nearest = pass.to_feature[pass.nearest];
        let edge = neighbours::<N>(2)
            .filter_map(|o| {
                bisector_distance(
                    nearest,
                    vector_to_feature(seed, pass.base, add(nearest_offset, o), pass.local),
                )
            })
            .fold(f32::MAX, f32::min);
        (
            edge.min(MAX_EDGE_DISTANCE),
            V::from_coords(feature_point(seed, add(pass.base, nearest_offset))),
        )
    }

    fn check<V: Space<N> + PartialEq + std::fmt::Debug, const N: usize>(sample: impl Fn(u32, V) -> EdgeDistance<V>) {
        for seed in [1, 3, 11] {
            for i in 0..30_000 {
                let p = V::from_coords(std::array::from_fn(|axis| {
                    unit(hash([i, seed as i32], 13, axis as u32)) * 16.0 - 8.0
                }));
                let e = sample(seed, p);
                let (distance, nearest) = brute_force(seed, p);
                assert_eq!(e.distance, distance, "at {p:?}");
                assert_eq!(e.points[0], nearest, "at {p:?}");
                if e.distance < MAX_EDGE_DISTANCE {
                    let [a, b] = e.points.map(|q| sub(q.coords(), p.coords()));
                    assert!((bisector_distance(a, b).unwrap() - e.distance).abs() < 1e-3, "at {p:?}");
                }
            }
        }
    }

    #[test]
    fn matches_brute_force() {
        check::<Vec2<f32>, 2>(|seed, p| WorleyEdgeDistance2D::new(seed).sample(p));
        check::<Vec3<f32>, 3>(|seed, p| WorleyEdgeDistance3D::new(seed).sample(p));
    }

    #[test]
    fn is_at_least_half_closest_2_points_diff() {
        for p in points::<2>(2000, 20.0) {
            let p = Vec2::from_array(p);
            let edge = WorleyEdgeDistance2D::new(11).sample(p).distance;
            let diff = WorleyClosest2PointsDiff2D::new(11).sample(p).diff;
            assert!(edge >= (diff * 0.5).min(MAX_EDGE_DISTANCE) - 1e-4, "at {p:?}");
        }
        for p in points::<3>(2000, 20.0) {
            let p = Vec3::from_array(p);
            let edge = WorleyEdgeDistance3D::new(11).sample(p).distance;
            let diff = WorleyClosest2PointsDiff3D::new(11).sample(p).diff;
            assert!(edge >= (diff * 0.5).min(MAX_EDGE_DISTANCE) - 1e-4, "at {p:?}");
        }
    }
}
