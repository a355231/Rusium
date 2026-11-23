// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `gfx::Quaternion` (ui/gfx/geometry/quaternion.{h,cc}).
//!
//! A quaternion `(x, y, z, w)` with `double` components, used for 3D rotation
//! (Slerp, Lerp, axis-angle and vector-to-vector construction).
//!
//! Dependencies: the C++ code uses `gfx::Vector3dF`, which is a float 3-vector,
//! and calls `DotProduct`, `CrossProduct`, `Length`, `LengthSquared` and
//! `InvScale` on it. Rather than depend on another crate, this crate defines a
//! private `Vec3` (f32 components, like `Vector3dF`) with just those
//! operations. The public API takes `[f32; 3]` arrays, so `Vec3` does not
//! appear in any public signature. The float rounding is the same as in C++.
//!
//! Differences from the C++ version:
//! - The C++ constructors `Quaternion(axis, angle)` and `Quaternion(from, to)`
//!   are `Quaternion::from_axis` and `Quaternion::from_vectors`.
//!   `FromAxisAngle(x, y, z, angle)` is `Quaternion::from_axis_angle`.
//! - Accessors and setters are kept: `x()`/`set_x()` and so on.
//! - `Length()` returns the squared length, not the length, as in C++. The name
//!   is kept so the port is easy to compare. Use `length()`.
//! - `ToString()` is `impl Display`. The output is the same, with the C
//!   printf `%f`, `%g` formats reproduced by `fmt_f` and `fmt_g`.
//! - `operator==` / `!=` are derived `PartialEq`, as in C++ (`= default`).
//! - `PrintTo` (gtest support) is not ported. It is a test-only hook.
//! - `Vector3dF` itself (its `Length`, `Scale`, etc. as public API) is not
//!   ported. Only the operations the quaternion code needs are in `Vec3`.

use std::fmt;
use std::ops::{Add, Div, Mul};

/// Threshold below which an axis or quaternion is treated as degenerate.
/// Same as `kEpsilon` in quaternion.cc.
const K_EPSILON: f64 = 1e-5;

/// Private 3-vector with `f32` components, matching `gfx::Vector3dF`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn from_array(a: [f32; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    /// `Vector3dF::LengthSquared`: computed in double.
    fn length_squared(self) -> f64 {
        let (x, y, z) = (self.x as f64, self.y as f64, self.z as f64);
        x * x + y * y + z * z
    }

    /// `Vector3dF::Length`: sqrt in double, rounded to float.
    fn length(self) -> f32 {
        self.length_squared().sqrt() as f32
    }

    /// `Vector3dF::Scale(float)`.
    fn scale(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
    }

    /// `Vector3dF::InvScale(float)`: divides each component.
    fn inv_scale(&mut self, inv: f32) {
        self.x /= inv;
        self.y /= inv;
        self.z /= inv;
    }

    /// `gfx::DotProduct`: computed in float.
    fn dot(self, other: Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// `gfx::CrossProduct`: computed in double, rounded to float per component.
    fn cross(self, other: Vec3) -> Vec3 {
        let (dx, dy, dz) = (self.x as f64, self.y as f64, self.z as f64);
        let (ox, oy, oz) = (other.x as f64, other.y as f64, other.z as f64);
        Vec3::new(
            (dy * oz - dz * oy) as f32,
            (dz * ox - dx * oz) as f32,
            (dx * oy - dy * ox) as f32,
        )
    }
}

/// A quaternion `(x, y, z, w)` with `f64` components. The default value is the
/// identity rotation `(0, 0, 0, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }
}

