// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `rect.{h,cc}`: `gfx::Rect`, an integer rectangle, with its
//! set operations (`intersect_rects`, `union_rects`, `subtract_rects`,
//! `bounding_rect`, `maximum_covered_rect`) and the scaling helpers
//! `scale_to_enclosing_rect`, `scale_to_enclosed_rect`,
//! `scale_to_rounded_rect` and `scale_to_enclosing_rect_ignoring_error`.
//!
//! The containment semantics are array-like: `(x, y)` is inside the rectangle
//! and `(x + width, y)` is not. The `right()` and `bottom()` edges are exclusive.
//!
//! The width and height are clamped so that `x + width` and `y + height`
//! never overflow `i32`. That keeps `right()` and `bottom()` plain sums.
//!
//! Differences from the C++ code:
//! - Constructors and setters are named for Rust. `Rect(int, int)` is
//!   `Rect::with_size`, `Rect(Size)` is `Rect::from_size`, and `Rect(Point,
//!   Size)` is `Rect::from_origin_size`. The C++ overloads that take one
//!   integer (`Inset(int)`, `Outset(int)`, `Offset(int, int)`) become
//!   `inset_uniform`, `outset_uniform` and `offset_xy`.
//! - Out-parameter functions return values: `SplitVertically` and
//!   `SplitHorizontally` return `(Rect, Rect)`.
//! - `operator<` is `Ord`, ordered by origin, then width, then height. This
//!   matches the C++ comparison.
//! - Where the C++ integer arithmetic could overflow (which is undefined
//!   behavior), the port saturates or widens to `i64`. Results are the same
//!   whenever the C++ does not overflow.
//! - `Win32 RECT` and `CGRect` conversions, the gtest `PrintTo` hook, and the
//!   `ToString()` `%s` formatting are not ported in the same way. `ToString()`
//!   is `Display`.
//! - `Rect::operator==` is `PartialEq`, which is derived.

use std::cmp::Ordering;
use std::fmt;

use crate::insets::Insets;
use crate::outsets::Outsets;
use crate::point::Point;
use crate::rect_conversions::to_enclosing_rect_ignoring_error;
use crate::rect_f::RectF;
use crate::size::Size;
use crate::vector::Vector2d;

/// Returns the width or height that keeps `x_or_y + width_or_height` inside
/// `i32`. This is the C++ `ClampWidthOrHeight`, which is
/// `base::ClampAdd(x_or_y, width_or_height) - x_or_y`.
const fn clamp_width_or_height(x_or_y: i32, width_or_height: i32) -> i32 {
    (x_or_y.saturating_add(width_or_height) as i64 - x_or_y as i64) as i32
}

/// The C++ `SaturatedClampRange`: picks an origin and span for the range
/// `[min, max]` when the exact span does not fit in an `i32`. Returns
/// `(origin, span)`.
fn saturated_clamp_range(min: i32, max: i32) -> (i32, i32) {
    if max < min {
        return (min, 0);
    }
    let effective_span = (max as i64 - min as i64).min(i32::MAX as i64) as i32;
    let span_loss = max as i64 - (min as i64 + effective_span as i64);

    // If the span fits in an i32, the simple computation is exact.
    if span_loss == 0 {
        return (min, effective_span);
    }

    // Otherwise approximate. Keep the side that is close to zero exact, and
    // move the other side, which is probably practically "infinite".
    const MAX_DIMENSION: u32 = (i32::MAX / 2) as u32;
    if max.unsigned_abs() < MAX_DIMENSION {
        // Maintain origin + span == max.
        ((max as i64 - effective_span as i64) as i32, effective_span)
    } else if min.unsigned_abs() < MAX_DIMENSION {
        // Maintain origin == min.
        (min, effective_span)
    } else {
        // Both are big, so keep the center.
        ((min as i64 + span_loss / 2) as i32, effective_span)
    }
}

/// An integer rectangle with an origin and a size (`gfx::Rect`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    origin: Point,
    size: Size,
}

