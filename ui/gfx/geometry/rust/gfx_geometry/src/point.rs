// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `gfx::Point` and `gfx::PointF`, plus `point_conversions` and the scaling
//! helpers from `point.cc` and `point_f.cc`.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::format::format_g;
use crate::numeric::{clamp_ceil, clamp_floor, clamp_round, cpp_max, cpp_min, lex_less};
use crate::vector::{Vector2d, Vector2dF};

/// A point with integer x and y coordinates. Arithmetic with vectors saturates
/// at the `i32` limits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Point { x, y }
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

    pub fn set_point(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Moves the point by `(delta_x, delta_y)`, saturating at the `i32`
    /// limits.
    pub fn offset(&mut self, delta_x: i32, delta_y: i32) {
        self.x = self.x.saturating_add(delta_x);
        self.y = self.y.saturating_add(delta_y);
    }

    pub fn is_origin(self) -> bool {
        self.x == 0 && self.y == 0
    }

    pub const fn offset_from_origin(self) -> Vector2d {
        Vector2d::new(self.x, self.y)
    }

    pub fn set_to_min(&mut self, other: Point) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
    }

    pub fn set_to_max(&mut self, other: Point) {
        self.x = self.x.max(other.x);
        self.y = self.y.max(other.y);
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.x, &mut self.y);
    }
}

/// Orders by `y` first, then `x`, matching `Point::operator<`.
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.y, self.x).cmp(&(other.y, other.x))
    }
}

impl fmt::Display for Point {
    /// `"%d,%d"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

impl AddAssign<Vector2d> for Point {
    fn add_assign(&mut self, vector: Vector2d) {
        self.x = self.x.saturating_add(vector.x());
        self.y = self.y.saturating_add(vector.y());
    }
}

impl SubAssign<Vector2d> for Point {
    fn sub_assign(&mut self, vector: Vector2d) {
        self.x = self.x.saturating_sub(vector.x());
        self.y = self.y.saturating_sub(vector.y());
    }
}

impl Add<Vector2d> for Point {
    type Output = Point;
    fn add(mut self, rhs: Vector2d) -> Point {
        self += rhs;
        self
    }
}

impl Sub<Vector2d> for Point {
    type Output = Point;
    fn sub(mut self, rhs: Vector2d) -> Point {
        self -= rhs;
        self
    }
}

impl Sub<Point> for Point {
    type Output = Vector2d;
    fn sub(self, rhs: Point) -> Vector2d {
        Vector2d::new(self.x.saturating_sub(rhs.x), self.y.saturating_sub(rhs.y))
    }
}

pub const fn point_at_offset_from_origin(offset_from_origin: Vector2d) -> Point {
    Point::new(offset_from_origin.x(), offset_from_origin.y())
}

pub const fn transpose_point(p: Point) -> Point {
    Point::new(p.y, p.x)
}

/// A point with float x and y coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointF {
    x: f32,
    y: f32,
}

