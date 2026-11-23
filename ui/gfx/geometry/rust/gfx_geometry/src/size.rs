// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `gfx::Size` and `gfx::SizeF`, plus `size_conversions` and the scaling
//! helpers from `size.cc` and `size_f.cc`.

use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::format::format_g;
use crate::numeric::{clamp_ceil, clamp_floor, clamp_round, cpp_max, cpp_min};

/// A size with non-negative integer width and height. Negative inputs are
/// clamped to 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Size {
    width: i32,
    height: i32,
}

impl Size {
    pub const fn new(width: i32, height: i32) -> Self {
        Size {
            width: if width > 0 { width } else { 0 },
            height: if height > 0 { height } else { 0 },
        }
    }

    pub const fn width(self) -> i32 {
        self.width
    }

    pub const fn height(self) -> i32 {
        self.height
    }

    pub fn set_width(&mut self, width: i32) {
        self.width = width.max(0);
    }

    pub fn set_height(&mut self, height: i32) {
        self.height = height.max(0);
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
        self.set_width(width);
        self.set_height(height);
    }

    /// Adds `grow_width` and `grow_height`, saturating at the `i32` limits and
    /// clamping the result to be non-negative.
    pub fn enlarge(&mut self, grow_width: i32, grow_height: i32) {
        self.set_size(
            self.width.saturating_add(grow_width),
            self.height.saturating_add(grow_height),
        );
    }

    /// Returns the area, panicking if it does not fit in `i32`. This is
    /// `Size::GetArea()`, which CHECK-fails in the same case.
    pub fn area(self) -> i32 {
        self.checked_area()
            .expect("Size::area() overflows int")
    }

    /// Returns the area, or `None` if it does not fit in `i32`. This is
    /// `Size::GetCheckedArea()`.
    pub fn checked_area(self) -> Option<i32> {
        self.width.checked_mul(self.height)
    }

    pub fn area64(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn set_to_min(&mut self, other: Size) {
        self.width = self.width.min(other.width);
        self.height = self.height.min(other.height);
    }

    pub fn set_to_max(&mut self, other: Size) {
        self.width = self.width.max(other.width);
        self.height = self.height.max(other.height);
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn is_zero(self) -> bool {
        self.width == 0 && self.height == 0
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.width, &mut self.height);
    }
}

impl fmt::Display for Size {
    /// `"%dx%d"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl AddAssign for Size {
    fn add_assign(&mut self, other: Size) {
        self.enlarge(other.width, other.height);
    }
}

impl SubAssign for Size {
    fn sub_assign(&mut self, other: Size) {
        // Widths are never negative, so negation cannot overflow.
        self.enlarge(-other.width, -other.height);
    }
}

impl Add for Size {
    type Output = Size;
    fn add(mut self, rhs: Size) -> Size {
        self += rhs;
        self
    }
}

impl Sub for Size {
    type Output = Size;
    fn sub(mut self, rhs: Size) -> Size {
        self -= rhs;
        self
    }
}

pub const fn transpose_size(s: Size) -> Size {
    Size::new(s.height, s.width)
}

/// A size with float width and height.
///
/// Dimensions at or below `TRIVIAL` (8 * float epsilon) are clamped to 0 on
/// every write. This includes NaN, which becomes 0. Positive infinity is kept.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeF {
    width: f32,
    height: f32,
}

/// Dimensions at or below this value are treated as zero.
const TRIVIAL: f32 = 8.0 * f32::EPSILON;

fn clamp_trivial(f: f32) -> f32 {
    if f > TRIVIAL {
        f
    } else {
        0.0
    }
}

/// Returns the smallest representable value above `f`, but not below
/// `TRIVIAL`. The result is capped at `f32::MAX`. This is the C++ `next()`,
/// which uses `std::nextafter(std::max(kTrivial, f), FLT_MAX)`.
fn next_width(f: f32) -> f32 {
    let x = cpp_max(TRIVIAL, f);
    if x >= f32::MAX {
        f32::MAX
    } else {
        x.next_up()
    }
}

