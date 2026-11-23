// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `gfx::Vector3dF` from `vector3d_f.{h,cc}`.

use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::format::format_g;
use crate::vector::Vector2dF;

/// A float 3D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector3dF {
    x: f32,
    y: f32,
    z: f32,
}

/// Vectors shorter than this (in double) are not normalized.
const NORMALIZE_EPSILON: f64 = 1.0e-6;

impl Vector3dF {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vector3dF { x, y, z }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    pub fn set_x(&mut self, x: f32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    pub fn set_z(&mut self, z: f32) {
        self.z = z;
    }

    /// True if all components are exactly 0.
    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }

    pub fn add(&mut self, other: Vector3dF) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }

    pub fn subtract(&mut self, other: Vector3dF) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }

    /// Component-wise minimum, using the C++ `a <= b ? a : b` form. This keeps
    /// `self` when NaN is involved, as the C++ does.
    pub fn set_to_min(&mut self, other: Vector3dF) {
        self.x = if self.x <= other.x { self.x } else { other.x };
        self.y = if self.y <= other.y { self.y } else { other.y };
        self.z = if self.z <= other.z { self.z } else { other.z };
    }

    /// Component-wise maximum, using the C++ `a >= b ? a : b` form.
    pub fn set_to_max(&mut self, other: Vector3dF) {
        self.x = if self.x >= other.x { self.x } else { other.x };
        self.y = if self.y >= other.y { self.y } else { other.y };
        self.z = if self.z >= other.z { self.z } else { other.z };
    }

    /// The square of the diagonal length, in double.
    pub fn length_squared(self) -> f64 {
        self.x as f64 * self.x as f64
            + self.y as f64 * self.y as f64
            + self.z as f64 * self.z as f64
    }

    /// The diagonal length, computed as `sqrt` in double and rounded to float.
    pub fn length(self) -> f32 {
        self.length_squared().sqrt() as f32
    }

    pub fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    pub fn scale_xyz(&mut self, x_scale: f32, y_scale: f32, z_scale: f32) {
        self.x *= x_scale;
        self.y *= y_scale;
        self.z *= z_scale;
    }

    pub fn inv_scale(&mut self, inv_scale: f32) {
        self.inv_scale_xyz(inv_scale, inv_scale, inv_scale);
    }

    pub fn inv_scale_xyz(&mut self, inv_x_scale: f32, inv_y_scale: f32, inv_z_scale: f32) {
        self.x /= inv_x_scale;
        self.y /= inv_y_scale;
        self.z /= inv_z_scale;
    }

    /// Replaces `self` with the cross product `self x other`. The products are
    /// computed in double and each result is rounded to float.
    pub fn cross(&mut self, other: Vector3dF) {
        let dx = self.x as f64;
        let dy = self.y as f64;
        let dz = self.z as f64;
        let x = (dy * other.z as f64 - dz * other.y as f64) as f32;
        let y = (dz * other.x as f64 - dx * other.z as f64) as f32;
        let z = (dx * other.y as f64 - dy * other.x as f64) as f32;
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Returns the unit vector in the direction of `self`, or `None` when
    /// `self` is too short to normalize reliably (squared length below
    /// `1e-12`). This is `GetNormalized`, whose output parameter is the
    /// result on success.
    pub fn get_normalized(self) -> Option<Vector3dF> {
        let length_squared = self.length_squared();
        if length_squared < NORMALIZE_EPSILON * NORMALIZE_EPSILON {
            return None;
        }
        let mut out = self;
        out.inv_scale(length_squared.sqrt() as f32);
        Some(out)
    }
}

impl fmt::Display for Vector3dF {
    /// `"[%g %g %g]"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} {} {}]",
            format_g(self.x as f64),
            format_g(self.y as f64),
            format_g(self.z as f64)
        )
    }
}

impl From<Vector2dF> for Vector3dF {
    /// The explicit `Vector3dF(const Vector2dF&)` conversion, with z = 0.
    fn from(v: Vector2dF) -> Vector3dF {
        Vector3dF::new(v.x(), v.y(), 0.0)
    }
}

impl AddAssign for Vector3dF {
    fn add_assign(&mut self, other: Vector3dF) {
        self.add(other);
    }
}

impl SubAssign for Vector3dF {
    fn sub_assign(&mut self, other: Vector3dF) {
        self.subtract(other);
    }
}

