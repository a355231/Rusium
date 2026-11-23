// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `outsets_f.h`: `gfx::OutsetsF`, the floating-point version of
//! `Outsets`. The struct and its arithmetic come from
//! `insets_outsets_f_base.rs`. The C++ `PrintTo` gtest hook is not ported.

use crate::insets_f::InsetsF;
use crate::insets_outsets_f_base::define_insets_outsets_f_base;

define_insets_outsets_f_base!(
    /// The floating-point widths of the four margins around a rectangle,
    /// `gfx::OutsetsF`.
    OutsetsF
);

impl OutsetsF {
    /// Converts to `InsetsF` by negating every edge (C++ `OutsetsF::ToInsets`).
    pub fn to_insets(self) -> InsetsF {
        InsetsF::tlbr(-self.top(), -self.left(), -self.bottom(), -self.right())
    }
}
