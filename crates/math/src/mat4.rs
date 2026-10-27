use std::ops::Mul;

use crate::{Mat, Mat3, Primitive};

use super::{Vec3, Vec4, num::Number};

pub type Mat4<T> = Mat<4, T>;

impl<T: Primitive> Mat4<T> {
    pub const fn offset(offset: Vec3<T>) -> Self {
        Self::from_array([
            [T::ONE, T::ZERO, T::ZERO, T::ZERO],
            [T::ZERO, T::ONE, T::ZERO, T::ZERO],
            [T::ZERO, T::ZERO, T::ONE, T::ZERO],
            [offset.x, offset.y, offset.z, T::ONE],
        ])
    }

    pub const fn scale(scale: Vec4<T>) -> Self {
        Self::from_array([
            [scale.x, T::ZERO, T::ZERO, T::ZERO],
            [T::ZERO, scale.y, T::ZERO, T::ZERO],
            [T::ZERO, T::ZERO, scale.z, T::ZERO],
            [T::ZERO, T::ZERO, T::ZERO, scale.w],
        ])
    }

    pub const fn ignored(&self, x: usize, y: usize) -> Mat3<T> {
        Mat3::from_array([
            [
                self.ignored_element(x, y, 0, 0),
                self.ignored_element(x, y, 0, 1),
                self.ignored_element(x, y, 0, 2),
            ],
            [
                self.ignored_element(x, y, 1, 0),
                self.ignored_element(x, y, 1, 1),
                self.ignored_element(x, y, 1, 2),
            ],
            [
                self.ignored_element(x, y, 2, 0),
                self.ignored_element(x, y, 2, 1),
                self.ignored_element(x, y, 2, 2),
            ],
        ])
    }
}

impl<T: Number> Mat4<T> {
    pub fn determinant(&self) -> T {
        let data = self.as_array();

        T::ZERO + data[0][0] * self.ignored(0, 0).determinant() - data[1][0] * self.ignored(1, 0).determinant()
            + data[2][0] * self.ignored(2, 0).determinant()
            - data[3][0] * self.ignored(3, 0).determinant()
    }

    pub fn minors(&self) -> Self {
        Self::from_array([
            [
                self.ignored(0, 0).determinant(),
                self.ignored(0, 1).determinant(),
                self.ignored(0, 2).determinant(),
                self.ignored(0, 3).determinant(),
            ],
            [
                self.ignored(1, 0).determinant(),
                self.ignored(1, 1).determinant(),
                self.ignored(1, 2).determinant(),
                self.ignored(1, 3).determinant(),
            ],
            [
                self.ignored(2, 0).determinant(),
                self.ignored(2, 1).determinant(),
                self.ignored(2, 2).determinant(),
                self.ignored(2, 3).determinant(),
            ],
            [
                self.ignored(3, 0).determinant(),
                self.ignored(3, 1).determinant(),
                self.ignored(3, 2).determinant(),
                self.ignored(3, 3).determinant(),
            ],
        ])
    }

    // todo: only for signed
    pub fn cofactor(&mut self) {
        for x in 0..4 {
            for y in 0..4 {
                if (x + y) % 2 != 0 {
                    self[x][y] *= T::ZERO - T::ONE;
                }
            }
        }
    }

    pub fn inverse(&self) -> Option<Self> {
        let mut inverse = self.minors();
        inverse.cofactor();
        inverse.transpose();

        let determinant = self.determinant();

        if determinant == T::ZERO {
            return None;
        }

        Some(inverse * (T::ONE / determinant))
    }

    pub fn mul_with_projection(self, rhs: Vec3<T>) -> Vec3<T> {
        let Vec4 { x, y, z, w } = self.mul(Vec4::new(rhs.x, rhs.y, rhs.z, T::ONE));

        Vec3::new(x, y, z) / w
    }

    pub fn mul_with_projection_as_dir(self, rhs: Vec3<T>) -> Vec3<T> {
        let Vec4 { x, y, z, w: _w } = self.mul(Vec4::new(rhs.x, rhs.y, rhs.z, T::ZERO));

        Vec3::new(x, y, z)
    }
}

impl<T: Number> Mul for Mat4<T> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::from_array([
            [
                Vec4::from(self.row(0)).dot(Vec4::from(*rhs.col(0))),
                Vec4::from(self.row(1)).dot(Vec4::from(*rhs.col(0))),
                Vec4::from(self.row(2)).dot(Vec4::from(*rhs.col(0))),
                Vec4::from(self.row(3)).dot(Vec4::from(*rhs.col(0))),
            ],
            [
                Vec4::from(self.row(0)).dot(Vec4::from(*rhs.col(1))),
                Vec4::from(self.row(1)).dot(Vec4::from(*rhs.col(1))),
                Vec4::from(self.row(2)).dot(Vec4::from(*rhs.col(1))),
                Vec4::from(self.row(3)).dot(Vec4::from(*rhs.col(1))),
            ],
            [
                Vec4::from(self.row(0)).dot(Vec4::from(*rhs.col(2))),
                Vec4::from(self.row(1)).dot(Vec4::from(*rhs.col(2))),
                Vec4::from(self.row(2)).dot(Vec4::from(*rhs.col(2))),
                Vec4::from(self.row(3)).dot(Vec4::from(*rhs.col(2))),
            ],
            [
                Vec4::from(self.row(0)).dot(Vec4::from(*rhs.col(3))),
                Vec4::from(self.row(1)).dot(Vec4::from(*rhs.col(3))),
                Vec4::from(self.row(2)).dot(Vec4::from(*rhs.col(3))),
                Vec4::from(self.row(3)).dot(Vec4::from(*rhs.col(3))),
            ],
        ])
    }
}

impl<T: Number> Mul<Vec4<T>> for Mat4<T> {
    type Output = Vec4<T>;

    fn mul(self, rhs: Vec4<T>) -> Self::Output {
        Vec4::from_array([
            Vec4::from(self.row(0)).dot(rhs),
            Vec4::from(self.row(1)).dot(rhs),
            Vec4::from(self.row(2)).dot(rhs),
            Vec4::from(self.row(3)).dot(rhs),
        ])
    }
}

// todo MatrixIndex for optimized consecutive lookup.
// todo const fn and inline where possible/needed.
