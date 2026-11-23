// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `outsets.h`: `gfx::Outsets`, the integer widths of the margins
//! around a rectangle. Its struct and arithmetic come from
//! `insets_outsets_base.rs`. The C++ `PrintTo` gtest hook is not ported.

use crate::insets::Insets;
use crate::insets_outsets_base::define_insets_outsets_base;

define_insets_outsets_base!(
    /// The integer widths of the four margins around a rectangle,
    /// `gfx::Outsets`. `Rect::outset` grows a rectangle by these amounts.
    Outsets
);

impl Outsets {
    /// Converts to `Insets` by negating every edge (C++ `Outsets::ToInsets`).
    pub fn to_insets(self) -> Insets {
        let mut v = Insets::default();
        v.set_left_right(self.left().saturating_neg(), self.right().saturating_neg())
            .set_top_bottom(self.top().saturating_neg(), self.bottom().saturating_neg());
        v
    }
}