impl Quaternion {
    /// Builds a quaternion from its components. Same as the C++ four-argument
    /// constructor.
    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    /// Rotation of `angle` radians about `axis`. The axis does not need to be
    /// unit length; the angle is scaled by its magnitude (as in C++). An axis
    /// shorter than 1e-5 gives the identity. Same as `Quaternion(axis, angle)`.
    pub fn from_axis(axis: [f32; 3], theta: f64) -> Self {
        let axis = Vec3::from_array(axis);
        // Rotation angle is the product of the angle and the magnitude of axis.
        let length = axis.length() as f64;
        if length.abs() < K_EPSILON {
            return Self::default();
        }

        let mut normalized = axis;
        normalized.inv_scale(length as f32);

        let theta = theta * 0.5;
        let s = theta.sin();
        Self::new(
            normalized.x as f64 * s,
            normalized.y as f64 * s,
            normalized.z as f64 * s,
            theta.cos(),
        )
    }

    /// Shortest rotation from direction `from` to direction `to`. The result
    /// is normalized. Same as `Quaternion(from, to)`.
    pub fn from_vectors(from: [f32; 3], to: [f32; 3]) -> Self {
        let from = Vec3::from_array(from);
        let to = Vec3::from_array(to);
        let dot = from.dot(to) as f64;
        let norm = (from.length_squared() * to.length_squared()).sqrt();
        let mut real = norm + dot;
        let axis = if real < K_EPSILON * norm {
            // The vectors point in opposite directions. Pick any perpendicular
            // axis.
            real = 0.0;
            if from.x.abs() > from.z.abs() {
                Vec3::new(-from.y, from.x, 0.0)
            } else {
                Vec3::new(0.0, -from.z, from.y)
            }
        } else {
            from.cross(to)
        };
        Self::new(axis.x as f64, axis.y as f64, axis.z as f64, real).normalized()
    }

    /// Rotation from an axis given as scalars and an angle in radians. A zero
    /// axis gives the identity. Same as `Quaternion::FromAxisAngle`.
    pub fn from_axis_angle(x: f64, y: f64, z: f64, angle: f64) -> Self {
        let length = (x * x + y * y + z * z).sqrt();
        if length.abs() < K_EPSILON {
            return Self::new(0.0, 0.0, 0.0, 1.0);
        }
        let scale = (0.5 * angle).sin() / length;
        Self::new(scale * x, scale * y, scale * z, (0.5 * angle).cos())
    }

    pub fn x(&self) -> f64 {
        self.x
    }
    pub fn set_x(&mut self, x: f64) {
        self.x = x;
    }
    pub fn y(&self) -> f64 {
        self.y
    }
    pub fn set_y(&mut self, y: f64) {
        self.y = y;
    }
    pub fn z(&self) -> f64 {
        self.z
    }
    pub fn set_z(&mut self, z: f64) {
        self.z = z;
    }
    pub fn w(&self) -> f64 {
        self.w
    }
    pub fn set_w(&mut self, w: f64) {
        self.w = w;
    }

    /// Conjugate `(-x, -y, -z, w)`. For a unit quaternion this is the inverse.
    pub fn inverse(&self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }

    /// Negates all four components: the same rotation, opposite orientation.
    pub fn flip(&self) -> Self {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }

    /// Spherical interpolation from `self` to `to` by `t`. `t` in [0, 1]
    /// interpolates, and other values extrapolate. Follows the shortest path.
    pub fn slerp(&self, to: Quaternion, t: f64) -> Quaternion {
        let mut from = *self;

        let mut cos_half_angle = from.x * to.x + from.y * to.y + from.z * to.z + from.w * to.w;
        if cos_half_angle < 0.0 {
            // (x, y, z, w) and (-x, -y, -z, -w) are the same rotation, so flip
            // one of them to get the shorter path.
            from = from.flip();
            cos_half_angle = -cos_half_angle;
        }

        // Keep acos well behaved at the boundary.
        if cos_half_angle > 1.0 {
            cos_half_angle = 1.0;
        }

        let sin_half_angle = (1.0 - cos_half_angle * cos_half_angle).sqrt();
        if sin_half_angle < K_EPSILON {
            // The quaternions share an axis and angle.
            return *self;
        }

        let half_angle = cos_half_angle.acos();

        let scale_a = ((1.0 - t) * half_angle).sin() / sin_half_angle;
        let scale_b = (t * half_angle).sin() / sin_half_angle;

        (scale_a * from) + (scale_b * to)
    }

