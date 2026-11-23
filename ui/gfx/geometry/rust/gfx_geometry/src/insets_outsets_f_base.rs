// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `insets_outsets_f_base.{h,cc}`: the shared floating-point
//! edge-thickness type behind `gfx::InsetsF` and `gfx::OutsetsF`.
//!
//! As with the integer base, a macro stamps out the struct and methods for
//! each concrete type, and the setters return `&mut Self` like the C++ `T&`.
//! Unlike the integer base, the float base does no clamping, so `width()`
//! and `height()` can overflow to infinity, as the C++ float arithmetic does.

/// Defines a float insets-like struct named `$name` with the four edge
/// thicknesses and the methods shared by `InsetsF` and `OutsetsF`.
macro_rules! define_insets_outsets_f_base {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq)]
        pub struct $name {
            top: f32,
            left: f32,
            bottom: f32,
            right: f32,
        }

        impl $name {
            /// The C++ `explicit InsetsOutsetsFBase(float all)` constructor.
            pub const fn new(all: f32) -> Self {
                Self { top: all, left: all, bottom: all, right: all }
            }

            pub const fn top(self) -> f32 {
                self.top
            }

            pub const fn left(self) -> f32 {
                self.left
            }

            pub const fn bottom(self) -> f32 {
                self.bottom
            }

            pub const fn right(self) -> f32 {
                self.right
            }

            /// The sum of the left and right values.
            pub const fn width(self) -> f32 {
                self.left + self.right
            }

            /// The sum of the top and bottom values.
            pub const fn height(self) -> f32 {
                self.top + self.bottom
            }

            /// Returns true if both the width and the height are exactly zero.
            pub fn is_empty(self) -> bool {
                self.width() == 0.0 && self.height() == 0.0
            }

            /// Swaps the x and y axes: top with left, and bottom with right.
            pub fn transpose(&mut self) {
                std::mem::swap(&mut self.top, &mut self.left);
                std::mem::swap(&mut self.bottom, &mut self.right);
            }

            pub fn set_top(&mut self, top: f32) -> &mut Self {
                self.top = top;
                self
            }

            pub fn set_left(&mut self, left: f32) -> &mut Self {
                self.left = left;
                self
            }

            pub fn set_bottom(&mut self, bottom: f32) -> &mut Self {
                self.bottom = bottom;
                self
            }

            pub fn set_right(&mut self, right: f32) -> &mut Self {
                self.right = right;
                self
            }

            /// The C++ `TLBR(top, left, bottom, right)` factory.
            pub fn tlbr(top: f32, left: f32, bottom: f32, right: f32) -> Self {
                let mut v = Self::default();
                v.set_top(top).set_left(left).set_bottom(bottom).set_right(right);
                v
            }

            /// The C++ `VH(vertical, horizontal)` factory.
            pub fn vh(vertical: f32, horizontal: f32) -> Self {
                Self::tlbr(vertical, horizontal, vertical, horizontal)
            }

            /// Sets each edge to the larger of itself and the matching edge of
            /// `other`, with `std::max` argument order for NaN.
            pub fn set_to_max(&mut self, other: Self) {
                use crate::numeric::cpp_max;
                self.top = cpp_max(self.top, other.top);
                self.left = cpp_max(self.left, other.left);
                self.bottom = cpp_max(self.bottom, other.bottom);
                self.right = cpp_max(self.right, other.right);
            }

            /// Multiplies the horizontal edges by `x_scale` and the vertical
            /// edges by `y_scale`.
            pub fn scale_xy(&mut self, x_scale: f32, y_scale: f32) {
                self.top *= y_scale;
                self.left *= x_scale;
                self.bottom *= y_scale;
                self.right *= x_scale;
            }

            /// Multiplies every edge by `scale`.
            pub fn scale(&mut self, scale: f32) {
                self.scale_xy(scale, scale);
            }
        }

        impl std::ops::AddAssign for $name {
            fn add_assign(&mut self, other: Self) {
                self.top += other.top;
                self.left += other.left;
                self.bottom += other.bottom;
                self.right += other.right;
            }
        }

        impl std::ops::SubAssign for $name {
            fn sub_assign(&mut self, other: Self) {
                self.top -= other.top;
                self.left -= other.left;
                self.bottom -= other.bottom;
                self.right -= other.right;
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

        impl std::ops::Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                Self::tlbr(-self.top, -self.left, -self.bottom, -self.right)
            }
        }

        impl std::fmt::Display for $name {
            /// `"x:%g,%g y:%g,%g"` with left, right, top, bottom, as in C++.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "x:{},{} y:{},{}",
                    crate::format::format_g(self.left as f64),
                    crate::format::format_g(self.right as f64),
                    crate::format::format_g(self.top as f64),
                    crate::format::format_g(self.bottom as f64)
                )
            }
        }
    };
}

pub(crate) use define_insets_outsets_f_base;
