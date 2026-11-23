// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `rect_f.{h,cc}`: `gfx::RectF`, a floating-point rectangle, with its
//! set operations, `map_rect`, `bounding_rect_f`, and the scaling helpers.
//!
//! The arithmetic follows the C++ float code, with the same operand order.
//! `std::min` and `std::max` are `cpp_min` and `cpp_max` from `numeric.rs`, so
//! NaN behaves as it does in C++.
//!
//! Naming: the free functions that C++ overloads on `Rect` have an `_f`
//! suffix here, so they do not collide with the integer versions. Examples:
//! `intersect_rects_f`, `union_rects_f`, `bounding_rect_f`, and
//! `maximum_covered_rect_f`. The C++ `Inset(float)` and `Outset(float)` overloads
//! become `inset_uniform` and `outset_uniform`, and `Offset(h, v)` becomes
//! `offset_xy`, as in `rect.rs`.
//!
//! Differences from the C++ code:
//! - `SplitVertically` and `SplitHorizontally` return `(RectF, RectF)`.
//! - `operator<` is `PartialOrd`. Like the C++ code, it is not a total order
//!   when there is a NaN coordinate.
//! - `IsExpressibleAsRect` checks each value against the `i32` range with a
//!   plain comparison. A NaN is out of range, as it is in C++.
//! - The Apple `CGRect` conversions and `PrintTo` are not ported.

use std::cmp::Ordering;
use std::fmt;

use crate::insets_f::InsetsF;
use crate::numeric::{cpp_max, cpp_min};
use crate::outsets_f::OutsetsF;
use crate::point::{scale_point, PointF};
use crate::rect::Rect;
use crate::size::{scale_size, SizeF};
use crate::vector::Vector2dF;

/// `base::IsValueInRangeForNumericType<int>` for a `float`: true when the
/// value is in the `i32` range. NaN is out of range.
fn in_i32_range(value: f32) -> bool {
    (-2147483648.0..2147483648.0).contains(&value)
}

/// Moves `origin` and `size` so that the span fits inside
/// `[dst_origin, dst_origin + dst_size]`. This is the C++ `AdjustAlongAxis`.
fn adjust_along_axis(dst_origin: f32, dst_size: f32, origin: &mut f32, size: &mut f32) {
    *size = cpp_min(dst_size, *size);
    if *origin < dst_origin {
        *origin = dst_origin;
    } else {
        *origin = cpp_min(dst_origin + dst_size, *origin + *size) - *size;
    }
}

/// A floating-point rectangle with an origin and a size (`gfx::RectF`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    origin: PointF,
    size: SizeF,
}