impl Rect {
    /// `Rect(x, y, width, height)`. A negative width or height is treated as
    /// zero, and the width is clamped so the far edge does not overflow.
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Rect {
            origin: Point::new(x, y),
            size: Size::new(
                clamp_width_or_height(x, width),
                clamp_width_or_height(y, height),
            ),
        }
    }

    /// `Rect(width, height)`: the origin is `(0, 0)`.
    pub const fn with_size(width: i32, height: i32) -> Self {
        Rect {
            origin: Point::new(0, 0),
            size: Size::new(width, height),
        }
    }

    /// `explicit Rect(const Size&)`: the origin is `(0, 0)`.
    pub const fn from_size(size: Size) -> Self {
        Rect {
            origin: Point::new(0, 0),
            size,
        }
    }

    /// `Rect(const Point& origin, const Size& size)`.
    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Rect {
            origin,
            size: Size::new(
                clamp_width_or_height(origin.x(), size.width()),
                clamp_width_or_height(origin.y(), size.height()),
            ),
        }
    }

    pub const fn x(self) -> i32 {
        self.origin.x()
    }

    /// Sets the x position and keeps the width.
    pub fn set_x(&mut self, x: i32) {
        self.origin.set_x(x);
        self.size.set_width(clamp_width_or_height(x, self.width()));
    }

    pub const fn y(self) -> i32 {
        self.origin.y()
    }

    /// Sets the y position and keeps the height.
    pub fn set_y(&mut self, y: i32) {
        self.origin.set_y(y);
        self.size.set_height(clamp_width_or_height(y, self.height()));
    }

    pub const fn width(self) -> i32 {
        self.size.width()
    }

    pub fn set_width(&mut self, width: i32) {
        self.size.set_width(clamp_width_or_height(self.x(), width));
    }

    pub const fn height(self) -> i32 {
        self.size.height()
    }

    pub fn set_height(&mut self, height: i32) {
        self.size.set_height(clamp_width_or_height(self.y(), height));
    }

    pub const fn origin(self) -> Point {
        self.origin
    }

    /// Sets the origin and re-clamps the width and height.
    pub fn set_origin(&mut self, origin: Point) {
        self.origin = origin;
        let (w, h) = (self.width(), self.height());
        self.set_width(w);
        self.set_height(h);
    }

    pub const fn size(self) -> Size {
        self.size
    }

    pub fn set_size(&mut self, size: Size) {
        self.set_width(size.width());
        self.set_height(size.height());
    }

    /// The exclusive right edge, `x + width`.
    pub const fn right(self) -> i32 {
        self.x() + self.width()
    }

    /// The exclusive bottom edge, `y + height`.
    pub const fn bottom(self) -> i32 {
        self.y() + self.height()
    }

    pub const fn top_right(self) -> Point {
        Point::new(self.right(), self.y())
    }

    pub const fn bottom_left(self) -> Point {
        Point::new(self.x(), self.bottom())
    }

    pub const fn bottom_right(self) -> Point {
        Point::new(self.right(), self.bottom())
    }

    pub const fn left_center(self) -> Point {
        Point::new(self.x(), self.y() + self.height() / 2)
    }

    pub const fn top_center(self) -> Point {
        Point::new(self.x() + self.width() / 2, self.y())
    }

    pub const fn right_center(self) -> Point {
        Point::new(self.right(), self.y() + self.height() / 2)
    }

    pub const fn bottom_center(self) -> Point {
        Point::new(self.x() + self.width() / 2, self.bottom())
    }

    /// The origin as a vector, `(x, y)`.
    pub const fn offset_from_origin(self) -> Vector2d {
        Vector2d::new(self.x(), self.y())
    }

    /// The C++ `SetRect`: sets the origin and then the width and height, which
    /// are clamped to stay inside `i32`.
    pub fn set_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.origin.set_point(x, y);
        self.set_width(width);
        self.set_height(height);
    }

    /// Sets the edges from `left`, `top`, `right` and `bottom`. This is safe
    /// when the width or height would overflow.
    pub fn set_by_bounds(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        self.set_horizontal_bounds(left, right);
        self.set_vertical_bounds(top, bottom);
    }

    pub fn set_horizontal_bounds(&mut self, left: i32, right: i32) {
        self.set_x(left);
        self.set_width(right.saturating_sub(left));
        if self.right() != right {
            self.adjust_for_saturated_right(right);
        }
    }

    pub fn set_vertical_bounds(&mut self, top: i32, bottom: i32) {
        self.set_y(top);
        self.set_height(bottom.saturating_sub(top));
        if self.bottom() != bottom {
            self.adjust_for_saturated_bottom(bottom);
        }
    }

    fn adjust_for_saturated_right(&mut self, right: i32) {
        let (new_x, width) = saturated_clamp_range(self.x(), right);
        self.set_x(new_x);
        self.size.set_width(width);
    }

    fn adjust_for_saturated_bottom(&mut self, bottom: i32) {
        let (new_y, height) = saturated_clamp_range(self.y(), bottom);
        self.set_y(new_y);
        self.size.set_height(height);
    }

    /// Shrinks the rectangle by `inset` on all four sides.
    pub fn inset_uniform(&mut self, inset: i32) {
        self.inset(Insets::new(inset));
    }

    /// Shrinks the rectangle by the given insets (C++ `Inset(const Insets&)`).
    pub fn inset(&mut self, insets: Insets) {
        self.origin += Vector2d::new(insets.left(), insets.top());
        self.set_width(self.width().saturating_sub(insets.width()));
        self.set_height(self.height().saturating_sub(insets.height()));
    }

    /// Expands the rectangle by `outset` on all four sides.
    pub fn outset_uniform(&mut self, outset: i32) {
        self.inset_uniform(outset.saturating_neg());
    }

    /// Expands the rectangle by the given outsets (C++ `Outset(const Outsets&)`).
    pub fn outset(&mut self, outsets: Outsets) {
        self.inset(outsets.to_insets());
    }

    /// Moves the rectangle by a horizontal and vertical distance.
    pub fn offset_xy(&mut self, horizontal: i32, vertical: i32) {
        self.offset(Vector2d::new(horizontal, vertical));
    }

    /// Moves the rectangle by `distance`.
    pub fn offset(&mut self, distance: Vector2d) {
        self.origin += distance;
        // Keep the width and height valid.
        let (w, h) = (self.width(), self.height());
        self.set_width(w);
        self.set_height(h);
    }

    /// Returns the insets that take `self` to `inner`. This is the C++
    /// `InsetsFrom`, which subtracts with saturation where C++ would overflow.
    pub fn insets_from(&self, inner: Rect) -> Insets {
        Insets::tlbr(
            inner.y().saturating_sub(self.y()),
            inner.x().saturating_sub(self.x()),
            self.bottom().saturating_sub(inner.bottom()),
            self.right().saturating_sub(inner.right()),
        )
    }

    /// Returns true if the area of the rectangle is zero.
    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// Returns true if `(point_x, point_y)` is inside the rectangle. The
    /// bottom and right edges are excluded.
    pub const fn contains_xy(self, point_x: i32, point_y: i32) -> bool {
        point_x >= self.x() && point_x < self.right() && point_y >= self.y() && point_y < self.bottom()
    }

    /// Returns true if `point` is inside the rectangle.
    pub const fn contains_point(self, point: Point) -> bool {
        self.contains_xy(point.x(), point.y())
    }

    /// Returns true if `rect` is entirely inside this rectangle.
    pub const fn contains(self, rect: Rect) -> bool {
        rect.x() >= self.x()
            && rect.right() <= self.right()
            && rect.y() >= self.y()
            && rect.bottom() <= self.bottom()
    }

    /// Returns true if the two rectangles overlap. An empty rectangle
    /// intersects nothing.
    pub fn intersects(self, rect: Rect) -> bool {
        !(self.is_empty()
            || rect.is_empty()
            || rect.x() >= self.right()
            || rect.right() <= self.x()
            || rect.y() >= self.bottom()
            || rect.bottom() <= self.y())
    }

    /// Sets this rectangle to its intersection with `rect`. An empty result
    /// becomes the zero rectangle `(0, 0, 0, 0)`.
    pub fn intersect(&mut self, rect: Rect) {
        if self.is_empty() || rect.is_empty() {
            self.set_rect(0, 0, 0, 0); // Throws away empty position.
            return;
        }

        let left = self.x().max(rect.x());
        let top = self.y().max(rect.y());
        let new_right = self.right().min(rect.right());
        let new_bottom = self.bottom().min(rect.bottom());

        if left >= new_right || top >= new_bottom {
            self.set_rect(0, 0, 0, 0); // Throws away empty position.
            return;
        }

        self.set_by_bounds(left, top, new_right, new_bottom);
    }

    /// Intersects with edge-inclusive geometry. Returns true if the two
    /// rectangles touch, even when the overlap has zero area. Returns false
    /// and sets the result to `(0, 0, 0, 0)` when they do not touch.
    pub fn inclusive_intersect(&mut self, rect: Rect) -> bool {
        let left = self.x().max(rect.x());
        let top = self.y().max(rect.y());
        let new_right = self.right().min(rect.right());
        let new_bottom = self.bottom().min(rect.bottom());

        // Return a clean empty rectangle for non-intersecting cases.
        if left > new_right || top > new_bottom {
            self.set_rect(0, 0, 0, 0);
            return false;
        }

        self.set_by_bounds(left, top, new_right, new_bottom);
        true
    }

    /// Sets this rectangle to the union with `rect`. If this rectangle is
    /// empty it becomes `rect`, and if `rect` is empty it is unchanged.
    pub fn union(&mut self, rect: Rect) {
        if self.is_empty() {
            *self = rect;
            return;
        }
        if rect.is_empty() {
            return;
        }

        self.union_even_if_empty(rect);
    }

    /// Like [`Rect::union`], but the result includes both rectangles even if
    /// either one is empty. For example, the union of `(100, 100, 0x0)` and
    /// `(200, 200, 50x0)` is `(100, 100, 150x100)`.
    pub fn union_even_if_empty(&mut self, rect: Rect) {
        self.set_by_bounds(
            self.x().min(rect.x()),
            self.y().min(rect.y()),
            self.right().max(rect.right()),
            self.bottom().max(rect.bottom()),
        );
    }

    /// Sets this rectangle to the bounding rectangle of `self` minus `rect`,
    /// i.e. `Region(self) - Region(rect)`.
    pub fn subtract(&mut self, rect: Rect) {
        if !self.intersects(rect) {
            return;
        }
        if rect.contains(*self) {
            self.set_rect(0, 0, 0, 0);
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
        self.set_by_bounds(rx, ry, rr, rb);
    }

    /// Fits as much of this rectangle as possible into `rect`. For example, an
    /// x-position of 2 with width 4 fit into `(0, 5)` becomes `x = 1, width = 4`.
    pub fn adjust_to_fit(&mut self, rect: Rect) {
        let (mut new_x, mut new_width) = (self.x(), self.width());
        let (mut new_y, mut new_height) = (self.y(), self.height());
        adjust_along_axis(rect.x(), rect.width(), &mut new_x, &mut new_width);
        adjust_along_axis(rect.y(), rect.height(), &mut new_y, &mut new_height);
        self.set_rect(new_x, new_y, new_width, new_height);
    }

    /// The center point, rounded toward the origin (`x + width / 2`).
    pub const fn center_point(self) -> Point {
        Point::new(self.x() + self.width() / 2, self.y() + self.height() / 2)
    }

    /// Becomes a rectangle with the same center point and the given `size`.
    pub fn to_centered_size(&mut self, size: Size) {
        let new_x = self.x() + (self.width() - size.width()) / 2;
        let new_y = self.y() + (self.height() - size.height()) / 2;
        self.set_rect(new_x, new_y, size.width(), size.height());
    }

    /// Becomes a rectangle with the same center point and a size capped at
    /// `to_size`.
    pub fn clamp_to_centered_size(&mut self, to_size: Size) {
        let mut new_size = self.size();
        new_size.set_to_min(to_size);
        self.to_centered_size(new_size);
    }

    /// Swaps the x and y axes.
    pub fn transpose(&mut self) {
        self.set_rect(self.y(), self.x(), self.height(), self.width());
    }

    /// Splits this rectangle into a left half and a right half. When the width
    /// is odd, the right half is one pixel wider. Returns `(left, right)`.
    pub fn split_vertically(self) -> (Rect, Rect) {
        let mut left_half = Rect::default();
        left_half.set_rect(self.x(), self.y(), self.width() / 2, self.height());
        let mut right_half = Rect::default();
        right_half.set_rect(
            left_half.right(),
            self.y(),
            self.width() - left_half.width(),
            self.height(),
        );
        (left_half, right_half)
    }

    /// Splits this rectangle into a top half and a bottom half. When the height
    /// is odd, the bottom half is one pixel taller. Returns `(top, bottom)`.
    pub fn split_horizontally(self) -> (Rect, Rect) {
        let mut top_half = Rect::default();
        top_half.set_rect(self.x(), self.y(), self.width(), self.height() / 2);
        let mut bottom_half = Rect::default();
        bottom_half.set_rect(
            self.x(),
            top_half.bottom(),
            self.width(),
            self.height() - top_half.height(),
        );
        (top_half, bottom_half)
    }

    /// Returns true if this rectangle shares a whole edge with `rect` (the same
    /// height and touching horizontally, or the same width and touching
    /// vertically). The rectangles must not overlap.
    pub const fn shares_edge_with(self, rect: Rect) -> bool {
        (self.y() == rect.y()
            && self.height() == rect.height()
            && (self.x() == rect.right() || self.right() == rect.x()))
            || (self.x() == rect.x()
                && self.width() == rect.width()
                && (self.y() == rect.bottom() || self.bottom() == rect.y()))
    }

    /// The Manhattan distance from the rectangle to `point`, or 0 if the point
    /// is inside. The sum saturates at `i32::MAX`.
    pub fn manhattan_distance_to_point(self, point: Point) -> i32 {
        let x_distance = i32::max(
            0,
            self.x().saturating_sub(point.x()).max(point.x().saturating_sub(self.right())),
        );
        let y_distance = i32::max(
            0,
            self.y().saturating_sub(point.y()).max(point.y().saturating_sub(self.bottom())),
        );
        x_distance.saturating_add(y_distance)
    }

    /// The Manhattan distance between the contents of two rectangles. It is 0
    /// if they overlap, and the smallest non-zero value if they share an edge.
    pub fn manhattan_internal_distance(self, rect: Rect) -> i32 {
        let mut c = self;
        c.union(rect);

        let x = (c.width() as i64 - self.width() as i64 - rect.width() as i64 + 1).max(0);
        let y = (c.height() as i64 - self.height() as i64 - rect.height() as i64 + 1).max(0);
        (x + y).min(i32::MAX as i64) as i32
    }

    /// True if each edge is within `tolerance` of the matching edge of `rect`.
    pub fn approximately_equal(self, rect: Rect, tolerance: i32) -> bool {
        let t = tolerance as i64;
        (self.x() as i64 - rect.x() as i64).abs() <= t
            && (self.y() as i64 - rect.y() as i64).abs() <= t
            && (self.right() as i64 - rect.right() as i64).abs() <= t
            && (self.bottom() as i64 - rect.bottom() as i64).abs() <= t
    }
}

