//! # fruits_math
//!
//! Linear-algebra and numeric primitives the engine builds on: vectors, square
//! matrices, quaternions, color parsing, and interpolation helpers.
//!
//! # How to use
//!
//! Every type here is generic over the element type. The numeric operations are
//! available for the built-in scalar types ([`f32`], [`f64`], the integer types,
//! and so on) through the [`Number`] and [`Primitive`] traits.
//!
//! #### Vector arithmetic
//!
//! [`Vec2`], [`Vec3`], and [`Vec4`] support the usual operators plus geometric
//! operations such as [`dot`](Vec3::dot), [`cross`](Vec3::cross),
//! [`length`](Vec3::length), and [`normalized`](Vec3::normalized).
//!
//! ```
//! use fruits_math::Vec3;
//!
//! let a = Vec3::new(1.0, 2.0, 3.0);
//! let b = Vec3::new(4.0, 5.0, 6.0);
//!
//! let sum = a + b;
//! let dot = a.dot(b);
//! let cross = a.cross(b);
//! let unit = a.normalized();
//! let len = a.length(); // always f64
//!
//! assert_eq!(sum, Vec3::new(5.0, 7.0, 9.0));
//! assert_eq!(dot, 32.0);
//! ```
//!
//! #### Swizzling
//!
//! Vectors expose generated accessors that reorder or resize their components,
//! Unity/GLSL style. The name lists the components to read; an `n` in the name
//! marks a slot filled by an extra argument instead of an existing component.
//!
//! ```
//! use fruits_math::{Vec2, Vec3};
//!
//! let v = Vec3::new(1.0, 2.0, 3.0);
//!
//! let xy: Vec2<f64> = v.xy();
//! let zx: Vec2<f64> = v.zx();
//!
//! assert_eq!(xy, Vec2::new(1.0, 2.0));
//! assert_eq!(zx, Vec2::new(3.0, 1.0));
//! ```
//!
//! #### Transforms with matrices
//!
//! [`Mat2`], [`Mat3`], and [`Mat4`] are square matrices used for transforms.
//! Build them from the helper constructors ([`offset`](Mat4::offset),
//! [`scale`](Mat4::scale), the `rotation_*` family) and apply them with `*`.
//! Matrices are column-major and multiply a column vector on the right.
//!
//! ```
//! use fruits_math::{Mat4, Vec3, Vec4};
//!
//! let translate = Mat4::<f32>::offset(Vec3::new(1.0, 2.0, 3.0));
//! let point = translate * Vec4::new(0.0, 0.0, 0.0, 1.0);
//!
//! assert_eq!(point, Vec4::new(1.0, 2.0, 3.0, 1.0));
//! ```
//!
//! #### Inverting a matrix
//!
//! [`inverse`](Mat3::inverse) returns [`None`] for a singular (zero-determinant)
//! matrix.
//!
//! ```
//! use fruits_math::Mat3;
//!
//! let rotation = Mat3::<f32>::rotation_z(std::f64::consts::FRAC_PI_2);
//! let inverse = rotation.inverse().expect("rotations are invertible");
//!
//! let _identity_ish = rotation * inverse;
//! ```
//!
//! #### Rotations with quaternions
//!
//! [`Quat`] builds rotations from an axis and angle, from per-axis angles, or
//! from a look direction, and converts to and from a [`Mat3`].
//!
//! ```
//! use fruits_math::{Quat, Vec3};
//!
//! let yaw = Quat::<f32>::rotation_axis_angle(Vec3::new(0.0, 1.0, 0.0), std::f64::consts::PI);
//! let matrix = yaw.to_matrix();
//! let chained = yaw * Quat::IDENTITY;
//! ```
//!
//! #### Interpolation
//!
//! [`lerp`] interpolates between two values, [`inv_lerp`] recovers the parameter,
//! and [`damp`] drives a value toward a target with a spring-like, frame-rate
//! independent response (it updates the caller's velocity in place).
//!
//! ```
//! use fruits_math::{damp, inv_lerp, lerp};
//!
//! assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
//! assert_eq!(inv_lerp(0.0, 10.0, 5.0), 0.5);
//!
//! let mut velocity = 0.0;
//! let smoothed = damp(0.0, 10.0, &mut velocity, 0.3, 1.0, 1.0 / 60.0);
//! ```
//!
//! #### Parsing colors
//!
//! The `parse_color_*` functions decode hex strings (with or without a leading
//! `#`) into `u8` or normalized `f32` channels, and are usable in `const`
//! contexts.
//!
//! ```
//! use fruits_math::{parse_color_rgb_u8, parse_color_rgba_f32};
//!
//! assert_eq!(parse_color_rgb_u8("#ff8800"), Some([0xff, 0x88, 0x00]));
//! assert_eq!(parse_color_rgba_f32("00000000"), Some([0.0, 0.0, 0.0, 0.0]));
//! ```
//!
//! #### Solving small equations
//!
//! [`eq_linear`] and [`eq_quadratic`] return result enums that distinguish no
//! solution, every value, and one or two concrete roots.
//!
//! ```
//! use fruits_math::{eq_quadratic, QuadraticEquationResult};
//!
//! // x^2 - 3x + 2 = 0  ->  roots 1 and 2
//! match eq_quadratic(1.0, -3.0, 2.0) {
//!     QuadraticEquationResult::Double(roots) => assert_eq!(roots, [1.0, 2.0]),
//!     _ => unreachable!(),
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Scalar traits
//!
//! [`Primitive`] and [`Number`] are *sealed* (their supertrait `Sealed` is
//! private), so downstream crates cannot implement them. [`Primitive`] supplies
//! `ZERO`/`ONE` and `into_f64`, and is implemented for every scalar including
//! [`bool`] and [`char`]; [`Number`] adds the arithmetic operator bounds plus
//! `from_f64`, and is implemented only for the numeric types. The two `*_f64`
//! conversions are the bridge used wherever a real-valued result is needed —
//! e.g. [`length`](Vec3::length) always returns [`f64`] regardless of `T`. The
//! `length`/`length_sq` names are flagged for a possible rename to `magnitude`
//! (see the `// todo` comments in `vec.rs`).
//!
//! #### Vector layout and the array views
//!
//! Each vector is `#[repr(C)]` with named fields, and the `as_array` /
//! `from_array_ref` / `into_array` views reinterpret that struct as a `[T; N]`
//! through pointer casts (`ManuallyDrop` for the by-value moves). This relies on
//! the C layout matching a plain array, so the field order and `repr(C)` must not
//! change. Vectors are marked [`AllBitVariationsValid`](fruits_utils::mem) and
//! [`AllBitsInit`](fruits_utils::mem) when `T` is, which lets them be uploaded as
//! raw bytes. The whole `Vec*` surface is generated by the `vec_impl!` macro;
//! edit the macro body, not per-type copies.
//!
//! #### Swizzle generation
//!
//! The swizzle accessors are emitted by the `fruits_swizzling::swizzling!{}`
//! proc-macro invocation in `vec.rs`, which generates every reorder/resize of
//! 2-, 3-, and 4-component vectors. A slot named `n` is not read from the source
//! vector; it becomes an extra function parameter, so a method like `xyn` widens
//! a `Vec2` to a `Vec3` by appending the supplied value.
//!
//! #### Matrix storage
//!
//! [`Mat`] stores `[[T; N]; N]` **column-major**: `data[x]` is column `x`, so
//! `mat[(x, y)]` and `mat[x][y]` address column `x`, row `y`. [`col`](Mat::col)
//! returns a borrow and is cheap; [`row`](Mat::row) copies elements out of each
//! column and is therefore a rebuild, not a view. [`Mat2`]/[`Mat3`]/[`Mat4`] are
//! type aliases for `Mat<2/3/4, T>` carrying the dimension-specific constructors.
//! Multiplication is defined as row-of-`self` dotted with column-of-`rhs`.
//!
//! #### Determinants and inverses
//!
//! [`inverse`](Mat3::inverse) uses the adjugate method: build the matrix of
//! [`minors`](Mat3::minors), apply [`cofactor`](Mat3::cofactor) sign flips,
//! [`transpose`](Mat::transpose), then divide by the [`determinant`](Mat3::determinant),
//! returning [`None`] when the determinant is zero. `minors` is computed via
//! `ignored`, which drops one row and column using the private
//! [`ignored_element`](Mat) index-shift helper. `cofactor` multiplies by
//! `ZERO - ONE`, so it is only meaningful for signed element types (noted with a
//! `// todo: only for signed`).
//!
//! #### Quaternions
//!
//! [`Quat`] is `#[repr(C)]` `(x, y, z, w)` and reuses [`Vec4`]'s array view for
//! [`normalized`](Quat::normalized). [`to_matrix`](Quat::to_matrix) /
//! [`from_matrix`](Quat::from_matrix) carry `// todo: check (maybe need
//! transposing)` comments — the column/row convention there has not been fully
//! verified against the column-major [`Mat3`]. [`look_rotation`](Quat::look_rotation)
//! falls back to alternate up axes when `forward` is parallel to `up`, and the
//! multiplication order in `Mul` composes `rhs` then `self`.
//!
//! #### Projection and assorted free functions
//!
//! [`perspective_proj_matrix`] builds a perspective matrix from fov/near/far/aspect;
//! an earlier formulation is kept commented out above the active one.
//! [`into_matrix4x4_with_pos`] embeds a [`Mat3`] rotation plus a [`Vec3`]
//! translation into a [`Mat4`]. The free slice helpers (`dot`, `cross`,
//! `normalized`, `lerp_slice`, …) operate on `[T; N]` directly and back the
//! method versions; the standing `// todo`s suggest loosening their [`Number`]
//! bound and making more of them `const`.
//!
//! #### Equation results
//!
//! [`LinearEquationResult`] and [`QuadraticEquationResult`] separate the
//! degenerate cases (`None` for no solution, `Any` for an identity) from the
//! concrete roots, so callers must match rather than assume a root exists.
//!
//! #### Serialization
//!
//! Vectors and quaternions derive [`Serialize`](serde::Serialize)/[`Deserialize`](serde::Deserialize),
//! but [`Mat`] has hand-written impls that encode the matrix as a sequence of
//! column sequences, deserializing through a `StackVec`-backed visitor. This crate
//! does not depend on `fruits_serialization`; the engine's own `Serializable`
//! impls for these types live in that crate instead.