impl SizeF {
    pub const fn new(width: f32, height: f32) -> Self {
        SizeF {
            width: if width > TRIVIAL { width } else { 0.0 },
            height: if height > TRIVIAL { height } else { 0.0 },
        }
    }

    pub const fn width(self) -> f32 {
        self.width
    }

    pub const fn height(self) -> f32 {
        self.height
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = clamp_trivial(width);
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = clamp_trivial(height);
    }

    pub fn set_size(&mut self, width: f32, height: f32) {
        self.set_width(width);
        self.set_height(height);
    }

    pub fn area(self) -> f32 {
        self.width * self.height
    }

    pub fn aspect_ratio(self) -> f32 {
        self.width / self.height
    }

    pub fn enlarge(&mut self, grow_width: f32, grow_height: f32) {
        self.set_size(self.width + grow_width, self.height + grow_height);
    }

    /// Uses `std::min` semantics, see `numeric::cpp_min`. The result is not
    /// re-clamped, as in C++.
    pub fn set_to_min(&mut self, other: SizeF) {
        self.width = cpp_min(self.width, other.width);
        self.height = cpp_min(self.height, other.height);
    }

    pub fn set_to_max(&mut self, other: SizeF) {
        self.width = cpp_max(self.width, other.width);
        self.height = cpp_max(self.height, other.height);
    }

    /// Expands the width to the next representable value, but not below
    /// `TRIVIAL`.
    pub fn set_to_next_width(&mut self) {
        self.width = next_width(self.width);
    }

    /// Expands the height to the next representable value, but not below
    /// `TRIVIAL`.
    pub fn set_to_next_height(&mut self) {
        self.height = next_width(self.height);
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0.0 || self.height == 0.0
    }

    pub const fn is_zero(self) -> bool {
        self.width == 0.0 && self.height == 0.0
    }

    pub fn scale(&mut self, scale: f32) {
        self.scale_xy(scale, scale);
    }

    pub fn scale_xy(&mut self, x_scale: f32, y_scale: f32) {
        self.set_size(self.width * x_scale, self.height * y_scale);
    }

    /// Divides both dimensions by `inv_scale`. The result is not clamped, as
    /// in C++.
    pub fn inv_scale(&mut self, inv_scale: f32) {
        self.inv_scale_xy(inv_scale, inv_scale);
    }

    pub fn inv_scale_xy(&mut self, inv_x_scale: f32, inv_y_scale: f32) {
        self.width /= inv_x_scale;
        self.height /= inv_y_scale;
    }

    pub fn transpose(&mut self) {
        std::mem::swap(&mut self.width, &mut self.height);
    }
}

impl fmt::Display for SizeF {
    /// `"%gx%g"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}",
            format_g(self.width as f64),
            format_g(self.height as f64)
        )
    }
}

impl From<Size> for SizeF {
    /// The explicit `SizeF(const Size&)` conversion. The result is clamped by
    /// the constructor, as in C++.
    fn from(s: Size) -> SizeF {
        SizeF::new(s.width() as f32, s.height() as f32)
    }
}

impl AddAssign for SizeF {
    fn add_assign(&mut self, other: SizeF) {
        self.set_size(self.width + other.width, self.height + other.height);
    }
}

impl SubAssign for SizeF {
    fn sub_assign(&mut self, other: SizeF) {
        self.set_size(self.width - other.width, self.height - other.height);
    }
}

impl Add for SizeF {
    type Output = SizeF;
    fn add(self, rhs: SizeF) -> SizeF {
        SizeF::new(self.width + rhs.width, self.height + rhs.height)
    }
}

impl Sub for SizeF {
    type Output = SizeF;
    fn sub(self, rhs: SizeF) -> SizeF {
        SizeF::new(self.width - rhs.width, self.height - rhs.height)
    }
}