/// Moves `origin` and `size` so that `[origin, origin + size]` fits inside
/// `[dst_origin, dst_origin + dst_size]`. This is the C++ `AdjustAlongAxis`.
fn adjust_along_axis(dst_origin: i32, dst_size: i32, origin: &mut i32, size: &mut i32) {
    *size = dst_size.min(*size);
    if *origin < dst_origin {
        *origin = dst_origin;
    } else {
        *origin = dst_origin
            .saturating_add(dst_size)
            .min((*origin).saturating_add(*size))
            .saturating_sub(*size);
    }
}

impl PartialOrd for Rect {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Orders by origin, then width, then height (C++ `Rect::operator<`).
impl Ord for Rect {
    fn cmp(&self, other: &Self) -> Ordering {
        self.origin
            .cmp(&other.origin)
            .then_with(|| self.width().cmp(&other.width()))
            .then_with(|| self.height().cmp(&other.height()))
    }
}

impl fmt::Display for Rect {
    /// `"%s %s"` with the origin and the size: `"x,y wxh"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.origin, self.size)
    }
}

impl std::ops::AddAssign<Vector2d> for Rect {
    fn add_assign(&mut self, offset: Vector2d) {
        self.offset(offset);
    }
}

impl std::ops::SubAssign<Vector2d> for Rect {
    fn sub_assign(&mut self, offset: Vector2d) {
        self.offset(-offset);
    }
}

