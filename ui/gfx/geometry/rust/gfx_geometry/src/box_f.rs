// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `box_f.{h,cc}`: `gfx::BoxF`, a 3D version of `RectF`, with the
//! positive z axis pointing toward the camera.
//!
//! Differences from the C++ code:
//! - The C++ `DCHECK_LE` checks in `ExpandTo(min, max)` are `debug_assert!`s.
//! - `BoxF::ToString()` uses `%f` for the three sizes. [`format_f`] reproduces
//!   that, including `-0.000000`, `nan` and `inf`.
//! - The C++ `PrintTo` gtest hook is not ported. `to_string()` is `Display`.

use std::fmt;

use crate::format::format_f;
use crate::numeric::{cpp_max, cpp_min};
use crate::point3::Point3F;
use crate::vector3::Vector3dF;

/// A box with an origin and non-negative width, height and depth
/// (`gfx::BoxF`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxF {
    origin: Point3F,
    width: f32,
    height: f32,
    depth: f32,
}

/// Clamps a size to `>= 0`. Like the C++ `x >= 0 ? x : 0`, NaN becomes 0.
const fn non_negative(v: f32) -> f32 {
    if v >= 0.0 {
        v
    } else {
        0.0
    }
}

/// Clamps a size to `>= 0` for the setters. Like the C++ `x < 0 ? 0 : x`, NaN
/// is kept.
fn clamp_below_zero(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else {
        v
    }
}

impl Default for BoxF {
    fn default() -> Self {
        BoxF::new(0.0, 0.0, 0.0)
    }
}

impl BoxF {
    /// `BoxF(width, height, depth)` with the origin at `(0, 0, 0)`.
    pub const fn new(width: f32, height: f32, depth: f32) -> Self {
        Self::from_origin(Point3F::new(0.0, 0.0, 0.0), width, height, depth)
    }

    /// `BoxF(x, y, z, width, height, depth)`.
    pub const fn new_at(x: f32, y: f32, z: f32, width: f32, height: f32, depth: f32) -> Self {
        Self::from_origin(Point3F::new(x, y, z), width, height, depth)
    }

    /// `BoxF(const Point3F& origin, width, height, depth)`. Negative sizes
    /// become zero.
    pub const fn from_origin(origin: Point3F, width: f32, height: f32, depth: f32) -> Self {
        BoxF {
            origin,
            width: non_negative(width),
            height: non_negative(height),
            depth: non_negative(depth),
        }
    }

    /// Scales all three axes by `scale`.
    pub fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    /// Scales each axis by its own factor.
    pub fn scale_xyz(&mut self, x_scale: f32, y_scale: f32, z_scale: f32) {
        self.origin.scale_xyz(x_scale, y_scale, z_scale);
        self.set_size(self.width * x_scale, self.height * y_scale, self.depth * z_scale);
    }

    /// Moves the box by `offset`.
    pub fn offset(&mut self, offset: Vector3dF) {
        self.origin += offset;
    }

    /// Returns true if the box has no interior points: at least two of its
    /// three sizes are zero.
    pub fn is_empty(&self) -> bool {
        (self.width == 0.0 && self.height == 0.0)
            || (self.width == 0.0 && self.depth == 0.0)
            || (self.height == 0.0 && self.depth == 0.0)
    }

    /// Sets this box to the union with `box_`, the smallest box containing
    /// both. An empty box is replaced by `box_`, and an empty `box_` is ignored.
    pub fn union(&mut self, box_: BoxF) {
        if self.is_empty() {
            *self = box_;
            return;
        }
        if box_.is_empty() {
            return;
        }
        self.expand_to_box(box_);
    }

    /// Expands this box to contain `point`, if necessary. Even if the box is
    /// empty, it still contains its origin afterward.
    pub fn expand_to_point(&mut self, point: Point3F) {
        self.expand_to_min_max(point, point);
    }

    /// Expands this box to contain `box_`, if necessary.
    pub fn expand_to_box(&mut self, box_: BoxF) {
        self.expand_to_min_max(
            box_.origin(),
            Point3F::new(box_.right(), box_.bottom(), box_.front()),
        );
    }

