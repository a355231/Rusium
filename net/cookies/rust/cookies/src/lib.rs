// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of the cookie-line parsing and cookie option units of
//! `net/cookies`:
//!
//! - `net/cookies/parsed_cookie.{h,cc}` -> [`parsed_cookie`]
//! - `net/cookies/cookie_constants.{h,cc}` -> [`cookie_constants`]
//! - `net/cookies/cookie_options.{h,cc}` -> [`cookie_options`]
//!
//! [`cookie_inclusion_status`] is a reduced stand-in for
//! `net::CookieInclusionStatus`; see that module for what is missing.
//!
//! # Differences from the C++ code
//!
//! - Inputs are `&str`, not arbitrary byte strings. Every position the parser
//!   works with is either a boundary of an ASCII delimiter or the end of the
//!   string, so slicing never splits a UTF-8 sequence. Non-UTF-8 input is
//!   impossible by construction. Control characters, `;`, `=` and the
//!   terminators (`\n`, `\r`, `\0`) are rejected exactly as in C++.
//! - Functions that take C++ iterators (`FindFirstTerminator`, `ParseToken`,
//!   `ParseValue`) take and return byte offsets into a `&str`.
//! - Out-parameters become return values: `ParsedCookie::new_with_status`
//!   returns the status alongside the cookie; `ParsedCookie::parse_token`
//!   returns `Option<Range<usize>>`.
//! - `ParsedCookie::name()` and `value()` return `""` for an invalid cookie.
//!   The C++ accessors index `pairs_[0]` without checking, which is undefined
//!   behavior on an empty vector.
//! - `ParsedCookie::number_of_attributes()` returns 0 for an invalid cookie.
//!   The C++ `pairs_.size() - 1` wraps around to `SIZE_MAX` in that case.
//! - `ParsedCookie::for_each_attribute()` returns `true` without calling the
//!   callback on an invalid cookie. The C++ code CHECK-fails there.
//! - Histogram-only sentinels (`kMaxValue`, `COUNT`) are not ported, since
//!   they only give the bounds of histogram buckets.
//! - Nested C++ types are hoisted to module level (for example
//!   `CookieOptions::SameSiteCookieContext::ContextType` becomes
//!   `cookie_options::ContextType`). `HttpMethod::KPut` is spelled `Put`.
//! - `DCHECK`s in `cookie_options` become `debug_assert!`s. The CHECK in the
//!   `ParsedCookie` constructor becomes `assert!`.
//!
//! # Not ported
//!
//! - `GetSchemeNameEnum(const GURL&)` and the `GURL`/`url::` constants it
//!   needs. The `CookieSourceSchemeName` enum itself is ported.
//! - `CookieInclusionStatus` beyond the reasons that the parsed-cookie unit
//!   emits. The SameSite, third-party-phaseout and exemption logic, and
//!   `GetDebugString()`, are not ported.
//! - `ParsedCookie` `EXPECT`-only helpers in the header are not needed.
//! - `base::FunctionRef` is replaced by a generic `FnMut` bound.
//! - `CookieOptions` access-time and HttpOnly flags are plain fields; the
//!   mojom default-value syncing comments are not applicable.
//! - `ParsedCookie::ValueMatchesParsedValue` and the other static helpers
//!   are ported as associated functions. Their iterator-based forms are
//!   expressed with byte offsets, as described above.
//! - `EstimateMemoryUsage`, `operator<<` and `PrintTo` for gtest are not
//!   ported. The `Display` impls in `cookie_options` give the same text as
//!   `PrintTo` where tests depend on it.

#![forbid(unsafe_code)]

pub mod cookie_constants;
pub mod cookie_inclusion_status;
pub mod cookie_options;
pub mod parsed_cookie;

pub use cookie_constants::*;
pub use cookie_inclusion_status::CookieInclusionStatus;
pub use cookie_options::{CookieOptions, SameSiteCookieContext};
pub use parsed_cookie::ParsedCookie;