impl std::ops::Add<Vector2d> for Rect {
    type Output = Rect;
    fn add(mut self, offset: Vector2d) -> Rect {
        self += offset;
        self
    }
}

impl std::ops::Sub<Vector2d> for Rect {
    type Output = Rect;
    fn sub(mut self, offset: Vector2d) -> Rect {
        self -= offset;
        self
    }
}

impl std::ops::Add<Rect> for Vector2d {
    type Output = Rect;
    fn add(self, rect: Rect) -> Rect {
        rect + self
    }
}

/// The intersection of `a` and `b` (`IntersectRects`).
pub fn intersect_rects(a: Rect, b: Rect) -> Rect {
    let mut result = a;
    result.intersect(b);
    result
}

/// The union of `a` and `b` (`UnionRects`).
pub fn union_rects(a: Rect, b: Rect) -> Rect {
    let mut result = a;
    result.union(b);
    result
}

/// The union of every rectangle in `rects`. The result is empty for an empty
/// slice (`UnionRects(base::span<const Rect>)`).
pub fn union_rects_slice(rects: &[Rect]) -> Rect {
    let mut result = Rect::default();
    for &rect in rects {
        result.union(rect);
    }
    result
}

/// The union of `a` and `b` that keeps both rectangles even if one is empty
/// (`UnionRectsEvenIfEmpty`).
pub fn union_rects_even_if_empty(a: Rect, b: Rect) -> Rect {
    let mut result = a;
    result.union_even_if_empty(b);
    result
}