impl Add for Vector3dF {
    type Output = Vector3dF;
    fn add(mut self, rhs: Vector3dF) -> Vector3dF {
        Vector3dF::add(&mut self, rhs);
        self
    }
}

impl Sub for Vector3dF {
    type Output = Vector3dF;
    fn sub(mut self, rhs: Vector3dF) -> Vector3dF {
        self.subtract(rhs);
        self
    }
}

impl Neg for Vector3dF {
    type Output = Vector3dF;
    fn neg(self) -> Vector3dF {
        Vector3dF::new(-self.x, -self.y, -self.z)
    }
}

/// The 3D dot product, computed in float.
pub fn dot_product_3d(lhs: Vector3dF, rhs: Vector3dF) -> f32 {
    lhs.x * rhs.x + lhs.y * rhs.y + lhs.z * rhs.z
}

/// The 3D cross product `lhs x rhs`.
pub fn cross_product_3d(lhs: Vector3dF, rhs: Vector3dF) -> Vector3dF {
    let mut result = lhs;
    result.cross(rhs);
    result
}

/// Scales each component of `v` by the matching factor.
pub fn scale_vector3d(v: Vector3dF, x_scale: f32, y_scale: f32, z_scale: f32) -> Vector3dF {
    let mut scaled = v;
    scaled.scale_xyz(x_scale, y_scale, z_scale);
    scaled
}

/// Scales each component of `v` by the matching component of `s`.
pub fn scale_vector3d_by(v: Vector3dF, s: Vector3dF) -> Vector3dF {
    scale_vector3d(v, s.x(), s.y(), s.z())
}

pub fn scale_vector3d_uniform(v: Vector3dF, scale: f32) -> Vector3dF {
    scale_vector3d(v, scale, scale, scale)
}

/// The angle between `base` and `other`, in degrees.
///
/// The cosine is clamped to `[-1, 1]` with `fmin`/`fmax` (so NaN becomes 1)
/// before `acos`, which keeps floating-point error from producing NaN.
pub fn angle_between_vectors_in_degrees(base: Vector3dF, other: Vector3dF) -> f32 {
    let cos = dot_product_3d(base, other) / base.length() / other.length();
    // `f32::min`/`max` (not `clamp`) so that NaN becomes 1, as with `fmin`/`fmax`.
    #[allow(clippy::manual_clamp)]
    let clamped = cos.min(1.0).max(-1.0);
    rad_to_deg(clamped.acos())
}

/// The clockwise angle from `base` to `other`, in degrees, viewed from the
/// side that `normal` points toward. The result is in `[0, 360)`.
pub fn clockwise_angle_between_vectors_in_degrees(
    base: Vector3dF,
    other: Vector3dF,
    normal: Vector3dF,
) -> f32 {
    let mut angle = angle_between_vectors_in_degrees(base, other);
    let cross = cross_product_3d(base, other);
    // If the cross product points along `normal`, the shortest angle is
    // counterclockwise with respect to `normal`, so reverse it.
    if dot_product_3d(cross, normal) > 0.0 {
        angle = 360.0 - angle;
    }
    angle
}

