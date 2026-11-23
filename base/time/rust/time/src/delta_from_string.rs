// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `base::TimeDeltaFromString`
//! (base/time/time_delta_from_string.{h,cc}). The parser follows the Go and
//! Abseil `ParseDuration` rules.

use crate::delta::{days, hours, microseconds, milliseconds, minutes, nanoseconds, seconds};
use crate::TimeDelta;

/// Result of `ConsumeDurationNumber`. The number is
/// `int_part + frac_part / frac_scale`, where `0 <= frac_part < frac_scale`
/// and `frac_scale` is 10 to the power of the number of fraction digits kept.
struct ParsedDecimal {
    int_part: i64,
    frac_part: i64,
    frac_scale: i64,
}

/// Consumes a leading decimal number from `s`. Returns `None` on overflow of
/// the integer part, or if there are no digits at all. A number such as `.5`
/// is accepted, and so is `5.`.
fn consume_duration_number(s: &mut &[u8]) -> Option<ParsedDecimal> {
    let mut res = ParsedDecimal {
        int_part: 0,
        frac_part: 0,
        frac_scale: 1,
    };
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        let d = i64::from(s[i] - b'0');
        if res.int_part > i64::MAX / 10 {
            return None;
        }
        res.int_part *= 10;
        if res.int_part > i64::MAX - d {
            return None;
        }
        res.int_part += d;
        i += 1;
    }
    let int_part_empty = i == 0;
    if i == s.len() || s[i] != b'.' {
        *s = &s[i..];
        return if int_part_empty { None } else { Some(res) };
    }
    i += 1; // consume '.'
    while i < s.len() && s[i].is_ascii_digit() {
        let d = i64::from(s[i] - b'0');
        // frac_part < frac_scale, so frac_part cannot overflow while
        // frac_scale stays in range. Digits past that point are dropped.
        if res.frac_scale <= i64::MAX / 10 {
            res.frac_part = res.frac_part * 10 + d;
            res.frac_scale *= 10;
        }
        i += 1;
    }
    *s = &s[i..];
    if int_part_empty && res.frac_scale == 1 {
        None
    } else {
        Some(res)
    }
}

/// Consumes a leading unit designator from `s`. The units are `ns`, `us`,
/// `ms`, `s`, `m`, `h` and `d`. `ms` must be checked before `m`.
fn consume_duration_unit(s: &mut &[u8]) -> Option<TimeDelta> {
    const UNITS: [(&[u8], fn() -> TimeDelta); 7] = [
        (b"ns", || nanoseconds(1)),
        (b"us", || microseconds(1)),
        (b"ms", || milliseconds(1)),
        (b"s", || seconds(1)),
        (b"m", || minutes(1)),
        (b"h", || hours(1)),
        (b"d", || days(1)),
    ];
    for (prefix, unit) in UNITS {
        if let Some(rest) = s.strip_prefix(prefix) {
            *s = rest;
            return Some(unit());
        }
    }
    None
}

/// Parses a duration such as `"300ms"`, `"-1.5h"` or `"2h45m"`. Port of
/// `TimeDeltaFromString`.
///
/// Valid units are `ns`, `us`, `ms`, `s`, `m`, `h` and `d`. The special values
/// `"0"`, `"+0"` and `"-0"` are zero, `"inf"` and `"+inf"` are `max()`, and
/// `"-inf"` is `min()`. Otherwise the input is a possibly signed sequence of
/// number-unit pairs. Each pair may have a fraction, and all pairs have the
/// same sign.
///
/// Returns `None` when parsing fails. Integer parts larger than `i64::MAX`
/// fail. An overflowing `number * unit` gives `±inf` instead.
pub fn time_delta_from_string(duration_string: &str) -> Option<TimeDelta> {
    let mut s = duration_string.as_bytes();
    let mut sign: i64 = 1;
    if let Some(rest) = s.strip_prefix(b"-") {
        sign = -1;
        s = rest;
    } else if let Some(rest) = s.strip_prefix(b"+") {
        s = rest;
    }
    if s.is_empty() {
        return None;
    }

    // Special values that don't need units.
    if s == b"0" {
        return Some(TimeDelta::default());
    }
    if s == b"inf" {
        return Some(if sign == 1 {
            TimeDelta::max()
        } else {
            TimeDelta::min()
        });
    }

    let mut delta = TimeDelta::default();
    while !s.is_empty() {
        let number = consume_duration_number(&mut s)?;
        let unit = consume_duration_unit(&mut s)?;
        if number.int_part != 0 {
            // Integer part: saturating, so overflow gives ±inf.
            delta += unit * (sign * number.int_part);
        }
        if number.frac_part != 0 {
            let fraction = (sign as f64) * (number.frac_part as f64) / (number.frac_scale as f64);
            delta += fraction * unit;
        }
    }
    Some(delta)
}