pub fn scale_size(s: SizeF, x_scale: f32, y_scale: f32) -> SizeF {
    let mut scaled = s;
    scaled.scale_xy(x_scale, y_scale);
    scaled
}

pub fn scale_size_uniform(s: SizeF, scale: f32) -> SizeF {
    scale_size(s, scale, scale)
}

pub const fn transpose_size_f(s: SizeF) -> SizeF {
    SizeF::new(s.height, s.width)
}

/// `ToFlooredSize`: each dimension is floored and saturated to `i32`.
pub fn to_floored_size(size: SizeF) -> Size {
    Size::new(clamp_floor(size.width), clamp_floor(size.height))
}

/// `ToCeiledSize`: each dimension is ceiled and saturated to `i32`.
pub fn to_ceiled_size(size: SizeF) -> Size {
    Size::new(clamp_ceil(size.width), clamp_ceil(size.height))
}

/// `ToRoundedSize`: each dimension is rounded half away from zero and
/// saturated to `i32`.
pub fn to_rounded_size(size: SizeF) -> Size {
    Size::new(clamp_round(size.width), clamp_round(size.height))
}

// As with the point helpers, a scale of exactly 1 returns the input unchanged.

pub fn scale_to_ceiled_size(size: Size, x_scale: f32, y_scale: f32) -> Size {
    if x_scale == 1.0 && y_scale == 1.0 {
        return size;
    }
    to_ceiled_size(scale_size(SizeF::from(size), x_scale, y_scale))
}

pub fn scale_to_ceiled_size_uniform(size: Size, scale: f32) -> Size {
    if scale == 1.0 {
        return size;
    }
    to_ceiled_size(scale_size_uniform(SizeF::from(size), scale))
}

pub fn scale_to_floored_size(size: Size, x_scale: f32, y_scale: f32) -> Size {
    if x_scale == 1.0 && y_scale == 1.0 {
        return size;
    }
    to_floored_size(scale_size(SizeF::from(size), x_scale, y_scale))
}

pub fn scale_to_floored_size_uniform(size: Size, scale: f32) -> Size {
    if scale == 1.0 {
        return size;
    }
    to_floored_size(scale_size_uniform(SizeF::from(size), scale))
}

pub fn scale_to_rounded_size(size: Size, x_scale: f32, y_scale: f32) -> Size {
    if x_scale == 1.0 && y_scale == 1.0 {
        return size;
    }
    to_rounded_size(scale_size(SizeF::from(size), x_scale, y_scale))
}

