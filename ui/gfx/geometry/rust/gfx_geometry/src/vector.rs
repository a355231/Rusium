// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `gfx::Vector2d` and `gfx::Vector2dF`, plus `vector2d_conversions` and the
//! helpers from `vector2d.cc` and `vector2d_f.{h,cc}`.

use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::format::format_g;
use crate::numeric::{clamp_ceil, clamp_floor, cpp_max, cpp_min};

/// An integer 2D vector. Addition, subtraction and negation saturate at the
/// `i32` limits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vector2d {
    x: i32,
    y: i32,
}

impl Vector2d {
    pub const fn new(x: i32, y: i32) -> Self {
        Vector2d { x, y }
    }

    pub const fn x(self) -> i32 {
        self.x
    }

    pub const fn y(self) -> i32 {
        self.y
    }

    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: i32) {
        self.y = y;
    }

    /// True if both components are 0.
    pub fn is_zero(self) -> bool {
        self.x == 0 && self.y == 0
    }

    /// Adds `other` to this vector, saturating at the `i32` limits.
    pub fn add(&mut self, other: Vector2d) {
        self.x = self.x.saturating_add(other.x);
        self.y = self.y.saturating_add(other.y);
    }

    /// Subtracts `other` from this vector, saturating at the `i32` limits.
    pub fn subtract(&mut self, other: Vector2d) {
        self.x = self.x.saturating_sub(other.x);
        self.y = self.y.saturating_sub(other.y);
    }

    pub fn set_to_min(&mut self, other: Vector2d) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
    }

    pub fn set_to_max(&mut self, other: Vector2d) {
        self.x = self.x.max(other.x);
        self.y = self.y.max(other.y);
    }

    /// The square of the diagonal length. Each square fits in `i64`. The sum
    /// only overflows when both components are `i32::MIN` (the true value is
    /// 2^63), which is undefined behavior in C++. Here it saturates to
    /// `i64::MAX`.
    pub fn length_squared(self) -> i64 {
        (self.x as i64 * self.x as i64).saturating_add(self.y as i64 * self.y as i64)
    }

    /// The diagonal length, computed as `sqrt` in double and rounded to float.
    pub fn length(self) -> f32 {
        (self.length_squared() as f64).sqrt() as f32
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.x, &mut self.y);
    }
}

impl fmt::Display for Vector2d {
    /// `"[%d %d]"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} {}]", self.x, self.y)
    }
}

impl AddAssign for Vector2d {
    fn add_assign(&mut self, other: Vector2d) {
        self.add(other);
    }
}

impl SubAssign for Vector2d {
    fn sub_assign(&mut self, other: Vector2d) {
        self.subtract(other);
    }
}

impl Add for Vector2d {
    type Output = Vector2d;
    fn add(mut self, rhs: Vector2d) -> Vector2d {
        Vector2d::add(&mut self, rhs);
        self
    }
}

impl Sub for Vector2d {
    type Output = Vector2d;
    fn sub(mut self, rhs: Vector2d) -> Vector2d {
        self.subtract(rhs);
        self
    }
}

impl Neg for Vector2d {
    type Output = Vector2d;
    /// Negates each component. `i32::MIN` saturates to `i32::MAX`.
    fn neg(self) -> Vector2d {
        Vector2d::new(self.x.saturating_neg(), self.y.saturating_neg())
    }
}

pub const fn transpose_vector2d(v: Vector2d) -> Vector2d {
    Vector2d::new(v.y, v.x)
}

/// A float 2D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2dF {
    x: f32,
    y: f32,
}

