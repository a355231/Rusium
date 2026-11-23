// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base/strings/escape.{h,cc}` and the parts of
//! `base/strings/utf_offset_string_conversions.{h,cc}` that the escape code
//! depends on.
//!
//! Covered: percent-escaping (`escape`), percent-unescaping with the
//! `UnescapeRule` flags (`unescape`), the UTF-8 to UTF-16 conversion that
//! records offset adjustments (`utf8`), and `OffsetAdjuster::AdjustOffset` and
//! `MergeSequentialAdjustments` (`offset`).
//!
//! Differences from the C++ version:
//! - Byte-oriented inputs are `&[u8]`, not `&str`, because the C++ accepts
//!   arbitrary bytes (for example `EscapePath("\x80\xff")`). Escape outputs are
//!   always ASCII and are returned as `String`. Unescape outputs are `Vec<u8>`,
//!   because unescaping can produce bytes that are not valid UTF-8.
//! - Out-parameters become return values. `UnescapeBinaryURLComponentSafe`
//!   returns `None` where the C++ returns `false` and clears its output.
//!   `UnescapeAndDecodeUTF8URLComponentWithAdjustments` always returns the
//!   adjustments. The C++ `nullptr` case gives the same string, so only the
//!   extra work differs.
//! - `UnescapeRule` is a bit-set struct with the same constants. C++
//!   `DCHECK`s become `debug_assert!`s, so the same misuse aborts only in debug
//!   builds. Release builds ignore unsupported flags, as the C++ does.
//! - `EscapeForHTML(std::u16string)` is not ported. The `&[u8]` version gives
//!   the same result on UTF-8 input. `UnescapeForHTML` takes `&str`, because
//!   UTF-16 lone surrogates cannot be represented in a Rust string. Its
//!   replacement table is ASCII, so the result is the same for valid text.
//! - `UnescapeAndDecodeUTF8URLComponentWithAdjustments` returns `String`.
//!   The C++ returns `std::u16string`, but every value it can produce is valid
//!   UTF-16 with no lone surrogates.
//! - `EscapeNSURLPrecursor` is compiled on every platform. The C++ version is
//!   Apple-only (`BUILDFLAG(IS_APPLE)`). Its output is the same.
//! - The UTF-8 decoder is a re-implementation of the ICU `CBU8_NEXT` rules
//!   (see `utf8.rs`) with no ICU dependency. Ill-formed input is handled the
//!   same way: each run of invalid bytes becomes one U+FFFD, and the run covers
//!   the longest valid prefix of a sequence (at least one byte).
//!
//! Not ported:
//! - `escape_fuzzer.cc`.
//! - `OffsetAdjuster::AdjustOffsets`, `UnadjustOffset(s)`,
//!   `UTF8ToUTF16AndAdjustOffsets` and `UTF16ToUTF8AndAdjustOffsets`. Escaping
//!   needs only `AdjustOffset` and `MergeSequentialAdjustments`.
//! - The `base::` helpers the C++ uses, such as `ASCIIToUTF16`, `StringPrintf`
//!   and `IsHexDigit`. Each is replaced by the standard library or
//!   re-implemented inside the crate.
//!
//! Testing: `src/tests.rs` ports every `TEST` in `escape_unittest.cc`. It also
//! adds randomized differential tests (a small xorshift PRNG) against
//! independent reference predicates and round-trip properties.

#![forbid(unsafe_code)]

mod escape;
mod offset;
mod unescape;
mod utf8;

pub use escape::*;
pub use offset::*;
pub use unescape::*;

#[cfg(test)]
mod tests;
