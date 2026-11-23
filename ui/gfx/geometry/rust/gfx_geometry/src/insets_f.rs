// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `insets_f.{h,cc}`: `gfx::InsetsF`, the floating-point version of
//! `Insets`, and `ScaleInsets`. The struct and its arithmetic come from
//! `insets_outsets_f_base.rs`. The C++ `PrintTo` gtest hook is not ported.

use crate::insets::Insets;
use crate::insets_outsets_f_base::define_insets_outsets_f_base;
use crate::outsets_f::OutsetsF;

define_insets_outsets_f_base!(
    /// The floating-point widths of the four edges of a rectangle, `gfx::InsetsF`.
    InsetsF
);

impl InsetsF {
    /// Converts to `OutsetsF` by negating every edge (C++ `InsetsF::ToOutsets`).
    pub fn to_outsets(self) -> OutsetsF {
        OutsetsF::tlbr(-self.top(), -self.left(), -self.bottom(), -self.right())
    }
}

/// The C++ `explicit operator InsetsF()` conversion. Each integer edge is
/// converted to `f32`, which can round values above 2^24.
impl From<Insets> for InsetsF {
    fn from(insets: Insets) -> Self {
        InsetsF::tlbr(
            insets.top() as f32,
            insets.left() as f32,
            insets.bottom() as f32,
            insets.right() as f32,
        )
    }
}

/// Scales every edge of `insets` by `x_scale` (horizontal) and `y_scale`
/// (vertical) (`ScaleInsets`).
pub fn scale_insets(mut insets: InsetsF, x_scale: f32, y_scale: f32) -> InsetsF {
    insets.scale_xy(x_scale, y_scale);
    insets
}

/// Uniform-scale form of [`scale_insets`], which takes the scale for both
/// axes.
pub fn scale_insets_uniform(insets: InsetsF, scale: f32) -> InsetsF {
    scale_insets(insets, scale, scale)
}
