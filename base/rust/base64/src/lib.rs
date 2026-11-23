// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base/base64.{h,cc}` and `base/base64url.{h,cc}`.
//!
//! - [`standard`] is RFC 4648 section 4 base64 (`base::Base64Encode`,
//!   `base::Base64Decode`).
//! - [`url`] is RFC 4648 section 5 base64url (`base::Base64UrlEncode`,
//!   `base::Base64UrlDecode`).
//!
//! Both are implemented natively with std only. The C++ code calls
//! `third_party/modp_b64` and, for encoding, an optional base64-simd path
//! behind `features::kBase64SimdBase64Encode`. Neither is depended on here.
//!
//! Differences from the C++ version:
//! - Inputs and outputs are `&[u8]`, `Vec<u8>` and `String`. The C++
//!   out-parameter overloads (`std::string*`) become return values, except
//!   `standard::encode_append`, which appends to a caller-owned `String` as
//!   `Base64EncodeAppend` does.
//! - Decoding returns `Option<Vec<u8>>`. `None` covers the C++ `false` return
//!   and `std::nullopt`. The C++ "output is untouched on failure" guarantee
//!   holds trivially because no output is passed in.
//! - The C++ in-place overloads (input and output alias) are not needed:
//!   Rust borrows prevent aliasing, so callers simply use the returned value.
//! - The feature-flag dispatch in `Base64Encode` is removed. The modp and
//!   base64-simd encoders produce identical output (standard alphabet, padded),
//!   so a single encoder is used.
//! - `Base64EncodeEarlyStartup` and `Base64UrlEncodeEarlyStartup` are not
//!   ported as separate functions. They exist only to avoid touching
//!   `base::Feature` during early startup, and this port has no such access.
//!   Their output is the same as `standard::encode` and `url::encode`.
//! - The `*ForTesting` hooks that select a specific encoder implementation are
//!   not ported, for the same reason.
//! - The C++ `CHECK` on oversized input (`MODP_B64_MAX_INPUT_LEN`) is kept as
//!   a panic in `standard::encoded_len`. A Rust slice is at most `isize::MAX`
//!   bytes, which is below that limit, so `standard::encode` cannot reach it.
//!   The C++ overflow check on the append length is kept as a panic
//!   in `standard::encode_append`.
//! - Decoding follows modp_b64's algorithm: for each non-canonical final
//!   character, the unused low bits are ignored and accepted, as in C++.
//!   The decoder uses a bit accumulator rather than modp's 4-byte chunks. The
//!   results are the same for all inputs.
//! - Unit tests are ported with the same inputs and expected outputs. The
//!   gtest death tests (`EXPECT_CHECK_DEATH`) become `#[should_panic]` tests
//!   where the same check is reachable. Randomized tests (the fuzz tests in
//!   base64url_unittest.cc) are replaced by seeded xorshift tests in the same
//!   crate. Those tests also check against a separate reference
//!   implementation.

#![forbid(unsafe_code)]

pub mod standard;
pub mod url;
