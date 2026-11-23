// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! StartsWith / EndsWith / RemovePrefix / RemoveSuffix from
//! base/strings/string_util.h.

use crate::code_unit::{to_lower_ascii_char, CodeUnit};

/// C++ `CompareCase`. Only ASCII case-insensitivity is supported.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CompareCase {
    /// Exact comparison (C++ `CompareCase::SENSITIVE`).
    #[default]
    Sensitive,
    /// ASCII letters compare equal regardless of case; all other units compare
    /// exactly (C++ `CompareCase::INSENSITIVE_ASCII`).
    InsensitiveAscii,
}

/// Compares `a` and `b` (which must have equal length) under `case`.
fn units_equal<T: CodeUnit>(a: &[T], b: &[T], case: CompareCase) -> bool {
    match case {
        CompareCase::Sensitive => a == b,
        CompareCase::InsensitiveAscii => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(&x, &y)| to_lower_ascii_char(x) == to_lower_ascii_char(y))
        }
    }
}

/// C++ `StartsWith`.
///
/// Intentional difference: the C++ case-insensitive path for 16-bit strings
/// lowercases through a table indexed by `static_cast<unsigned char>(c)`. That
/// ignores the high byte, so `u"Łbc"` matched `u"Abc"`. This port applies
/// ASCII-only folding to every code unit, as the documentation describes.
pub fn starts_with<T: CodeUnit>(s: &[T], prefix: &[T], case: CompareCase) -> bool {
    s.len() >= prefix.len() && units_equal(&s[..prefix.len()], prefix, case)
}

/// C++ `EndsWith`. See [`starts_with`] for the case-folding note.
pub fn ends_with<T: CodeUnit>(s: &[T], suffix: &[T], case: CompareCase) -> bool {
    s.len() >= suffix.len() && units_equal(&s[s.len() - suffix.len()..], suffix, case)
}

/// C++ `RemovePrefix`: the part of `s` after `prefix`, or `None` if `s` does
/// not start with `prefix`.
pub fn remove_prefix<'a, T: CodeUnit>(
    s: &'a [T],
    prefix: &[T],
    case: CompareCase,
) -> Option<&'a [T]> {
    if starts_with(s, prefix, case) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// C++ `RemoveSuffix`: the part of `s` before `suffix`, or `None` if `s` does
/// not end with `suffix`.
pub fn remove_suffix<'a, T: CodeUnit>(
    s: &'a [T],
    suffix: &[T],
    case: CompareCase,
) -> Option<&'a [T]> {
    if ends_with(s, suffix, case) {
        Some(&s[..s.len() - suffix.len()])
    } else {
        None
    }
}
