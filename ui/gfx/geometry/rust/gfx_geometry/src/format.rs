// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Reproduces the `printf` conversions used by the C++ `ToString()` methods:
//! `%g` for floats, and `%d` (which is Rust's default integer formatting).

/// Formats `value` the way C `printf("%g", value)` does, with the default
/// precision of 6 significant digits.
///
/// Trailing zeros are removed, and the exponent form is used when the decimal
/// exponent is below -4 or at least 6. Negative zero prints as `-0`, and
/// NaN and infinity print as `nan`, `-nan`, `inf` and `-inf`.
pub fn format_g(value: f64) -> String {
    const PRECISION: i32 = 6;

    if value.is_nan() {
        return if value.is_sign_negative() { "-nan".to_string() } else { "nan".to_string() };
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf".to_string() } else { "inf".to_string() };
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0".to_string() } else { "0".to_string() };
    }

    // The exponent is taken after rounding to PRECISION significant digits,
    // as the C standard specifies.
    let scientific = format!("{:.*e}", (PRECISION - 1) as usize, value);
    let (mantissa, exponent) =
        scientific.split_once('e').expect("Rust scientific format always contains 'e'");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");

    if !(-4..PRECISION).contains(&exponent) {
        let mantissa = strip_trailing_zeros(mantissa);
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exponent.abs())
    } else {
        let decimals = (PRECISION - 1 - exponent) as usize;
        strip_trailing_zeros(&format!("{:.*}", decimals, value))
    }
}

fn strip_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::format_g;

    #[test]
    fn matches_printf_g() {
        assert_eq!(format_g(1.0), "1");
        assert_eq!(format_g(2.5), "2.5");
        assert_eq!(format_g(1.03125), "1.03125");
        assert_eq!(format_g(-3.0), "-3");
        assert_eq!(format_g(0.0), "0");
        assert_eq!(format_g(-0.0), "-0");
        assert_eq!(format_g(0.0001), "0.0001");
        assert_eq!(format_g(0.00001), "1e-05");
        assert_eq!(format_g(100000.0), "100000");
        assert_eq!(format_g(1000000.0), "1e+06");
        assert_eq!(format_g(1234567.0), "1.23457e+06");
        assert_eq!(format_g(-1.5e-7), "-1.5e-07");
        assert_eq!(format_g(f64::INFINITY), "inf");
        assert_eq!(format_g(f64::NEG_INFINITY), "-inf");
        assert_eq!(format_g(f64::NAN), "nan");
        assert_eq!(format_g(-f64::NAN), "-nan");
    }

    #[test]
    fn rounding_can_change_the_exponent() {
        // 999999.5 rounds to 1.00000e+06 in %g, so the exponent form is used.
        assert_eq!(format_g(999999.5), "1e+06");
        // 9.999995 rounds to 10.0000, so the fixed form uses one fewer decimal.
        assert_eq!(format_g(9.999995), "10");
    }
}