/// `a` minus `b`, as the bounding rectangle of the difference (`SubtractRects`).
pub fn subtract_rects(a: Rect, b: Rect) -> Rect {
    let mut result = a;
    result.subtract(b);
    result
}

/// The rectangle with `p1` and `p2` as opposite corners. This is the smallest
/// rectangle containing both points, but points on the right or bottom edge
/// are outside (`BoundingRect`).
pub fn bounding_rect(p1: Point, p2: Point) -> Rect {
    let mut result = Rect::default();
    result.set_by_bounds(
        p1.x().min(p2.x()),
        p1.y().min(p2.y()),
        p1.x().max(p2.x()),
        p1.y().max(p2.y()),
    );
    result
}

/// Scales `rect` by `x_scale` and `y_scale` and returns the enclosing rectangle,
/// clamping each component that would overflow (`ScaleToEnclosingRect`).
pub fn scale_to_enclosing_rect(rect: Rect, x_scale: f32, y_scale: f32) -> Rect {
    use crate::numeric::{clamp_ceil, clamp_floor};
    if x_scale == 1.0 && y_scale == 1.0 {
        return rect;
    }
    let x = clamp_floor(rect.x() as f32 * x_scale);
    let y = clamp_floor(rect.y() as f32 * y_scale);
    let r = if rect.width() == 0 { x } else { clamp_ceil(rect.right() as f32 * x_scale) };
    let b = if rect.height() == 0 { y } else { clamp_ceil(rect.bottom() as f32 * y_scale) };
    let mut result = Rect::default();
    result.set_by_bounds(x, y, r, b);
    result
}