mod colors;
mod equations;
mod mat;
mod mat2;
mod mat3;
mod mat4;
mod num;
mod quat;
mod vec;

pub use colors::*;
pub use equations::*;
pub use mat::*;
pub use mat2::*;
pub use mat3::*;
pub use mat4::*;
pub use num::*;
pub use quat::*;
pub use vec::*;

pub fn into_matrix4x4_with_pos<T: Primitive>(mat: Mat3<T>, pos: Vec3<T>) -> Mat4<T> {
    Mat4::from_array([
        [mat[0][0], mat[0][1], mat[0][2], T::ZERO],
        [mat[1][0], mat[1][1], mat[1][2], T::ZERO],
        [mat[2][0], mat[2][1], mat[2][2], T::ZERO],
        [pos.x, pos.y, pos.z, T::ONE],
    ])
}

pub fn perspective_proj_matrix(fov: f32, near: f32, far: f32, aspect: f32) -> Mat4<f32> {
    // todo
    // let s = -1_f32 / ((fov / 2_f32).tan());

    // Mat4::<f32>::from_array([
    //     [s, 0_f32, 0_f32, 0_f32],
    //     [0_f32, s, 0_f32, 0_f32],
    //     [0_f32, 0_f32, (-far / (far - near)), 1_f32],
    //     [0_f32, 0_f32, ((-far * near) / (far - near)), 0_f32],
    // ])

    let s = 1_f32 / ((fov / 2_f32).tan());

    Mat4::<f32>::from_array([
        [(s / aspect), 0_f32, 0_f32, 0_f32],
        [0_f32, s, 0_f32, 0_f32],
        [0_f32, 0_f32, (far / (far - near)), 1_f32],
        [0_f32, 0_f32, ((-far * near) / (far - near)), 0_f32],
    ])
}

