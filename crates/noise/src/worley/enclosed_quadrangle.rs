use aphyn_math::*;

use super::{FIRST_PASS_2D, add, first_pass_index, jitter};

// corners x fastest: [c00, c10, c01, c11]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnclosedQuadrangle {
    pub points: [Vec2<f32>; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct WorleyEnclosedQuadrangle2D {
    seed: u32,
}

impl WorleyEnclosedQuadrangle2D {
    pub fn new(seed: u32) -> Self {
        Self { seed }
    }

    pub fn sample(&self, p: Vec2<f32>) -> EnclosedQuadrangle {
        let floor = Vec2::new(p.x.floor(), p.y.floor());
        let base = [floor.x as i32, floor.y as i32];
        let local = p - floor;
        // features of the 3x3 cells around the sample, relative to `floor`
        let features =
            FIRST_PASS_2D.map(|o| Vec2::new(o[0] as f32, o[1] as f32) + Vec2::from_array(jitter(self.seed, add(base, o))));

        let mut best = (f32::MIN, [0; 4], [0.0; 4]);
        // quad `a` spans x in [a, a + 2), so only quads with a min corner in {base − 1, base}² can hold the sample
        'search: for qy in -1..=0 {
            for qx in -1..=0 {
                let corners = [[0, 0], [1, 0], [0, 1], [1, 1]].map(|[x, y]| first_pass_index([qx + x, qy + y]));
                let q = corners.map(|i| features[i]);
                for triangle in triangles(q) {
                    let w = barycentric(triangle.map(|i| q[i]), local);
                    let min_w = w[0].min(w[1]).min(w[2]);
                    if min_w > best.0 {
                        let mut weights = [0.0; 4];
                        for (k, &i) in triangle.iter().enumerate() {
                            weights[i] = w[k];
                        }
                        best = (min_w, corners, weights);
                        if min_w >= 0.0 {
                            break 'search;
                        }
                    }
                }
            }
        }

        let (_, corners, weights) = best;
        EnclosedQuadrangle {
            points: corners.map(|i| floor + features[i]),
            weights,
        }
    }
}

// Quads tile the plane and are simple but may be non-convex, so they are split along an interior diagonal: c00–c11 when
// both its triangles keep the quad's counter-clockwise winding, c10–c01 otherwise.
fn triangles(q: [Vec2<f32>; 4]) -> [[usize; 3]; 2] {
    match orient(q[0], q[1], q[3]) > 0.0 && orient(q[0], q[3], q[2]) > 0.0 {
        true => [[0, 1, 3], [0, 3, 2]],
        false => [[0, 1, 2], [1, 3, 2]],
    }
}

fn orient(a: Vec2<f32>, b: Vec2<f32>, c: Vec2<f32>) -> f32 {
    let (ab, ac) = (b - a, c - a);
    ab.x * ac.y - ab.y * ac.x
}

fn barycentric([a, b, c]: [Vec2<f32>; 3], p: Vec2<f32>) -> [f32; 3] {
    let area = orient(a, b, c);
    let wa = orient(p, b, c) / area;
    let wb = orient(a, p, c) / area;
    [wa, wb, 1.0 - wa - wb]
}

#[cfg(test)]
mod tests {
    use super::super::feature_point;
    use super::super::test_utils::*;
    use super::*;

    #[test]
    fn weights_reconstruct_sample() {
        let worley = WorleyEnclosedQuadrangle2D::new(4);
        for p in points::<2>(20_000, 30.0) {
            let p = Vec2::from_array(p);
            let q = worley.sample(p);
            assert!(q.weights.iter().all(|&w| w >= -1e-5), "{q:?} at {p:?}");
            assert!((q.weights.iter().sum::<f32>() - 1.0).abs() < 1e-5);
            let rebuilt = (0..4).fold(Vec2::splat(0.0), |acc, i| acc + q.points[i] * q.weights[i]);
            assert!((rebuilt - p).length_sq() < 1e-8, "{rebuilt:?} vs {p:?}");
            assert_eq!(q, worley.sample(p));
        }
    }

    #[test]
    fn points_are_features_of_a_2x2_block() {
        let worley = WorleyEnclosedQuadrangle2D::new(8);
        for p in points::<2>(5_000, 30.0) {
            let q = worley.sample(Vec2::from_array(p));
            let c00 = q.points[0].map(|c| c.floor() as i32).into_array();
            for (i, o) in [[0, 0], [1, 0], [0, 1], [1, 1]].into_iter().enumerate() {
                let expected = Vec2::from_array(feature_point(8, add(c00, o)));
                assert!((q.points[i] - expected).length_sq() < 1e-8);
            }
        }
    }

    #[test]
    fn triangles_tile_the_plane() {
        // exactly one candidate triangle holds each sample away from edges
        let seed = 21;
        for p in points::<2>(20_000, 30.0) {
            let p = Vec2::from_array(p);
            let base = p.map(|c| c.floor() as i32).into_array();
            let mut inside = 0;
            for qy in -2..=1 {
                for qx in -2..=1 {
                    let q = [[0, 0], [1, 0], [0, 1], [1, 1]]
                        .map(|[x, y]| Vec2::from_array(feature_point(seed, add(base, [qx + x, qy + y]))));
                    for t in triangles(q) {
                        let w = barycentric(t.map(|i| q[i]), p);
                        inside += (w.iter().all(|&w| w > 1e-4)) as u32;
                    }
                }
            }
            assert!(inside <= 1, "{inside} triangles hold {p:?}");
        }
    }

    #[test]
    fn is_send_and_sync() {
        fn check<T: Send + Sync + Copy>() {}
        check::<WorleyEnclosedQuadrangle2D>();
    }
}
