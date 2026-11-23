// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Formatting helpers that match the C++ `std::ostream` output used by the
//! `operator<<` functions in base/time/time.cc.

/// Formats `v` as `std::ostream << double` does by default: `%g` with 6
/// significant digits. Infinities are `inf` and `-inf`, NaN is `nan`.
pub(crate) fn format_general(v: f64) -> String {
    const PRECISION: i32 = 6;
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    // The exponent is taken after rounding to PRECISION digits, as %g does.
    let scientific = format!("{:.*e}", (PRECISION - 1) as usize, v);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust scientific formatting always contains 'e'");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    if exponent < -4 || exponent >= PRECISION {
        format!(
            "{}e{}{:02}",
            strip_trailing_zeros(mantissa),
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        )
    } else {
        let decimals = (PRECISION - 1 - exponent) as usize;
        strip_trailing_zeros(&format!("{:.*}", decimals, v))
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
    use super::*;

    #[test]
    fn matches_ostream_default_double_format() {
        assert_eq!(format_general(0.0), "0");
        assert_eq!(format_general(0.5), "0.5");
        assert_eq!(format_general(-10.0), "-10");
        assert_eq!(format_general(1.0), "1");
        assert_eq!(format_general(1_000_000.0), "1e+06");
        assert_eq!(format_general(0.0001), "0.0001");
        assert_eq!(format_general(0.00001), "1e-05");
        assert_eq!(format_general(f64::INFINITY), "inf");
        assert_eq!(format_general(f64::NEG_INFINITY), "-inf");
    }
}