pub fn lerp<T: Number>(a: T, b: T, t: T) -> T {
    a + (b - a) * t
}

pub fn inv_lerp<T: Number>(a: T, b: T, x: T) -> T {
    (x - a) / (b - a)
}

pub fn damp(
    current: f32,
    target: f32,
    velocity: &mut f32,
    smooth_time: f32,
    damping: f32,
    delta_time: f32,
) -> f32 {
    let smooth_time = smooth_time.max(1e-4);
    let omega = 2.0 / smooth_time;

    // Let the damping be fully free (negative included!)
    let zeta = damping;

    // Solve as usual for displacement from target
    let x0 = current - target;
    let v0 = *velocity;
    let t = delta_time;

    let new_x;
    let new_v;

    if zeta < 1.0 {
        // Underdamped **OR negative damping**
        //
        // For negative ζ, the exponent becomes POSITIVE
        // and the oscillation amplitude grows each frame.
        let omega_d_sq = 1.0 - zeta * zeta;
        let omega_d = if omega_d_sq > 0.0 {
            omega * omega_d_sq.sqrt()
        } else {
            // If ζ < -1, we switch into real exponent regime
            // (it will be handled below)
            0.0
        };

        if omega_d_sq > 0.0 {
            // Underdamped branch (ζ < 1)
            let exp = (-zeta * omega * t).exp();

            let c1 = x0;
            let c2 = (v0 + zeta * omega * x0) / omega_d;

            let cos = (omega_d * t).cos();
            let sin = (omega_d * t).sin();

            new_x = exp * (c1 * cos + c2 * sin);
            new_v = exp * (-zeta * omega * (c1 * cos + c2 * sin)
                + (-c1 * omega_d * sin + c2 * omega_d * cos));
        } else {
            // ζ <= -1: real exponential instability (no oscillation)
            let tmp = (zeta * zeta - 1.0).sqrt();
            let r1 = -omega * (zeta - tmp);
            let r2 = -omega * (zeta + tmp);

            let denom = r1 - r2;
            let c1 = (v0 - r2 * x0) / denom;
            let c2 = x0 - c1;

            let e1 = (r1 * t).exp();
            let e2 = (r2 * t).exp();

            new_x = c1 * e1 + c2 * e2;
            new_v = c1 * r1 * e1 + c2 * r2 * e2;
        }
    } else if (zeta - 1.0).abs() < 1e-6 {
        // Critical damping
        let exp = (-omega * t).exp();
        let c1 = x0;
        let c2 = v0 + omega * x0;

        new_x = (c1 + c2 * t) * exp;
        new_v = exp * (c2 - omega * (c1 + c2 * t));
    } else {
        // Overdamped
        let tmp = (zeta * zeta - 1.0).sqrt();
        let r1 = -omega * (zeta - tmp);
        let r2 = -omega * (zeta + tmp);

        let denom = r1 - r2;
        let c1 = (v0 - r2 * x0) / denom;
        let c2 = x0 - c1;

        let e1 = (r1 * t).exp();
        let e2 = (r2 * t).exp();

        new_x = c1 * e1 + c2 * e2;
        new_v = c1 * r1 * e1 + c2 * r2 * e2;
    }

    *velocity = new_v;
    target + new_x
}