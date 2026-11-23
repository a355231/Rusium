// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `insets.{h,cc}`: `gfx::Insets`, the integer widths of the top,
//! left, bottom and right borders of a rectangle, and the scale helpers
//! `ScaleToCeiledInsets`, `ScaleToFlooredInsets` and `ScaleToRoundedInsets`.
//!
//! The struct and its arithmetic come from `insets_outsets_base.rs`. Unlike C++,
//! `-x` for `i32::MIN` saturates to `i32::MAX` instead of overflowing, as the
//! C++ `base::ClampedNumeric` negation does. The C++ `PrintTo` gtest hook is not
//! ported.

use crate::insets_conversions::{to_ceiled_insets, to_floored_insets, to_rounded_insets};
use crate::insets_f::{scale_insets, InsetsF};
use crate::insets_outsets_base::define_insets_outsets_base;
use crate::outsets::Outsets;
use crate::vector::Vector2d;

define_insets_outsets_base!(
    /// The integer widths of the four edges of a rectangle, `gfx::Insets`.
    ///
    /// Insets shrink a rectangle: `Rect::inset` removes `top`, `left`, `bottom`
    /// and `right` from the edges. They are also used as border widths.
    Insets
);

impl Insets {
    /// Converts to `Outsets` by negating every edge (C++ `Insets::ToOutsets`).
    pub fn to_outsets(self) -> Outsets {
        let mut v = Outsets::default();
        v.set_left_right(self.left().saturating_neg(), self.right().saturating_neg())
            .set_top_bottom(self.top().saturating_neg(), self.bottom().saturating_neg());
        v
    }

    /// Adjusts the vertical and horizontal dimensions by `vector`. Offsetting
    /// the insets and then applying them to a rectangle matches offsetting
    /// the rectangle and then applying the insets.
    pub fn offset(&mut self, vector: Vector2d) {
        let left = self.left().saturating_add(vector.x());
        let right = self.right().saturating_sub(vector.x());
        let top = self.top().saturating_add(vector.y());
        let bottom = self.bottom().saturating_sub(vector.y());
        self.set_left_right(left, right).set_top_bottom(top, bottom);
    }
}

impl std::ops::Add<Vector2d> for Insets {
    type Output = Insets;
    fn add(mut self, offset: Vector2d) -> Insets {
        self.offset(offset);
        self
    }
}

/// Scales `insets` and rounds each edge up (`ScaleToCeiledInsets`).
pub fn scale_to_ceiled_insets(insets: Insets, x_scale: f32, y_scale: f32) -> Insets {
    if x_scale == 1.0 && y_scale == 1.0 {
        return insets;
    }
    to_ceiled_insets(scale_insets(InsetsF::from(insets), x_scale, y_scale))
}

/// Uniform-scale form of [`scale_to_ceiled_insets`].
pub fn scale_to_ceiled_insets_uniform(insets: Insets, scale: f32) -> Insets {
    if scale == 1.0 {
        return insets;
    }
    to_ceiled_insets(scale_insets(InsetsF::from(insets), scale, scale))
}

/// Scales `insets` and rounds each edge down (`ScaleToFlooredInsets`).
pub fn scale_to_floored_insets(insets: Insets, x_scale: f32, y_scale: f32) -> Insets {
    if x_scale == 1.0 && y_scale == 1.0 {
        return insets;
    }
    to_floored_insets(scale_insets(InsetsF::from(insets), x_scale, y_scale))
}

/// Uniform-scale form of [`scale_to_floored_insets`].
pub fn scale_to_floored_insets_uniform(insets: Insets, scale: f32) -> Insets {
    if scale == 1.0 {
        return insets;
    }
    to_floored_insets(scale_insets(InsetsF::from(insets), scale, scale))
}

/// Scales `insets` and rounds each edge to the nearest integer
/// (`ScaleToRoundedInsets`).
pub fn scale_to_rounded_insets(insets: Insets, x_scale: f32, y_scale: f32) -> Insets {
    if x_scale == 1.0 && y_scale == 1.0 {
        return insets;
    }
    to_rounded_insets(scale_insets(InsetsF::from(insets), x_scale, y_scale))
}

/// Uniform-scale form of [`scale_to_rounded_insets`].
pub fn scale_to_rounded_insets_uniform(insets: Insets, scale: f32) -> Insets {
    if scale == 1.0 {
        return insets;
    }
    to_rounded_insets(scale_insets(InsetsF::from(insets), scale, scale))
}
