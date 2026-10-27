use std::fmt::Debug;

use aphyn_math::{Mat4, Quat, Vec3};
use aphyn_serialization::*;

use crate::LineBoundType;

#[repr(C, u8)]
#[derive(Copy, Clone, PartialEq, Debug, Serializable)]
pub enum CollisionShape {
    Point(Vec3<f32>),
    Line(CollisionLine),
    Aabb(CollisionAabb),
    Box(CollisionBox),
    Sphere(CollisionSphere),
    Triangle([Vec3<f32>; 3]),
}
impl Default for CollisionShape {
    fn default() -> Self {
        Self::Aabb(CollisionAabb::default())
    }
}

impl From<Vec3<f32>> for CollisionShape {
    fn from(value: Vec3<f32>) -> Self {
        Self::Point(value)
    }
}
impl From<CollisionLine> for CollisionShape {
    fn from(value: CollisionLine) -> Self {
        Self::Line(value)
    }
}
impl From<CollisionAabb> for CollisionShape {
    fn from(value: CollisionAabb) -> Self {
        Self::Aabb(value)
    }
}
impl From<CollisionBox> for CollisionShape {
    fn from(value: CollisionBox) -> Self {
        Self::Box(value)
    }
}
impl From<CollisionSphere> for CollisionShape {
    fn from(value: CollisionSphere) -> Self {
        Self::Sphere(value)
    }
}
impl From<[Vec3<f32>; 3]> for CollisionShape {
    fn from(value: [Vec3<f32>; 3]) -> Self {
        Self::Triangle(value)
    }
}

impl CollisionLine {
    pub const fn into_shape(self) -> CollisionShape {
        CollisionShape::Line(self)
    }
}
impl CollisionAabb {
    pub const fn into_shape(self) -> CollisionShape {
        CollisionShape::Aabb(self)
    }
}
impl CollisionBox {
    pub const fn into_shape(self) -> CollisionShape {
        CollisionShape::Box(self)
    }
}
impl CollisionSphere {
    pub const fn into_shape(self) -> CollisionShape {
        CollisionShape::Sphere(self)
    }
}

impl CollisionShape {
    pub fn to_aabb(&self) -> CollisionAabb {
        match self {
            CollisionShape::Point(collision_point) => CollisionAabb {
                center: *collision_point,
                extents: Vec3::splat(0.0),
            },
            CollisionShape::Line(collision_line) => {
                if collision_line.bounds != LineBoundType::SEGMENT {
                    panic!("Trying to create infinite AABB.");
                }

                CollisionAabb::from_points([collision_line.start, collision_line.end].into_iter())
            }
            CollisionShape::Aabb(collision_aabb) => *collision_aabb,
            CollisionShape::Box(collision_box) => {
                let ext = collision_box.extents;

                let points = [
                    ext * Vec3::new(-1.0, -1.0, -1.0),
                    ext * Vec3::new(-1.0, -1.0, 1.0),
                    ext * Vec3::new(-1.0, 1.0, -1.0),
                    ext * Vec3::new(-1.0, 1.0, 1.0),
                    ext * Vec3::new(1.0, -1.0, -1.0),
                    ext * Vec3::new(1.0, -1.0, 1.0),
                    ext * Vec3::new(1.0, 1.0, -1.0),
                    ext * Vec3::new(1.0, 1.0, 1.0),
                ];

                CollisionAabb::from_points(
                    points
                        .iter()
                        .map(|p| collision_box.center + collision_box.rotation.to_matrix() * *p),
                )
            }
            CollisionShape::Sphere(collision_sphere) => CollisionAabb {
                center: collision_sphere.center,
                extents: Vec3::splat(collision_sphere.radius),
            },
            CollisionShape::Triangle(collision_triangle) => CollisionAabb::from_points(collision_triangle.into_iter().copied()),
        }
    }

