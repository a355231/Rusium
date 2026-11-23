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
//! - `rect.rs`: `gfx::Rect` and its set operations and scaling helpers
//!   (`rect.{h,cc}`).
//! - `rect_f.rs`: `gfx::RectF` (`rect_f.{h,cc}`).
//! - `rect_conversions.rs`: `RectF` to `Rect` conversions
//!   (`rect_conversions.{h,cc}`).
//! - `insets.rs`, `outsets.rs`, `insets_f.rs`, `outsets_f.rs`: `gfx::Insets`,
//!   `gfx::Outsets`, `gfx::InsetsF` and `gfx::OutsetsF`, with the shared bases
//!   in `insets_outsets_base.rs` and `insets_outsets_f_base.rs`
//!   (`insets.{h,cc}`, `outsets.h`, `insets_f.{h,cc}`, `outsets_f.h`,
//!   `insets_outsets_base.{h,cc}`, `insets_outsets_f_base.{h,cc}`).
//! - `insets_conversions.rs`: `InsetsF` to `Insets` conversions
//!   (`insets_conversions.{h,cc}`).
//! - `box_f.rs`: `gfx::BoxF` (`box_f.{h,cc}`).
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
//! Rect and insets differences:
//! - `Rect` and `RectF` constructors and set operations are named for Rust
//!   (see `rect.rs`). Free functions that C++ overloads on `Rect` and `RectF`
//!   have an `_f` suffix on the `RectF` versions.
//! - `RectF` float rounding follows the C++ code, including the `nextafter`
//!   growth in `BoundingRect` and `UnionEvenIfEmpty`.
//! - `ToString()` of `Insets`/`Outsets` prints integers in decimal. The C++
//!   passes `int` to `%g`, which is undefined behavior.
//! - `BoxF::ToString()` uses `%f` (`format::format_f`).
//!
//! Not ported:
//! - Windows (`POINT`, `SIZE`, `RECT`, `DWORD` constructors and `ToPOINT`,
//!   `ToSIZE`, `ToRECT`) and Apple (`CGPoint`/`CGSize`/`CGRect` constructors and
//!   `ToCGPoint`/`ToCGSize`/`ToCGRect`) platform conversions.
//! - `skia_conversions.{h,cc}` (Skia `SkPoint`/`SkISize` conversions) and the
//!   `mojom/` type traits.
//! - `PrintTo` (gtest printers), `WriteIntoTrace` (perfetto tracing), and the
//!   `FORWARD_DECLARE_TEST`/`FRIEND_TEST` hooks.
//! - `rrect_f.{h,cc}`, `rrect_f_builder.{h,cc}`, `triangle`, `line_f`, `quad_f`,
//!   `axis_transform2d`, `transform`, `matrix3`, `quaternion`, `cubic_bezier`,
//!   and `linear_gradient` (not in this port).
//! - Everything else in `ui/gfx/geometry/` (transform, matrix, quaternion,
//!   rrect, and so on), which later waves port.

#![forbid(unsafe_code)]

pub mod box_f;
pub mod format;
pub mod insets;
pub mod insets_conversions;
pub mod insets_f;
pub mod insets_outsets_base;
pub mod insets_outsets_f_base;
pub mod numeric;
pub mod outsets;
pub mod outsets_f;
pub mod point;
pub mod point3;
pub mod rect;
pub mod rect_conversions;
pub mod rect_f;
pub mod size;
pub mod vector;
pub mod vector3;

#[cfg(test)]
mod test_util;

pub use point::{
    point_at_offset_from_origin, point_f_at_offset_from_origin, scale_point, scale_point_uniform,
    scale_to_ceiled_point, scale_to_ceiled_point_uniform, scale_to_floored_point,
    scale_to_floored_point_uniform, scale_to_rounded_point, scale_to_rounded_point_uniform,
    to_ceiled_point, to_floored_point, to_rounded_point, transpose_point, transpose_point_f, Point,
    PointF,
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
    cross_product_2d, dot_product_2d, normalize_vector2d, scale_vector2d, scale_vector2d_uniform,
    to_ceiled_vector2d, to_floored_vector2d, to_rounded_vector2d, transpose_vector2d,
    transpose_vector2d_f, Vector2d, Vector2dF,
};
pub use vector3::{
    angle_between_vectors_in_degrees, clockwise_angle_between_vectors_in_degrees, cross_product_3d,
    dot_product_3d, scale_vector3d, scale_vector3d_by, scale_vector3d_uniform, Vector3dF,
};
pub use box_f::{scale_box, scale_box_uniform, union_boxes, BoxF};
pub use insets::{
    scale_to_ceiled_insets, scale_to_ceiled_insets_uniform, scale_to_floored_insets,
    scale_to_floored_insets_uniform, scale_to_rounded_insets, scale_to_rounded_insets_uniform,
    Insets,
};
pub use insets_conversions::{to_ceiled_insets, to_floored_insets, to_rounded_insets};
pub use insets_f::{scale_insets, scale_insets_uniform, InsetsF};
pub use outsets::Outsets;
pub use outsets_f::OutsetsF;
pub use rect::{
    bounding_rect, intersect_rects, maximum_covered_rect, scale_to_enclosed_rect,
    scale_to_enclosed_rect_uniform, scale_to_enclosing_rect, scale_to_enclosing_rect_ignoring_error,
    scale_to_enclosing_rect_uniform, scale_to_rounded_rect, scale_to_rounded_rect_uniform,
    subtract_rects, union_rects, union_rects_even_if_empty, union_rects_slice, Rect,
};
pub use rect_conversions::{
    is_nearest_rect_within_distance, to_enclosed_rect, to_enclosed_rect_ignoring_error,
    to_enclosing_rect, to_enclosing_rect_ignoring_error, to_floored_rect_deprecated,
    to_nearest_rect, to_rounded_rect,
};
pub use rect_f::{
    bounding_rect_f, intersect_rects_f, map_rect, maximum_covered_rect_f, scale_rect,
    scale_rect_by_size, scale_rect_uniform, subtract_rects_f, transpose_rect_f, union_rects_f,
    union_rects_even_if_empty_f, union_rects_slice_f, RectF,
};