pub fn scale_to_rounded_size_uniform(size: Size, scale: f32) -> Size {
    if scale == 1.0 {
        return size;
    }
    to_rounded_size(scale_size_uniform(SizeF::from(size), scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric::clamp_floor;
    use crate::test_util::{float_eq, XorShift64};

    // ---- size_unittest.cc -------------------------------------------------

    #[test]
    fn size_test_set_to_min_max() {
        let mut a = Size::default();
        a = Size::new(3, 5);
        assert_eq!(Size::new(3, 5).to_string(), a.to_string());
        a.set_to_max(Size::new(2, 4));
        assert_eq!(Size::new(3, 5).to_string(), a.to_string());
        a.set_to_max(Size::new(3, 5));
        assert_eq!(Size::new(3, 5).to_string(), a.to_string());
        a.set_to_max(Size::new(4, 2));
        assert_eq!(Size::new(4, 5).to_string(), a.to_string());
        a.set_to_max(Size::new(8, 10));
        assert_eq!(Size::new(8, 10).to_string(), a.to_string());
        a.set_to_min(Size::new(9, 11));
        assert_eq!(Size::new(8, 10).to_string(), a.to_string());
        a.set_to_min(Size::new(8, 10));
        assert_eq!(Size::new(8, 10).to_string(), a.to_string());
        a.set_to_min(Size::new(11, 9));
        assert_eq!(Size::new(8, 9).to_string(), a.to_string());
        a.set_to_min(Size::new(7, 11));
        assert_eq!(Size::new(7, 9).to_string(), a.to_string());
        a.set_to_min(Size::new(3, 5));
        assert_eq!(Size::new(3, 5).to_string(), a.to_string());
    }

    #[test]
    fn size_test_enlarge() {
        let mut test = Size::new(3, 4);
        test.enlarge(5, -8);
        assert_eq!(test, Size::new(8, -4));
    }

    #[test]
    fn size_test_integer_overflow() {
        let int_max = i32::MAX;
        let int_min = i32::MIN;

        let max_size = Size::new(int_max, int_max);
        let min_size = Size::new(int_min, int_min);
        let mut test;

        test = Size::default();
        test.enlarge(int_max, int_max);
        assert_eq!(test, max_size);

        test = Size::default();
        test.enlarge(int_min, int_min);
        assert_eq!(test, min_size);

        test = Size::new(10, 20);
        test.enlarge(int_max, int_max);
        assert_eq!(test, max_size);

        test = Size::new(-10, -20);
        test.enlarge(int_min, int_min);
        assert_eq!(test, min_size);
    }

    #[test]
    fn size_test_operator_add_sub() {
        let mut lhs = Size::new(100, 20);
        let rhs = Size::new(50, 10);
        lhs += rhs;
        assert_eq!(Size::new(150, 30), lhs);
        lhs = Size::new(100, 20);
        assert_eq!(Size::new(150, 30), lhs + rhs);
        lhs = Size::new(100, 20);
        lhs -= rhs;
        assert_eq!(Size::new(50, 10), lhs);
        lhs = Size::new(100, 20);
        assert_eq!(Size::new(50, 10), lhs - rhs);
    }

    #[test]
    fn size_test_operator_add_overflow() {
        let int_max = i32::MAX;
        let lhs = Size::new(int_max, int_max);
        let rhs = Size::new(int_max, int_max);
        assert_eq!(Size::new(int_max, int_max), lhs + rhs);
    }

    #[test]
    fn size_test_operator_sub_clamp_at_zero() {
        let lhs = Size::new(10, 10);
        let rhs = Size::new(100, 100);
        assert_eq!(Size::new(0, 0), lhs - rhs);
        let mut lhs = Size::new(10, 10);
        let rhs = Size::new(100, 100);
        lhs -= rhs;
        assert_eq!(Size::new(0, 0), lhs);
    }

    #[test]
    fn size_test_operator_compare() {
        let lhs = Size::new(100, 20);
        let mut rhs = Size::new(50, 10);
        assert!(lhs != rhs);
        assert!(!(lhs == rhs));
        rhs = Size::new(100, 20);
        assert!(lhs == rhs);
        assert!(!(lhs != rhs));
    }

    #[test]
    fn size_test_transpose() {
        let mut s = Size::new(1, 2);
        assert_eq!(Size::new(2, 1), transpose_size(s));
        s.transpose();
        assert_eq!(Size::new(2, 1), s);
    }

    // ---- size_f_unittest.cc -----------------------------------------------

    #[test]
    fn size_f_test_size_to_size_f() {
        // Check that explicit conversion from integer to float works.
        let a = Size::new(10, 20);
        assert_eq!(10.0, SizeF::from(a).width());
        assert_eq!(20.0, SizeF::from(a).height());
        let b = SizeF::new(10.0, 20.0);
        assert_eq!(b, SizeF::from(a));
    }

    #[test]
    fn size_f_test_to_floored_size() {
        assert_eq!(Size::new(0, 0), to_floored_size(SizeF::new(0.0, 0.0)));
        assert_eq!(Size::new(0, 0), to_floored_size(SizeF::new(0.0001, 0.0001)));
        assert_eq!(Size::new(0, 0), to_floored_size(SizeF::new(0.4999, 0.4999)));
        assert_eq!(Size::new(0, 0), to_floored_size(SizeF::new(0.5, 0.5)));
        assert_eq!(Size::new(0, 0), to_floored_size(SizeF::new(0.9999, 0.9999)));
        assert_eq!(Size::new(10, 10), to_floored_size(SizeF::new(10.0, 10.0)));
        assert_eq!(Size::new(10, 10), to_floored_size(SizeF::new(10.0001, 10.0001)));
        assert_eq!(Size::new(10, 10), to_floored_size(SizeF::new(10.4999, 10.4999)));
        assert_eq!(Size::new(10, 10), to_floored_size(SizeF::new(10.5, 10.5)));
        assert_eq!(Size::new(10, 10), to_floored_size(SizeF::new(10.9999, 10.9999)));
    }

    #[test]
    fn size_f_test_to_ceiled_size() {
        assert_eq!(Size::new(0, 0), to_ceiled_size(SizeF::new(0.0, 0.0)));
        assert_eq!(Size::new(1, 1), to_ceiled_size(SizeF::new(0.0001, 0.0001)));
        assert_eq!(Size::new(1, 1), to_ceiled_size(SizeF::new(0.4999, 0.4999)));
        assert_eq!(Size::new(1, 1), to_ceiled_size(SizeF::new(0.5, 0.5)));
        assert_eq!(Size::new(1, 1), to_ceiled_size(SizeF::new(0.9999, 0.9999)));
        assert_eq!(Size::new(10, 10), to_ceiled_size(SizeF::new(10.0, 10.0)));
        assert_eq!(Size::new(11, 11), to_ceiled_size(SizeF::new(10.0001, 10.0001)));
        assert_eq!(Size::new(11, 11), to_ceiled_size(SizeF::new(10.4999, 10.4999)));
        assert_eq!(Size::new(11, 11), to_ceiled_size(SizeF::new(10.5, 10.5)));
        assert_eq!(Size::new(11, 11), to_ceiled_size(SizeF::new(10.9999, 10.9999)));
    }

    #[test]
    fn size_f_test_to_rounded_size() {
        assert_eq!(Size::new(0, 0), to_rounded_size(SizeF::new(0.0, 0.0)));
        assert_eq!(Size::new(0, 0), to_rounded_size(SizeF::new(0.0001, 0.0001)));
        assert_eq!(Size::new(0, 0), to_rounded_size(SizeF::new(0.4999, 0.4999)));
        assert_eq!(Size::new(1, 1), to_rounded_size(SizeF::new(0.5, 0.5)));
        assert_eq!(Size::new(1, 1), to_rounded_size(SizeF::new(0.9999, 0.9999)));
        assert_eq!(Size::new(10, 10), to_rounded_size(SizeF::new(10.0, 10.0)));
        assert_eq!(Size::new(10, 10), to_rounded_size(SizeF::new(10.0001, 10.0001)));
        assert_eq!(Size::new(10, 10), to_rounded_size(SizeF::new(10.4999, 10.4999)));
        assert_eq!(Size::new(11, 11), to_rounded_size(SizeF::new(10.5, 10.5)));
        assert_eq!(Size::new(11, 11), to_rounded_size(SizeF::new(10.9999, 10.9999)));
    }

    #[test]
    fn size_f_test_set_to_min_max() {
        let mut a = SizeF::default();
        a = SizeF::new(3.5, 5.5);
        assert_eq!(SizeF::new(3.5, 5.5).to_string(), a.to_string());
        a.set_to_max(SizeF::new(2.5, 4.5));
        assert_eq!(SizeF::new(3.5, 5.5).to_string(), a.to_string());
        a.set_to_max(SizeF::new(3.5, 5.5));
        assert_eq!(SizeF::new(3.5, 5.5).to_string(), a.to_string());
        a.set_to_max(SizeF::new(4.5, 2.5));
        assert_eq!(SizeF::new(4.5, 5.5).to_string(), a.to_string());
        a.set_to_max(SizeF::new(8.5, 10.5));
        assert_eq!(SizeF::new(8.5, 10.5).to_string(), a.to_string());
        a.set_to_min(SizeF::new(9.5, 11.5));
        assert_eq!(SizeF::new(8.5, 10.5).to_string(), a.to_string());
        a.set_to_min(SizeF::new(8.5, 10.5));
        assert_eq!(SizeF::new(8.5, 10.5).to_string(), a.to_string());
        a.set_to_min(SizeF::new(11.5, 9.5));
        assert_eq!(SizeF::new(8.5, 9.5).to_string(), a.to_string());
        a.set_to_min(SizeF::new(7.5, 11.5));
        assert_eq!(SizeF::new(7.5, 9.5).to_string(), a.to_string());
        a.set_to_min(SizeF::new(3.5, 5.5));
        assert_eq!(SizeF::new(3.5, 5.5).to_string(), a.to_string());
    }

    #[test]
    fn size_f_test_operator_add_sub() {
        let mut lhs = SizeF::new(100.5, 20.0);
        let rhs = SizeF::new(50.0, 10.25);
        lhs += rhs;
        assert_eq!(SizeF::new(150.5, 30.25), lhs);
        lhs = SizeF::new(100.0, 20.25);
        assert_eq!(SizeF::new(150.0, 30.5), lhs + rhs);
        lhs = SizeF::new(100.5, 20.0);
        lhs -= rhs;
        assert_eq!(SizeF::new(50.5, 9.75), lhs);
        lhs = SizeF::new(100.0, 20.75);
        assert_eq!(SizeF::new(50.0, 10.5), lhs - rhs);
        assert_eq!(SizeF::new(0.0, 0.0), rhs - lhs);
        let mut rhs = rhs;
        rhs -= lhs;
        assert_eq!(SizeF::new(0.0, 0.0), rhs);
    }

    #[test]
    fn size_f_test_is_empty() {
        let clearly_trivial = TRIVIAL / 2.0;
        let massize_dimension = 4e13_f32;

        // First, using the constructor.
        assert!(SizeF::new(clearly_trivial, 1.0).is_empty());
        assert!(SizeF::new(0.01, clearly_trivial).is_empty());
        assert!(SizeF::new(0.0, 0.0).is_empty());
        assert!(!SizeF::new(0.01, 0.01).is_empty());

        // Then use the setter.
        let mut test = SizeF::new(2.0, 1.0);
        assert!(!test.is_empty());
        test.set_size(clearly_trivial, 1.0);
        assert!(test.is_empty());
        test.set_size(0.01, clearly_trivial);
        assert!(test.is_empty());
        test.set_size(0.0, 0.0);
        assert!(test.is_empty());
        test.set_size(0.01, 0.01);
        assert!(!test.is_empty());

        // Now just one dimension at a time.
        test.set_width(clearly_trivial);
        assert!(test.is_empty());
        test.set_width(massize_dimension);
        test.set_height(clearly_trivial);
        assert!(test.is_empty());
        test.set_width(clearly_trivial);
        test.set_height(massize_dimension);
        assert!(test.is_empty());
        test.set_width(2.0);
        assert!(!test.is_empty());
    }

    #[test]
    fn size_f_test_clamps_to_zero() {
        let clearly_trivial = TRIVIAL / 2.0;
        let nearly_trivial = TRIVIAL * 1.5;

        let mut test = SizeF::new(clearly_trivial, 1.0);
        assert!(float_eq(0.0, test.width()));
        assert!(float_eq(1.0, test.height()));

        test.set_size(0.01, clearly_trivial);
        assert!(float_eq(0.01, test.width()));
        assert!(float_eq(0.0, test.height()));

        test.set_size(nearly_trivial, nearly_trivial);
        assert!(float_eq(nearly_trivial, test.width()));
        assert!(float_eq(nearly_trivial, test.height()));

        test.scale(0.5);
        assert!(float_eq(0.0, test.width()));
        assert!(float_eq(0.0, test.height()));

        test.set_size(0.0, 0.0);
        test.enlarge(clearly_trivial, clearly_trivial);
        test.enlarge(clearly_trivial, clearly_trivial);
        test.enlarge(clearly_trivial, clearly_trivial);
        assert_eq!(SizeF::new(0.0, 0.0), test);
    }

    #[test]
    fn size_f_test_consistent_clamping() {
        let mut resized = SizeF::default();
        resized.set_size(TRIVIAL, 0.0);
        assert_eq!(SizeF::new(TRIVIAL, 0.0), resized);
        resized.set_size(0.0, TRIVIAL);
        assert_eq!(SizeF::new(0.0, TRIVIAL), resized);
    }

    #[test]
    fn size_f_test_transpose() {
        let mut s = SizeF::new(1.5, 2.5);
        assert_eq!(SizeF::new(2.5, 1.5), transpose_size_f(s));
        s.transpose();
        assert_eq!(SizeF::new(2.5, 1.5), s);
    }

    #[test]
    fn size_f_test_to_string() {
        assert_eq!("1x2", SizeF::new(1.0, 2.0).to_string());
        assert_eq!("1.03125x2.5", SizeF::new(1.03125, 2.5).to_string());
    }

    // ---- Extra tests ------------------------------------------------------

    #[test]
    fn size_f_test_next_width_grows_by_one_ulp_and_caps_at_max() {
        let mut s = SizeF::new(1.0, 1.0);
        s.set_to_next_width();
        assert_eq!(1.0_f32.next_up(), s.width());
        assert_eq!(1.0, s.height());

        let mut zero = SizeF::default();
        zero.set_to_next_height();
        assert_eq!(TRIVIAL.next_up(), zero.height());

        let mut big = SizeF::new(f32::MAX, 1.0);
        big.set_to_next_width();
        assert_eq!(f32::MAX, big.width());
    }

    #[test]
    fn size_f_test_nan_and_infinity_clamping() {
        assert!(SizeF::new(f32::NAN, 1.0).width() == 0.0);
        assert_eq!(f32::INFINITY, SizeF::new(f32::INFINITY, 1.0).width());
    }

    #[test]
    fn size_f_test_random_rounding_matches_clamp_floor() {
        let mut rng = XorShift64::new(11);
        for _ in 0..5_000 {
            let v = rng.next_f32_finite_range(-1.0e9, 1.0e9);
            let s = SizeF::new(v, v);
            let floored = to_floored_size(s);
            if v > TRIVIAL {
                assert_eq!(floored.width(), clamp_floor(v));
            } else {
                assert_eq!(floored.width(), 0);
            }
        }
    }

    #[test]
    fn size_test_area_and_checked_area() {
        assert_eq!(6, Size::new(2, 3).area());
        assert_eq!(Some(6), Size::new(2, 3).checked_area());
        assert_eq!(None, Size::new(i32::MAX, 2).checked_area());
        assert_eq!(2 * i32::MAX as u64, Size::new(i32::MAX, 2).area64());
    }

    #[test]
    #[should_panic(expected = "overflows int")]
    fn size_test_area_panics_on_overflow() {
        let _ = Size::new(i32::MAX, 2).area();
    }

    #[test]
    fn size_test_scale_to_helpers() {
        let s = Size::new(7, 9);
        assert_eq!(s, scale_to_ceiled_size(s, 1.0, 1.0));
        assert_eq!(s, scale_to_rounded_size_uniform(s, 1.0));
        assert_eq!(Size::new(14, 18), scale_to_rounded_size_uniform(s, 2.0));
        assert_eq!(Size::new(4, 5), scale_to_ceiled_size(s, 0.5, 0.5));
        assert_eq!(Size::new(3, 4), scale_to_floored_size_uniform(s, 0.5));
    }

    #[test]
    fn size_test_from_negative_clamps_to_zero() {
        assert_eq!(Size::new(0, 0), Size::new(-5, -1));
        let mut s = Size::new(3, 3);
        s.set_width(-1);
        assert_eq!(Size::new(0, 3), s);
        s.set_to_min(Size::new(2, 2));
        assert_eq!(Size::new(0, 2), s);
    }
}