impl RectF {
    /// `RectF(x, y, width, height)`.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        RectF {
            origin: PointF::new(x, y),
            size: SizeF::new(width, height),
        }
    }

    /// `RectF(width, height)`: the origin is `(0, 0)`.
    pub const fn with_size(width: f32, height: f32) -> Self {
        RectF {
            origin: PointF::new(0.0, 0.0),
            size: SizeF::new(width, height),
        }
    }

    /// `explicit RectF(const SizeF&)`: the origin is `(0, 0)`.
    pub const fn from_size(size: SizeF) -> Self {
        RectF {
            origin: PointF::new(0.0, 0.0),
            size,
        }
    }

    /// `RectF(const PointF& origin, const SizeF& size)`.
    pub const fn from_origin_size(origin: PointF, size: SizeF) -> Self {
        RectF { origin, size }
    }

    pub const fn x(self) -> f32 {
        self.origin.x()
    }

    pub fn set_x(&mut self, x: f32) {
        self.origin.set_x(x);
    }

    pub const fn y(self) -> f32 {
        self.origin.y()
    }

    pub fn set_y(&mut self, y: f32) {
        self.origin.set_y(y);
    }

    pub const fn width(self) -> f32 {
        self.size.width()
    }

    pub fn set_width(&mut self, width: f32) {
        self.size.set_width(width);
    }

    pub const fn height(self) -> f32 {
        self.size.height()
    }

    pub fn set_height(&mut self, height: f32) {
        self.size.set_height(height);
    }

    pub const fn origin(self) -> PointF {
        self.origin
    }

    pub fn set_origin(&mut self, origin: PointF) {
        self.origin = origin;
    }

    pub const fn size(self) -> SizeF {
        self.size
    }

    pub fn set_size(&mut self, size: SizeF) {
        self.size = size;
    }

    pub const fn right(self) -> f32 {
        self.x() + self.width()
    }

    pub const fn bottom(self) -> f32 {
        self.y() + self.height()
    }

    pub const fn top_right(self) -> PointF {
        PointF::new(self.right(), self.y())
    }

    pub const fn bottom_left(self) -> PointF {
        PointF::new(self.x(), self.bottom())
    }

    pub const fn bottom_right(self) -> PointF {
        PointF::new(self.right(), self.bottom())
    }

    pub const fn left_center(self) -> PointF {
        PointF::new(self.x(), self.y() + self.height() / 2.0)
    }

    pub const fn top_center(self) -> PointF {
        PointF::new(self.x() + self.width() / 2.0, self.y())
    }

    pub const fn right_center(self) -> PointF {
        PointF::new(self.right(), self.y() + self.height() / 2.0)
    }

    pub const fn bottom_center(self) -> PointF {
        PointF::new(self.x() + self.width() / 2.0, self.bottom())
    }

    /// The origin as a vector, `(x, y)`.
    pub const fn offset_from_origin(self) -> Vector2dF {
        Vector2dF::new(self.x(), self.y())
    }

    /// Sets the origin and the size (C++ `SetRect`). The size is clamped the
    /// way `SizeF::SetSize` clamps it.
    pub fn set_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.origin.set_point(x, y);
        self.size.set_size(width, height);
    }

    /// Shrinks the rectangle by `inset` on all four sides.
    pub fn inset_uniform(&mut self, inset: f32) {
        self.inset(InsetsF::new(inset));
    }

    /// Shrinks the rectangle by the given insets.
    pub fn inset(&mut self, insets: InsetsF) {
        self.origin += Vector2dF::new(insets.left(), insets.top());
        self.set_width(self.width() - insets.width());
        self.set_height(self.height() - insets.height());
    }

    /// Expands the rectangle by `outset` on all four sides.
    pub fn outset_uniform(&mut self, outset: f32) {
        self.inset(InsetsF::new(-outset));
    }

    /// Expands the rectangle by the given outsets.
    pub fn outset(&mut self, outsets: OutsetsF) {
        self.inset(outsets.to_insets());
    }

    /// Moves the rectangle by `(horizontal, vertical)`.
    pub fn offset_xy(&mut self, horizontal: f32, vertical: f32) {
        self.origin += Vector2dF::new(horizontal, vertical);
    }

    /// Moves the rectangle by `distance`.
    pub fn offset(&mut self, distance: Vector2dF) {
        self.origin += distance;
    }

    /// The insets that take `self` to `inner` (`InsetsFrom`).
    pub fn insets_from(&self, inner: RectF) -> InsetsF {
        InsetsF::tlbr(
            inner.y() - self.y(),
            inner.x() - self.x(),
            self.bottom() - inner.bottom(),
            self.right() - inner.right(),
        )
    }

    /// Returns true if the area is zero.
    pub const fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// Returns true if `(point_x, point_y)` is inside. The right and bottom
    /// edges are excluded.
    pub const fn contains_xy(self, point_x: f32, point_y: f32) -> bool {
        point_x >= self.x() && point_x < self.right() && point_y >= self.y() && point_y < self.bottom()
    }

    /// Returns true if `point` is inside. The right and bottom edges are
    /// excluded.
    pub const fn contains_point(self, point: PointF) -> bool {
        self.contains_xy(point.x(), point.y())
    }

    /// Like [`RectF::contains_xy`], but the right and bottom edges are included.
    pub const fn inclusive_contains_xy(self, point_x: f32, point_y: f32) -> bool {
        point_x >= self.x() && point_x <= self.right() && point_y >= self.y() && point_y <= self.bottom()
    }

    /// Like [`RectF::contains_point`], but the right and bottom edges are included.
    pub const fn inclusive_contains_point(self, point: PointF) -> bool {
        self.inclusive_contains_xy(point.x(), point.y())
    }

    /// Returns true if `rect` is entirely inside this rectangle.
    pub const fn contains(self, rect: RectF) -> bool {
        rect.x() >= self.x()
            && rect.right() <= self.right()
            && rect.y() >= self.y()
            && rect.bottom() <= self.bottom()
    }

    /// Returns true if the two rectangles overlap. An empty rectangle
    /// intersects nothing.
    pub const fn intersects(self, rect: RectF) -> bool {
        !self.is_empty()
            && !rect.is_empty()
            && rect.x() < self.right()
            && rect.right() > self.x()
            && rect.y() < self.bottom()
            && rect.bottom() > self.y()
    }

    /// Sets this rectangle to its intersection with `rect`. An empty result
    /// becomes `(0, 0, 0, 0)`.
    pub fn intersect(&mut self, rect: RectF) {
        if self.is_empty() || rect.is_empty() {
            self.set_rect(0.0, 0.0, 0.0, 0.0);
            return;
        }

        let rx = cpp_max(self.x(), rect.x());
        let ry = cpp_max(self.y(), rect.y());
        let rr = cpp_min(self.right(), rect.right());
        let rb = cpp_min(self.bottom(), rect.bottom());

        if rx >= rr || ry >= rb {
            self.set_rect(0.0, 0.0, 0.0, 0.0);
            return;
        }
        self.set_rect(rx, ry, rr - rx, rb - ry);
    }

    /// Intersects with edge-inclusive geometry. Returns true if the rectangles
    /// touch, and false (setting the result to `(0, 0, 0, 0)`) if they do not.
    pub fn inclusive_intersect(&mut self, rect: RectF) -> bool {
        let rx = cpp_max(self.x(), rect.x());
        let ry = cpp_max(self.y(), rect.y());
        let rr = cpp_min(self.right(), rect.right());
        let rb = cpp_min(self.bottom(), rect.bottom());

        // Return a clean empty rectangle for non-intersecting cases.
        if rx > rr || ry > rb {
            self.set_rect(0.0, 0.0, 0.0, 0.0);
            return false;
        }
        self.set_rect(rx, ry, rr - rx, rb - ry);
        true
    }

    /// Sets this rectangle to the union with `rect`. If this rectangle is empty
    /// it becomes `rect`, and if `rect` is empty it is unchanged.
    pub fn union(&mut self, rect: RectF) {
        if self.is_empty() {
            *self = rect;
            return;
        }
        if rect.is_empty() {
            return;
        }
        self.union_even_if_empty(rect);
    }

    /// Like [`RectF::union`], but the result includes both rectangles even if
    /// either one is empty.
    ///
    /// Float rounding can leave the right or bottom edge just short of the
    /// other rectangle's edge. The size is then bumped to the next
    /// representable float, so the result contains both.
    pub fn union_even_if_empty(&mut self, rect: RectF) {
        let rx = cpp_min(self.x(), rect.x());
        let ry = cpp_min(self.y(), rect.y());
        let rr = cpp_max(self.right(), rect.right());
        let rb = cpp_max(self.bottom(), rect.bottom());
        self.set_rect(rx, ry, rr - rx, rb - ry);

        // Due to floating errors and SizeF's clamping, the new rect may not
        // fully contain the original rects at the right or bottom. Grow it.
        if self.right() < rr && self.width() < f32::MAX {
            self.size.set_to_next_width();
        }
        if self.bottom() < rb && self.height() < f32::MAX {
            self.size.set_to_next_height();
        }
    }

    /// Sets this rectangle to the bounding rectangle of `self` minus `rect`.
    pub fn subtract(&mut self, rect: RectF) {
        if !self.intersects(rect) {
            return;
        }
        if rect.contains(*self) {
            self.set_rect(0.0, 0.0, 0.0, 0.0);
            return;
        }

        let mut rx = self.x();
        let mut ry = self.y();
        let mut rr = self.right();
        let mut rb = self.bottom();

        if rect.y() <= self.y() && rect.bottom() >= self.bottom() {
            // Complete intersection in the y-direction.
            if rect.x() <= self.x() {
                rx = rect.right();
            } else if rect.right() >= self.right() {
                rr = rect.x();
            }
        } else if rect.x() <= self.x() && rect.right() >= self.right() {
            // Complete intersection in the x-direction.
            if rect.y() <= self.y() {
                ry = rect.bottom();
            } else if rect.bottom() >= self.bottom() {
                rb = rect.y();
            }
        }
        self.set_rect(rx, ry, rr - rx, rb - ry);
    }

    /// Fits as much of this rectangle as possible into `rect`.
    pub fn adjust_to_fit(&mut self, rect: RectF) {
        let (mut new_x, mut new_width) = (self.x(), self.width());
        let (mut new_y, mut new_height) = (self.y(), self.height());
        adjust_along_axis(rect.x(), rect.width(), &mut new_x, &mut new_width);
        adjust_along_axis(rect.y(), rect.height(), &mut new_y, &mut new_height);
        self.set_rect(new_x, new_y, new_width, new_height);
    }

    /// The center point (`x + width / 2`, `y + height / 2`).
    pub const fn center_point(self) -> PointF {
        PointF::new(self.x() + self.width() / 2.0, self.y() + self.height() / 2.0)
    }

    /// Caps the size at `size` and keeps the center point.
    pub fn clamp_to_centered_size(&mut self, size: SizeF) {
        let new_width = cpp_min(self.width(), size.width());
        let new_height = cpp_min(self.height(), size.height());
        let new_x = self.x() + (self.width() - new_width) / 2.0;
        let new_y = self.y() + (self.height() - new_height) / 2.0;
        self.set_rect(new_x, new_y, new_width, new_height);
    }

    /// Swaps the x and y axes.
    pub fn transpose(&mut self) {
        self.set_rect(self.y(), self.x(), self.height(), self.width());
    }

    /// Splits into a left half and a right half. Returns `(left, right)`.
    pub fn split_vertically(self) -> (RectF, RectF) {
        let mut left_half = RectF::default();
        left_half.set_rect(self.x(), self.y(), self.width() / 2.0, self.height());
        let mut right_half = RectF::default();
        right_half.set_rect(
            left_half.right(),
            self.y(),
            self.width() - left_half.width(),
            self.height(),
        );
        (left_half, right_half)
    }

    /// Splits into a top half and a bottom half. Returns `(top, bottom)`.
    pub fn split_horizontally(self) -> (RectF, RectF) {
        let mut top_half = RectF::default();
        top_half.set_rect(self.x(), self.y(), self.width(), self.height() / 2.0);
        let mut bottom_half = RectF::default();
        bottom_half.set_rect(
            self.x(),
            top_half.bottom(),
            self.width(),
            self.height() - top_half.height(),
        );
        (top_half, bottom_half)
    }

    /// Returns true if the rectangles share a whole edge and do not overlap.
    pub const fn shares_edge_with(self, rect: RectF) -> bool {
        (self.y() == rect.y()
            && self.height() == rect.height()
            && (self.x() == rect.right() || self.right() == rect.x()))
            || (self.x() == rect.x()
                && self.width() == rect.width()
                && (self.y() == rect.bottom() || self.bottom() == rect.y()))
    }

    /// The Manhattan distance from the rectangle to `point`, or 0 if the point
    /// is inside.
    pub fn manhattan_distance_to_point(self, point: PointF) -> f32 {
        let x_distance = cpp_max(
            0.0,
            cpp_max(self.x() - point.x(), point.x() - self.right()),
        );
        let y_distance = cpp_max(
            0.0,
            cpp_max(self.y() - point.y(), point.y() - self.bottom()),
        );
        x_distance + y_distance
    }

    /// The Manhattan distance between the contents of two rectangles. It is 0
    /// when they overlap. The `f32` epsilon keeps shared edges from giving 0.
    pub fn manhattan_internal_distance(self, rect: RectF) -> f32 {
        let mut c = self;
        c.union(rect);

        let epsilon = f32::EPSILON;
        let x = cpp_max(0.0, c.width() - self.width() - rect.width() + epsilon);
        let y = cpp_max(0.0, c.height() - self.height() - rect.height() + epsilon);
        x + y
    }

    /// The point in the rectangle nearest to `point`.
    pub fn closest_point(self, point: PointF) -> PointF {
        PointF::new(
            cpp_min(cpp_max(point.x(), self.x()), self.right()),
            cpp_min(cpp_max(point.y(), self.y()), self.bottom()),
        )
    }

    /// Scales the origin and the size by `scale`.
    pub fn scale(&mut self, scale: f32) {
        self.scale_xy(scale, scale);
    }

    /// Scales the origin and the size by `x_scale` and `y_scale`.
    pub fn scale_xy(&mut self, x_scale: f32, y_scale: f32) {
        self.set_origin(scale_point(self.origin, x_scale, y_scale));
        self.set_size(scale_size(self.size, x_scale, y_scale));
    }

    /// Divides the origin and the size by `inv_scale`.
    pub fn inv_scale(&mut self, inv_scale: f32) {
        self.inv_scale_xy(inv_scale, inv_scale);
    }

    /// Divides the origin and the size by `x_scale` and `y_scale`.
    pub fn inv_scale_xy(&mut self, x_scale: f32, y_scale: f32) {
        self.origin.inv_scale_xy(x_scale, y_scale);
        self.size.inv_scale_xy(x_scale, y_scale);
    }

    /// Returns true if all six values (`x`, `y`, `width`, `height`, `right`,
    /// `bottom`) fit in an `i32`. This is the C++ `IsExpressibleAsRect`.
    pub fn is_expressible_as_rect(self) -> bool {
        in_i32_range(self.x())
            && in_i32_range(self.y())
            && in_i32_range(self.width())
            && in_i32_range(self.height())
            && in_i32_range(self.right())
            && in_i32_range(self.bottom())
    }

    /// True if each edge is within the matching tolerance of `rect`.
    pub fn approximately_equal(self, rect: RectF, tolerance_x: f32, tolerance_y: f32) -> bool {
        (self.x() - rect.x()).abs() <= tolerance_x
            && (self.y() - rect.y()).abs() <= tolerance_y
            && (self.right() - rect.right()).abs() <= tolerance_x
            && (self.bottom() - rect.bottom()).abs() <= tolerance_y
    }
}

