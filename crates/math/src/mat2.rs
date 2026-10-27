use std::ops::Mul;

use crate::{Mat, Primitive};

use super::{Vec2, num::Number};

pub type Mat2<T> = Mat<2, T>;

impl<T: Primitive> Mat2<T> {
    pub const fn offset(offset: T) -> Mat2<T> {
        Mat2::<T>::from_array([[T::ONE, T::ZERO], [offset, T::ZERO]])
    }

    pub const fn scale(scale: Vec2<T>) -> Mat2<T> {
        Mat2::<T>::from_array([[scale.x, T::ZERO], [T::ZERO, scale.y]])
    }

    pub const fn ignored(&self, x: usize, y: usize) -> T {
        self.ignored_element(x, y, 0, 0)
    }

    pub fn minors(&self) -> Mat2<T> {
        Mat2::from_array([
            [self.ignored(0, 0), self.ignored(0, 1)],
            [self.ignored(1, 0), self.ignored(1, 1)],
        ])
    }
}
impl<T: Number> Mat2<T> {
    pub fn from_rotation(angle: f64) -> Self {
        let (sin, cos) = angle.sin_cos();

        Self::from_array([[T::from_f64(cos), T::from_f64(sin)], [T::from_f64(-sin), T::from_f64(cos)]])
    }

    pub fn determinant(&self) -> T {
        let data = self.as_array();

        data[0][0] * data[1][1] - data[0][1] * data[1][0]
    }

    // todo: only for signed
    pub fn cofactor(&mut self) {
        for x in 0..2 {
            for y in 0..2 {
                if (x + y) % 2 != 0 {
                    self[x][y] *= T::ZERO - T::ONE;
                }
            }
        }
    }

    pub fn inverse(&self) -> Option<Mat2<T>> {
        let mut inverse = self.minors();
        inverse.cofactor();
        inverse.transpose();

        let determinant = self.determinant();

        if determinant == T::ZERO {
            return None;
        }

        Some(inverse * (T::ONE / determinant))
    }

    pub fn mul_with_projection(&self, rhs: T) -> T {
        let Vec2 { x, y } = self.mul(Vec2::new(rhs, T::ONE));
        x / y
    }
}

impl<T: Number> Mul for Mat2<T> {
    type Output = Self;

    fn mul(self, rhs: Mat2<T>) -> Self::Output {
        Self::from_array([
            [
                Vec2::from(self.row(0)).dot(Vec2::from(*rhs.col(0))),
                Vec2::from(self.row(1)).dot(Vec2::from(*rhs.col(0))),
            ],
            [
                Vec2::from(self.row(0)).dot(Vec2::from(*rhs.col(1))),
                Vec2::from(self.row(1)).dot(Vec2::from(*rhs.col(1))),
            ],
        ])
    }
}

impl<T: Number> Mul<Vec2<T>> for Mat2<T> {
    type Output = Vec2<T>;

    fn mul(self, rhs: Vec2<T>) -> Self::Output {
        Vec2::from_array([Vec2::from(self.row(0)).dot(rhs), Vec2::from(self.row(1)).dot(rhs)])
    }
}

// todo MatrixIndex for optimized consecutive lookup.
// todo const fn and inline where possible/needed.