/// `base::RadToDeg<float>`: `rad * (180 / pi)`, evaluated in float.
fn rad_to_deg(rad: f32) -> f32 {
    rad * (180.0 / std::f32::consts::PI)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{double_eq, float_eq, XorShift64};

    // ---- vector3d_f_unittest.cc -------------------------------------------

    #[test]
    fn vector3d_f_test_is_zero() {
        let float_zero = Vector3dF::new(0.0, 0.0, 0.0);
        let float_nonzero = Vector3dF::new(0.1, -0.1, 0.1);
        assert!(float_zero.is_zero());
        assert!(!float_nonzero.is_zero());
    }

    #[test]
    fn vector3d_f_test_add() {
        let f1 = Vector3dF::new(3.1, 5.1, 2.7);
        let f2 = Vector3dF::new(4.3, -1.3, 8.1);
        let float_tests = [
            (Vector3dF::new(3.1, 5.1, 2.7), f1 + Vector3dF::default()),
            (Vector3dF::new(3.1 + 4.3, 5.1 - 1.3, 2.7 + 8.1), f1 + f2),
            (Vector3dF::new(3.1 - 4.3, 5.1 + 1.3, 2.7 - 8.1), f1 - f2),
        ];
        for (expected, actual) in float_tests {
            assert_eq!(expected.to_string(), actual.to_string());
        }
    }

    #[test]
    fn vector3d_f_test_negative() {
        let float_tests = [
            (Vector3dF::new(-0.0, -0.0, -0.0), -Vector3dF::new(0.0, 0.0, 0.0)),
            (Vector3dF::new(-0.3, -0.3, -0.3), -Vector3dF::new(0.3, 0.3, 0.3)),
            (Vector3dF::new(0.3, 0.3, 0.3), -Vector3dF::new(-0.3, -0.3, -0.3)),
            (Vector3dF::new(-0.3, 0.3, -0.3), -Vector3dF::new(0.3, -0.3, 0.3)),
            (Vector3dF::new(0.3, -0.3, -0.3), -Vector3dF::new(-0.3, 0.3, 0.3)),
            (Vector3dF::new(-0.3, -0.3, 0.3), -Vector3dF::new(0.3, 0.3, -0.3)),
        ];
        for (expected, actual) in float_tests {
            assert_eq!(expected.to_string(), actual.to_string());
        }
    }

    #[test]
    fn vector3d_f_test_scale() {
        // Each row is {x, y, z, sx, sy, sz}.
        let triple_values: [[f32; 6]; 20] = [
            [4.5, 1.2, 1.8, 3.3, 5.6, 4.2],
            [4.5, -1.2, -1.8, 3.3, 5.6, 4.2],
            [4.5, 1.2, -1.8, 3.3, 5.6, 4.2],
            // The C++ initializer has five values here, so the sixth is 0.
            [4.5, -1.2 - 1.8, 3.3, 5.6, 4.2, 0.0],
            [4.5, 1.2, 1.8, 3.3, -5.6, -4.2],
            [4.5, 1.2, 1.8, -3.3, -5.6, -4.2],
            [4.5, 1.2, -1.8, 3.3, -5.6, -4.2],
            [4.5, 1.2, -1.8, -3.3, -5.6, -4.2],
            [-4.5, 1.2, 1.8, 3.3, 5.6, 4.2],
            [-4.5, 1.2, 1.8, 0.0, 5.6, 4.2],
            [-4.5, 1.2, -1.8, 3.3, 5.6, 4.2],
            [-4.5, 1.2, -1.8, 0.0, 5.6, 4.2],
            [-4.5, 1.2, 1.8, 3.3, 0.0, 4.2],
            [4.5, 0.0, 1.8, 3.3, 5.6, 4.2],
            [-4.5, 1.2, -1.8, 3.3, 0.0, 4.2],
            [4.5, 0.0, -1.8, 3.3, 5.6, 4.2],
            [-4.5, 1.2, 1.8, 3.3, 5.6, 0.0],
            [-4.5, 1.2, -1.8, 3.3, 5.6, 0.0],
            [0.0, 1.2, 0.0, 3.3, 5.6, 4.2],
            [0.0, 1.2, 1.8, 3.3, 5.6, 4.2],
        ];
        for t in triple_values {
            let mut v = Vector3dF::new(t[0], t[1], t[2]);
            v.scale_xyz(t[3], t[4], t[5]);
            assert_eq!(t[0] * t[3], v.x());
            assert_eq!(t[1] * t[4], v.y());
            assert_eq!(t[2] * t[5], v.z());
            let v2 = scale_vector3d(Vector3dF::new(t[0], t[1], t[2]), t[3], t[4], t[5]);
            assert_eq!(t[0] * t[3], v2.x());
            assert_eq!(t[1] * t[4], v2.y());
            assert_eq!(t[2] * t[5], v2.z());
        }

        // Each row is {x, y, z, s}.
        let single_values: [[f32; 4]; 14] = [
            [4.5, 1.2, 1.8, 3.3],
            [4.5, -1.2, 1.8, 3.3],
            [4.5, 1.2, -1.8, 3.3],
            [4.5, -1.2, -1.8, 3.3],
            // The C++ initializer has three values here, so the fourth is 0.
            [-4.5, 1.2, 3.3, 0.0],
            [-4.5, 1.2, 0.0, 0.0],
            [-4.5, 1.2, 1.8, 3.3],
            [-4.5, 1.2, 1.8, 0.0],
            [4.5, 0.0, 1.8, 3.3],
            [0.0, 1.2, 1.8, 3.3],
            [4.5, 0.0, 1.8, 3.3],
            [0.0, 1.2, 1.8, 3.3],
            [4.5, 1.2, 0.0, 3.3],
            [4.5, 1.2, 0.0, 3.3],
        ];
        for t in single_values {
            let mut v = Vector3dF::new(t[0], t[1], t[2]);
            v.scale(t[3]);
            assert_eq!(t[0] * t[3], v.x());
            assert_eq!(t[1] * t[3], v.y());
            assert_eq!(t[2] * t[3], v.z());
            let v2 = scale_vector3d_uniform(Vector3dF::new(t[0], t[1], t[2]), t[3]);
            assert_eq!(t[0] * t[3], v2.x());
            assert_eq!(t[1] * t[3], v2.y());
            assert_eq!(t[2] * t[3], v2.z());
        }
    }

    #[test]
    fn vector3d_f_test_length() {
        let float_values: [[f32; 3]; 14] = [
            [0.0, 0.0, 0.0],
            [10.5, 20.5, 8.5],
            [20.5, 10.5, 8.5],
            [8.5, 20.5, 10.5],
            [10.5, 8.5, 20.5],
            [-10.5, -20.5, -8.5],
            [-20.5, 10.5, -8.5],
            [-8.5, -20.5, -10.5],
            [-10.5, -8.5, -20.5],
            [10.5, -20.5, 8.5],
            [-10.5, 20.5, 8.5],
            [10.5, -20.5, -8.5],
            [-10.5, 20.5, -8.5],
            // A large vector that fails if the Length function doesn't use
            // double precision internally.
            [
                1236278317862780234892374893213178027.12122348904204230,
                335890352589839028212313231225425134332.38123,
                27861786423846742743236423478236784678.236713617231,
            ],
        ];
        for f in float_values {
            let v0 = f[0] as f64;
            let v1 = f[1] as f64;
            let v2 = f[2] as f64;
            let length_squared = v0 * v0 + v1 * v1 + v2 * v2;
            let length = length_squared.sqrt();
            let vector = Vector3dF::new(f[0], f[1], f[2]);
            assert!(double_eq(length_squared, vector.length_squared()));
            assert!(float_eq(length as f32, vector.length()));
        }
    }

    #[test]
    fn vector3d_f_test_dot_product() {
        let tests = [
            (0.0, Vector3dF::new(1.0, 0.0, 0.0), Vector3dF::new(0.0, 1.0, 1.0)),
            (0.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(1.0, 0.0, 1.0)),
            (0.0, Vector3dF::new(0.0, 0.0, 1.0), Vector3dF::new(1.0, 1.0, 0.0)),
            (3.0, Vector3dF::new(1.0, 1.0, 1.0), Vector3dF::new(1.0, 1.0, 1.0)),
            (1.2, Vector3dF::new(1.2, -1.2, 1.2), Vector3dF::new(1.0, 1.0, 1.0)),
            (1.2, Vector3dF::new(1.0, 1.0, 1.0), Vector3dF::new(1.2, -1.2, 1.2)),
            (38.72, Vector3dF::new(1.1, 2.2, 3.3), Vector3dF::new(4.4, 5.5, 6.6)),
        ];
        for (expected, input1, input2) in tests {
            let actual = dot_product_3d(input1, input2);
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn vector3d_f_test_cross_product() {
        let tests = [
            (Vector3dF::default(), Vector3dF::default(), Vector3dF::new(1.0, 1.0, 1.0)),
            (Vector3dF::default(), Vector3dF::new(1.0, 1.0, 1.0), Vector3dF::default()),
            (Vector3dF::default(), Vector3dF::new(1.0, 1.0, 1.0), Vector3dF::new(1.0, 1.0, 1.0)),
            (
                Vector3dF::default(),
                Vector3dF::new(1.6, 10.6, -10.6),
                Vector3dF::new(1.6, 10.6, -10.6),
            ),
            (
                Vector3dF::new(1.0, -1.0, 0.0),
                Vector3dF::new(1.0, 1.0, 1.0),
                Vector3dF::new(0.0, 0.0, 1.0),
            ),
            (
                Vector3dF::new(-1.0, 0.0, 1.0),
                Vector3dF::new(1.0, 1.0, 1.0),
                Vector3dF::new(0.0, 1.0, 0.0),
            ),
            (
                Vector3dF::new(0.0, 1.0, -1.0),
                Vector3dF::new(1.0, 1.0, 1.0),
                Vector3dF::new(1.0, 0.0, 0.0),
            ),
            (
                Vector3dF::new(-1.0, 1.0, 0.0),
                Vector3dF::new(0.0, 0.0, 1.0),
                Vector3dF::new(1.0, 1.0, 1.0),
            ),
            (
                Vector3dF::new(1.0, 0.0, -1.0),
                Vector3dF::new(0.0, 1.0, 0.0),
                Vector3dF::new(1.0, 1.0, 1.0),
            ),
            (
                Vector3dF::new(0.0, -1.0, 1.0),
                Vector3dF::new(1.0, 0.0, 0.0),
                Vector3dF::new(1.0, 1.0, 1.0),
            ),
        ];
        for (i, (expected, input1, input2)) in tests.into_iter().enumerate() {
            let actual = cross_product_3d(input1, input2);
            assert_eq!(expected.to_string(), actual.to_string(), "case {i}");
        }
    }

    #[test]
    fn vector3d_f_test_clamp_vector3d_f() {
        let mut a = Vector3dF::new(3.5, 5.5, 7.5);
        assert_eq!(Vector3dF::new(3.5, 5.5, 7.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(2.0, 4.5, 6.5));
        assert_eq!(Vector3dF::new(3.5, 5.5, 7.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(3.5, 5.5, 7.5));
        assert_eq!(Vector3dF::new(3.5, 5.5, 7.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(4.5, 2.0, 6.5));
        assert_eq!(Vector3dF::new(4.5, 5.5, 7.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(3.5, 6.5, 6.5));
        assert_eq!(Vector3dF::new(4.5, 6.5, 7.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(3.5, 5.5, 8.5));
        assert_eq!(Vector3dF::new(4.5, 6.5, 8.5).to_string(), a.to_string());
        a.set_to_max(Vector3dF::new(8.5, 10.5, 12.5));
        assert_eq!(Vector3dF::new(8.5, 10.5, 12.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(9.5, 11.5, 13.5));
        assert_eq!(Vector3dF::new(8.5, 10.5, 12.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(8.5, 10.5, 12.5));
        assert_eq!(Vector3dF::new(8.5, 10.5, 12.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(7.5, 11.5, 13.5));
        assert_eq!(Vector3dF::new(7.5, 10.5, 12.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(9.5, 9.5, 13.5));
        assert_eq!(Vector3dF::new(7.5, 9.5, 12.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(9.5, 11.5, 11.5));
        assert_eq!(Vector3dF::new(7.5, 9.5, 11.5).to_string(), a.to_string());
        a.set_to_min(Vector3dF::new(3.5, 5.5, 7.5));
        assert_eq!(Vector3dF::new(3.5, 5.5, 7.5).to_string(), a.to_string());
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn vector3d_f_test_angle_between_vectors_in_degrees() {
        let tests = [
            (0.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, 1.0, 0.0)),
            (90.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, 0.0, 1.0)),
            (
                45.0,
                Vector3dF::new(0.0, 1.0, 0.0),
                Vector3dF::new(0.0, 0.70710678188, 0.70710678188),
            ),
            (180.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, -1.0, 0.0)),
            // Two vectors close enough together that the cosine can round
            // above 1 without the clamp, which would make acos return NaN.
            (
                0.0,
                Vector3dF::new(0.0, -0.990842, -0.003177),
                Vector3dF::new(0.0, -0.999995, -0.003124),
            ),
        ];
        for (expected, input1, input2) in tests {
            let actual = angle_between_vectors_in_degrees(input1, input2);
            assert!(float_eq(expected, actual), "{expected} vs {actual}");
            let actual = angle_between_vectors_in_degrees(input2, input1);
            assert!(float_eq(expected, actual), "{expected} vs {actual}");
        }
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn vector3d_f_test_clockwise_angle_between_vectors_in_degrees() {
        let tests = [
            (0.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, 1.0, 0.0)),
            (90.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, 0.0, -1.0)),
            (
                45.0,
                Vector3dF::new(0.0, -1.0, 0.0),
                Vector3dF::new(0.0, -0.70710678188, 0.70710678188),
            ),
            (180.0, Vector3dF::new(0.0, -1.0, 0.0), Vector3dF::new(0.0, 1.0, 0.0)),
            (270.0, Vector3dF::new(0.0, 1.0, 0.0), Vector3dF::new(0.0, 0.0, 1.0)),
        ];
        let normal_vector = Vector3dF::new(1.0, 0.0, 0.0);
        for (expected, input1, input2) in tests {
            let mut actual =
                clockwise_angle_between_vectors_in_degrees(input1, input2, normal_vector);
            assert!(float_eq(expected, actual), "{expected} vs {actual}");
            actual = -clockwise_angle_between_vectors_in_degrees(input2, input1, normal_vector);
            if actual < 0.0 {
                actual += 360.0;
            }
            assert!(float_eq(expected, actual), "{expected} vs {actual}");
        }
    }

    #[test]
    fn vector3d_f_test_get_normalized() {
        // (expected success, input, expected normalized). The normalized value
        // is only checked on success.
        let tests = [
            (false, Vector3dF::new(0.0, 0.0, 0.0), Vector3dF::new(0.0, 0.0, 0.0)),
            (
                false,
                Vector3dF::new(f32::MIN_POSITIVE, f32::MIN_POSITIVE, f32::MIN_POSITIVE),
                Vector3dF::new(f32::MIN_POSITIVE, f32::MIN_POSITIVE, f32::MIN_POSITIVE),
            ),
            (true, Vector3dF::new(1.0, 0.0, 0.0), Vector3dF::new(1.0, 0.0, 0.0)),
            (true, Vector3dF::new(f32::MAX, 0.0, 0.0), Vector3dF::new(1.0, 0.0, 0.0)),
        ];
        for (expected, v, normalized) in tests {
            match v.get_normalized() {
                Some(n) => {
                    assert!(expected, "unexpected success for {v}");
                    assert_eq!(normalized.to_string(), n.to_string());
                }
                None => {
                    assert!(!expected, "unexpected failure for {v}");
                }
            }
        }
    }

    #[test]
    fn vector3d_f_test_to_string() {
        assert_eq!("[1.03125 2.5 -3]", Vector3dF::new(1.03125, 2.5, -3.0).to_string());
    }

    // ---- Extra tests ------------------------------------------------------

    #[test]
    fn vector3d_f_test_from_vector2d_has_zero_z() {
        assert_eq!(Vector3dF::new(1.0, 2.0, 0.0), Vector3dF::from(Vector2dF::new(1.0, 2.0)));
    }

    #[test]
    fn vector3d_f_test_cross_is_orthogonal_to_inputs() {
        let mut rng = XorShift64::new(9);
        for _ in 0..2_000 {
            let a = Vector3dF::new(
                rng.next_f32_finite_range(-100.0, 100.0),
                rng.next_f32_finite_range(-100.0, 100.0),
                rng.next_f32_finite_range(-100.0, 100.0),
            );
            let b = Vector3dF::new(
                rng.next_f32_finite_range(-100.0, 100.0),
                rng.next_f32_finite_range(-100.0, 100.0),
                rng.next_f32_finite_range(-100.0, 100.0),
            );
            let c = cross_product_3d(a, b);
            // The dot products are small compared to |a||b|, up to rounding.
            let scale = a.length() * b.length() + 1.0;
            assert!(dot_product_3d(c, a).abs() <= 1e-4 * scale);
            assert!(dot_product_3d(c, b).abs() <= 1e-4 * scale);
        }
    }

    #[test]
    fn vector3d_f_test_nan_in_set_to_min_follows_cpp_ternary() {
        // NaN in self fails `self <= other`, so other's value is taken.
        let mut a = Vector3dF::new(f32::NAN, 1.0, 1.0);
        a.set_to_min(Vector3dF::new(0.0, 0.0, 0.0));
        assert_eq!(0.0, a.x());
        assert_eq!(0.0, a.y());

        // NaN in other fails `self <= other`, so NaN is taken.
        let mut b = Vector3dF::new(1.0, 1.0, 1.0);
        b.set_to_min(Vector3dF::new(f32::NAN, 0.0, 0.0));
        assert!(b.x().is_nan());
        assert_eq!(0.0, b.y());
    }
}
