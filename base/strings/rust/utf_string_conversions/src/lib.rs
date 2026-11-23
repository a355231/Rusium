// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of Chromium's UTF-8 / UTF-16 / wide string conversions.
//!
//! Ported from:
//! - base/strings/utf_string_conversions.{h,cc}
//! - base/strings/utf_string_conversion_utils.{h,cc}
//! - base/strings/utf_offset_string_conversions.{h,cc}
//! - the UTF-8 and UTF-16 macros from base/third_party/icu/icu_utf.h
//!   (`icu_utf.h` only; there is no `icu_utf.cc` in this checkout).
//!
//! # Representation
//!
//! - UTF-8 input is `&[u8]`, because it may be ill-formed. Output is `String`.
//! - UTF-16 is `&[u16]` and `Vec<u16>`.
//! - Wide (`wchar_t`) is UTF-32, so it is `&[u32]` and `Vec<u32>`. Values that
//!   are not Unicode scalar values are ill-formed.
//! - A valid code point is a `char`.
//!
//! # Ill-formed input
//!
//! Each ill-formed unit becomes one U+FFFD, following the same rules as the C++
//! (`DoUTFConversion`, `CBU8_NEXT`). The UTF-8 rule is that a lead byte and the
//! valid continuation bytes after it are consumed together (the "maximal
//! subpart"). Encoded surrogates, overlong forms, and values above U+10FFFF are
//! rejected. Lone UTF-16 surrogates each become one U+FFFD. Non-characters are
//! valid and pass through. Each conversion returns [`Converted`] with
//! `success == false` if any replacement was written. The `_lossy` functions
//! return the text only, like the C++ overloads that return a string.
//!
//! # Differences from the C++
//!
//! - Out-parameters are replaced by return values. The `bool` result becomes
//!   [`Converted::success`]. Adjustment outputs become the second element of a
//!   tuple.
//! - Overloads that take a pointer and length (`const char*`, `const wchar_t*`,
//!   `const char16_t*`) are not ported; slices cover them.
//! - `ReadUnicodeCharacter` returns `Option<char>`. C++ returns a `bool` and
//!   leaves a possibly invalid `UChar32` in its output. That value is not
//!   exposed here.
//! - `WriteUnicodeCharacter` takes a `char`, so surrogates and out-of-range
//!   values cannot be written. The C++ would write ill-formed bytes for them.
//! - Offsets use `Option<usize>`. `None` is `std::u16string::npos`, and an
//!   input `None` is returned unchanged. The C++ `limit` default of `npos` is
//!   `None` here.
//! - `OffsetAdjuster`'s static methods are free functions. `size_t` arithmetic
//!   wraps as in C++, through `wrapping_*`.
//! - The DCHECKs in `MergeSequentialAdjustments` are `debug_assert!`s.
//! - `ASCIIToUTF16`, `UTF16ToASCII`, and the two DCHECKs on ASCII do not check
//!   that the input is ASCII. Output matches a release build of the C++ (widen
//!   each byte, or truncate each unit to 8 bits).
//! - The ASCII fast path (`features::kUtfConversionAsciiFastPath`) and
//!   `strings_internal::InitializeUtfStringConversionsFeatures` are not ported.
//!   Output and `success` are the same either way. The `PrepareForUTF8Output`
//!   and `PrepareForUTF16Or32Output` reserve heuristics are not ported either.
//! - The C++ unit-test pairs that compare the fast path with the legacy path
//!   are ported as comparisons against std's decoders, because one code path
//!   remains here.
//! - The `wchar_t` 16-bit (Windows) variants are not ported. `WideToUTF16`
//!   and `UTF16ToWide` with 16-bit wide are no-ops, and `ASCIIToWide` and
//!   `WideToASCII` exist only in that build. Wide is always UTF-32 in this
//!   crate. The 16-bit test expectations are ported against UTF-16 directly.
//! - The `static_assert` overloads that reject string literals are not ported.
//!   They are compile-time checks in C++, and Rust needs no equivalent.
//! - `EstimateMemoryUsage` is not ported (no `base::trace_event`).
//!
//! # Modules
//!
//! - [`conversion_utils`]: code-point predicates, readers, and writers.
//! - [`conversions`]: UTF-8/UTF-16/UTF-32 conversions.
//! - [`offset_conversions`]: offset adjustment and conversions that report it.

#![forbid(unsafe_code)]

mod icu_utf;

pub mod conversion_utils;
pub mod conversions;
pub mod offset_conversions;

#[cfg(test)]
mod test_util;

pub use conversion_utils::{
    count_unicode_characters, is_valid_character, is_valid_codepoint, read_unicode_character_utf16,
    read_unicode_character_utf32, read_unicode_character_utf8, write_unicode_character_utf16,
    write_unicode_character_utf32, write_unicode_character_utf8, REPLACEMENT_CHARACTER,
};
pub use conversions::{
    ascii_to_utf16, utf16_to_ascii, utf16_to_utf8, utf16_to_utf8_lossy, utf16_to_wide,
    utf16_to_wide_lossy, utf8_to_utf16, utf8_to_utf16_lossy, utf8_to_wide, utf8_to_wide_lossy,
    wide_to_utf16, wide_to_utf16_lossy, wide_to_utf8, wide_to_utf8_lossy, Converted,
};
pub use offset_conversions::{
    adjust_offset, adjust_offsets, merge_sequential_adjustments, unadjust_offset, unadjust_offsets,
    utf16_to_utf8_and_adjust_offsets, utf8_to_utf16_and_adjust_offsets,
    utf8_to_utf16_with_adjustments, Adjustment,
};