impl PointF {
    pub const fn new(x: f32, y: f32) -> Self {
        PointF { x, y }
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

    pub fn set_point(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn offset(&mut self, delta_x: f32, delta_y: f32) {
        self.x += delta_x;
        self.y += delta_y;
    }

    pub fn is_origin(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    pub const fn offset_from_origin(self) -> Vector2dF {
        Vector2dF::new(self.x, self.y)
    }

    /// Uses `std::min` semantics, see `numeric::cpp_min`.
    pub fn set_to_min(&mut self, other: PointF) {
        self.x = cpp_min(self.x, other.x);
        self.y = cpp_min(self.y, other.y);
    }

    /// Uses `std::max` semantics, see `numeric::cpp_max`.
    pub fn set_to_max(&mut self, other: PointF) {
        self.x = cpp_max(self.x, other.x);
        self.y = cpp_max(self.y, other.y);
    }

    /// Scales both coordinates by `scale`.
    pub fn scale(&mut self, scale: f32) {
        self.scale_xy(scale, scale);
    }

    pub fn scale_xy(&mut self, x_scale: f32, y_scale: f32) {
        self.set_point(self.x * x_scale, self.y * y_scale);
    }

    /// Divides both coordinates by `inv_scale`.
    pub fn inv_scale(&mut self, inv_scale: f32) {
        self.inv_scale_xy(inv_scale, inv_scale);
    }

    pub fn inv_scale_xy(&mut self, inv_x_scale: f32, inv_y_scale: f32) {
        self.x /= inv_x_scale;
        self.y /= inv_y_scale;
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.x, &mut self.y);
    }

    /// Returns true if the straight-line distance to `rhs` is less than
    /// `allowed_distance`. `allowed_distance` must be positive. The C++ code
    /// checks this with `DCHECK`, which is a `debug_assert!` here.
    pub fn is_within_distance(self, rhs: PointF, allowed_distance: f32) -> bool {
        debug_assert!(allowed_distance > 0.0);
        let diff_x = self.x - rhs.x;
        let diff_y = self.y - rhs.y;
        let distance = (diff_x * diff_x + diff_y * diff_y).sqrt();
        distance < allowed_distance
    }
}

/// Lexicographic on `(y, x)` with tuple `<` semantics, matching
/// `PointF::operator<`. Two points can be incomparable if they contain NaN.
impl PartialOrd for PointF {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if lex_less((self.y, self.x), (other.y, other.x)) {
            Some(Ordering::Less)
        } else if lex_less((other.y, other.x), (self.y, self.x)) {
            Some(Ordering::Greater)
        } else if self == other {
            Some(Ordering::Equal)
        } else {
            None
        }
    }
}

impl fmt::Display for PointF {
    /// `"%g,%g"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", format_g(self.x as f64), format_g(self.y as f64))
    }
}

impl From<Point> for PointF {
    /// The explicit `PointF(const Point&)` conversion. Each coordinate is
    /// converted with `static_cast<float>`.
    fn from(p: Point) -> PointF {
        PointF::new(p.x() as f32, p.y() as f32)
    }
}

impl AddAssign<Vector2dF> for PointF {
    fn add_assign(&mut self, vector: Vector2dF) {
        self.x += vector.x();
        self.y += vector.y();
    }
}

impl SubAssign<Vector2dF> for PointF {
    fn sub_assign(&mut self, vector: Vector2dF) {
        self.x -= vector.x();
        self.y -= vector.y();
    }
}

impl Add<Vector2dF> for PointF {
    type Output = PointF;
    fn add(mut self, rhs: Vector2dF) -> PointF {
        self += rhs;
        self
    }
}

impl Sub<Vector2dF> for PointF {
    type Output = PointF;
    fn sub(mut self, rhs: Vector2dF) -> PointF {
        self -= rhs;
        self
    }
}

impl Sub<PointF> for PointF {
    type Output = Vector2dF;
    fn sub(self, rhs: PointF) -> Vector2dF {
        Vector2dF::new(self.x - rhs.x, self.y - rhs.y)
    }
}

pub const fn point_f_at_offset_from_origin(offset_from_origin: Vector2dF) -> PointF {
    PointF::new(offset_from_origin.x(), offset_from_origin.y())
}

/// Returns `p` with each coordinate multiplied by the matching scale.
pub fn scale_point(p: PointF, x_scale: f32, y_scale: f32) -> PointF {
    let mut scaled = p;
    scaled.scale_xy(x_scale, y_scale);
    scaled
}

pub fn scale_point_uniform(p: PointF, scale: f32) -> PointF {
    scale_point(p, scale, scale)
}

pub const fn transpose_point_f(p: PointF) -> PointF {
    PointF::new(p.y, p.x)
}

/// `ToFlooredPoint`: each coordinate is floored and saturated to `i32`.
pub fn to_floored_point(point: PointF) -> Point {
    Point::new(clamp_floor(point.x), clamp_floor(point.y))
}

/// `ToCeiledPoint`: each coordinate is ceiled and saturated to `i32`.
pub fn to_ceiled_point(point: PointF) -> Point {
    Point::new(clamp_ceil(point.x), clamp_ceil(point.y))
}

