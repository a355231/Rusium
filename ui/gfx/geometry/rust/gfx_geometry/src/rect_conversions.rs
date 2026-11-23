// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `rect_conversions.{h,cc}`: converts a `RectF` to a `Rect` by
//! flooring, ceiling, rounding, or the "ignoring error" variants that snap an
//! edge to the integer grid when it is within `error` of it.
//!
//! Differences from the C++ code:
//! - The C++ default `error = 0.001f` of `ToEnclosingRectIgnoringError` is
//!   not a default argument here. Pass `0.001` explicitly.
//! - The C++ `DCHECK`s in `ToNearestRect` are `debug_assert!`s, so they run in
//!   debug builds and not in release builds.

use crate::numeric::{clamp_ceil, clamp_floor, clamp_round};
use crate::rect::Rect;
use crate::rect_f::RectF;

/// `FloorIgnoringError`: rounds to nearest if that is within `error`,
/// otherwise rounds down. The difference is taken in `f32`, as in C++.
fn floor_ignoring_error(f: f32, error: f32) -> i32 {
    let rounded = clamp_round(f);
    if (rounded as f32 - f).abs() < error {
        rounded
    } else {
        clamp_floor(f)
    }
}

/// `CeilIgnoringError`: rounds to nearest if that is within `error`,
/// otherwise rounds up.
fn ceil_ignoring_error(f: f32, error: f32) -> i32 {
    let rounded = clamp_round(f);
    if (rounded as f32 - f).abs() < error {
        rounded
    } else {
        clamp_ceil(f)
    }
}

/// The smallest integer rectangle containing `rect` (`ToEnclosingRect`).
/// A zero width or height keeps the edge on the same integer.
pub fn to_enclosing_rect(rect: RectF) -> Rect {
    let left = clamp_floor(rect.x());
    // `r.width() ? ...` in C++: a NaN width is truthy, so use `!= 0.0`.
    let right = if rect.width() != 0.0 { clamp_ceil(rect.right()) } else { left };
    let top = clamp_floor(rect.y());
    let bottom = if rect.height() != 0.0 { clamp_ceil(rect.bottom()) } else { top };
    let mut result = Rect::default();
    result.set_by_bounds(left, top, right, bottom);
    result
}

/// [`to_enclosing_rect`], but each edge snaps to the nearest integer when it
/// is within `error` of it (`ToEnclosingRectIgnoringError`).
pub fn to_enclosing_rect_ignoring_error(rect: RectF, error: f32) -> Rect {
    let left = floor_ignoring_error(rect.x(), error);
    let right = if rect.width() != 0.0 {
        ceil_ignoring_error(rect.right(), error)
    } else {
        left
    };
    let top = floor_ignoring_error(rect.y(), error);
    let bottom = if rect.height() != 0.0 {
        ceil_ignoring_error(rect.bottom(), error)
    } else {
        top
    };
    let mut result = Rect::default();
    result.set_by_bounds(left, top, right, bottom);
    result
}

/// The largest integer rectangle inside `rect` (`ToEnclosedRect`).
pub fn to_enclosed_rect(rect: RectF) -> Rect {
    let mut result = Rect::default();
    result.set_by_bounds(
        clamp_ceil(rect.x()),
        clamp_ceil(rect.y()),
        clamp_floor(rect.right()),
        clamp_floor(rect.bottom()),
    );
    result
}

/// [`to_enclosed_rect`], but each edge snaps to the nearest integer when it is
/// within `error` of it (`ToEnclosedRectIgnoringError`).
pub fn to_enclosed_rect_ignoring_error(rect: RectF, error: f32) -> Rect {
    let left = ceil_ignoring_error(rect.x(), error);
    let right = if rect.width() != 0.0 {
        floor_ignoring_error(rect.right(), error)
    } else {
        left
    };
    let top = ceil_ignoring_error(rect.y(), error);
    let bottom = if rect.height() != 0.0 {
        floor_ignoring_error(rect.bottom(), error)
    } else {
        top
    };
    let mut result = Rect::default();
    result.set_by_bounds(left, top, right, bottom);
    result
}

/// Rounds each edge to the nearest integer. `rect` is expected to be
/// integral already, so this should be used only when the edges are within
/// about 0.01 of an integer (`ToNearestRect`). Use [`to_enclosing_rect`] or
/// [`to_enclosed_rect`] otherwise.
pub fn to_nearest_rect(rect: RectF) -> Rect {
    let float_min_x = rect.x();
    let float_min_y = rect.y();
    let float_max_x = rect.right();
    let float_max_y = rect.bottom();

    let min_x = clamp_round(float_min_x);
    let min_y = clamp_round(float_min_y);
    let max_x = clamp_round(float_max_x);
    let max_y = clamp_round(float_max_y);

    // If these fail, the wrong method was used: ToEnclosingRect or
    // ToEnclosedRect should be used instead.
    debug_assert!((min_x as f32 - float_min_x).abs() < 0.01f32);
    debug_assert!((min_y as f32 - float_min_y).abs() < 0.01f32);
    debug_assert!((max_x as f32 - float_max_x).abs() < 0.01f32);
    debug_assert!((max_y as f32 - float_max_y).abs() < 0.01f32);

    let mut result = Rect::default();
    result.set_by_bounds(min_x, min_y, max_x, max_y);
    result
}

/// Returns true if every edge of `rect` is within `distance` of an integer
/// (`IsNearestRectWithinDistance`).
pub fn is_nearest_rect_within_distance(rect: RectF, distance: f32) -> bool {
    let float_min_x = rect.x();
    let float_min_y = rect.y();
    let float_max_x = rect.right();
    let float_max_y = rect.bottom();

    let min_x = clamp_round(float_min_x);
    let min_y = clamp_round(float_min_y);
    let max_x = clamp_round(float_max_x);
    let max_y = clamp_round(float_max_y);

    (min_x as f32 - float_min_x).abs() < distance
        && (min_y as f32 - float_min_y).abs() < distance
        && (max_x as f32 - float_max_x).abs() < distance
        && (max_y as f32 - float_max_y).abs() < distance
}

/// Rounds each edge to the nearest integer and builds the rectangle between
/// them (`ToRoundedRect`).
pub fn to_rounded_rect(rect: RectF) -> Rect {
    let mut result = Rect::default();
    result.set_by_bounds(
        clamp_round(rect.x()),
        clamp_round(rect.y()),
        clamp_round(rect.right()),
        clamp_round(rect.bottom()),
    );
    result
}

/// Floors the origin and each size component, then builds a `Rect`
/// (`ToFlooredRectDeprecated`). The size is floored, not the far edge, so this
/// differs from [`to_enclosing_rect`] when the origin is not integral.
pub fn to_floored_rect_deprecated(rect: RectF) -> Rect {
    Rect::new(
        clamp_floor(rect.x()),
        clamp_floor(rect.y()),
        clamp_floor(rect.width()),
        clamp_floor(rect.height()),
    )
}

#[cfg(test)]
mod tests;
