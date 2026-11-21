use crate::{Mat4, Vec3, Vec4};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: Vec3<f32>,
    pub max: Vec3<f32>,
}

impl Aabb {
    pub const fn new(min: Vec3<f32>, max: Vec3<f32>) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: impl IntoIterator<Item = Vec3<f32>>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;

        Some(points.fold(Self::new(first, first), |aabb, point| Self {
            min: aabb.min.zip_copied(point, f32::min),
            max: aabb.max.zip_copied(point, f32::max),
        }))
    }

    pub fn center(&self) -> Vec3<f32> {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3<f32> {
        (self.max - self.min) * 0.5
    }

    // bounds of the transformed box, `matrix` must be affine
    pub fn transformed(&self, matrix: &Mat4<f32>) -> Self {
        let center = matrix.mul_with_projection(self.center());
        let extents = self.extents();
        let m = matrix.as_array();
        let extents = Vec3::from_fn(|row| (0..3).map(|col| m[col][row].abs() * extents.as_array()[col]).sum::<f32>());

        Self::new(center - extents, center + extents)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Frustum {
    pub planes: [Vec4<f32>; 6],
}

impl Frustum {
    // planes point inwards; clip depth is `[0, w]` in either depth direction
    pub fn from_world_to_clip(world_to_clip: &Mat4<f32>) -> Self {
        let m = world_to_clip.as_array();
        let row = |i: usize| Vec4::new(m[0][i], m[1][i], m[2][i], m[3][i]);
        let (x, y, z, w) = (row(0), row(1), row(2), row(3));

        Self {
            planes: [w + x, w - x, w + y, w - y, z, w - z],
        }
    }

    // conservative: may accept boxes slightly outside near the frustum's edges
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        self.planes.iter().all(|plane| {
            let farthest = Vec3::new(
                if plane.x >= 0.0 { aabb.max.x } else { aabb.min.x },
                if plane.y >= 0.0 { aabb.max.y } else { aabb.min.y },
                if plane.z >= 0.0 { aabb.max.z } else { aabb.min.z },
            );

            plane.x * farthest.x + plane.y * farthest.y + plane.z * farthest.z + plane.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mat3, orthographic_proj_matrix, perspective_proj_matrix};

    fn cube(center: Vec3<f32>, half: f32) -> Aabb {
        Aabb::new(center - Vec3::splat(half), center + Vec3::splat(half))
    }

    fn approx(a: Vec3<f32>, b: Vec3<f32>) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn from_points_bounds_all_points() {
        assert_eq!(Aabb::from_points([]), None);

        let aabb = Aabb::from_points([Vec3::new(1.0, -2.0, 3.0), Vec3::new(-1.0, 4.0, 0.0), Vec3::new(0.0, 0.0, 5.0)]).unwrap();

        assert_eq!(aabb, Aabb::new(Vec3::new(-1.0, -2.0, 0.0), Vec3::new(1.0, 4.0, 5.0)));
        assert_eq!(aabb.center(), Vec3::new(0.0, 1.0, 2.5));
        assert_eq!(aabb.extents(), Vec3::new(1.0, 3.0, 2.5));
    }

    #[test]
    fn transformed_bounds_the_transformed_corners() {
        let aabb = Aabb::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));

        let moved = aabb.transformed(&Mat4::offset(Vec3::new(10.0, 0.0, -5.0)));
        assert!(approx(moved.min, Vec3::new(9.0, -2.0, -8.0)) && approx(moved.max, Vec3::new(11.0, 2.0, -2.0)));

        let scaled = aabb.transformed(&Mat4::scale(Vec4::new(2.0, 0.5, -1.0, 1.0)));
        assert!(approx(scaled.min, Vec3::new(-2.0, -1.0, -3.0)) && approx(scaled.max, Vec3::new(2.0, 1.0, 3.0)));

        // 90 degrees around z swaps the x and y extents
        let rotation = Mat3::from_array([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
        let rotated = aabb.transformed(&rotation.into_4x4_with_offset(Vec3::new(0.0, 0.0, 1.0)));
        assert!(approx(rotated.min, Vec3::new(-2.0, -1.0, -2.0)) && approx(rotated.max, Vec3::new(2.0, 1.0, 4.0)));
    }

    #[test]
    fn perspective_frustum_culls_outside_boxes() {
        let frustum = Frustum::from_world_to_clip(&perspective_proj_matrix(std::f32::consts::FRAC_PI_2, 0.1, 100.0, 1.0));

        assert!(frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 10.0), 1.0)));
        assert!(
            frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 0.0), 1.0)),
            "straddles the near plane"
        );
        assert!(
            frustum.intersects_aabb(&cube(Vec3::new(10.5, 0.0, 10.0), 1.0)),
            "straddles the side"
        );
        assert!(
            frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 100.5), 1.0)),
            "straddles the far plane"
        );

        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, -5.0), 1.0)), "behind");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(15.0, 0.0, 10.0), 1.0)), "right");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, -15.0, 10.0), 1.0)), "below");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 105.0), 1.0)), "beyond far");
    }

    #[test]
    fn orthographic_frustum_culls_outside_boxes() {
        let view = Mat4::offset(Vec3::new(0.0, 0.0, 5.0));
        let frustum = Frustum::from_world_to_clip(&(orthographic_proj_matrix(10.0, 0.0, 20.0, 2.0) * view));

        assert!(frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 0.0), 1.0)));
        assert!(frustum.intersects_aabb(&cube(Vec3::new(9.5, 4.5, 10.0), 1.0)));
        assert!(
            frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, -5.5), 1.0)),
            "straddles the near plane"
        );

        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, -7.0), 1.0)), "behind");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(11.5, 0.0, 0.0), 1.0)), "right");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, 6.5, 0.0), 1.0)), "above");
        assert!(!frustum.intersects_aabb(&cube(Vec3::new(0.0, 0.0, 17.0), 1.0)), "beyond far");
    }
}