    /// Expands the box to contain both `min` and `max`, and `origin + size`.
    /// Each component of `min` must be at most the matching component of `max`.
    fn expand_to_min_max(&mut self, min: Point3F, max: Point3F) {
        debug_assert!(min.x() <= max.x());
        debug_assert!(min.y() <= max.y());
        debug_assert!(min.z() <= max.z());

        let min_x = cpp_min(self.x(), min.x());
        let min_y = cpp_min(self.y(), min.y());
        let min_z = cpp_min(self.z(), min.z());
        let max_x = cpp_max(self.right(), max.x());
        let max_y = cpp_max(self.bottom(), max.y());
        let max_z = cpp_max(self.front(), max.z());

        self.origin.set_point(min_x, min_y, min_z);
        self.width = max_x - min_x;
        self.height = max_y - min_y;
        self.depth = max_z - min_z;
    }

    pub const fn x(&self) -> f32 {
        self.origin.x()
    }

    pub fn set_x(&mut self, x: f32) {
        self.origin.set_x(x);
    }

    pub const fn y(&self) -> f32 {
        self.origin.y()
    }

    pub fn set_y(&mut self, y: f32) {
        self.origin.set_y(y);
    }

    pub const fn z(&self) -> f32 {
        self.origin.z()
    }

    pub fn set_z(&mut self, z: f32) {
        self.origin.set_z(z);
    }

    pub const fn width(&self) -> f32 {
        self.width
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = clamp_below_zero(width);
    }

    pub const fn height(&self) -> f32 {
        self.height
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = clamp_below_zero(height);
    }

    pub const fn depth(&self) -> f32 {
        self.depth
    }

    pub fn set_depth(&mut self, depth: f32) {
        self.depth = clamp_below_zero(depth);
    }

    /// The far edge in x, `x + width`.
    pub const fn right(&self) -> f32 {
        self.x() + self.width
    }

    /// The far edge in y, `y + height`.
    pub const fn bottom(&self) -> f32 {
        self.y() + self.height
    }

    /// The far edge in z, `z + depth`.
    pub const fn front(&self) -> f32 {
        self.z() + self.depth
    }

    /// Sets the three sizes, each clamped with [`clamp_below_zero`].
    pub fn set_size(&mut self, width: f32, height: f32, depth: f32) {
        self.width = clamp_below_zero(width);
        self.height = clamp_below_zero(height);
        self.depth = clamp_below_zero(depth);
    }

    pub const fn origin(&self) -> Point3F {
        self.origin
    }

    pub fn set_origin(&mut self, origin: Point3F) {
        self.origin = origin;
    }
}

impl fmt::Display for BoxF {
    /// `"%s %fx%fx%f"`: the origin, then the width, height and depth.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}x{}x{}",
            self.origin,
            format_f(self.width),
            format_f(self.height),
            format_f(self.depth)
        )
    }
}

impl std::ops::AddAssign<Vector3dF> for BoxF {
    fn add_assign(&mut self, offset: Vector3dF) {
        self.offset(offset);
    }
}

impl std::ops::Add<Vector3dF> for BoxF {
    type Output = BoxF;
    fn add(self, v: Vector3dF) -> BoxF {
        BoxF::new_at(
            self.x() + v.x(),
            self.y() + v.y(),
            self.z() + v.z(),
            self.width,
            self.height,
            self.depth,
        )
    }
}

/// The smallest box containing `a` and `b` (`UnionBoxes`).
pub fn union_boxes(a: BoxF, b: BoxF) -> BoxF {
    let mut result = a;
    result.union(b);
    result
}

/// Scales each axis of `b` by its own factor. The origin and the sizes scale
/// together (`ScaleBox`).
pub fn scale_box(b: BoxF, x_scale: f32, y_scale: f32, z_scale: f32) -> BoxF {
    BoxF::new_at(
        b.x() * x_scale,
        b.y() * y_scale,
        b.z() * z_scale,
        b.width() * x_scale,
        b.height() * y_scale,
        b.depth() * z_scale,
    )
}

/// Uniform-scale form of [`scale_box`].
pub fn scale_box_uniform(b: BoxF, scale: f32) -> BoxF {
    scale_box(b, scale, scale, scale)
}

#[cfg(test)]
mod tests;