/// `ToRoundedPoint`: each coordinate is rounded half away from zero and
/// saturated to `i32`.
pub fn to_rounded_point(point: PointF) -> Point {
    Point::new(clamp_round(point.x), clamp_round(point.y))
}

// The scale_to_* helpers return `point` unchanged when the scale is exactly
// 1, so a scale of 1 never goes through float conversion.

pub fn scale_to_ceiled_point(point: Point, x_scale: f32, y_scale: f32) -> Point {
    if x_scale == 1.0 && y_scale == 1.0 {
        return point;
    }
    to_ceiled_point(scale_point(PointF::from(point), x_scale, y_scale))
}

pub fn scale_to_ceiled_point_uniform(point: Point, scale: f32) -> Point {
    if scale == 1.0 {
        return point;
    }
    to_ceiled_point(scale_point_uniform(PointF::from(point), scale))
}

pub fn scale_to_floored_point(point: Point, x_scale: f32, y_scale: f32) -> Point {
    if x_scale == 1.0 && y_scale == 1.0 {
        return point;
    }
    to_floored_point(scale_point(PointF::from(point), x_scale, y_scale))
}

pub fn scale_to_floored_point_uniform(point: Point, scale: f32) -> Point {
    if scale == 1.0 {
        return point;
    }
    to_floored_point(scale_point_uniform(PointF::from(point), scale))
}

pub fn scale_to_rounded_point(point: Point, x_scale: f32, y_scale: f32) -> Point {
    if x_scale == 1.0 && y_scale == 1.0 {
        return point;
    }
    to_rounded_point(scale_point(PointF::from(point), x_scale, y_scale))
}

