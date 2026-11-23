use aphyn_math::*;

use super::{FirstPass, Space, hash, unit, worley_noise};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestPoint<V> {
    pub distance_sq: f32,
    pub point: V,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closest2PointsDiff<V> {
    pub diff: f32,
    pub points: [ClosestPoint<V>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellValue<V> {
    pub distance_sq: f32,
    pub point: V,
    pub value: f32,
}

worley_noise!(WorleyClosestPoint2D, Vec2<f32>, 2 => ClosestPoint<Vec2<f32>>, closest_point);
worley_noise!(WorleyClosestPoint3D, Vec3<f32>, 3 => ClosestPoint<Vec3<f32>>, closest_point);
worley_noise!(WorleyClosest2Points2D, Vec2<f32>, 2 => [ClosestPoint<Vec2<f32>>; 2], closest_2_points);
worley_noise!(WorleyClosest2Points3D, Vec3<f32>, 3 => [ClosestPoint<Vec3<f32>>; 2], closest_2_points);
worley_noise!(WorleyClosest2PointsDiff2D, Vec2<f32>, 2 => Closest2PointsDiff<Vec2<f32>>, closest_2_points_diff);
worley_noise!(WorleyClosest2PointsDiff3D, Vec3<f32>, 3 => Closest2PointsDiff<Vec3<f32>>, closest_2_points_diff);
worley_noise!(WorleyCellValue2D, Vec2<f32>, 2 => CellValue<Vec2<f32>>, cell_value);
worley_noise!(WorleyCellValue3D, Vec3<f32>, 3 => CellValue<Vec3<f32>>, cell_value);

fn closest_point<V: Space<N>, const N: usize>(seed: u32, p: V) -> ClosestPoint<V> {
    FirstPass::new(seed, p).closest(seed, 0)
}

fn closest_2_points<V: Space<N>, const N: usize>(seed: u32, p: V) -> [ClosestPoint<V>; 2] {
    let pass = FirstPass::new(seed, p);
    [pass.closest(seed, 0), pass.closest(seed, 1)]
}

fn closest_2_points_diff<V: Space<N>, const N: usize>(seed: u32, p: V) -> Closest2PointsDiff<V> {
    let points = closest_2_points(seed, p);
    Closest2PointsDiff {
        diff: points[1].distance_sq.sqrt() - points[0].distance_sq.sqrt(),
        points,
    }
}

fn cell_value<V: Space<N>, const N: usize>(seed: u32, p: V) -> CellValue<V> {
    let pass = FirstPass::new(seed, p);
    let ClosestPoint { distance_sq, point } = pass.closest(seed, 0);
    // salts 0..N are the jitter axes
    let value = unit(hash(pass.cell::<V>(pass.nearest), seed, N as u32));
    CellValue {
        distance_sq,
        point,
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::super::{add, feature_point, length_sq, sub};
    use super::*;

    fn brute_force_nearest<V: Space<N>, const N: usize>(seed: u32, p: [f32; N]) -> [f32; N] {
        let base = p.map(|c| c.floor() as i32);
        neighbours::<N>(1)
            .map(|o| feature_point(seed, add(base, o)))
            .min_by(|a, b| length_sq(sub(*a, p)).total_cmp(&length_sq(sub(*b, p))))
            .unwrap()
    }

    #[test]
    fn deterministic_and_seeded() {
        let mut differs = false;
        for p in points::<3>(500, 20.0) {
            let p = Vec3::from_array(p);
            let a = WorleyCellValue3D::new(5).sample(p);
            assert_eq!(a, WorleyCellValue3D::new(5).sample(p));
            differs |= a != WorleyCellValue3D::new(6).sample(p);
            assert!(a.distance_sq.is_finite());
        }
        assert!(differs);
    }

    #[test]
    fn closest_point_matches_brute_force() {
        for p in points::<2>(3000, 20.0) {
            let c = WorleyClosestPoint2D::new(3).sample(Vec2::from_array(p));
            assert_eq!(c.point.into_array(), brute_force_nearest::<Vec2<f32>, 2>(3, p), "at {p:?}");
            assert!((c.distance_sq - length_sq(sub(c.point.into_array(), p))).abs() < 1e-4);
        }
        for p in points::<3>(3000, 20.0) {
            let c = WorleyClosestPoint3D::new(3).sample(Vec3::from_array(p));
            assert_eq!(c.point.into_array(), brute_force_nearest::<Vec3<f32>, 3>(3, p), "at {p:?}");
            assert!((c.distance_sq - length_sq(sub(c.point.into_array(), p))).abs() < 1e-4);
        }
    }

    #[test]
    fn closest_point_is_zero_at_feature_point() {
        let feature = Vec3::from_array(feature_point(11, [3, -2, 5]));
        let c = WorleyClosestPoint3D::new(11).sample(feature);
        assert!(c.distance_sq < 1e-6);
        assert_eq!(c.point, feature);

        let feature = Vec2::from_array(feature_point(11, [-7, 4]));
        assert!(WorleyClosestPoint2D::new(11).sample(feature).distance_sq < 1e-6);
    }

    #[test]
    fn closest_2_points_sorted_and_diff_consistent() {
        for p in points::<2>(2000, 20.0) {
            let d = WorleyClosest2PointsDiff2D::new(9).sample(Vec2::from_array(p));
            assert_eq!(d.points, WorleyClosest2Points2D::new(9).sample(Vec2::from_array(p)));
            assert!(d.points[0].distance_sq <= d.points[1].distance_sq);
            assert_ne!(d.points[0].point, d.points[1].point);
            assert!(d.diff >= 0.0);
        }
        for p in points::<3>(2000, 20.0) {
            let d = WorleyClosest2PointsDiff3D::new(9).sample(Vec3::from_array(p));
            assert!(d.points[0].distance_sq <= d.points[1].distance_sq);
            assert_eq!(d.diff, d.points[1].distance_sq.sqrt() - d.points[0].distance_sq.sqrt());
        }
    }

    #[test]
    fn cell_value_is_constant_near_feature_point_and_in_range() {
        let worley = WorleyCellValue3D::new(11);
        let feature = Vec3::from_array(feature_point(11, [3, -2, 5]));
        let v = worley.sample(feature).value;
        assert_eq!(v, worley.sample(feature + Vec3::splat(0.01)).value);

        let worley = WorleyCellValue2D::new(11);
        for p in points::<2>(2000, 20.0) {
            let c = worley.sample(Vec2::from_array(p));
            assert!((0.0..1.0).contains(&c.value));
            assert_eq!(c.point, WorleyClosestPoint2D::new(11).sample(Vec2::from_array(p)).point);
        }
    }

    #[test]
    fn noises_are_send_and_sync() {
        fn check<T: Send + Sync + Copy>() {}
        check::<WorleyClosestPoint2D>();
        check::<WorleyClosest2Points3D>();
        check::<WorleyClosest2PointsDiff3D>();
        check::<WorleyCellValue3D>();
    }
}