impl Vector2dF {
    pub const fn new(x: f32, y: f32) -> Self {
        Vector2dF { x, y }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub fn set_x(&mut self, x: f32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    /// True if both components are exactly 0.
    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    pub fn add(&mut self, other: Vector2dF) {
        self.x += other.x;
        self.y += other.y;
    }

    pub fn subtract(&mut self, other: Vector2dF) {
        self.x -= other.x;
        self.y -= other.y;
    }

    /// Uses `std::min` semantics, see `numeric::cpp_min`.
    pub fn set_to_min(&mut self, other: Vector2dF) {
        self.x = cpp_min(self.x, other.x);
        self.y = cpp_min(self.y, other.y);
    }

    /// Uses `std::max` semantics, see `numeric::cpp_max`.
    pub fn set_to_max(&mut self, other: Vector2dF) {
        self.x = cpp_max(self.x, other.x);
        self.y = cpp_max(self.y, other.y);
    }

    /// The square of the diagonal length, in double.
    pub fn length_squared(self) -> f64 {
        self.x as f64 * self.x as f64 + self.y as f64 * self.y as f64
    }

    /// The diagonal length, computed with `hypotf`.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn aspect_ratio(self) -> f32 {
        self.x / self.y
    }

    /// The angle from the positive x axis in radians, in `(-pi, pi]`, from
    /// `atan2f(y, x)`.
    pub fn slope_angle_radians(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn scale(&mut self, scale: f32) {
        self.scale_xy(scale, scale);
    }

    pub fn scale_xy(&mut self, x_scale: f32, y_scale: f32) {
        self.x *= x_scale;
        self.y *= y_scale;
    }

    pub fn inv_scale(&mut self, inv_scale: f32) {
        self.inv_scale_xy(inv_scale, inv_scale);
    }

    pub fn inv_scale_xy(&mut self, inv_x_scale: f32, inv_y_scale: f32) {
        self.x /= inv_x_scale;
        self.y /= inv_y_scale;
    }

    pub fn normalize(&mut self) {
        self.inv_scale(self.length());
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.x, &mut self.y);
    }
}

impl fmt::Display for Vector2dF {
    /// `"[%g %g]"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} {}]", format_g(self.x as f64), format_g(self.y as f64))
    }
}

impl From<Vector2d> for Vector2dF {
    /// The implicit `operator Vector2dF()` conversion. Each component is
    /// converted with `static_cast<float>`.
    fn from(v: Vector2d) -> Vector2dF {
        Vector2dF::new(v.x() as f32, v.y() as f32)
    }
}

impl AddAssign for Vector2dF {
    fn add_assign(&mut self, other: Vector2dF) {
        self.add(other);
    }
}

impl SubAssign for Vector2dF {
    fn sub_assign(&mut self, other: Vector2dF) {
        self.subtract(other);
    }
}

impl Add for Vector2dF {
    type Output = Vector2dF;
    fn add(mut self, rhs: Vector2dF) -> Vector2dF {
        Vector2dF::add(&mut self, rhs);
        self
    }
}

impl Sub for Vector2dF {
    type Output = Vector2dF;
    fn sub(mut self, rhs: Vector2dF) -> Vector2dF {
        self.subtract(rhs);
        self
    }
}

impl Neg for Vector2dF {
    type Output = Vector2dF;
    fn neg(self) -> Vector2dF {
        Vector2dF::new(-self.x, -self.y)
    }
}

/// The 2D cross product (determinant), computed in double.
pub fn cross_product_2d(lhs: Vector2dF, rhs: Vector2dF) -> f64 {
    lhs.x as f64 * rhs.y as f64 - lhs.y as f64 * rhs.x as f64
}

/// The 2D dot product, computed in double.
pub fn dot_product_2d(lhs: Vector2dF, rhs: Vector2dF) -> f64 {
    lhs.x as f64 * rhs.x as f64 + lhs.y as f64 * rhs.y as f64
}

/// Scales each component of `v` by the matching factor.
pub fn scale_vector2d(v: Vector2dF, x_scale: f32, y_scale: f32) -> Vector2dF {
    let mut scaled = v;
    scaled.scale_xy(x_scale, y_scale);
    scaled
}

pub fn scale_vector2d_uniform(v: Vector2dF, scale: f32) -> Vector2dF {
    scale_vector2d(v, scale, scale)
}

pub const fn transpose_vector2d_f(v: Vector2dF) -> Vector2dF {
    Vector2dF::new(v.y, v.x)
}

/// Returns a unit vector in the direction of `v`.
pub fn normalize_vector2d(v: Vector2dF) -> Vector2dF {
    let mut normal = v;
    normal.normalize();
    normal
}

/// `ToFlooredVector2d`: each component is floored and saturated to `i32`.
pub fn to_floored_vector2d(v: Vector2dF) -> Vector2d {
    Vector2d::new(clamp_floor(v.x), clamp_floor(v.y))
}

/// `ToCeiledVector2d`: each component is ceiled and saturated to `i32`.
pub fn to_ceiled_vector2d(v: Vector2dF) -> Vector2d {
    Vector2d::new(clamp_ceil(v.x), clamp_ceil(v.y))
}