impl From<Rect> for RectF {
    /// The explicit `RectF(const Rect&)` conversion. Each integer converts to
    /// `f32`, which rounds values above 2^24.
    fn from(r: Rect) -> Self {
        RectF::new(
            r.x() as f32,
            r.y() as f32,
            r.width() as f32,
            r.height() as f32,
        )
    }
}

impl PartialOrd for RectF {
    /// The C++ `operator<`: origin first, then width, then height. Each step
    /// returns `None` when a NaN makes the comparison undefined, which
    /// matches the C++ code returning false.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.origin != other.origin {
            return self.origin.partial_cmp(&other.origin);
        }
        if self.width() != other.width() {
            return self.width().partial_cmp(&other.width());
        }
        self.height().partial_cmp(&other.height())
    }
}

impl fmt::Display for RectF {
    /// `"%s %s"` with the origin and the size.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.origin, self.size)
    }
}

impl std::ops::AddAssign<Vector2dF> for RectF {
    fn add_assign(&mut self, offset: Vector2dF) {
        self.origin += offset;
    }
}

impl std::ops::SubAssign<Vector2dF> for RectF {
    fn sub_assign(&mut self, offset: Vector2dF) {
        self.origin -= offset;
    }
}

impl std::ops::Add<Vector2dF> for RectF {
    type Output = RectF;
    fn add(self, rhs: Vector2dF) -> RectF {
        RectF::new(self.x() + rhs.x(), self.y() + rhs.y(), self.width(), self.height())
    }
}

