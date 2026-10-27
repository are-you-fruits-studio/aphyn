use std::ops::Mul;

use crate::{Mat, Mat2, Mat4, Primitive, Quat};

use super::{Vec2, Vec3, num::Number};

pub type Mat3<T> = Mat<3, T>;

impl<T: Primitive> Mat3<T> {
    pub const fn offset(offset: Vec2<T>) -> Self {
        Self::from_array([
            [T::ONE, T::ZERO, T::ZERO],
            [T::ZERO, T::ONE, T::ZERO],
            [offset.x, offset.y, T::ONE],
        ])
    }

    pub const fn scale(scale: Vec3<T>) -> Self {
        Self::from_array([
            [scale.x, T::ZERO, T::ZERO],
            [T::ZERO, scale.y, T::ZERO],
            [T::ZERO, T::ZERO, scale.z],
        ])
    }

    pub const fn into_4x4(&self) -> Mat4<T> {
        let data = self.as_array();

        Mat4::from_array([
            [data[0][0], data[0][1], data[0][2], T::ZERO],
            [data[1][0], data[1][1], data[1][2], T::ZERO],
            [data[2][0], data[2][1], data[2][2], T::ZERO],
            [T::ZERO, T::ZERO, T::ZERO, T::ONE],
        ])
    }

    pub const fn into_4x4_with_offset(&self, offset: Vec3<T>) -> Mat4<T> {
        let data = self.as_array();

        Mat4::from_array([
            [data[0][0], data[0][1], data[0][2], T::ZERO],
            [data[1][0], data[1][1], data[1][2], T::ZERO],
            [data[2][0], data[2][1], data[2][2], T::ZERO],
            [offset.x, offset.y, offset.z, T::ONE],
        ])
    }

    pub const fn ignored(&self, x: usize, y: usize) -> Mat2<T> {
        Mat2::from_array([
            [self.ignored_element(x, y, 0, 0), self.ignored_element(x, y, 0, 1)],
            [self.ignored_element(x, y, 1, 0), self.ignored_element(x, y, 1, 1)],
        ])
    }
}
impl<T: Number> Mat3<T> {
    // todo
    // pub const fn from_euler(euler: Vec3<f64>) -> Self {
    //     // todo: check axis order
    //     let [c, b, a] = euler.into_array();

    //     let sin_a = a.sin();
    //     let sin_b = b.sin();
    //     let sin_c = c.sin();

    //     let cos_a = a.cos();
    //     let cos_b = b.cos();
    //     let cos_c = c.cos();

    //     Self::from_array([
    //         [],
    //         [],
    //         [],
    //     ])
    // }

    pub fn rotation_x(angle: f64) -> Self {
        let matrix = Mat2::from_rotation(angle);

        Self::from_array([
            [T::ONE, T::ZERO, T::ZERO],
            [T::ZERO, matrix[0][0], matrix[0][1]],
            [T::ZERO, matrix[1][0], matrix[1][1]],
        ])
    }

    pub fn rotation_y(angle: f64) -> Self {
        let matrix = Mat2::from_rotation(angle);

        Self::from_array([
            [matrix[0][0], T::ZERO, matrix[1][0]],
            [T::ZERO, T::ONE, T::ZERO],
            [matrix[0][1], T::ZERO, matrix[1][1]],
        ])
    }

    pub fn rotation_z(angle: f64) -> Self {
        let matrix = Mat2::from_rotation(angle);

        Self::from_array([
            [matrix[0][0], matrix[0][1], T::ZERO],
            [matrix[1][0], matrix[1][1], T::ZERO],
            [T::ZERO, T::ZERO, T::ONE],
        ])
    }

    pub fn rotation_euler(euler: Vec3<f64>) -> Self {
        // todo: optimize and allow for various axis order

        Self::rotation_y(euler.y) * Self::rotation_x(euler.x) * Self::rotation_z(euler.z)
    }

    pub fn determinant(&self) -> T {
        let data = self.as_array();

        T::ZERO + data[0][0] * self.ignored(0, 0).determinant() - data[1][0] * self.ignored(1, 0).determinant()
            + data[2][0] * self.ignored(2, 0).determinant()
    }

    pub fn minors(&self) -> Self {
        Self::from_array([
            [
                self.ignored(0, 0).determinant(),
                self.ignored(0, 1).determinant(),
                self.ignored(0, 2).determinant(),
            ],
            [
                self.ignored(1, 0).determinant(),
                self.ignored(1, 1).determinant(),
                self.ignored(1, 2).determinant(),
            ],
            [
                self.ignored(2, 0).determinant(),
                self.ignored(2, 1).determinant(),
                self.ignored(2, 2).determinant(),
            ],
        ])
    }

    // todo: only for signed
    pub fn cofactor(&mut self) {
        for x in 0..3 {
            for y in 0..3 {
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

    pub fn mul_with_projection(self, rhs: Vec2<T>) -> Vec2<T> {
        let Vec3 { x, y, z } = self.mul(Vec3::new(rhs.x, rhs.y, T::ONE));

        Vec2::new(x, y) / z
    }

    pub fn to_lossy_scale(&self) -> Vec3<T> {
        Vec3 {
            x: T::from_f64(crate::vec::length(&self[0])),
            y: T::from_f64(crate::vec::length(&self[1])),
            z: T::from_f64(crate::vec::length(&self[2])),
        }
    }

    pub fn to_lossy_scale_rotation(&self) -> (Vec3<T>, Quat<T>) {
        // todo: check
        let scale = self.to_lossy_scale();

        let mut rot_mat = *self;

        for i in 0..scale.as_array().len() {
            if scale[i] != T::ZERO {
                rot_mat[i] = crate::vec::div_by(&rot_mat[i], scale.x);
            }
        }

        (scale, Quat::from_matrix(rot_mat))
    }
}

impl<T: Number> Mul for Mat3<T> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let self_row_0 = self.row(0);
        let self_row_1 = self.row(1);
        let self_row_2 = self.row(2);

        let rhs_col_0 = *rhs.col(0);
        let rhs_col_1 = *rhs.col(1);
        let rhs_col_2 = *rhs.col(2);

        Self::from_array([
            [
                Vec3::from(self_row_0).dot(Vec3::from(rhs_col_0)),
                Vec3::from(self_row_1).dot(Vec3::from(rhs_col_0)),
                Vec3::from(self_row_2).dot(Vec3::from(rhs_col_0)),
            ],
            [
                Vec3::from(self_row_0).dot(Vec3::from(rhs_col_1)),
                Vec3::from(self_row_1).dot(Vec3::from(rhs_col_1)),
                Vec3::from(self_row_2).dot(Vec3::from(rhs_col_1)),
            ],
            [
                Vec3::from(self_row_0).dot(Vec3::from(rhs_col_2)),
                Vec3::from(self_row_1).dot(Vec3::from(rhs_col_2)),
                Vec3::from(self_row_2).dot(Vec3::from(rhs_col_2)),
            ],
        ])
    }
}

impl<T: Number> Mul<Vec3<T>> for Mat3<T> {
    type Output = Vec3<T>;

    fn mul(self, rhs: Vec3<T>) -> Self::Output {
        Vec3::from_array([
            Vec3::from(self.row(0)).dot(rhs),
            Vec3::from(self.row(1)).dot(rhs),
            Vec3::from(self.row(2)).dot(rhs),
        ])
    }
}

// todo MatrixIndex for optimized consecutive lookup.
// todo const fn and inline where possible/needed.