/// Uniform-scale form of [`scale_to_enclosing_rect`].
pub fn scale_to_enclosing_rect_uniform(rect: Rect, scale: f32) -> Rect {
    scale_to_enclosing_rect(rect, scale, scale)
}

/// Scales `rect` and returns the largest rectangle inside the scaled one
/// (`ScaleToEnclosedRect`).
pub fn scale_to_enclosed_rect(rect: Rect, x_scale: f32, y_scale: f32) -> Rect {
    use crate::numeric::{clamp_ceil, clamp_floor};
    if x_scale == 1.0 && y_scale == 1.0 {
        return rect;
    }
    let x = clamp_ceil(rect.x() as f32 * x_scale);
    let y = clamp_ceil(rect.y() as f32 * y_scale);
    let r = if rect.width() == 0 { x } else { clamp_floor(rect.right() as f32 * x_scale) };
    let b = if rect.height() == 0 { y } else { clamp_floor(rect.bottom() as f32 * y_scale) };
    let mut result = Rect::default();
    result.set_by_bounds(x, y, r, b);
    result
}

/// Uniform-scale form of [`scale_to_enclosed_rect`].
pub fn scale_to_enclosed_rect_uniform(rect: Rect, scale: f32) -> Rect {
    scale_to_enclosed_rect(rect, scale, scale)
}

