// Copyright 2020 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `insets_conversions.{h,cc}`: converts `InsetsF` to `Insets` by
//! flooring, ceiling, or rounding each edge. Each edge goes through the
//! saturating `numeric` helpers, as `base::ClampFloor` and its siblings do.

use crate::insets::Insets;
use crate::insets_f::InsetsF;
use crate::numeric::{clamp_ceil, clamp_floor, clamp_round};

/// Rounds each edge down (`ToFlooredInsets`).
pub fn to_floored_insets(insets: InsetsF) -> Insets {
    Insets::tlbr(
        clamp_floor(insets.top()),
        clamp_floor(insets.left()),
        clamp_floor(insets.bottom()),
        clamp_floor(insets.right()),
    )
}

/// Rounds each edge up (`ToCeiledInsets`).
pub fn to_ceiled_insets(insets: InsetsF) -> Insets {
    Insets::tlbr(
        clamp_ceil(insets.top()),
        clamp_ceil(insets.left()),
        clamp_ceil(insets.bottom()),
        clamp_ceil(insets.right()),
    )
}

/// Rounds each edge to the nearest integer (`ToRoundedInsets`).
pub fn to_rounded_insets(insets: InsetsF) -> Insets {
    Insets::tlbr(
        clamp_round(insets.top()),
        clamp_round(insets.left()),
        clamp_round(insets.bottom()),
        clamp_round(insets.right()),
    )
}
