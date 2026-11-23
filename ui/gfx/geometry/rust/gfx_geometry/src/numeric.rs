// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Saturating arithmetic and clamping helpers.
//!
//! These mirror `base::ClampAdd`, `base::ClampSub`, `base::ClampFloor`,
//! `base::ClampCeil`, `base::ClampRound` and `gfx::ClampFloatGeometry`.

/// Upper bound used by `gfx::ClampFloatGeometry`:
/// `numeric_limits<float>::max() / 1e6`, rounded to float as the C++ return
/// conversion does.
pub const FLOAT_GEOMETRY_MAX: f32 = (f32::MAX as f64 / 1e6) as f32;

/// Lower bound used by `gfx::ClampFloatGeometry`:
/// `numeric_limits<float>::lowest() / 1e6`.
pub const FLOAT_GEOMETRY_LOWEST: f32 = (f32::MIN as f64 / 1e6) as f32;

/// Clamps `value` to `[FLOAT_GEOMETRY_LOWEST, FLOAT_GEOMETRY_MAX]`, returning
/// 0 for NaN. This is `gfx::ClampFloatGeometry`, which takes `float`, `double`
/// or `long double`. Float inputs are widened to `f64` before the call. The
/// widening is exact, so the comparisons are the same as the C++ ones.
pub fn clamp_float_geometry(value: f64) -> f32 {
    if value.is_nan() {
        0.0
    } else if value > FLOAT_GEOMETRY_MAX as f64 {
        FLOAT_GEOMETRY_MAX
    } else if value < FLOAT_GEOMETRY_LOWEST as f64 {
        FLOAT_GEOMETRY_LOWEST
    } else {
        value as f32
    }
}

/// `base::ClampFloor<int>`: rounds toward negative infinity and saturates to
/// `i32`. NaN becomes 0.
pub fn clamp_floor(value: f32) -> i32 {
    value.floor() as i32
}

/// `base::ClampCeil<int>`: rounds toward positive infinity and saturates to
/// `i32`. NaN becomes 0.
pub fn clamp_ceil(value: f32) -> i32 {
    value.ceil() as i32
}

/// `base::ClampRound<int>`: rounds to nearest with ties away from zero, and
/// saturates to `i32`. NaN becomes 0.
pub fn clamp_round(value: f32) -> i32 {
    value.round() as i32
}

/// `std::min(a, b)`: returns `b` only if `b < a`, otherwise `a`. For NaN this
/// differs from `f32::min`, which ignores NaN.
pub fn cpp_min<T: PartialOrd>(a: T, b: T) -> T {
    if b < a {
        b
    } else {
        a
    }
}

/// `std::max(a, b)`: returns `b` only if `a < b`, otherwise `a`.
pub fn cpp_max<T: PartialOrd>(a: T, b: T) -> T {
    if a < b {
        b
    } else {
        a
    }
}