/// `ToRoundedVector2d`: each component is computed as `floor(x + 0.5)` in
/// float, saturated to `i32`. This is not `round()`. It matches Blink's
/// `LayoutUnit::Round()` and gives the same direction for negative offsets.
pub fn to_rounded_vector2d(v: Vector2dF) -> Vector2d {
    Vector2d::new(clamp_floor(v.x + 0.5), clamp_floor(v.y + 0.5))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{double_eq, float_eq, XorShift64};

    // ---- vector2d_unittest.cc ---------------------------------------------

    #[test]
    fn vector2d_test_is_zero() {
        assert!(Vector2d::default().is_zero());
        assert!(Vector2d::new(0, 0).is_zero());
        assert!(!Vector2d::new(1, 0).is_zero());
        assert!(!Vector2d::new(0, -2).is_zero());
        assert!(!Vector2d::new(1, -2).is_zero());
    }

    #[test]
    fn vector2d_test_add() {
        let i1 = Vector2d::new(3, 5);
        let i2 = Vector2d::new(4, -1);
        assert_eq!(Vector2d::new(3, 5), i1 + Vector2d::default());
        assert_eq!(Vector2d::new(3 + 4, 5 - 1), i1 + i2);
        assert_eq!(Vector2d::new(3 - 4, 5 + 1), i1 - i2);
    }

    #[test]
    fn vector2d_test_negative() {
        assert_eq!(Vector2d::new(0, 0), -Vector2d::new(0, 0));
        assert_eq!(Vector2d::new(-3, -3), -Vector2d::new(3, 3));
        assert_eq!(Vector2d::new(3, 3), -Vector2d::new(-3, -3));
        assert_eq!(Vector2d::new(-3, 3), -Vector2d::new(3, -3));
        assert_eq!(Vector2d::new(3, -3), -Vector2d::new(-3, 3));
    }

    #[test]
    fn vector2d_test_length() {
        let values = [[0, 0], [10, 20], [20, 10], [-10, -20], [-20, 10], [10, -20]];
        for value in values {
            let v0 = value[0];
            let v1 = value[1];
            let length_squared = v0 as f64 * v0 as f64 + v1 as f64 * v1 as f64;
            let length = length_squared.sqrt();
            let vector = Vector2d::new(v0, v1);
            assert_eq!(length_squared as i64, vector.length_squared());
            assert_eq!(length as f32, vector.length());
        }
    }

    #[test]
    fn vector2d_test_set_to_min_max() {
        let mut a = Vector2d::new(3, 5);
        assert_eq!(Vector2d::new(3, 5), a);
        a.set_to_max(Vector2d::new(2, 4));
        assert_eq!(Vector2d::new(3, 5), a);
        a.set_to_max(Vector2d::new(3, 5));
        assert_eq!(Vector2d::new(3, 5), a);
        a.set_to_max(Vector2d::new(4, 2));
        assert_eq!(Vector2d::new(4, 5), a);
        a.set_to_max(Vector2d::new(8, 10));
        assert_eq!(Vector2d::new(8, 10), a);
        a.set_to_min(Vector2d::new(9, 11));
        assert_eq!(Vector2d::new(8, 10), a);
        a.set_to_min(Vector2d::new(8, 10));
        assert_eq!(Vector2d::new(8, 10), a);
        a.set_to_min(Vector2d::new(11, 9));
        assert_eq!(Vector2d::new(8, 9), a);
        a.set_to_min(Vector2d::new(7, 11));
        assert_eq!(Vector2d::new(7, 9), a);
        a.set_to_min(Vector2d::new(3, 5));
        assert_eq!(Vector2d::new(3, 5), a);
    }

    #[test]
    fn vector2d_test_integer_overflow() {
        let int_max = i32::MAX;
        let int_min = i32::MIN;
        let max_vector = Vector2d::new(int_max, int_max);
        let min_vector = Vector2d::new(int_min, int_min);
        let mut test;

        test = Vector2d::default();
        test += Vector2d::new(int_max, int_max);
        assert_eq!(test, max_vector);

        test = Vector2d::default();
        test += Vector2d::new(int_min, int_min);
        assert_eq!(test, min_vector);

        test = Vector2d::new(10, 20);
        test += Vector2d::new(int_max, int_max);
        assert_eq!(test, max_vector);

        test = Vector2d::new(-10, -20);
        test += Vector2d::new(int_min, int_min);
        assert_eq!(test, min_vector);

        test = Vector2d::default();
        test -= Vector2d::new(int_max, int_max);
        assert_eq!(test, Vector2d::new(-int_max, -int_max));

        test = Vector2d::default();
        test -= Vector2d::new(int_min, int_min);
        assert_eq!(test, max_vector);

        test = Vector2d::new(10, 20);
        test -= Vector2d::new(int_min, int_min);
        assert_eq!(test, max_vector);

        test = Vector2d::new(-10, -20);
        test -= Vector2d::new(int_max, int_max);
        assert_eq!(test, min_vector);

        test = Vector2d::default();
        test -= Vector2d::new(int_min, int_min);
        assert_eq!(test, max_vector);

        test = -Vector2d::new(int_min, int_min);
        assert_eq!(test, max_vector);
    }

    #[test]
    fn vector2d_test_transpose() {
        let mut v = Vector2d::new(1, -2);
        assert_eq!(Vector2d::new(-2, 1), transpose_vector2d(v));
        v.transpose();
        assert_eq!(Vector2d::new(-2, 1), v);
    }

    // ---- vector2d_f_unittest.cc -------------------------------------------

    #[test]
    fn vector2d_test_vector2d_to_vector2d_f() {
        let i = Vector2d::new(3, 4);
        let f = Vector2dF::from(i);
        assert_eq!(f, Vector2dF::new(3.0, 4.0));
    }

    #[test]
    fn vector2d_f_test_is_zero() {
        assert!(Vector2dF::default().is_zero());
        assert!(Vector2dF::new(0.0, 0.0).is_zero());
        assert!(!Vector2dF::new(0.1, 0.0).is_zero());
        assert!(!Vector2dF::new(0.0, -0.1).is_zero());
        assert!(!Vector2dF::new(0.1, -0.1).is_zero());
    }

    #[test]
    fn vector2d_f_test_add() {
        let f1 = Vector2dF::new(3.1, 5.1);
        let f2 = Vector2dF::new(4.3, -1.3);
        assert_vector2df_eq(Vector2dF::new(3.1, 5.1), f1 + Vector2dF::default());
        assert_vector2df_eq(Vector2dF::new(3.1 + 4.3, 5.1 - 1.3), f1 + f2);
        assert_vector2df_eq(Vector2dF::new(3.1 - 4.3, 5.1 + 1.3), f1 - f2);
    }

    #[test]
    fn vector2d_f_test_negative() {
        assert_vector2df_eq(Vector2dF::default(), -Vector2dF::default());
        assert_vector2df_eq(Vector2dF::new(-0.3, -0.3), -Vector2dF::new(0.3, 0.3));
        assert_vector2df_eq(Vector2dF::new(0.3, 0.3), -Vector2dF::new(-0.3, -0.3));
        assert_vector2df_eq(Vector2dF::new(-0.3, 0.3), -Vector2dF::new(0.3, -0.3));
        assert_vector2df_eq(Vector2dF::new(0.3, -0.3), -Vector2dF::new(-0.3, 0.3));
    }

    #[test]
    fn vector2d_f_test_scale() {
        let double_values = [
            [4.5f32, 1.2, 3.3, 5.6],
            [4.5, -1.2, 3.3, 5.6],
            [4.5, 1.2, 3.3, -5.6],
            [4.5, 1.2, -3.3, -5.6],
            [-4.5, 1.2, 3.3, 5.6],
            [-4.5, 1.2, 0.0, 5.6],
            [-4.5, 1.2, 3.3, 0.0],
            [4.5, 0.0, 3.3, 5.6],
            [0.0, 1.2, 3.3, 5.6],
        ];
        for values in double_values {
            let mut v = Vector2dF::new(values[0], values[1]);
            v.scale_xy(values[2], values[3]);
            assert_eq!(v.x(), values[0] * values[2]);
            assert_eq!(v.y(), values[1] * values[3]);
            let v2 = scale_vector2d(Vector2dF::new(values[0], values[1]), values[2], values[3]);
            assert_eq!(values[0] * values[2], v2.x());
            assert_eq!(values[1] * values[3], v2.y());
        }

        let single_values = [
            [4.5f32, 1.2, 3.3],
            [4.5, -1.2, 3.3],
            [4.5, 1.2, 3.3],
            [4.5, 1.2, -3.3],
            [-4.5, 1.2, 3.3],
            [-4.5, 1.2, 0.0],
            [-4.5, 1.2, 3.3],
            [4.5, 0.0, 3.3],
            [0.0, 1.2, 3.3],
        ];
        for values in single_values {
            let mut v = Vector2dF::new(values[0], values[1]);
            v.scale(values[2]);
            assert_eq!(v.x(), values[0] * values[2]);
            assert_eq!(v.y(), values[1] * values[2]);
            let v2 = scale_vector2d_uniform(Vector2dF::new(values[0], values[1]), values[2]);
            assert_eq!(values[0] * values[2], v2.x());
            assert_eq!(values[1] * values[2], v2.y());
        }
    }

    #[test]
    fn vector2d_f_test_set_to_min_max() {
        let mut a = Vector2dF::new(3.5, 5.5);
        assert_eq!(Vector2dF::new(3.5, 5.5), a);
        a.set_to_max(Vector2dF::new(2.5, 4.5));
        assert_eq!(Vector2dF::new(3.5, 5.5), a);
        a.set_to_max(Vector2dF::new(3.5, 5.5));
        assert_eq!(Vector2dF::new(3.5, 5.5), a);
        a.set_to_max(Vector2dF::new(4.5, 2.5));
        assert_eq!(Vector2dF::new(4.5, 5.5), a);
        a.set_to_max(Vector2dF::new(8.5, 10.5));
        assert_eq!(Vector2dF::new(8.5, 10.5), a);
        a.set_to_min(Vector2dF::new(9.5, 11.5));
        assert_eq!(Vector2dF::new(8.5, 10.5), a);
        a.set_to_min(Vector2dF::new(8.5, 10.5));
        assert_eq!(Vector2dF::new(8.5, 10.5), a);
        a.set_to_min(Vector2dF::new(11.5, 9.5));
        assert_eq!(Vector2dF::new(8.5, 9.5), a);
        a.set_to_min(Vector2dF::new(7.5, 11.5));
        assert_eq!(Vector2dF::new(7.5, 9.5), a);
        a.set_to_min(Vector2dF::new(3.5, 5.5));
        assert_eq!(Vector2dF::new(3.5, 5.5), a);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn vector2d_f_test_length() {
        let k_float_max = f32::MAX;
        assert!(float_eq(0.0, Vector2dF::new(0.0, 0.0).length()));
        assert!(float_eq(1.0, Vector2dF::new(1.0, 0.0).length()));
        assert!(float_eq(1.414214, Vector2dF::new(1.0, 1.0).length()));
        assert!(float_eq(2.236068, Vector2dF::new(-1.0, -2.0).length()));
        // The Pythagorean triples 3-4-5 and 5-12-13.
        assert!(float_eq(5.0, Vector2dF::new(3.0, 4.0).length()));
        assert!(float_eq(13.0, Vector2dF::new(5.0, 12.0).length()));
        // Very small numbers.
        assert!(float_eq(0.7071068e-20, Vector2dF::new(0.5e-20, 0.5e-20).length()));
        // Very large numbers.
        assert!(float_eq(0.7071068e20, Vector2dF::new(0.5e20, 0.5e20).length()));
        assert!(float_eq(k_float_max, Vector2dF::new(k_float_max, 0.0).length()));
        assert!(float_eq(k_float_max, Vector2dF::new(k_float_max, k_float_max).length()));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn vector2d_f_test_slope_angle_radians() {
        // The function is required to be very accurate, so use a smaller
        // tolerance than EXPECT_FLOAT_EQ().
        let tolerance = 1e-7_f32;
        let pi = 3.1415927_f32;
        let near = |expected: f32, actual: f32| {
            assert!((expected - actual).abs() <= tolerance, "{expected} vs {actual}")
        };
        near(0.0, Vector2dF::new(0.0, 0.0).slope_angle_radians());
        near(0.0, Vector2dF::new(1.0, 0.0).slope_angle_radians());
        near(pi / 4.0, Vector2dF::new(1.0, 1.0).slope_angle_radians());
        near(pi / 2.0, Vector2dF::new(0.0, 1.0).slope_angle_radians());
        near(pi, Vector2dF::new(-50.0, 0.0).slope_angle_radians());
        near(-pi * 3.0 / 4.0, Vector2dF::new(-50.0, -50.0).slope_angle_radians());
        near(-pi / 4.0, Vector2dF::new(1.0, -1.0).slope_angle_radians());
    }

    #[test]
    fn vector2d_f_test_transpose() {
        let mut v = Vector2dF::new(-1.5, 2.5);
        assert_eq!(Vector2dF::new(2.5, -1.5), transpose_vector2d_f(v));
        v.transpose();
        assert_eq!(Vector2dF::new(2.5, -1.5), v);
    }

    #[test]
    fn vector2d_f_test_to_string() {
        assert_eq!("[1 2]", Vector2dF::new(1.0, 2.0).to_string());
        assert_eq!("[1.03125 2.5]", Vector2dF::new(1.03125, 2.5).to_string());
    }

    // ---- Extra tests ------------------------------------------------------

    #[test]
    fn vector2d_test_length_squared_does_not_overflow() {
        let v = Vector2d::new(i32::MIN, i32::MIN);
        // The exact value 2^63 does not fit, so it saturates.
        assert_eq!(i64::MAX, v.length_squared());
        assert_eq!(
            (i32::MIN as i64) * (i32::MIN as i64) + 9,
            Vector2d::new(i32::MIN, 3).length_squared()
        );
    }

    #[test]
    fn vector2d_test_negation_of_int_min_saturates() {
        assert_eq!(Vector2d::new(i32::MAX, 0), -Vector2d::new(i32::MIN, 0));
    }

    #[test]
    fn vector2d_f_test_length_squared_is_double() {
        let v = Vector2dF::new(1.0e20, 1.0e20);
        // 1e20 is not exact in float, so compare against the exact f64 square.
        let x = 1.0e20_f32 as f64;
        assert!(double_eq(2.0 * x * x, v.length_squared()));
    }

    #[test]
    fn vector2d_f_test_cross_and_dot() {
        let a = Vector2dF::new(1.0, 2.0);
        let b = Vector2dF::new(3.0, 4.0);
        assert_eq!(-2.0, cross_product_2d(a, b));
        assert_eq!(11.0, dot_product_2d(a, b));
    }

    #[test]
    fn vector2d_f_test_normalize() {
        let n = normalize_vector2d(Vector2dF::new(3.0, 4.0));
        assert!(float_eq(0.6, n.x()));
        assert!(float_eq(0.8, n.y()));
    }

    #[test]
    fn vector2d_conversions_round_as_documented() {
        let v = Vector2dF::new(-0.5, 2.5);
        assert_eq!(Vector2d::new(-1, 2), to_floored_vector2d(v));
        assert_eq!(Vector2d::new(0, 3), to_ceiled_vector2d(v));
        // floor(x + 0.5): -0.5 + 0.5 = 0 and 2.5 + 0.5 = 3.
        assert_eq!(Vector2d::new(0, 3), to_rounded_vector2d(v));
        assert_eq!(
            Vector2d::new(i32::MAX, i32::MIN),
            to_floored_vector2d(Vector2dF::new(1e30, -1e30))
        );
        assert_eq!(Vector2d::new(0, 0), to_rounded_vector2d(Vector2dF::new(f32::NAN, f32::NAN)));
    }

    #[test]
    fn vector2d_f_test_random_add_sub_matches_float_reference() {
        let mut rng = XorShift64::new(3);
        for _ in 0..5_000 {
            let a = Vector2dF::new(
                rng.next_f32_finite_range(-1e6, 1e6),
                rng.next_f32_finite_range(-1e6, 1e6),
            );
            let b = Vector2dF::new(
                rng.next_f32_finite_range(-1e6, 1e6),
                rng.next_f32_finite_range(-1e6, 1e6),
            );
            assert_eq!(Vector2dF::new(a.x() + b.x(), a.y() + b.y()), a + b);
            assert_eq!(Vector2dF::new(a.x() - b.x(), a.y() - b.y()), a - b);
        }
    }

    #[test]
    fn vector2d_random_add_sub_matches_saturating_reference() {
        let mut rng = XorShift64::new(5);
        for _ in 0..5_000 {
            let a = Vector2d::new(rng.next_i32(), rng.next_i32());
            let b = Vector2d::new(rng.next_i32(), rng.next_i32());
            let clamp = |v: i64| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            assert_eq!(
                Vector2d::new(
                    clamp(a.x() as i64 + b.x() as i64),
                    clamp(a.y() as i64 + b.y() as i64)
                ),
                a + b
            );
            assert_eq!(
                Vector2d::new(
                    clamp(a.x() as i64 - b.x() as i64),
                    clamp(a.y() as i64 - b.y() as i64)
                ),
                a - b
            );
        }
    }

    fn assert_vector2df_eq(expected: Vector2dF, actual: Vector2dF) {
        assert!(
            float_eq(expected.x(), actual.x()) && float_eq(expected.y(), actual.y()),
            "expected {expected}, got {actual}"
        );
    }
}
