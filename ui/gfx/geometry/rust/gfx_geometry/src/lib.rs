// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of the basic geometry types in `ui/gfx/geometry/`:
//!
//! - `point.rs`: `gfx::Point`, `gfx::PointF`, and the point conversions and
//!   scaling helpers (`point.{h,cc}`, `point_f.{h,cc}`,
//!   `point_conversions.{h,cc}`).
//! - `point3.rs`: `gfx::Point3F` (`point3_f.{h,cc}`).
//! - `size.rs`: `gfx::Size`, `gfx::SizeF`, and the size conversions and
//!   scaling helpers (`size.{h,cc}`, `size_f.{h,cc}`,
//!   `size_conversions.{h,cc}`).
//! - `vector.rs`: `gfx::Vector2d`, `gfx::Vector2dF`, and the vector
//!   conversions (`vector2d.{h,cc}`, `vector2d_f.{h,cc}`,
//!   `vector2d_conversions.{h,cc}`).
//! - `vector3.rs`: `gfx::Vector3dF` (`vector3d_f.{h,cc}`).
//! - `numeric.rs`: the saturating and clamping helpers the C++ code takes from
//!   `base::ClampAdd`, `base::ClampSub`, `base::ClampFloor`, `base::ClampCeil`,
//!   `base::ClampRound` and `gfx::ClampFloatGeometry`
//!   (`clamp_float_geometry.h`).
//!
//! Naming: C++ overloads that differ only in the number of scale factors
//! become `f(x, y)` plus `f_uniform(s)` for free functions, and `scale(s)` /
//! `scale_xy(x, y)` for methods.
//!
//! Differences from the C++ code:
//! - `ToString()` is `Display`, so `to_string()` works. The `%g` and `%d`
//!   formats are reproduced by `format.rs`, including `-0` and `nan`/`inf`.
//! - `Vector3dF::GetNormalized(Vector3dF* out) -> bool` becomes
//!   `Vector3dF::get_normalized(self) -> Option<Vector3dF>`. The C++ output
//!   parameter also holds the unnormalized input when it returns false; here
//!   that case is `None` and the caller keeps its own value.
//! - `Size::GetArea()` CHECK-fails on overflow. `Size::area()` panics in the
//!   same case. `Size::GetCheckedArea()` is `Size::checked_area()`, which
//!   returns `Option<i32>`.
//! - `Point` and `Point::operator<` map to `Ord`/`PartialOrd`, ordered by
//!   `(y, x)` as in C++. `PointF` gets only `PartialOrd`. The comparison
//!   follows `std::tie` semantics, so NaN coordinates compare as the C++ does.
//! - `PointF::IsWithinDistance`'s `DCHECK(allowed_distance > 0)` is a
//!   `debug_assert!`.
//! - `std::min`/`std::max` on floats use the C++ argument-order semantics
//!   (`cpp_min`/`cpp_max` in `numeric.rs`), not `f32::min`/`f32::max`, which
//!   differ for NaN. The `fmin`/`fmax` call in the angle computation uses
//!   `f32::min`/`f32::max` because C++ uses `fmin`/`fmax` there.
//! - `Vector3dF::cross` and `CrossProduct` use the C++ double intermediates
//!   and float results. `Vector2dF::length` uses `hypotf` through
//!   `f32::hypot`, and `SlopeAngleRadians` uses `atan2f` through `f32::atan2`.
//! - Integer conversions (`ClampFloor`, `ClampCeil`, `ClampRound`) are
//!   `f32 as i32`, which saturates and maps NaN to 0 the same way
//!   `base::saturated_cast` does. Inputs are `f32` only, because every caller
//!   in this family passes a `float`.
//!
//! Not ported:
//! - Windows (`POINT`, `SIZE`, `DWORD` constructors and `ToPOINT`/`ToSIZE`)
//!   and Apple (`CGPoint`/`CGSize` constructors and `ToCGPoint`/`ToCGSize`)
//!   platform conversions.
//! - `skia_conversions.{h,cc}` (Skia `SkPoint`/`SkISize` conversions) and the
//!   `mojom/` type traits.
//! - `PrintTo` (gtest printers), `WriteIntoTrace` (perfetto tracing), and the
//!   `FORWARD_DECLARE_TEST`/`FRIEND_TEST` hooks.
//! - Everything else in `ui/gfx/geometry/` (rect, insets, transform, matrix,
//!   quaternion, and so on), which later waves port.

#![forbid(unsafe_code)]

pub mod format;
pub mod numeric;
pub mod point;
pub mod point3;
pub mod size;
pub mod vector;
pub mod vector3;

#[cfg(test)]
mod test_util;

pub use point::{
    point_at_offset_from_origin, point_f_at_offset_from_origin, scale_point,
    scale_point_uniform, scale_to_ceiled_point, scale_to_ceiled_point_uniform,
    scale_to_floored_point, scale_to_floored_point_uniform, scale_to_rounded_point,
    scale_to_rounded_point_uniform, to_ceiled_point, to_floored_point, to_rounded_point,
    transpose_point, transpose_point_f, Point, PointF,
};
pub use point3::{
    point3_at_offset_from_origin, scale_point3, scale_point3_by, scale_point3_uniform, Point3F,
};
pub use size::{
    scale_size, scale_size_uniform, scale_to_ceiled_size, scale_to_ceiled_size_uniform,
    scale_to_floored_size, scale_to_floored_size_uniform, scale_to_rounded_size,
    scale_to_rounded_size_uniform, to_ceiled_size, to_floored_size, to_rounded_size,
    transpose_size, transpose_size_f, Size, SizeF,
};
pub use vector::{
    normalize_vector2d, scale_vector2d, scale_vector2d_uniform, to_ceiled_vector2d,
    to_floored_vector2d, to_rounded_vector2d, transpose_vector2d, transpose_vector2d_f,
    cross_product_2d, dot_product_2d, Vector2d, Vector2dF,
};
pub use vector3::{
    angle_between_vectors_in_degrees, clockwise_angle_between_vectors_in_degrees,
    cross_product_3d, dot_product_3d, scale_vector3d, scale_vector3d_by, scale_vector3d_uniform,
    Vector3dF,
};
