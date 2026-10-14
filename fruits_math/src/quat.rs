use std::ops::Mul;

use fruits_utils::mem::{AllBitVariationsValid, AllBitsInit};
use serde::{Deserialize, Serialize};

use crate::{Mat3, Number, Vec3, Vec4};

#[derive(Default, Copy, Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Quat<N> {
    pub x: N,
    pub y: N,
    pub z: N,
    pub w: N,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RotationOrder3D {
    Xyz,
    Xzy,
    Yxz,
    Yzx,
    Zxy,
    Zyx,
}

impl RotationOrder3D {
    pub const fn axes(self) -> [usize; 3] {
        match self {
            Self::Xyz => [0, 1, 2],
            Self::Xzy => [0, 2, 1],
            Self::Yxz => [1, 0, 2],
            Self::Yzx => [1, 2, 0],
            Self::Zxy => [2, 0, 1],
            Self::Zyx => [2, 1, 0],
        }
    }
}

impl<N: Number> Quat<N> {
    pub const IDENTITY: Self = Self::new(N::ZERO, N::ZERO, N::ZERO, N::ONE);

    // todo: check (maybe need transposing)
    pub fn to_matrix(&self) -> Mat3<N> {
        let Quat { x, y, z, w } = *self;

        let n_1 = N::ONE;
        let n_2 = n_1 + n_1;

        let xy2 = x * y * n_2;
        let yy2 = y * y * n_2;
        let zz2 = z * z * n_2;
        let zw2 = z * w * n_2;
        let xz2 = x * z * n_2;
        let yw2 = y * w * n_2;
        let xx2 = x * x * n_2;
        let yz2 = y * z * n_2;
        let xw2 = x * w * n_2;

        Mat3::from_array([
            [n_1 - yy2 - zz2, xy2 + zw2, xz2 - yw2],
            [xy2 - zw2, n_1 - xx2 - zz2, yz2 + xw2],
            [xz2 + yw2, yz2 - xw2, n_1 - xx2 - yy2],
        ])
    }

    // todo: check (maybe need transposing)
    pub fn from_matrix(m: Mat3<N>) -> Self {
        let n_1 = N::ONE;
        let n_2 = n_1 + n_1;
        let n_4 = n_2 + n_2;

        let m = m.into_array();

        let tr = m[0][0] + m[1][1] + m[2][2];

        if tr > N::ZERO {
            // S=4*qw
            let s = N::from_f64((tr + n_1).into_f64().sqrt()) * n_2;

            let w = s / n_4;
            let x = (m[1][2] - m[2][1]) / s;
            let y = (m[2][0] - m[0][2]) / s;
            let z = (m[0][1] - m[1][0]) / s;

            Quat { x, y, z, w }
        } else if (m[0][0] > m[1][1]) && (m[0][0] > m[2][2]) {
            // S=4*qx
            let s = N::from_f64((n_1 + m[0][0] - m[1][1] - m[2][2]).into_f64().sqrt()) * n_2;

            let w = (m[1][2] - m[2][1]) / s;
            let x = s / n_4;
            let y = (m[1][0] + m[0][1]) / s;
            let z = (m[2][0] + m[0][2]) / s;

            Quat { x, y, z, w }
        } else if m[1][1] > m[2][2] {
            // S=4*qy
            let s = N::from_f64((n_1 + m[1][1] - m[0][0] - m[2][2]).into_f64().sqrt()) * n_2;

            let w = (m[2][0] - m[0][2]) / s;
            let x = (m[1][0] + m[0][1]) / s;
            let y = s / n_4;
            let z = (m[2][1] + m[1][2]) / s;

            Quat { x, y, z, w }
        } else {
            // S=4*qz
            let s = N::from_f64((n_1 + m[2][2] - m[0][0] - m[1][1]).into_f64().sqrt()) * n_2;

            let w = (m[0][1] - m[1][0]) / s;
            let x = (m[2][0] + m[0][2]) / s;
            let y = (m[2][1] + m[1][2]) / s;
            let z = s / n_4;

            Quat { x, y, z, w }
        }
    }

    pub fn rotation_x(rad: f64) -> Self {
        let half_angle = rad / 2.0;

        Self {
            x: N::from_f64(half_angle.sin()),
            y: N::ZERO,
            z: N::ZERO,
            w: N::from_f64(half_angle.cos()),
        }
    }

    pub fn rotation_y(rad: f64) -> Self {
        let half_angle = rad / 2.0;

        Self {
            x: N::ZERO,
            y: N::from_f64(half_angle.sin()),
            z: N::ZERO,
            w: N::from_f64(half_angle.cos()),
        }
    }

    pub fn rotation_z(rad: f64) -> Self {
        let half_angle = rad / 2.0;

        Self {
            x: N::ZERO,
            y: N::ZERO,
            z: N::from_f64(half_angle.sin()),
            w: N::from_f64(half_angle.cos()),
        }
    }