    /// Linear interpolation, then normalized. Rarely what you want; see C++.
    pub fn lerp(&self, q: Quaternion, t: f64) -> Quaternion {
        (((1.0 - t) * *self) + (t * q)).normalized()
    }

    /// Squared length `x^2 + y^2 + z^2 + w^2`. Not the square root, as in C++.
    pub fn length(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Scales to unit length. Returns `self` unchanged if its squared length is
    /// below 1e-5.
    pub fn normalized(&self) -> Quaternion {
        let length = self.length();
        if length < K_EPSILON {
            return *self;
        }
        *self / length.sqrt()
    }
}

impl Add for Quaternion {
    type Output = Quaternion;
    fn add(self, q: Quaternion) -> Quaternion {
        Quaternion::new(q.x + self.x, q.y + self.y, q.z + self.z, q.w + self.w)
    }
}

/// Hamilton product, as in C++ `operator*(const Quaternion&)`.
impl Mul for Quaternion {
    type Output = Quaternion;
    fn mul(self, q: Quaternion) -> Quaternion {
        let (a, b) = (self, q);
        Quaternion::new(
            a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
            a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
            a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
            a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
        )
    }
}

impl Mul<f64> for Quaternion {
    type Output = Quaternion;
    fn mul(self, s: f64) -> Quaternion {
        Quaternion::new(self.x * s, self.y * s, self.z * s, self.w * s)
    }
}

impl Mul<Quaternion> for f64 {
    type Output = Quaternion;
    fn mul(self, q: Quaternion) -> Quaternion {
        Quaternion::new(q.x * self, q.y * self, q.z * self, q.w * self)
    }
}

/// Division by a scalar. Like C++, this multiplies by `1.0 / s`, so the
/// rounding matches the C++ result.
impl Div<f64> for Quaternion {
    type Output = Quaternion;
    fn div(self, s: f64) -> Quaternion {
        let inv = 1.0 / s;
        self * inv
    }
}

/// `printf("%f")` for a double. Non-finite values print as in glibc.
fn fmt_f(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    format!("{v:.6}")
}

/// Removes trailing zeros after a decimal point, and then the point itself.
fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// `printf("%g")` with the default precision of 6.
fn fmt_g(v: f64) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan" } else { "nan" }.to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    // Round to 6 significant digits first; the exponent is taken after
    // rounding, as C does.
    let sci = format!("{v:.5e}");
    let (mantissa, exp) = sci.split_once('e').expect("exponent present");
    let exp: i32 = exp.parse().expect("exponent is an integer");
    if !(-4..6).contains(&exp) {
        format!("{}e{}{:02}", trim_zeros(mantissa), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (5 - exp) as usize;
        trim_zeros(&format!("{v:.decimals$}"))
    }
}

/// Same output as the C++ `ToString()`:
/// `[x y z w], v:[vx vy vz], θ:<angle>π`, with the `%f`, `%g` formats above.
impl fmt::Display for Quaternion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // q = (cos(|v_theta|/2), v_theta/|v_theta| * sin(|v_theta|/2))
        let abs_theta = (self.w.acos() * 2.0) as f32;
        let scale = (1.0 / (abs_theta as f64 * 0.5).sin()) as f32;
        let mut v = Vec3::new(self.x as f32, self.y as f32, self.z as f32);
        v.scale(scale);
        let pi = std::f32::consts::PI;
        write!(
            f,
            "[{} {} {} {}], v:[{} {} {}], θ:{}π",
            fmt_f(self.x),
            fmt_f(self.y),
            fmt_f(self.z),
            fmt_f(self.w),
            fmt_g(v.x as f64),
            fmt_g(v.y as f64),
            fmt_g(v.z as f64),
            fmt_f((abs_theta / pi) as f64),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON_D: f64 = 1e-7;

    /// Mirrors gtest `EXPECT_FLOAT_EQ`: both values are rounded to float and
    /// must be within 4 ULPs.
    fn expect_float_eq(actual: f64, expected: f64) {
        assert!(
            almost_equal_f32(actual as f32, expected as f32),
            "expected {expected} (float) got {actual} (float)"
        );
    }

    /// gtest `FloatingPoint<float>::AlmostEquals` with kMaxUlps = 4.
    fn almost_equal_f32(a: f32, b: f32) -> bool {
        if a.is_nan() || b.is_nan() {
            return false;
        }
        // Map the sign-and-magnitude bit pattern to an ordered integer, so
        // that -0.0 and +0.0 are equal and the distance counts representable
        // floats.
        fn ordinal(x: f32) -> i64 {
            let bits = x.to_bits();
            if bits & 0x8000_0000 != 0 {
                -((bits & 0x7fff_ffff) as i64)
            } else {
                bits as i64
            }
        }
        (ordinal(a) - ordinal(b)).abs() <= 4
    }

    /// Mirrors the C++ `CompareQuaternions` helper.
    fn compare_quaternions(a: Quaternion, b: Quaternion) {
        expect_float_eq(a.x(), b.x());
        expect_float_eq(a.y(), b.y());
        expect_float_eq(a.z(), b.z());
        expect_float_eq(a.w(), b.w());
    }

    /// Mirrors the C++ `EXPECT_QUATERNION` macro (absolute error 1e-7).
    fn expect_quaternion(expected: Quaternion, actual: Quaternion) {
        for (e, a) in [
            (expected.x(), actual.x()),
            (expected.y(), actual.y()),
            (expected.z(), actual.z()),
            (expected.w(), actual.w()),
        ] {
            assert!((e - a).abs() <= EPSILON_D, "expected {e} got {a}");
        }
    }

    fn expect_near(actual: f64, expected: f64, epsilon: f64) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected} +/- {epsilon}, got {actual}"
        );
    }

    #[test]
    fn default_construction() {
        compare_quaternions(Quaternion::new(0.0, 0.0, 0.0, 1.0), Quaternion::default());
    }

    #[test]
    fn axis_angle_common() {
        let radians = 0.5;
        let q = Quaternion::from_axis([1.0, 0.0, 0.0], radians);
        compare_quaternions(
            Quaternion::new((radians / 2.0).sin(), 0.0, 0.0, (radians / 2.0).cos()),
            q,
        );
    }

    #[test]
    fn vector_to_vector_rotation() {
        let q = Quaternion::from_vectors([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let r = Quaternion::from_axis([0.0, 0.0, 1.0], std::f32::consts::PI as f64 / 2.0);

        expect_float_eq(r.x(), q.x());
        expect_float_eq(r.y(), q.y());
        expect_float_eq(r.z(), q.z());
        expect_float_eq(r.w(), q.w());
    }

    #[test]
    fn axis_angle_with_zero_length_axis() {
        let q = Quaternion::from_axis([0.0, 0.0, 0.0], 0.5);
        // If the axis of zero length, we should assume the default values.
        compare_quaternions(q, Quaternion::default());
    }

    #[test]
    fn addition() {
        for v in [0.0f32, 1.0, 100.0] {
            // The C++ test computes `2 * t` etc. in float.
            let t = v;
            let a = Quaternion::new(t as f64, (2.0 * t) as f64, (3.0 * t) as f64, (4.0 * t) as f64);
            let b = Quaternion::new(
                (5.0 * t) as f64,
                (4.0 * t) as f64,
                (3.0 * t) as f64,
                (2.0 * t) as f64,
            );
            let sum = a + b;
            let tq = t as f64;
            compare_quaternions(Quaternion::new(tq, tq, tq, tq) * 6.0, sum);
        }
    }

    #[test]
    fn multiplication() {
        let cases = [
            (
                Quaternion::new(1.0, 0.0, 0.0, 0.0),
                Quaternion::new(1.0, 0.0, 0.0, 0.0),
                Quaternion::new(0.0, 0.0, 0.0, -1.0),
            ),
            (
                Quaternion::new(0.0, 1.0, 0.0, 0.0),
                Quaternion::new(0.0, 1.0, 0.0, 0.0),
                Quaternion::new(0.0, 0.0, 0.0, -1.0),
            ),
            (
                Quaternion::new(0.0, 0.0, 1.0, 0.0),
                Quaternion::new(0.0, 0.0, 1.0, 0.0),
                Quaternion::new(0.0, 0.0, 0.0, -1.0),
            ),
            (
                Quaternion::new(0.0, 0.0, 0.0, 1.0),
                Quaternion::new(0.0, 0.0, 0.0, 1.0),
                Quaternion::new(0.0, 0.0, 0.0, 1.0),
            ),
            (
                Quaternion::new(1.0, 2.0, 3.0, 4.0),
                Quaternion::new(5.0, 6.0, 7.0, 8.0),
                Quaternion::new(24.0, 48.0, 48.0, -6.0),
            ),
            (
                Quaternion::new(5.0, 6.0, 7.0, 8.0),
                Quaternion::new(1.0, 2.0, 3.0, 4.0),
                Quaternion::new(32.0, 32.0, 56.0, -6.0),
            ),
        ];

        for (a, b, expected) in cases {
            let product = a * b;
            compare_quaternions(expected, product);
        }
    }

    #[test]
    fn scaling() {
        for s in [0.0f64, 10.0, 100.0] {
            let q = Quaternion::new(1.0, 2.0, 3.0, 4.0);
            let expected = Quaternion::new(s, 2.0 * s, 3.0 * s, 4.0 * s);
            compare_quaternions(expected, q * s);
            compare_quaternions(expected, s * q);
            if s > 0.0 {
                compare_quaternions(expected, q / (1.0 / s));
            }
        }
    }

    #[test]
    fn normalization() {
        let q = Quaternion::new(1.0, -1.0, 1.0, -1.0);
        expect_near(q.length(), 4.0, EPSILON_D);

        let q = q.normalized();

        expect_near(q.length(), 1.0, EPSILON_D);
        expect_near(q.x(), 0.5, EPSILON_D);
        expect_near(q.y(), -0.5, EPSILON_D);
        expect_near(q.z(), 0.5, EPSILON_D);
        expect_near(q.w(), -0.5, EPSILON_D);
    }

    #[test]
    fn lerp() {
        for i in 1..100usize {
            let a = Quaternion::new(0.0, 0.0, 0.0, 0.0);
            let b = Quaternion::new(1.0, 2.0, 3.0, 4.0);
            let t = i as f32 / 100.0f32;
            let interpolated = a.lerp(b, t as f64);
            let s = 1.0 / 30.0f64.sqrt();
            compare_quaternions(Quaternion::new(1.0, 2.0, 3.0, 4.0) * s, interpolated);
        }

        let a = Quaternion::new(4.0, 3.0, 2.0, 1.0);
        let b = Quaternion::new(1.0, 2.0, 3.0, 4.0);
        compare_quaternions(a.normalized(), a.lerp(b, 0.0));
        compare_quaternions(b.normalized(), a.lerp(b, 1.0));
        compare_quaternions(Quaternion::new(1.0, 1.0, 1.0, 1.0).normalized(), a.lerp(b, 0.5));
    }

    #[test]
    fn slerp() {
        let axis = [1.0f32, 1.0, 1.0];
        let start_radians = -0.5;
        let stop_radians = 0.5;
        let start = Quaternion::from_axis(axis, start_radians);
        let stop = Quaternion::from_axis(axis, stop_radians);

        for i in 0..100usize {
            let t = i as f32 / 100.0f32;
            let radians = (1.0 - t as f64) * start_radians + t as f64 * stop_radians;
            let expected = Quaternion::from_axis(axis, radians);
            let interpolated = start.slerp(stop, t as f64);
            expect_quaternion(expected, interpolated);
        }
    }

    #[test]
    fn slerp_opposite_angles() {
        let axis = [1.0f32, 1.0, 1.0];
        let start_radians = -std::f64::consts::PI / 2.0;
        let stop_radians = std::f64::consts::PI / 2.0;
        let start = Quaternion::from_axis(axis, start_radians);
        let stop = Quaternion::from_axis(axis, stop_radians);

        // When quaternions are pointed in the fully opposite direction, this is
        // ambiguous, so we rotate as per https://www.w3.org/TR/css-transforms-1/
        let expected = Quaternion::from_axis(axis, 0.0);

        let interpolated = start.slerp(stop, 0.5);
        expect_quaternion(expected, interpolated);
    }

    #[test]
    fn slerp_rotate_x_rotate_y() {
        let start = Quaternion::from_axis([1.0, 0.0, 0.0], std::f64::consts::PI / 2.0);
        let stop = Quaternion::from_axis([0.0, 1.0, 0.0], std::f64::consts::PI / 2.0);
        let interpolated = start.slerp(stop, 0.5);

        let expected_angle = (1.0f64 / 3.0).acos();
        let xy = (0.5 * expected_angle).sin() / 2.0f64.sqrt();
        let expected = Quaternion::new(xy, xy, 0.0, (0.5 * expected_angle).cos());
        expect_quaternion(expected, interpolated);
    }

    #[test]
    fn slerp360() {
        let start = Quaternion::new(0.0, 0.0, 0.0, -1.0); // 360 degree rotation.
        let stop = Quaternion::from_axis([0.0, 0.0, 1.0], std::f64::consts::PI / 2.0);
        let interpolated = start.slerp(stop, 0.5);
        let expected_half_angle = std::f64::consts::PI / 8.0;
        let expected =
            Quaternion::new(0.0, 0.0, expected_half_angle.sin(), expected_half_angle.cos());
        expect_quaternion(expected, interpolated);
    }

    #[test]
    fn slerp_equivalent_quaternions() {
        let start = Quaternion::from_axis([1.0, 0.0, 0.0], std::f64::consts::PI / 3.0);
        let stop = start.flip();
        let interpolated = start.slerp(stop, 0.5);
        expect_quaternion(start, interpolated);
    }

    #[test]
    fn slerp_quaternion_with_inverse() {
        let start = Quaternion::from_axis([1.0, 0.0, 0.0], std::f64::consts::PI / 3.0);
        let stop = start.inverse();
        let interpolated = start.slerp(stop, 0.5);
        let expected = Quaternion::new(0.0, 0.0, 0.0, 1.0);
        expect_quaternion(expected, interpolated);
    }

    #[test]
    fn slerp_obtuse_angle() {
        let start = Quaternion::from_axis([1.0, 1.0, 0.0], std::f64::consts::PI / 2.0);
        let stop = Quaternion::from_axis([0.0, 1.0, -1.0], 3.0 * std::f64::consts::PI / 2.0);
        let interpolated = start.slerp(stop, 0.5);
        let expected_half_angle = -(0.5f64).atan();
        let xz = expected_half_angle.sin() / 2.0f64.sqrt();
        let expected = Quaternion::new(xz, 0.0, xz, -expected_half_angle.cos());
        expect_quaternion(expected, interpolated);
    }

    #[test]
    fn equals() {
        assert!(Quaternion::default() == Quaternion::default());
        assert!(Quaternion::default() == Quaternion::new(0.0, 0.0, 0.0, 1.0));
        assert!(Quaternion::new(1.0, 5.2, -8.5, 222.2) == Quaternion::new(1.0, 5.2, -8.5, 222.2));
        assert!(Quaternion::default() != Quaternion::new(1.0, 0.0, 0.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(0.0, 1.0, 0.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(0.0, 0.0, 1.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn not_equals() {
        assert!(!(Quaternion::default() != Quaternion::default()));
        assert!(
            !(Quaternion::new(1.0, 5.2, -8.5, 222.2) != Quaternion::new(1.0, 5.2, -8.5, 222.2))
        );
        assert!(Quaternion::default() != Quaternion::new(1.0, 0.0, 0.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(0.0, 1.0, 0.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(0.0, 0.0, 1.0, 0.0));
        assert!(Quaternion::default() != Quaternion::new(1.0, 0.0, 0.0, 1.0));
    }

    // Extra tests (not in the C++ suite) below.

    /// Small xorshift PRNG for the randomized tests.
    struct XorShift64(u64);

    impl XorShift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn next_unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }

        fn next_range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next_unit()
        }

        /// Random unit quaternion (uniform on S^3 via normalized Gaussians is
        /// overkill here; uniform components then normalize is enough).
        fn next_unit_quaternion(&mut self) -> Quaternion {
            loop {
                let q = Quaternion::new(
                    self.next_range(-1.0, 1.0),
                    self.next_range(-1.0, 1.0),
                    self.next_range(-1.0, 1.0),
                    self.next_range(-1.0, 1.0),
                );
                if q.length() > 0.01 {
                    return q.normalized();
                }
            }
        }
    }

    /// Rotates `v` by `q` via `q * (v, 0) * q^-1`, for unit `q`.
    fn rotate(q: Quaternion, v: [f64; 3]) -> [f64; 3] {
        let p = q * Quaternion::new(v[0], v[1], v[2], 0.0) * q.inverse();
        [p.x(), p.y(), p.z()]
    }

    fn expect_vec_near(a: [f64; 3], b: [f64; 3], eps: f64) {
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() <= eps, "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn product_composes_rotations_random() {
        // rotate(q1 * q2, v) == rotate(q1, rotate(q2, v)) for unit quaternions.
        let mut rng = XorShift64(0x1234_5678_9abc_def1);
        for _ in 0..1000 {
            let q1 = rng.next_unit_quaternion();
            let q2 = rng.next_unit_quaternion();
            let v =
                [rng.next_range(-1.0, 1.0), rng.next_range(-1.0, 1.0), rng.next_range(-1.0, 1.0)];
            expect_vec_near(rotate(q1 * q2, v), rotate(q1, rotate(q2, v)), 1e-12);
        }
    }

    #[test]
    fn product_is_associative_random() {
        let mut rng = XorShift64(0x0bad_cafe_f00d_1111);
        for _ in 0..1000 {
            let a = rng.next_unit_quaternion();
            let b = rng.next_unit_quaternion();
            let c = rng.next_unit_quaternion();
            let l = (a * b) * c;
            let r = a * (b * c);
            expect_quaternion(l, r);
        }
    }

    #[test]
    fn axis_angle_rotates_like_rodrigues_random() {
        // Compare against the Rodrigues formula. Uses the same f32-rounded
        // axis that from_axis sees, so the reference matches exactly.
        let mut rng = XorShift64(0x7777_0000_1111_2222);
        for _ in 0..500 {
            let axis = [
                rng.next_range(-2.0, 2.0) as f32,
                rng.next_range(-2.0, 2.0) as f32,
                rng.next_range(-2.0, 2.0) as f32,
            ];
            let angle = rng.next_range(-6.0, 6.0);
            let q = Quaternion::from_axis(axis, angle);
            let n64 = [axis[0] as f64, axis[1] as f64, axis[2] as f64];
            let len = (n64[0] * n64[0] + n64[1] * n64[1] + n64[2] * n64[2]).sqrt();
            let n = [n64[0] / len, n64[1] / len, n64[2] / len];
            let v =
                [rng.next_range(-1.0, 1.0), rng.next_range(-1.0, 1.0), rng.next_range(-1.0, 1.0)];
            let (c, s) = (angle.cos(), angle.sin());
            let nv = n[0] * v[0] + n[1] * v[1] + n[2] * v[2];
            let cross =
                [n[1] * v[2] - n[2] * v[1], n[2] * v[0] - n[0] * v[2], n[0] * v[1] - n[1] * v[0]];
            let expected = [
                v[0] * c + cross[0] * s + n[0] * nv * (1.0 - c),
                v[1] * c + cross[1] * s + n[1] * nv * (1.0 - c),
                v[2] * c + cross[2] * s + n[2] * nv * (1.0 - c),
            ];
            // The axis is rounded to f32 before normalizing, so allow 1e-6.
            expect_vec_near(rotate(q, v), expected, 1e-6);
        }
    }

    #[test]
    fn slerp_endpoints_and_unit_length_random() {
        let mut rng = XorShift64(0x3141_5926_5358_9793);
        for _ in 0..1000 {
            let a = rng.next_unit_quaternion();
            let mut b = rng.next_unit_quaternion();
            // Make the shorter path explicit: slerp keeps the endpoints in the
            // same hemisphere.
            if a.x() * b.x() + a.y() * b.y() + a.z() * b.z() + a.w() * b.w() < 0.0 {
                b = b.flip();
            }
            expect_quaternion(a, a.slerp(b, 0.0));
            expect_quaternion(b, a.slerp(b, 1.0));
            let t = rng.next_unit();
            let m = a.slerp(b, t);
            expect_near(m.length(), 1.0, 1e-9);
        }
    }

    #[test]
    fn normalized_is_unit_random() {
        let mut rng = XorShift64(0x2718_2818_2845_9045);
        for _ in 0..1000 {
            let q = Quaternion::new(
                rng.next_range(-100.0, 100.0),
                rng.next_range(-100.0, 100.0),
                rng.next_range(-100.0, 100.0),
                rng.next_range(-100.0, 100.0),
            );
            expect_near(q.normalized().length(), 1.0, 1e-12);
        }
    }

    #[test]
    fn normalized_leaves_tiny_quaternion_unchanged() {
        // Squared length below 1e-5 is returned as is.
        let q = Quaternion::new(1e-4, 0.0, 0.0, 0.0);
        assert_eq!(q.normalized(), q);
    }

    #[test]
    fn from_vectors_opposite_directions_is_half_turn_perpendicular() {
        // Opposite vectors: the axis is perpendicular and the real part is 0,
        // so the result is a 180 degree rotation.
        let q = Quaternion::from_vectors([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]);
        expect_near(q.w(), 0.0, 1e-12);
        expect_near(q.length(), 1.0, 1e-12);
        // Rotating x by q gives -x.
        expect_vec_near(rotate(q, [1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0], 1e-6);
    }

    #[test]
    fn from_axis_angle_matches_from_axis_and_zero_axis_is_identity() {
        let q1 = Quaternion::from_axis_angle(0.0, 0.0, 2.0, 1.0);
        let q2 = Quaternion::from_axis([0.0, 0.0, 2.0], 1.0);
        expect_quaternion(q1, q2);
        assert_eq!(Quaternion::from_axis_angle(0.0, 0.0, 0.0, 3.0), Quaternion::default());
    }

    #[test]
    fn display_matches_printf_format() {
        // Rotation of pi/2 about x: w = cos(pi/4), x = sin(pi/4).
        let q = Quaternion::from_axis_angle(1.0, 0.0, 0.0, std::f64::consts::FRAC_PI_2);
        assert_eq!(q.to_string(), "[0.707107 0.000000 0.000000 0.707107], v:[1 0 0], θ:0.500000π");
    }

    #[test]
    fn fmt_g_matches_printf() {
        assert_eq!(fmt_g(0.0), "0");
        assert_eq!(fmt_g(1.0), "1");
        assert_eq!(fmt_g(-2.5), "-2.5");
        assert_eq!(fmt_g(123456.0), "123456");
        assert_eq!(fmt_g(1234567.0), "1.23457e+06");
        assert_eq!(fmt_g(0.0001), "0.0001");
        assert_eq!(fmt_g(0.00001), "1e-05");
    }

    #[test]
    fn cross_and_dot_match_reference() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-4.0, 5.0, 0.5);
        assert_eq!(a.dot(b), -4.0 + 10.0 + 1.5);
        assert_eq!(
            a.cross(b),
            Vec3::new(2.0 * 0.5 - 3.0 * 5.0, 3.0 * -4.0 - 1.0 * 0.5, 1.0 * 5.0 - 2.0 * -4.0)
        );
        assert_eq!(
            Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0)),
            Vec3::new(0.0, 0.0, 1.0)
        );
    }
}