pub fn scale_to_rounded_point_uniform(point: Point, scale: f32) -> Point {
    if scale == 1.0 {
        return point;
    }
    to_rounded_point(scale_point_uniform(PointF::from(point), scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric::clamp_round;
    use crate::test_util::XorShift64;

    // ---- point_unittest.cc ------------------------------------------------

    #[test]
    fn point_test_is_origin() {
        assert!(!Point::new(1, 0).is_origin());
        assert!(!Point::new(0, 1).is_origin());
        assert!(!Point::new(1, 2).is_origin());
        assert!(!Point::new(-1, 0).is_origin());
        assert!(!Point::new(0, -1).is_origin());
        assert!(!Point::new(-1, -2).is_origin());
        assert!(Point::new(0, 0).is_origin());
    }

    #[test]
    fn point_test_vector_arithmetic() {
        let a = Point::new(1, 5);
        let v1 = Vector2d::new(3, -3);
        let v2 = Vector2d::new(-8, 1);

        let tests = [
            (Point::new(4, 2), a + v1),
            (Point::new(-2, 8), a - v1),
            (a, a - v1 + v1),
            (a, a + v1 - v1),
            (a, a + Vector2d::new(0, 0)),
            (Point::new(12, 1), a + v1 - v2),
            (Point::new(-10, 9), a - v1 + v2),
        ];
        for (expected, actual) in tests {
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn point_test_offset_from_point() {
        let a = Point::new(1, 5);
        let b = Point::new(-20, 8);
        assert_eq!(Vector2d::new(-20 - 1, 8 - 5), b - a);
    }

    #[test]
    fn point_test_set_to_min_max() {
        let mut a = Point::new(3, 5);
        assert_eq!(Point::new(3, 5), a);
        a.set_to_max(Point::new(2, 4));
        assert_eq!(Point::new(3, 5), a);
        a.set_to_max(Point::new(3, 5));
        assert_eq!(Point::new(3, 5), a);
        a.set_to_max(Point::new(4, 2));
        assert_eq!(Point::new(4, 5), a);
        a.set_to_max(Point::new(8, 10));
        assert_eq!(Point::new(8, 10), a);

        a.set_to_min(Point::new(9, 11));
        assert_eq!(Point::new(8, 10), a);
        a.set_to_min(Point::new(8, 10));
        assert_eq!(Point::new(8, 10), a);
        a.set_to_min(Point::new(11, 9));
        assert_eq!(Point::new(8, 9), a);
        a.set_to_min(Point::new(7, 11));
        assert_eq!(Point::new(7, 9), a);
        a.set_to_min(Point::new(3, 5));
        assert_eq!(Point::new(3, 5), a);
    }

    #[test]
    fn point_test_offset() {
        let mut test = Point::new(3, 4);
        test.offset(5, -8);
        assert_eq!(test, Point::new(8, -4));
    }

    #[test]
    fn point_test_vector_math() {
        let mut test = Point::new(3, 4);
        test += Vector2d::new(5, -8);
        assert_eq!(test, Point::new(8, -4));

        let mut test2 = Point::new(3, 4);
        test2 -= Vector2d::new(5, -8);
        assert_eq!(test2, Point::new(-2, 12));
    }

    #[test]
    fn point_test_integer_overflow() {
        let int_max = i32::MAX;
        let int_min = i32::MIN;

        let max_point = Point::new(int_max, int_max);
        let min_point = Point::new(int_min, int_min);
        let mut test;

        test = Point::default();
        test.offset(int_max, int_max);
        assert_eq!(test, max_point);

        test = Point::default();
        test.offset(int_min, int_min);
        assert_eq!(test, min_point);

        test = Point::new(10, 20);
        test.offset(int_max, int_max);
        assert_eq!(test, max_point);

        test = Point::new(-10, -20);
        test.offset(int_min, int_min);
        assert_eq!(test, min_point);

        test = Point::default();
        test += Vector2d::new(int_max, int_max);
        assert_eq!(test, max_point);

        test = Point::default();
        test += Vector2d::new(int_min, int_min);
        assert_eq!(test, min_point);

        test = Point::new(10, 20);
        test += Vector2d::new(int_max, int_max);
        assert_eq!(test, max_point);

        test = Point::new(-10, -20);
        test += Vector2d::new(int_min, int_min);
        assert_eq!(test, min_point);

        test = Point::default();
        test -= Vector2d::new(int_max, int_max);
        assert_eq!(test, Point::new(-int_max, -int_max));

        test = Point::default();
        test -= Vector2d::new(int_min, int_min);
        assert_eq!(test, max_point);

        test = Point::new(10, 20);
        test -= Vector2d::new(int_min, int_min);
        assert_eq!(test, max_point);

        test = Point::new(-10, -20);
        test -= Vector2d::new(int_max, int_max);
        assert_eq!(test, min_point);
    }

    #[test]
    fn point_test_transpose() {
        let mut p = Point::new(1, -2);
        assert_eq!(Point::new(-2, 1), transpose_point(p));
        p.transpose();
        assert_eq!(Point::new(-2, 1), p);
    }

    // ---- Extra tests for the integer Point ---------------------------------

    #[test]
    fn point_test_ordering_is_by_y_then_x() {
        assert!(Point::new(5, 1) < Point::new(0, 2));
        assert!(Point::new(1, 2) < Point::new(2, 2));
        assert!(!(Point::new(2, 2) < Point::new(1, 2)));
        let mut v = vec![Point::new(2, 1), Point::new(0, 0), Point::new(1, 1)];
        v.sort();
        assert_eq!(v, vec![Point::new(0, 0), Point::new(1, 1), Point::new(2, 1)]);
    }

    #[test]
    fn point_test_random_offset_matches_saturating_reference() {
        let mut rng = XorShift64::new(1);
        for _ in 0..5_000 {
            let (x, y) = (rng.next_i32(), rng.next_i32());
            let (dx, dy) = (rng.next_i32(), rng.next_i32());
            let mut p = Point::new(x, y);
            p.offset(dx, dy);
            let clamp = |v: i64| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            assert_eq!(p, Point::new(clamp(x as i64 + dx as i64), clamp(y as i64 + dy as i64)));

            let diff = Point::new(x, y) - Point::new(dx, dy);
            assert_eq!(
                diff,
                Vector2d::new(clamp(x as i64 - dx as i64), clamp(y as i64 - dy as i64))
            );
        }
    }

    // ---- point_f_unittest.cc ----------------------------------------------

    #[test]
    fn point_f_test_point_to_point_f() {
        // Check that explicit conversion from integer to float works.
        let a = Point::new(10, 20);
        let b = PointF::from(a);
        assert_eq!(a.x() as f32, b.x());
        assert_eq!(a.y() as f32, b.y());
    }

    #[test]
    fn point_f_test_is_origin() {
        assert!(!PointF::new(0.1, 0.0).is_origin());
        assert!(!PointF::new(0.0, 0.1).is_origin());
        assert!(!PointF::new(0.1, 2.0).is_origin());
        assert!(!PointF::new(-0.1, 0.0).is_origin());
        assert!(!PointF::new(0.0, -0.1).is_origin());
        assert!(!PointF::new(-0.1, -2.0).is_origin());
        assert!(PointF::new(0.0, 0.0).is_origin());
    }

    #[test]
    fn point_f_test_to_rounded_point() {
        assert_eq!(Point::new(0, 0), to_rounded_point(PointF::new(0.0, 0.0)));
        assert_eq!(Point::new(0, 0), to_rounded_point(PointF::new(0.0001, 0.0001)));
        assert_eq!(Point::new(0, 0), to_rounded_point(PointF::new(0.4999, 0.4999)));
        assert_eq!(Point::new(1, 1), to_rounded_point(PointF::new(0.5, 0.5)));
        assert_eq!(Point::new(1, 1), to_rounded_point(PointF::new(0.9999, 0.9999)));
        assert_eq!(Point::new(10, 10), to_rounded_point(PointF::new(10.0, 10.0)));
        assert_eq!(Point::new(10, 10), to_rounded_point(PointF::new(10.0001, 10.0001)));
        assert_eq!(Point::new(10, 10), to_rounded_point(PointF::new(10.4999, 10.4999)));
        assert_eq!(Point::new(11, 11), to_rounded_point(PointF::new(10.5, 10.5)));
        assert_eq!(Point::new(11, 11), to_rounded_point(PointF::new(10.9999, 10.9999)));
        assert_eq!(Point::new(-10, -10), to_rounded_point(PointF::new(-10.0, -10.0)));
        assert_eq!(Point::new(-10, -10), to_rounded_point(PointF::new(-10.0001, -10.0001)));
        assert_eq!(Point::new(-10, -10), to_rounded_point(PointF::new(-10.4999, -10.4999)));
        assert_eq!(Point::new(-11, -11), to_rounded_point(PointF::new(-10.5, -10.5)));
        assert_eq!(Point::new(-11, -11), to_rounded_point(PointF::new(-10.9999, -10.9999)));
    }

    #[test]
    fn point_f_test_scale() {
        assert_eq!(PointF::new(2.0, -2.0), scale_point_uniform(PointF::new(1.0, -1.0), 2.0));
        assert_eq!(PointF::new(2.0, -2.0), scale_point(PointF::new(1.0, -1.0), 2.0, 2.0));
        let mut zero = PointF::default();
        let mut one = PointF::new(1.0, -1.0);
        zero.scale(2.0);
        zero.scale_xy(3.0, 1.5);
        one.scale(2.0);
        one.scale_xy(3.0, 1.5);
        assert_eq!(PointF::default(), zero);
        assert_eq!(PointF::new(6.0, -3.0), one);
    }

    #[test]
    fn point_f_test_set_to_min_max() {
        let mut a = PointF::new(3.5, 5.5);
        assert_eq!(PointF::new(3.5, 5.5), a);
        a.set_to_max(PointF::new(2.5, 4.5));
        assert_eq!(PointF::new(3.5, 5.5), a);
        a.set_to_max(PointF::new(3.5, 5.5));
        assert_eq!(PointF::new(3.5, 5.5), a);
        a.set_to_max(PointF::new(4.5, 2.5));
        assert_eq!(PointF::new(4.5, 5.5), a);
        a.set_to_max(PointF::new(8.5, 10.5));
        assert_eq!(PointF::new(8.5, 10.5), a);
        a.set_to_min(PointF::new(9.5, 11.5));
        assert_eq!(PointF::new(8.5, 10.5), a);
        a.set_to_min(PointF::new(8.5, 10.5));
        assert_eq!(PointF::new(8.5, 10.5), a);
        a.set_to_min(PointF::new(11.5, 9.5));
        assert_eq!(PointF::new(8.5, 9.5), a);
        a.set_to_min(PointF::new(7.5, 11.5));
        assert_eq!(PointF::new(7.5, 9.5), a);
        a.set_to_min(PointF::new(3.5, 5.5));
        assert_eq!(PointF::new(3.5, 5.5), a);
    }

    #[test]
    fn point_f_test_is_within_distance() {
        let mut pt = PointF::new(10.0, 10.0);
        assert!(pt.is_within_distance(PointF::new(10.0, 10.0), 0.0000000000001));
        assert!(!pt.is_within_distance(PointF::new(8.0, 8.0), 1.0));
        pt = PointF::new(-10.0, -10.0);
        assert!(!pt.is_within_distance(PointF::new(10.0, 10.0), 10.0));
        assert!(pt.is_within_distance(PointF::new(-9.9988, -10.0013), 0.0017689));
        pt = PointF::new(f32::MAX, f32::MAX);
        assert!(!pt.is_within_distance(PointF::new(f32::MIN_POSITIVE, f32::MIN_POSITIVE), 100.0));
    }

    #[test]
    fn point_f_test_transpose() {
        let mut p = PointF::new(-1.5, 2.5);
        assert_eq!(PointF::new(2.5, -1.5), transpose_point_f(p));
        p.transpose();
        assert_eq!(PointF::new(2.5, -1.5), p);
    }

    #[test]
    fn point_f_test_to_string() {
        assert_eq!("1,2", PointF::new(1.0, 2.0).to_string());
        assert_eq!("1.03125,2.5", PointF::new(1.03125, 2.5).to_string());
    }

    // ---- Extra tests for PointF and the conversions ------------------------

    #[test]
    fn point_f_test_partial_ord_with_nan_is_incomparable() {
        let nan = PointF::new(f32::NAN, 0.0);
        assert_eq!(PointF::new(1.0, 2.0).partial_cmp(&PointF::new(1.0, 3.0)), Some(Ordering::Less));
        assert_eq!(
            PointF::new(1.0, 2.0).partial_cmp(&PointF::new(1.0, 2.0)),
            Some(Ordering::Equal)
        );
        assert_eq!(nan.partial_cmp(&nan), None);
    }

    #[test]
    fn point_test_conversion_functions_round_as_documented() {
        let p = PointF::new(-0.5, 2.5);
        assert_eq!(Point::new(-1, 2), to_floored_point(p));
        assert_eq!(Point::new(0, 3), to_ceiled_point(p));
        assert_eq!(Point::new(-1, 3), to_rounded_point(p));
    }

    #[test]
    fn point_test_scale_to_helpers_return_input_for_unit_scale() {
        let p = Point::new(7, -9);
        assert_eq!(p, scale_to_ceiled_point(p, 1.0, 1.0));
        assert_eq!(p, scale_to_floored_point_uniform(p, 1.0));
        assert_eq!(p, scale_to_rounded_point(p, 1.0, 1.0));
        assert_eq!(Point::new(14, -18), scale_to_rounded_point_uniform(p, 2.0));
        assert_eq!(Point::new(4, -4), scale_to_ceiled_point_uniform(p, 0.5));
        assert_eq!(Point::new(3, -5), scale_to_floored_point(p, 0.5, 0.5));
    }

    #[test]
    fn point_f_test_random_round_helpers_match_clamp_reference() {
        let mut rng = XorShift64::new(7);
        for _ in 0..5_000 {
            let v = rng.next_f32_finite_range(-2.0e9, 2.0e9);
            let p = PointF::new(v, -v);
            let r = to_rounded_point(p);
            assert_eq!(r.x(), clamp_round(v));
            assert_eq!(r.y(), clamp_round(-v));
        }
    }
}