/// `std::tuple<T, T>` less-than for `(a.0, a.1) < (b.0, b.1)`, using the same
/// element-by-element `<` tests as the standard library, so NaN falls through
/// to the second element.
pub fn lex_less<T: PartialOrd>(a: (T, T), b: (T, T)) -> bool {
    if a.0 < b.0 {
        return true;
    }
    if b.0 < a.0 {
        return false;
    }
    a.1 < b.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::XorShift64;

    #[test]
    fn clamp_float_geometry_special_values() {
        assert_eq!(clamp_float_geometry(f64::NAN), 0.0);
        assert_eq!(clamp_float_geometry(f64::INFINITY), FLOAT_GEOMETRY_MAX);
        assert_eq!(clamp_float_geometry(f64::NEG_INFINITY), FLOAT_GEOMETRY_LOWEST);
        assert_eq!(clamp_float_geometry(1e30), FLOAT_GEOMETRY_MAX);
        assert_eq!(clamp_float_geometry(-1e30), FLOAT_GEOMETRY_LOWEST);
        assert_eq!(clamp_float_geometry(1.5), 1.5);
        assert_eq!(clamp_float_geometry(-0.25), -0.25);
    }

    #[test]
    fn clamp_float_geometry_bounds_are_below_float_max() {
        assert!(FLOAT_GEOMETRY_MAX < f32::MAX / 1e5);
        assert!(FLOAT_GEOMETRY_LOWEST > f32::MIN / 1e5);
        assert_eq!(clamp_float_geometry(FLOAT_GEOMETRY_MAX as f64), FLOAT_GEOMETRY_MAX);
    }

    #[test]
    fn clamp_round_ties_away_from_zero() {
        assert_eq!(clamp_round(0.4999), 0);
        assert_eq!(clamp_round(0.5), 1);
        assert_eq!(clamp_round(-0.5), -1);
        assert_eq!(clamp_round(-10.5), -11);
        assert_eq!(clamp_round(10.4999), 10);
    }

    #[test]
    fn clamp_floor_and_ceil() {
        assert_eq!(clamp_floor(-0.5), -1);
        assert_eq!(clamp_floor(0.9999), 0);
        assert_eq!(clamp_ceil(0.0001), 1);
        assert_eq!(clamp_ceil(-0.5), 0);
        assert_eq!(clamp_ceil(10.0), 10);
    }

    #[test]
    fn clamp_int_conversions_saturate_and_map_nan_to_zero() {
        assert_eq!(clamp_floor(f32::INFINITY), i32::MAX);
        assert_eq!(clamp_floor(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(clamp_ceil(3.0e9), i32::MAX);
        assert_eq!(clamp_round(-3.0e9), i32::MIN);
        assert_eq!(clamp_round(f32::NAN), 0);
        assert_eq!(clamp_floor(f32::NAN), 0);
        assert_eq!(clamp_ceil(f32::NAN), 0);
        // Exactly the edge of i32 range.
        assert_eq!(clamp_floor(2147483520.0), 2147483520);
        assert_eq!(clamp_floor(2147483648.0), i32::MAX);
    }

    #[test]
    fn cpp_min_max_argument_order_with_nan() {
        let nan = f32::NAN;
        // std::min(nan, 1) returns nan (first argument), unlike f32::min.
        assert!(cpp_min(nan, 1.0).is_nan());
        assert_eq!(cpp_min(1.0, nan), 1.0);
        assert!(cpp_max(nan, 1.0).is_nan());
        assert_eq!(cpp_max(1.0, nan), 1.0);
        assert_eq!(cpp_min(2, 3), 2);
        assert_eq!(cpp_max(2, 3), 3);
    }

    #[test]
    fn lex_less_matches_tuple_semantics() {
        assert!(lex_less((1, 2), (1, 3)));
        assert!(!lex_less((1, 3), (1, 2)));
        assert!(lex_less((0, 9), (1, 0)));
        assert!(!lex_less((1, 0), (1, 0)));
        // NaN in the first element is incomparable, so the second decides.
        assert!(lex_less((f32::NAN, 1.0), (f32::NAN, 2.0)));
    }

    #[test]
    fn clamp_round_matches_reference_for_random_floats() {
        // Reference: round in f64, then saturate by hand.
        let mut rng = XorShift64::new(0x9e37_79b9_7f4a_7c15);
        for _ in 0..20_000 {
            let v = rng.next_f32_finite_range(-3.0e9, 3.0e9);
            let r = (v as f64).round();
            let expected = if r >= i32::MAX as f64 {
                i32::MAX
            } else if r <= i32::MIN as f64 {
                i32::MIN
            } else {
                r as i32
            };
            assert_eq!(clamp_round(v), expected, "value {v}");
        }
    }

    #[test]
    fn clamp_float_geometry_matches_reference_for_random_doubles() {
        let mut rng = XorShift64::new(42);
        for _ in 0..20_000 {
            let v = rng.next_f64_finite_range(-1e40, 1e40);
            let expected = if v > FLOAT_GEOMETRY_MAX as f64 {
                FLOAT_GEOMETRY_MAX
            } else if v < FLOAT_GEOMETRY_LOWEST as f64 {
                FLOAT_GEOMETRY_LOWEST
            } else {
                v as f32
            };
            assert_eq!(clamp_float_geometry(v), expected, "value {v}");
            assert!(clamp_float_geometry(v).is_finite());
        }
    }
}
