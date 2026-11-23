// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `insets_outsets_base.{h,cc}`: the shared integer edge-thickness
//! type behind `gfx::Insets` and `gfx::Outsets`.
//!
//! C++ uses the CRTP template `InsetsOutsetsBase<T>`. Rust has no CRTP, so a
//! macro, `define_insets_outsets_base!`, stamps out the same struct and
//! methods for each concrete type (`insets.rs` and `outsets.rs`). The setters
//! return `&mut Self`, like the C++ `T&` return values, so C++ chains such as
//! `Insets().set_left_right(1, 2).set_top_bottom(3, 4)` become
//! `let mut i = Insets::default(); i.set_left_right(1, 2).set_top_bottom(3, 4);`.
//!
//! Every stored bottom/right value is clamped so that `top + bottom` and
//! `left + right` never overflow `i32`, so `width()` and `height()` are plain
//! sums that cannot overflow.

/// Returns `bottom_or_right` clamped so that `top_or_left + bottom_or_right`
/// saturates at `i32` limits. This is the C++ `ClampBottomOrRight`, which is
/// `base::ClampAdd(top_or_left, bottom_or_right) - top_or_left`.
///
/// The result always fits in `i32`: the saturated sum lies in `i32`, and
/// subtracting `top_or_left` from it can only saturate the value that was
/// already out of range.
pub(crate) const fn clamp_bottom_or_right(top_or_left: i32, bottom_or_right: i32) -> i32 {
    (top_or_left.saturating_add(bottom_or_right) as i64 - top_or_left as i64) as i32
}

/// Defines an integer insets-like struct named `$name` with the four edge
/// thicknesses and the methods shared by `Insets` and `Outsets`.
macro_rules! define_insets_outsets_base {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name {
            top: i32,
            left: i32,
            bottom: i32,
            right: i32,
        }

        impl $name {
            /// The C++ `explicit InsetsOutsetsBase(int all)` constructor: all
            /// four edges get `all`, with the bottom and right clamped as in
            /// the setters.
            pub const fn new(all: i32) -> Self {
                Self {
                    top: all,
                    left: all,
                    bottom: crate::insets_outsets_base::clamp_bottom_or_right(all, all),
                    right: crate::insets_outsets_base::clamp_bottom_or_right(all, all),
                }
            }

            pub const fn top(self) -> i32 {
                self.top
            }

            pub const fn left(self) -> i32 {
                self.left
            }

            pub const fn bottom(self) -> i32 {
                self.bottom
            }

            pub const fn right(self) -> i32 {
                self.right
            }

            /// The sum of the left and right values.
            pub const fn width(self) -> i32 {
                self.left + self.right
            }

            /// The sum of the top and bottom values.
            pub const fn height(self) -> i32 {
                self.top + self.bottom
            }

            /// `(width(), height())` as a `Size`. Like the C++ `Size(int, int)`
            /// constructor, a negative dimension becomes zero.
            pub const fn size(self) -> $crate::size::Size {
                $crate::size::Size::new(self.width(), self.height())
            }

            /// Returns true if both the width and the height are zero.
            pub const fn is_empty(self) -> bool {
                self.width() == 0 && self.height() == 0
            }

            /// Swaps the x and y axes: top with left, and bottom with right.
            pub fn transpose(&mut self) {
                std::mem::swap(&mut self.top, &mut self.left);
                std::mem::swap(&mut self.bottom, &mut self.right);
            }

            pub fn set_top(&mut self, top: i32) -> &mut Self {
                self.top = top;
                self.bottom = crate::insets_outsets_base::clamp_bottom_or_right(top, self.bottom);
                self
            }

            pub fn set_left(&mut self, left: i32) -> &mut Self {
                self.left = left;
                self.right = crate::insets_outsets_base::clamp_bottom_or_right(left, self.right);
                self
            }

            pub fn set_bottom(&mut self, bottom: i32) -> &mut Self {
                self.bottom = crate::insets_outsets_base::clamp_bottom_or_right(self.top, bottom);
                self
            }

            pub fn set_right(&mut self, right: i32) -> &mut Self {
                self.right = crate::insets_outsets_base::clamp_bottom_or_right(self.left, right);
                self
            }

            /// Sets the left and right edges together. This clamps less than
            /// the single-edge setters, as the C++ comment says.
            pub fn set_left_right(&mut self, left: i32, right: i32) -> &mut Self {
                self.left = left;
                self.right = crate::insets_outsets_base::clamp_bottom_or_right(left, right);
                self
            }

            /// Sets the top and bottom edges together.
            pub fn set_top_bottom(&mut self, top: i32, bottom: i32) -> &mut Self {
                self.top = top;
                self.bottom = crate::insets_outsets_base::clamp_bottom_or_right(top, bottom);
                self
            }

            /// The C++ `TLBR(top, left, bottom, right)` factory.
            pub fn tlbr(top: i32, left: i32, bottom: i32, right: i32) -> Self {
                let mut v = Self::default();
                v.set_top_bottom(top, bottom).set_left_right(left, right);
                v
            }

            /// The C++ `VH(vertical, horizontal)` factory.
            pub fn vh(vertical: i32, horizontal: i32) -> Self {
                Self::tlbr(vertical, horizontal, vertical, horizontal)
            }

            /// Sets each edge to the larger of itself and the matching edge of
            /// `other`.
            pub fn set_to_max(&mut self, other: Self) {
                self.top = self.top.max(other.top);
                self.left = self.left.max(other.left);
                self.bottom = self.bottom.max(other.bottom);
                self.right = self.right.max(other.right);
            }
        }

        impl std::ops::AddAssign for $name {
            fn add_assign(&mut self, other: Self) {
                use crate::insets_outsets_base::clamp_bottom_or_right;
                self.top = self.top.saturating_add(other.top);
                self.left = self.left.saturating_add(other.left);
                self.bottom = clamp_bottom_or_right(self.top, self.bottom.saturating_add(other.bottom));
                self.right = clamp_bottom_or_right(self.left, self.right.saturating_add(other.right));
            }
        }

        impl std::ops::SubAssign for $name {
            fn sub_assign(&mut self, other: Self) {
                use crate::insets_outsets_base::clamp_bottom_or_right;
                self.top = self.top.saturating_sub(other.top);
                self.left = self.left.saturating_sub(other.left);
                self.bottom = clamp_bottom_or_right(self.top, self.bottom.saturating_sub(other.bottom));
                self.right = clamp_bottom_or_right(self.left, self.right.saturating_sub(other.right));
            }
        }

        impl std::ops::Add for $name {
            type Output = Self;
            fn add(mut self, rhs: Self) -> Self {
                self += rhs;
                self
            }
        }

        impl std::ops::Sub for $name {
            type Output = Self;
            fn sub(mut self, rhs: Self) -> Self {
                self -= rhs;
                self
            }
        }

        /// The C++ unary `operator-`, which negates each edge with saturation.
        impl std::ops::Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                let mut v = Self::default();
                v.set_left_right(self.left.saturating_neg(), self.right.saturating_neg())
                    .set_top_bottom(self.top.saturating_neg(), self.bottom.saturating_neg());
                v
            }
        }

        impl std::fmt::Display for $name {
            /// `"x:%g,%g y:%g,%g"` with left, right, top, bottom.
            ///
            /// The C++ code passes `int` arguments to `%g`, which is undefined
            /// behavior. This prints the integers in decimal instead.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "x:{},{} y:{},{}",
                    self.left, self.right, self.top, self.bottom
                )
            }
        }
    };
}

pub(crate) use define_insets_outsets_base;
