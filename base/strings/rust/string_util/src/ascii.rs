// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! ASCII case conversion, case-insensitive comparison, and ASCII checks from
//! base/strings/string_util.h.

use std::cmp::Ordering;

use crate::code_unit::{to_lower_ascii_char, CodeUnit};

/// C++ `ToLowerASCII(std::string_view)` / `ToLowerASCII(std::u16string_view)`.
/// Non-ASCII units are returned unchanged.
pub fn to_lower_ascii<T: CodeUnit>(s: &[T]) -> Vec<T> {
    s.iter().map(|&c| to_lower_ascii_char(c)).collect()
}

/// C++ `ToUpperASCII` for slices. Non-ASCII units are returned unchanged.
pub fn to_upper_ascii<T: CodeUnit>(s: &[T]) -> Vec<T> {
    s.iter().map(|&c| crate::code_unit::to_upper_ascii_char(c)).collect()
}

/// `&str` form of [`to_lower_ascii`]. Only ASCII letters change, so the result
/// is still valid UTF-8.
pub fn to_lower_ascii_str(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// `&str` form of [`to_upper_ascii`].
pub fn to_upper_ascii_str(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// C++ `CompareCaseInsensitiveASCII`. Compares ASCII case-insensitively,
/// returning `Less`, `Equal`, or `Greater` (C++ returns -1, 0, or 1). Units are
/// compared as unsigned values, so the order does not depend on the signedness
/// of `char`.
pub fn compare_case_insensitive_ascii<T: CodeUnit>(a: &[T], b: &[T]) -> Ordering {
    for (&x, &y) in a.iter().zip(b.iter()) {
        let lx = to_lower_ascii_char(x);
        let ly = to_lower_ascii_char(y);
        match lx.cmp(&ly) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    // One string ended first, or they are equal. Shorter sorts first.
    a.len().cmp(&b.len())
}

/// C++ `EqualsCaseInsensitiveASCII`, including its mixed-width overloads. The
/// two inputs may have different code unit widths. Units are compared as
/// their numeric values after ASCII lowercasing.
pub fn equals_case_insensitive_ascii<T: CodeUnit, U: CodeUnit>(a: &[T], b: &[U]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(&x, &y)| {
            to_lower_ascii_char(x).to_u32() == to_lower_ascii_char(y).to_u32()
        })
}

/// C++ `CaseInsensitiveCompareASCII` functor, as a function: true when two
/// code units are equal after ASCII lowercasing. Useful with iterator
/// adapters in place of the C++ functor used with std::search.
pub fn eq_ignore_ascii_case_unit<T: CodeUnit>(x: T, y: T) -> bool {
    to_lower_ascii_char(x) == to_lower_ascii_char(y)
}

/// C++ `IsStringASCII`. True when every unit is in 0x00-0x7F (true for empty).
///
/// Unlike the C++ word-at-a-time loop, this always reads the whole input and
/// combines the units with OR before checking, so the amount of work depends
/// only on the length.
pub fn is_string_ascii<T: CodeUnit>(s: &[T]) -> bool {
    let mut acc = 0u32;
    for &c in s {
        acc |= c.to_u32();
    }
    acc & !0x7F == 0
}

/// C++ `FindFirstNonASCII`. Returns the index of the first unit above 0x7F,
/// or `s.len()` when all units are ASCII.
pub fn find_first_non_ascii<T: CodeUnit>(s: &[T]) -> usize {
    s.iter().position(|c| c.to_u32() > 0x7F).unwrap_or(s.len())
}

/// C++ `EqualsASCII`: case-sensitive comparison of UTF-16 text against an
/// ASCII string. The C++ behavior is undefined when `ascii` is not ASCII. This
/// port returns false in that case.
pub fn equals_ascii(s: &[u16], ascii: &str) -> bool {
    s.len() == ascii.len()
        && s.iter()
            .zip(ascii.bytes())
            .all(|(&u, b)| b < 0x80 && u == u16::from(b))
}

/// C++ `HexDigitToInt`: the value of a hex digit (`'0'`-`'9'`, `'a'`-`'f'`,
/// `'A'`-`'F'`). The C++ precondition is checked only in debug builds with
/// DCHECK; this port uses a debug assertion with the same precondition.
pub fn hex_digit_to_int<T: CodeUnit>(c: T) -> u8 {
    debug_assert!(crate::code_unit::is_hex_digit(c), "not a hex digit: {c:?}");
    let v = c.to_u32();
    let value = if (u32::from(b'0')..=u32::from(b'9')).contains(&v) {
        v.wrapping_sub(u32::from(b'0'))
    } else if (u32::from(b'A')..=u32::from(b'F')).contains(&v) {
        v.wrapping_sub(u32::from(b'A')).wrapping_add(10)
    } else {
        v.wrapping_sub(u32::from(b'a')).wrapping_add(10)
    };
    value as u8
}
