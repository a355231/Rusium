// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of the string helpers in `base/strings/string_util.{h,cc}`,
//! `string_util_impl_helpers.h`, `string_util_internal.h`,
//! `whitespace_constants.h`, `trim_string_internal.h`, and `join_string_internal.h`.
//!
//! # Representation
//!
//! The C++ functions are templated over `char` (arbitrary bytes, UTF-8 by
//! convention) and `char16_t` (UTF-16). Here both are handled by the sealed
//! [`CodeUnit`] trait, implemented for `u8` and `u16`. The 8-bit functions take
//! `&[u8]`, not `&str`, because the C++ accepts non-UTF-8 bytes and tests them,
//! for example in [`is_string_utf8`]. Where the result is always valid UTF-8, a
//! `&str` or `String` helper is also provided (for example
//! [`to_lower_ascii_str`]).
//!
//! Out-parameters become return values. Functions that wrote into `output`
//! return the new value. Functions that also returned a `bool` (for example
//! `ReplaceChars`) return `(Vec<T>, bool)`. Functions that returned a
//! `TrimPositions` return `(TrimPositions, Vec<T>)`.
//!
//! # Intentional differences
//!
//! - `CompareCaseInsensitiveASCII` returns `std::cmp::Ordering`, not -1/0/1.
//! - `StartsWith`, `EndsWith`, `RemovePrefix`, and `RemoveSuffix` apply
//!   ASCII-only case folding to 16-bit units. The C++ 16-bit case-insensitive
//!   path ignores the high byte of each unit (see [`starts_with`]).
//! - Mixed-width `EqualsCaseInsensitiveASCII` compares units as unsigned
//!   values. The C++ `char` overloads compare a non-ASCII byte as a signed
//!   value, so they never match a UTF-16 unit above 0x7F.
//! - `IsStringASCII` and `FindFirstNonASCII` are not word-at-a-time. The first
//!   reads every unit, and the second stops at the first match. Both are
//!   correct. The timing claim in the C++ comment is not reproduced exactly.
//! - `ReplaceStringPlaceholders` with a single substitution and an offset
//!   returns `Option<usize>` where the C++ CHECK-fails (see
//!   [`replace_string_placeholders_with_offset`]).
//! - `TruncateUTF8ToByteSize` and its `&str` form return slices, not an
//!   out-parameter. The C++ `DCHECK` on `byte_size <= INT32_MAX` is not
//!   reproduced, because `usize` lengths are not truncated.
//! - `HexDigitToInt` on an invalid digit is a debug assertion, as in the C++
//!   DCHECK. In release builds the result is a wrapped value, as in C++.
//! - `WriteInto` returns a zero-filled `Vec` instead of a raw pointer into the
//!   string. A caller fills the Vec directly.
//! - `IsWprintfFormatPortable` takes `&str`. Each `char` is one code unit.
//!
//! # Not ported
//!
//! - `wchar_t` overloads: `IsStringASCII(wstring_view)`, `FindFirstNonASCII`,
//!   `EqualsCaseInsensitiveASCII` with wide strings, `wcslcpy`, and the
//!   `wchar_t` forms of `RemovePrefix`/`RemoveSuffix`. They are platform
//!   specific and are not wired in.
//! - Windows-only code: `string_util_win.{h,cc}`, and the `as_wcstr` and
//!   `as_u16cstr` helpers (`WCHAR_T_IS_16_BIT` only).
//! - `string_util_posix.h` (`vsnprintf`, `vswprintf`) and `snprintf`. Use
//!   `format!` and `write!`.
//! - `EmptyString()` and `EmptyString16()` return global references. Use
//!   `String::new()` or `""`.
//! - `string_util_perftest.cc` and the fuzzers are not ported.
//! - `ASCIIToUTF16` and `UTF16ToASCII` live in `utf_string_conversions`, which
//!   is a different unit. The unit tests use that sibling crate.
//! - The `constexpr` static_assert checks in the C++ tests run as normal
//!   runtime asserts, because Rust has no constexpr `JoinString`.
//! - `CaseInsensitiveCompareASCII` as a functor type is replaced by
//!   [`eq_ignore_ascii_case_unit`].

#![forbid(unsafe_code)]

mod affix;
mod ascii;
mod buffer;
mod code_unit;
mod format;
mod placeholders;
mod replace;
mod trim;
mod utf8;

pub use affix::{ends_with, remove_prefix, remove_suffix, starts_with, CompareCase};
pub use ascii::{
    compare_case_insensitive_ascii, equals_ascii, equals_case_insensitive_ascii,
    eq_ignore_ascii_case_unit, find_first_non_ascii, hex_digit_to_int, is_string_ascii,
    to_lower_ascii, to_lower_ascii_str, to_upper_ascii, to_upper_ascii_str,
};
pub use buffer::{strlcpy, write_into};
pub use code_unit::{
    is_ascii_alpha, is_ascii_alphanumeric, is_ascii_control, is_ascii_digit, is_ascii_lower,
    is_ascii_punctuation, is_ascii_printable, is_ascii_upper, is_ascii_whitespace, is_hex_digit,
    is_unicode_control, is_unicode_whitespace, is_whitespace, to_lower_ascii_char,
    to_upper_ascii_char, CodeUnit, INFRA_ASCII_WHITESPACE, UTF8_BYTE_ORDER_MARK,
    WHITESPACE_ASCII, WHITESPACE_ASCII_NO_CR_LF, WHITESPACE_NO_CR_LF_UTF16, WHITESPACE_UTF16,
};
pub use format::is_wprintf_format_portable;
pub use placeholders::{
    replace_string_placeholders, replace_string_placeholders_strict,
    replace_string_placeholders_with_offset, replace_string_placeholders_with_offsets,
};
pub use replace::{
    join_string, remove_chars, replace_chars, replace_first_substring_after_offset,
    replace_substrings_after_offset,
};
pub use trim::{
    collapse_whitespace, collapse_whitespace_ascii, collapse_whitespace_ascii_str,
    contains_only_chars, trim_string, trim_string_to_vec, trim_whitespace, trim_whitespace_ascii,
    trim_whitespace_ascii_str, trim_whitespace_ascii_to_vec, trim_whitespace_to_vec,
    TrimPositions,
};
pub use utf8::{
    is_string_utf8, is_string_utf8_allowing_noncharacters, truncate_utf8_to_byte_size,
    truncate_utf8_to_byte_size_str,
};