    pub fn apply_matrix_lossy(&self, mat: Mat4<f32>) -> Self {
        // todo: check
        match self {
            CollisionShape::Point(collision_point) => CollisionShape::Point(mat.mul_with_projection(*collision_point)),
            CollisionShape::Line(collision_line) => CollisionShape::Line(CollisionLine {
                start: mat.mul_with_projection(collision_line.start),
                end: mat.mul_with_projection(collision_line.end),
                bounds: collision_line.bounds,
            }),
            CollisionShape::Aabb(collision_aabb) => CollisionShape::Aabb(CollisionAabb {
                center: mat.mul_with_projection(collision_aabb.center),
                extents: collision_aabb.extents,
            }),
            CollisionShape::Box(collision_box) => {
                let (lossy_scale, rotation) = mat.ignored(3, 3).to_lossy_scale_rotation();

                CollisionShape::Box(CollisionBox {
                    center: mat.mul_with_projection(collision_box.center),
                    extents: lossy_scale * collision_box.extents,
                    rotation: rotation * collision_box.rotation,
                })
            }
            CollisionShape::Sphere(collision_sphere) => {
                let lossy_scale = mat.ignored(3, 3).to_lossy_scale();

                let avg_scale = lossy_scale.into_array().iter().sum::<f32>() / 3.0;

                CollisionShape::Sphere(CollisionSphere {
                    center: mat.mul_with_projection(collision_sphere.center),
                    radius: avg_scale * collision_sphere.radius,
                })
            }
            CollisionShape::Triangle(collision_triangle) => {
                CollisionShape::Triangle(collision_triangle.map(|p| mat.mul_with_projection(p)))
            }
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Serializable)]
pub struct CollisionBox {
    pub center: Vec3<f32>,
    pub extents: Vec3<f32>,
    pub rotation: Quat<f32>,
}
impl Default for CollisionBox {
    fn default() -> Self {
        Self {
            center: Vec3::splat(0.0),
            extents: Vec3::splat(0.5),
            rotation: Quat::IDENTITY,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Serializable)]
pub struct CollisionAabb {
    pub center: Vec3<f32>,
    pub extents: Vec3<f32>,
}
impl Default for CollisionAabb {
    fn default() -> Self {
        Self {
            center: Vec3::splat(0.0),
            extents: Vec3::splat(0.5),
        }
    }
}
impl CollisionAabb {
    pub fn from_min_max(min: Vec3<f32>, max: Vec3<f32>) -> Self {
        Self {
            center: (min + max) * 0.5,
            extents: (max - min) * 0.5,
        }
    }

    pub fn min(&self) -> Vec3<f32> {
        self.center - self.extents
    }

    pub fn max(&self) -> Vec3<f32> {
        self.center + self.extents
    }

    pub fn from_points(mut points: impl Iterator<Item = Vec3<f32>>) -> Self {
        let Some(first) = points.next() else {
            return Self {
                center: Vec3::splat(0.0),
                extents: Vec3::splat(0.0),
            };
        };

        let mut min = first;
        let mut max = first;

        for point in points {
            min = min.zip_copied(point, f32::min);
            max = max.zip_copied(point, f32::max);
        }

        Self::from_min_max(min, max)
    }

    pub fn merge(self, other: Self) -> Self {
        let min = self.min().zip_copied(other.min(), f32::min);
        let max = self.max().zip_copied(other.max(), f32::max);

        Self::from_min_max(min, max)
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Serializable)]
pub struct CollisionLine {
    pub start: Vec3<f32>,
    pub end: Vec3<f32>,
    pub bounds: LineBoundType,
}
impl Default for CollisionLine {
    fn default() -> Self {
        Self {
            start: Vec3::splat(0.0),
            end: Vec3::new(0.0, 0.0, 1.0),
            bounds: LineBoundType::SEGMENT,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Serializable)]
pub struct CollisionSphere {
    pub center: Vec3<f32>,
    pub radius: f32,
}
impl Default for CollisionSphere {
    fn default() -> Self {
        Self {
            center: Vec3::splat(0.0),
            radius: 0.5,
        }
    }
}