impl std::ops::Sub<Vector2dF> for RectF {
    type Output = RectF;
    fn sub(self, rhs: Vector2dF) -> RectF {
        RectF::new(self.x() - rhs.x(), self.y() - rhs.y(), self.width(), self.height())
    }
}

impl std::ops::Add<RectF> for Vector2dF {
    type Output = RectF;
    fn add(self, rect: RectF) -> RectF {
        rect + self
    }
}

/// The intersection of `a` and `b` (`IntersectRects`).
pub fn intersect_rects_f(a: RectF, b: RectF) -> RectF {
    let mut result = a;
    result.intersect(b);
    result
}

/// The union of `a` and `b` (`UnionRects`).
pub fn union_rects_f(a: RectF, b: RectF) -> RectF {
    let mut result = a;
    result.union(b);
    result
}

/// The union of every rectangle in `rects`. The result is empty for an empty
/// slice.
pub fn union_rects_slice_f(rects: &[RectF]) -> RectF {
    let mut result = RectF::default();
    for &rect in rects {
        result.union(rect);
    }
    result
}

/// The union of `a` and `b` that keeps both rectangles even if one is empty.
pub fn union_rects_even_if_empty_f(a: RectF, b: RectF) -> RectF {
    let mut result = a;
    result.union_even_if_empty(b);
    result
}