/// Scales the four corners of `rect` and rounds each to the nearest integer
/// (`ScaleToRoundedRect`). This keeps the relative positions of rectangles
/// when they are scaled.
pub fn scale_to_rounded_rect(rect: Rect, x_scale: f32, y_scale: f32) -> Rect {
    use crate::numeric::clamp_round;
    if x_scale == 1.0 && y_scale == 1.0 {
        return rect;
    }
    let x = clamp_round(rect.x() as f32 * x_scale);
    let y = clamp_round(rect.y() as f32 * y_scale);
    let r = if rect.width() == 0 { x } else { clamp_round(rect.right() as f32 * x_scale) };
    let b = if rect.height() == 0 { y } else { clamp_round(rect.bottom() as f32 * y_scale) };
    let mut result = Rect::default();
    result.set_by_bounds(x, y, r, b);
    result
}

/// Uniform-scale form of [`scale_to_rounded_rect`].
pub fn scale_to_rounded_rect_uniform(rect: Rect, scale: f32) -> Rect {
    scale_to_rounded_rect(rect, scale, scale)
}

/// Scales `rect` by `scale` and takes the enclosing rectangle, snapping an edge
/// to the integer grid when it is within `error` of it
/// (`ScaleToEnclosingRectIgnoringError`). The C++ default `error` is `0.001`,
/// which cc and viz use. Pass that value to match them.
pub fn scale_to_enclosing_rect_ignoring_error(rect: Rect, scale: f32, error: f32) -> Rect {
    let mut rect_f = RectF::from(rect);
    rect_f.scale(scale);
    to_enclosing_rect_ignoring_error(rect_f, error)
}

/// The largest rectangle covered by `a` or `b`: the bigger of the two, or a
/// rectangle formed by extending their intersection (`MaximumCoveredRect`).
pub fn maximum_covered_rect(a: Rect, b: Rect) -> Rect {
    // Check a or b by itself.
    let mut maximum = a;
    let mut maximum_area = a.size().area64();
    if b.size().area64() > maximum_area {
        maximum = b;
        maximum_area = b.size().area64();
    }
    // Check the regions that include the intersection of a and b. Expanding
    // the intersection vertically or horizontally keeps it covered by a or b.
    let mut intersection = a;
    intersection.inclusive_intersect(b);
    if !intersection.size().is_zero() {
        let mut vert_expanded_intersection = intersection;
        vert_expanded_intersection
            .set_vertical_bounds(a.y().min(b.y()), a.bottom().max(b.bottom()));
        if vert_expanded_intersection.size().area64() > maximum_area {
            maximum = vert_expanded_intersection;
            maximum_area = vert_expanded_intersection.size().area64();
        }
        let mut horiz_expanded_intersection = intersection;
        horiz_expanded_intersection
            .set_horizontal_bounds(a.x().min(b.x()), a.right().max(b.right()));
        if horiz_expanded_intersection.size().area64() > maximum_area {
            maximum = horiz_expanded_intersection;
        }
    }
    maximum
}

#[cfg(test)]
mod tests;