    pub fn rotation_axis_angle(mut axis: Vec3<N>, rad: f64) -> Self {
        if axis == Vec3::splat(N::from_f64(0.0)) {
            return Self::IDENTITY;
        }

        axis = axis.normalized();

        let half_angle = rad / 2.0;

        let half_angle_sin = N::from_f64(half_angle.sin());

        Self {
            x: axis.x * half_angle_sin,
            y: axis.y * half_angle_sin,
            z: axis.z * half_angle_sin,
            w: N::from_f64(half_angle.cos()),
        }
    }

    pub fn look_rotation(forward: Vec3<N>, mut up: Vec3<N>) -> Self {
        // todo: check and edge-cases
        let forward = forward.normalized();

        if forward.length() < 1e-6 {
            return Quat::IDENTITY;
        }

        let mut right = Vec3::cross(up, forward).normalized();

        if right.length_sq() < N::from_f64(1e-6) {
            up = Vec3::cross(forward, Vec3::X);
            if up.length_sq() < N::from_f64(1e-6) {
                up = Vec3::cross(forward, Vec3::Y);
            }

            right = Vec3::cross(up, forward).normalized();
        }

        let corrected_up = Vec3::cross(forward, right);

        // Construct rotation matrix
        let m = Mat3::<N>::from_array([right.into_array(), corrected_up.into_array(), forward.into_array()]);

        // Convert matrix to quaternion
        Self::from_matrix(m)
    }

    pub fn from_euler_angles(angles: Vec3<f64>, order: RotationOrder3D) -> Self {
        let angles = angles.into_array();

        let [i, j, k] = order.axes().map(|axis| match axis {
            0 => Self::rotation_x(angles[0]),
            1 => Self::rotation_y(angles[1]),
            _ => Self::rotation_z(angles[2]),
        });

        k * j * i
    }

    pub fn to_euler_angles(&self, order: RotationOrder3D) -> Vec3<f64> {
        let [i, j, k] = order.axes();
        let [x, y, z, w] = self.into_array().map(|v| v.into_f64());

        let s = 2.0 / (x * x + y * y + z * z + w * w);

        // row-major rotation matrix: m[row][col]
        let m = [
            [1.0 - s * (y * y + z * z), s * (x * y - z * w), s * (x * z + y * w)],
            [s * (x * y + z * w), 1.0 - s * (x * x + z * z), s * (y * z - x * w)],
            [s * (x * z - y * w), s * (y * z + x * w), 1.0 - s * (x * x + y * y)],
        ];

        let sign = if (j + 3 - i) % 3 == 1 { 1.0 } else { -1.0 };

        let sin_b = (-sign * m[k][i]).clamp(-1.0, 1.0);
        let b = sin_b.asin();

        let (a, c) = if sin_b.abs() < 1.0 - 1e-6 {
            (f64::atan2(sign * m[k][j], m[k][k]), f64::atan2(sign * m[j][i], m[i][i]))
        } else {
            // gimbal lock: the first and last axes coincide, put the whole rotation into the last one
            (0.0, f64::atan2(-sign * m[i][j], m[j][j]))
        };

        let mut angles = [0.0; 3];
        angles[i] = a;
        angles[j] = b;
        angles[k] = c;

        Vec3::from_array(angles)
    }

    pub const fn new(x: N, y: N, z: N, w: N) -> Self {
        Self { x, y, z, w }
    }

    pub const fn splat(v: N) -> Self {
        Self::new(v, v, v, v)
    }

    pub const fn as_array(&self) -> &[N; 4] {
        unsafe { std::mem::transmute(self) }
    }

    pub fn as_array_mut(&mut self) -> &mut [N; 4] {
        unsafe { std::mem::transmute(self) }
    }

    pub const fn from_array_ref(a: &[N; 4]) -> &Self {
        unsafe { std::mem::transmute(a) }
    }

    pub fn from_array_mut(a: &mut [N; 4]) -> &mut Self {
        unsafe { std::mem::transmute(a) }
    }

    pub const fn into_array(self) -> [N; 4] {
        let Self { x, y, z, w } = self;
        [x, y, z, w]
    }

    pub const fn from_array(a: [N; 4]) -> Self {
        let [x, y, z, w] = a;
        Self { x, y, z, w }
    }

    pub fn normalized(&self) -> Self {
        Self::from_array(*Vec4::from_array_ref(self.as_array()).normalized().as_array())
    }
}

impl<N: Number> Mul for Quat<N> {
    type Output = Quat<N>;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::Output {
            w: rhs.w * self.w - rhs.x * self.x - rhs.y * self.y - rhs.z * self.z,
            x: rhs.w * self.x + rhs.x * self.w - rhs.y * self.z + rhs.z * self.y,
            y: rhs.w * self.y + rhs.x * self.z + rhs.y * self.w - rhs.z * self.x,
            z: rhs.w * self.z - rhs.x * self.y + rhs.y * self.x + rhs.z * self.w,
        }
    }
}

unsafe impl<T: AllBitVariationsValid + Number> AllBitVariationsValid for Quat<T> {}
unsafe impl<T: AllBitsInit + Number> AllBitsInit for Quat<T> {}