/// `a` minus `b`, as the bounding rectangle of the difference.
pub fn subtract_rects_f(a: RectF, b: RectF) -> RectF {
    let mut result = a;
    result.subtract(b);
    result
}

/// Scales `r` by `x_scale` and `y_scale`, scaling each of the four values
/// independently (`ScaleRect`).
pub fn scale_rect(r: RectF, x_scale: f32, y_scale: f32) -> RectF {
    RectF::new(
        r.x() * x_scale,
        r.y() * y_scale,
        r.width() * x_scale,
        r.height() * y_scale,
    )
}

/// Uniform-scale form of [`scale_rect`].
pub fn scale_rect_uniform(r: RectF, scale: f32) -> RectF {
    scale_rect(r, scale, scale)
}

/// Scales `r` by the width and height of `size` (`ScaleRect(r, SizeF)`).
pub fn scale_rect_by_size(r: RectF, size: SizeF) -> RectF {
    scale_rect(r, size.width(), size.height())
}

/// Swaps the x and y axes of `r` (`TransposeRect`).
pub fn transpose_rect_f(r: RectF) -> RectF {
    RectF::new(r.y(), r.x(), r.height(), r.width())
}

/// Builds the rectangle with `p1` at the top-left and `p2` at the bottom-right.
///
/// If the exact width or height cannot be represented as a float, it is grown
/// to the next float so the result includes both points (`BoundingRect`).
pub fn bounding_rect_f(p1: PointF, p2: PointF) -> RectF {
    let left = cpp_min(p1.x(), p2.x());
    let top = cpp_min(p1.y(), p2.y());
    let right = cpp_max(p1.x(), p2.x());
    let bottom = cpp_max(p1.y(), p2.y());
    let mut width = right - left;
    let mut height = bottom - top;

    // If the precision is lost, grow to the next value so both ends are in.
    if left + width != right {
        width = width.next_up();
        if width.is_infinite() {
            width = f32::MAX;
        }
    }
    if top + height != bottom {
        height = height.next_up();
        if height.is_infinite() {
            height = f32::MAX;
        }
    }
    RectF::new(left, top, width, height)
}

/// The largest rectangle covered by `a` or `b` (`MaximumCoveredRect`).
pub fn maximum_covered_rect_f(a: RectF, b: RectF) -> RectF {
    // Check a or b by itself.
    let mut maximum = a;
    let mut maximum_area = a.size().area();
    if b.size().area() > maximum_area {
        maximum = b;
        maximum_area = b.size().area();
    }
    // Check the regions that include the intersection of a and b.
    let mut intersection = a;
    intersection.inclusive_intersect(b);
    if !intersection.size().is_zero() {
        let mut vert_expanded_intersection = intersection;
        vert_expanded_intersection.set_y(cpp_min(a.y(), b.y()));
        vert_expanded_intersection.set_height(
            cpp_max(a.bottom(), b.bottom()) - vert_expanded_intersection.y(),
        );
        if vert_expanded_intersection.size().area() > maximum_area {
            maximum = vert_expanded_intersection;
            maximum_area = vert_expanded_intersection.size().area();
        }
        let mut horiz_expanded_intersection = intersection;
        horiz_expanded_intersection.set_x(cpp_min(a.x(), b.x()));
        horiz_expanded_intersection
            .set_width(cpp_max(a.right(), b.right()) - horiz_expanded_intersection.x());
        if horiz_expanded_intersection.size().area() > maximum_area {
            maximum = horiz_expanded_intersection;
        }
    }
    maximum
}

/// Maps `r` from the coordinate space of `src_rect` to that of `dest_rect`.
/// An empty `src_rect` gives the empty rectangle `(0, 0, 0, 0)` (`MapRect`).
pub fn map_rect(r: RectF, src_rect: RectF, dest_rect: RectF) -> RectF {
    if src_rect.is_empty() {
        return RectF::default();
    }
    let width_scale = dest_rect.width() / src_rect.width();
    let height_scale = dest_rect.height() / src_rect.height();
    RectF::new(
        dest_rect.x() + (r.x() - src_rect.x()) * width_scale,
        dest_rect.y() + (r.y() - src_rect.y()) * height_scale,
        r.width() * width_scale,
        r.height() * height_scale,
    )
}

#[cfg(test)]
mod tests;
