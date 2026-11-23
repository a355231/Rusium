// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::HttpUtil` (net/http/http_util.{h,cc}).
//!
//! The C++ class is a set of static functions. Here they are free functions,
//! and the three iterator classes are ported as structs. Header data is handled
//! as raw bytes (`&[u8]`), because HTTP header values may contain bytes that
//! are not valid UTF-8 (obs-text). Owned results are `Vec<u8>` for the same
//! reason.
//!
//! Ported (`http_util_unittest.cc` cases in parentheses):
//! - `ParseContentType` with the `ParseMimeType` logic it needs
//!   (`ParseContentType`, `ParseContentResetCharset`)
//! - `ParseRangeHeader`, `ParseFetchSingleRange`, `ParseContentRangeHeaderFor206`
//!   (`ParseRangeHeader`, `ParseFetchSingleRange*` x9, `ParseContentRangeHeader`)
//! - `ParseRetryAfterHeader`, `TimeFormatHTTP` (`ParseRetryAfterHeader`,
//!   `TimeFormatHTTP`)
//! - `HasValidators`, `HasStrongValidators` (same names)
//! - `IsMethodSafe`, `IsMethodIdempotent`, `IsSafeHeader`, `IsValidHeaderName`,
//!   `IsValidHeaderValue`, `IsNonCoalescingHeader`, `IsTokenChar`, `IsToken`,
//!   `IsParmName`, `IsLWS`, `IsControlChar`, `TrimLWS` (same names)
//! - `Unquote`, `StrictUnquote`, `Quote` (same names)
//! - `LocateStartOfStatusLine`, `LocateEndOfHeaders`,
//!   `LocateEndOfAdditionalHeaders`, `AssembleRawHeaders`,
//!   `ConvertHeadersBackToHTTPResponse` (same names)
//! - `ExpandLanguageList`, `GenerateAcceptLanguageHeader` (same names)
//! - `ParseAcceptEncoding`, `ParseContentEncoding` (same names)
//! - `GetStatusCodesForHistogram`, `MapStatusCodeForHistogram`
//! - `HeadersContainMultipleCopiesOfField`, as `has_conflicting_values`
//! - `HeadersIterator`, `ValuesIterator`, `NameValuePairsIterator`
//!
//! Not ported:
//! - `GenerateRequestLine`, `SpecForRequest` (and `RequestUrlSanitize`, its
//!   test): they need `GURL` and `url_util`.
//! - `TrimLWS` overload on `std::string::const_iterator`: the byte-slice and
//!   offset overloads replace it.
//! - `HeadersContainMultipleCopiesOfField` takes `HttpResponseHeaders`. The
//!   logic is ported over a slice of already-extracted values.
//! - `ParseFetchSingleRange`/`ParseRangeHeader` return a local
//!   `HttpByteRange` (see below), not the `net::HttpByteRange` class.
//! - The `FUZZ_TEST` harness. Its property (no crash on arbitrary input) is
//!   checked with a seeded random test instead.
//!
//! Intentional differences from the C++ code:
//! - `ParseRangeHeader` and `ParseFetchSingleRange` return a local
//!   `HttpByteRange` with `Option` fields. The C++ class uses -1 as "not
//!   specified", so `bytes=0--1` is accepted there as open-ended. Here -1 is a
//!   parsed value and the range is rejected.
//! - Functions that take a `std::string*` out-parameter return an owned value
//!   (`Option`, `Vec<u8>`) instead. `ParseContentType` takes a
//!   `&mut ContentType` state, because it accumulates across several
//!   `Content-Type` headers.
//! - `ParseContentType` always reports `boundary`. The C++ caller may pass
//!   nullptr to skip it.
//! - `NameValuePairsIterator::get_next()` returns `false` after an error,
//!   instead of CHECK-failing as the C++ code does on a call after error.
//! - `ParseAcceptEncoding` and `ParseContentEncoding` return `None` on failure
//!   and do not return a partial set.
//! - `base::TrimWhitespace(TRIM_WHITESPACE)` trims Unicode whitespace in
//!   `ExpandLanguageList`. Here only the ASCII whitespace set
//!   (space, \t, \n, \v, \f, \r) is trimmed.
//! - `AcceptLanguageBuilder::AddLanguageCode` DCHECKs that its input is ASCII.
//!   Here there is no check and the input is passed through.
//! - `ParseRetryAfterHeader` and `HasValidators`/`HasStrongValidators` use
//!   `Time::from_utc_string`/`Time::from_string` from `time.rs`. Those accept a
//!   narrower set of date formats than `base::Time::FromString`, and treat
//!   zone-less input as UTC, not local time.
//! - `Time` is a microsecond count. It has no clock and no time zone.
//! - Strings that are passed to C-string based C++ code (`Time::FromString`
//!   and friends) are cut at the first NUL byte, as they are in C++.

#![forbid(unsafe_code)]

use std::collections::{BTreeSet, HashSet};

mod byte_range;
mod mime;
mod time;

pub use byte_range::HttpByteRange;
pub use time::{Exploded, Time, TimeDelta};

/// Linear white space (SP | HT), `HTTP_LWS` in the C++ code.
pub const HTTP_LWS: &[u8] = b" \t";

/// An HTTP version, compared as (major, minor), like `net::HttpVersion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpVersion {
    major: u16,
    minor: u16,
}

impl HttpVersion {
    /// Creates a version from its major and minor numbers.
    pub const fn new(major: u16, minor: u16) -> Self {
        HttpVersion { major, minor }
    }
}

/// Parsed `Content-Type` state, accumulated across one or more headers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentType {
    /// Lowercased MIME type, such as `text/html`.
    pub mime_type: Vec<u8>,
    /// Lowercased charset. Empty if the last type change dropped it.
    pub charset: Vec<u8>,
    /// True once a charset has been seen for the current type.
    pub had_charset: bool,
    /// The unquoted `boundary` parameter, if one was present.
    pub boundary: Vec<u8>,
}

/// The three values from a `Content-Range` header, as returned by
/// [`parse_content_range_header_for_206`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    /// Inclusive position of the first byte.
    pub first_byte_position: i64,
    /// Inclusive position of the last byte.
    pub last_byte_position: i64,
    /// Total size of the resource.
    pub instance_length: i64,
}

// ---------------------------------------------------------------------------
// Byte helpers.
// ---------------------------------------------------------------------------

/// Returns true for SP and HT, matching `HttpUtil::IsLWS`.
pub fn is_lws(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

/// ASCII whitespace as in `base::IsAsciiWhitespace`, which includes \v.
fn is_ascii_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn trim_ascii_ws(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&c| !is_ascii_ws(c)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&c| !is_ascii_ws(c))
        .map_or(start, |i| i + 1);
    &s[start..end.max(start)]
}

/// Trims leading and trailing LWS (SP and HT) from `s`.
pub fn trim_lws(s: &[u8]) -> &[u8] {
    let (begin, end) = trim_lws_range(s, 0, s.len());
    &s[begin..end]
}

/// Trims LWS from the sub-range `begin..end` of `s` and returns the new
/// bounds, like the offset overload of `HttpUtil::TrimLWS`.
///
/// Panics if `end > s.len()`, where the C++ version would read out of bounds.
pub fn trim_lws_range(s: &[u8], begin: usize, end: usize) -> (usize, usize) {
    let mut b = begin;
    let mut e = end;
    while b < e && is_lws(s[b]) {
        b += 1;
    }
    while b < e && is_lws(s[e - 1]) {
        e -= 1;
    }
    (b, e)
}

fn starts_with_ignore_case(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// Returns `s` up to (not including) the first NUL, for APIs that take a C
/// string in the C++ code.
fn truncate_at_nul(s: &[u8]) -> &[u8] {
    match s.iter().position(|&c| c == 0) {
        Some(i) => &s[..i],
        None => s,
    }
}

/// Parses `s` the way `base::StringToInt64` does. An optional '+' or '-' may
/// come first, then one or more ASCII digits. Whitespace, trailing characters
/// and out-of-range values are rejected.
fn string_to_int64(s: &[u8]) -> Option<i64> {
    let (negative, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut magnitude: i128 = 0;
    for &c in digits {
        magnitude = magnitude * 10 + (c - b'0') as i128;
        if magnitude > (1i128 << 64) {
            return None;
        }
    }
    let value = if negative { -magnitude } else { magnitude };
    i64::try_from(value).ok()
}

/// Parses a non-negative decimal (`ParseUint32` with `NON_NEGATIVE`): digits
/// only, leading zeros allowed, no sign.
fn parse_uint32_non_negative(s: &[u8]) -> Option<u32> {
    if s.is_empty() || !s.iter().all(u8::is_ascii_digit) {
        return None;
    }
    s.iter().try_fold(0u32, |acc, &c| {
        acc.checked_mul(10)?.checked_add((c - b'0') as u32)
    })
}

/// A set of byte values, used for tokenizer delimiters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ByteSet([u64; 4]);

impl ByteSet {
    fn new(bytes: &[u8]) -> Self {
        let mut bits = [0u64; 4];
        for &b in bytes {
            bits[(b >> 6) as usize] |= 1u64 << (b & 63);
        }
        ByteSet(bits)
    }

    fn contains(&self, b: u8) -> bool {
        (self.0[(b >> 6) as usize] >> (b & 63)) & 1 == 1
    }
}

/// A splitter with the same rules as `base::StringViewTokenizer` for the
/// cases this crate uses.
///
/// - `delims` is a set. Any byte in it ends a token.
/// - With `quote` set, a quote byte opens a quoted run. Delimiters inside it do
///   not split the token. A backslash inside the run escapes the next byte. A
///   matching quote closes the run.
/// - Empty tokens are skipped unless `return_empty` is set. With
///   `return_empty`, the start and end of the input count as delimiters, so
///   an empty input gives one empty token.
#[derive(Debug, Clone)]
struct Tokenizer<'a> {
    input: &'a [u8],
    delims: ByteSet,
    quote: Option<u8>,
    return_empty: bool,
    pos: usize,
    done: bool,
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a [u8], delims: &[u8], quote: Option<u8>, return_empty: bool) -> Self {
        Tokenizer {
            input,
            delims: ByteSet::new(delims),
            quote,
            return_empty,
            pos: 0,
            done: false,
        }
    }

    /// Returns the byte range of the next token within `input`.
    fn next_token(&mut self) -> Option<(usize, usize)> {
        let len = self.input.len();
        loop {
            if self.done {
                return None;
            }
            let start = self.pos;
            let mut j = start;
            let mut in_quote = false;
            let mut in_escape = false;
            let mut quote_char = 0u8;
            while j < len {
                let c = self.input[j];
                if in_quote {
                    if in_escape {
                        in_escape = false;
                    } else if c == b'\\' {
                        in_escape = true;
                    } else if c == quote_char {
                        in_quote = false;
                    }
                } else {
                    if self.delims.contains(c) {
                        break;
                    }
                    quote_char = c;
                    in_quote = self.quote == Some(c);
                }
                j += 1;
            }
            if j == len {
                self.done = true;
            } else {
                self.pos = j + 1;
            }
            if j > start || self.return_empty {
                return Some((start, j));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Token and string predicates.
// ---------------------------------------------------------------------------

/// Returns true if `c` is a `tchar` (RFC 7230 section 3.2.6).
pub fn is_token_char(c: u8) -> bool {
    match c {
        0..=0x20 | 0x7f..=0xff => false,
        b'(' | b')' | b'<' | b'>' | b'@' | b',' | b';' | b':' | b'\\' | b'"' | b'/' | b'['
        | b']' | b'?' | b'=' | b'{' | b'}' => false,
        _ => true,
    }
}

/// Returns true if `s` is a non-empty token (RFC 7230 section 3.2.6).
pub fn is_token(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(|&c| is_token_char(c))
}

/// Returns true if `s` is a valid `parmname` (RFC 5987 section 3.2.1).
pub fn is_param_name(s: &[u8]) -> bool {
    !s.is_empty()
        && s
            .iter()
            .all(|&c| is_token_char(c) && c != b'*' && c != b'\'' && c != b'%')
}

/// Returns true for a control character (CTL, RFC 5234 appendix B.1).
pub fn is_control_char(c: u8) -> bool {
    c <= 0x1f || c == 0x7f
}

/// Returns true if `name` is a valid HTTP header name. Same as `is_token`.
pub fn is_valid_header_name(name: &[u8]) -> bool {
    is_token(name)
}

/// Returns false if `value` contains NUL, CR or LF. This is only a sanity
/// check, not a full RFC 2616 validation.
pub fn is_valid_header_value(value: &[u8]) -> bool {
    !value.iter().any(|&c| c == 0 || c == b'\r' || c == b'\n')
}

/// Returns true if `method` is a "safe" method (RFC 7231 section 4.2.1).
pub fn is_method_safe(method: &[u8]) -> bool {
    matches!(method, b"GET" | b"HEAD" | b"OPTIONS" | b"TRACE")
}

/// Returns true if `method` is idempotent (RFC 7231 section 4.2.2).
pub fn is_method_idempotent(method: &[u8]) -> bool {
    is_method_safe(method) || method == b"PUT" || method == b"DELETE"
}

const FORBIDDEN_HEADER_FIELDS: &[&[u8]] = &[
    b"accept-charset",
    b"accept-encoding",
    b"access-control-request-headers",
    b"access-control-request-method",
    b"connection",
    b"content-length",
    b"cookie",
    b"cookie2",
    b"date",
    b"dnt",
    b"expect",
    b"host",
    b"keep-alive",
    b"origin",
    b"referer",
    b"set-cookie",
    b"te",
    b"trailer",
    b"transfer-encoding",
    b"upgrade",
    // Still listed, though no longer banned by the fetch standard. See
    // crbug.com/571722.
    b"user-agent",
    b"via",
];

const FORBIDDEN_HEADER_FIELDS_WITH_FORBIDDEN_METHOD: &[&[u8]] =
    &[b"x-http-method", b"x-http-method-override", b"x-method-override"];

const FORBIDDEN_METHODS: &[&[u8]] = &[b"connect", b"trace", b"track"];

/// Returns true if it is safe to let script set a header with this name and
/// value (https://fetch.spec.whatwg.org/#forbidden-request-header). Does not
/// check that the header is valid.
pub fn is_safe_header(name: &[u8], value: &[u8]) -> bool {
    if starts_with_ignore_case(name, b"proxy-") || starts_with_ignore_case(name, b"sec-") {
        return false;
    }
    if FORBIDDEN_HEADER_FIELDS
        .iter()
        .any(|field| name.eq_ignore_ascii_case(field))
    {
        return false;
    }
    if FORBIDDEN_HEADER_FIELDS_WITH_FORBIDDEN_METHOD
        .iter()
        .any(|field| name.eq_ignore_ascii_case(field))
    {
        let mut methods = ValuesIterator::new(value, b',', true);
        while methods.get_next() {
            let method = methods.value();
            if FORBIDDEN_METHODS
                .iter()
                .any(|forbidden| method.eq_ignore_ascii_case(forbidden))
            {
                return false;
            }
        }
    }
    true
}

const NON_COALESCING_HEADERS: &[&[u8]] = &[
    b"date",
    b"expires",
    b"last-modified",
    // See bug 1050541.
    b"location",
    b"retry-after",
    b"set-cookie",
    // Auth challenges mix space-separated tokens with comma-separated
    // properties, so they cannot be joined on commas.
    b"www-authenticate",
    b"proxy-authenticate",
    // STS: user agents must ignore every STS header after the first.
    b"strict-transport-security",
];

/// Returns true for headers whose repeated values must not be joined with
/// commas, because the values may themselves contain commas.
pub fn is_non_coalescing_header(name: &[u8]) -> bool {
    NON_COALESCING_HEADERS
        .iter()
        .any(|header| name.eq_ignore_ascii_case(header))
}

/// Returns true if `value` has at least two different entries. Equivalent to
/// `HttpUtil::HeadersContainMultipleCopiesOfField` over the values already
/// read for one field name. Each later value is compared with the first one,
/// as the C++ code does.
pub fn has_conflicting_values(values: &[&[u8]]) -> bool {
    match values.split_first() {
        Some((first, rest)) => rest.iter().any(|value| value != first),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Trimming, quoting and status lines.
// ---------------------------------------------------------------------------

fn is_quote(c: u8) -> bool {
    c == b'"'
}

fn unquote_impl(s: &[u8], strict: bool) -> Option<Vec<u8>> {
    if s.is_empty() || !is_quote(s[0]) {
        return None;
    }
    // No terminal quote mark.
    if s.len() < 2 || s[0] != s[s.len() - 1] {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let mut prev_escape = false;
    let mut out = Vec::with_capacity(inner.len());
    for &c in inner {
        if c == b'\\' && !prev_escape {
            prev_escape = true;
            continue;
        }
        if strict && !prev_escape && is_quote(c) {
            return None;
        }
        prev_escape = false;
        out.push(c);
    }
    // A terminal quote that was escaped.
    if strict && prev_escape {
        return None;
    }
    Some(out)
}

/// Removes the surrounding quotes from a quoted-string and unescapes
/// quoted-pairs (RFC 2616 section 2.2). If `s` is not a quoted-string, returns
/// it unchanged.
pub fn unquote(s: &[u8]) -> Vec<u8> {
    unquote_impl(s, false).unwrap_or_else(|| s.to_vec())
}

/// Like `unquote`, but also rejects anything that is not a valid
/// quoted-string: no quotes, mismatched quotes, an unescaped embedded quote
/// or an escaped terminal quote. Returns `None` in those cases.
pub fn strict_unquote(s: &[u8]) -> Option<Vec<u8>> {
    unquote_impl(s, true)
}

/// Wraps `s` in quotes and escapes '"' and '\\' inside it. The reverse of
/// `unquote`.
pub fn quote(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    out.push(b'"');
    for &c in s {
        if c == b'"' || c == b'\\' {
            out.push(b'\\');
        }
        out.push(c);
    }
    out.push(b'"');
    out
}

/// Returns the offset of the "http" that starts a status line. Up to 4 bytes
/// of junk may come before it, as in Mozilla. Returns `None` if it is not
/// found.
pub fn locate_start_of_status_line(buf: &[u8]) -> Option<usize> {
    const SLOP: usize = 4;
    const HTTP_LEN: usize = 4;
    if buf.len() < HTTP_LEN {
        return None;
    }
    let i_max = (buf.len() - HTTP_LEN).min(SLOP);
    (0..=i_max).find(|&i| buf[i..i + HTTP_LEN].eq_ignore_ascii_case(b"http"))
}

fn locate_end_of_headers_helper(buf: &[u8], start: usize, accept_empty: bool) -> Option<usize> {
    // With `accept_empty`, a leading line break ends an empty header list.
    let mut last_c = if accept_empty { b'\n' } else { 0u8 };
    let mut was_lf = accept_empty;
    for (i, &c) in buf.iter().enumerate().skip(start) {
        if c == b'\n' {
            if was_lf {
                return Some(i + 1);
            }
            was_lf = true;
        } else if c != b'\r' || last_c != b'\n' {
            was_lf = false;
        }
        last_c = c;
    }
    None
}

/// Returns the index just past the end-of-headers marker, searching from
/// `start`. The marker is a blank line, which may be CRLF CRLF, LF LF, or a
/// mix such as LF CRLF. The first line of `buf` is the status line.
pub fn locate_end_of_headers(buf: &[u8], start: usize) -> Option<usize> {
    locate_end_of_headers_helper(buf, start, false)
}

/// Like `locate_end_of_headers`, but for header lists with no status line,
/// such as trailers and multipart parts. A single leading line break counts
/// as an empty header list.
pub fn locate_end_of_additional_headers(buf: &[u8], start: usize) -> Option<usize> {
    locate_end_of_headers_helper(buf, start, true)
}

/// A header line can be continued only if it has a non-empty name, and the
/// name does not start with LWS.
fn is_line_segment_continuable(line: &[u8]) -> bool {
    let Some(colon) = line.iter().position(|&c| c == b':') else {
        return false;
    };
    let name = &line[..colon];
    !name.is_empty() && !is_lws(name[0])
}

fn remove_leading_non_lws(s: &[u8]) -> &[u8] {
    s.iter()
        .position(|&c| !is_lws(c))
        .map_or(&[][..], |i| &s[i..])
}

/// Turns raw response header bytes into the NUL-separated form used by
/// `HttpResponseHeaders`. It normalizes line endings to NUL, and joins
/// continuation lines (lines that start with LWS) to the line above with one
/// space. Any NUL bytes in the input are removed first. `input` should end at
/// the end-of-headers marker.
pub fn assemble_raw_headers(input: &[u8]) -> Vec<u8> {
    // Skip any junk before the status line.
    let input = match locate_start_of_status_line(input) {
        Some(start) => &input[start..],
        None => input,
    };

    let status_line_end = input
        .iter()
        .position(|&c| c == b'\r' || c == b'\n')
        .unwrap_or(input.len());
    let mut raw = Vec::with_capacity(input.len() + 2);
    raw.extend_from_slice(&input[..status_line_end]);

    let rest = &input[status_line_end..];
    let mut prev_line_continuable = false;
    let mut lines = Tokenizer::new(rest, b"\r\n", None, false);
    while let Some((b, e)) = lines.next_token() {
        let line = &rest[b..e];
        if prev_line_continuable && is_lws(line[0]) {
            raw.push(b' ');
            raw.extend_from_slice(remove_leading_non_lws(line));
        } else {
            raw.push(b'\n');
            raw.extend_from_slice(line);
            prev_line_continuable = is_line_segment_continuable(line);
        }
    }
    raw.extend_from_slice(b"\n\n");

    // Remove embedded NULs first, so that the NULs written below are the only
    // separators.
    raw.retain(|&c| c != 0);
    for c in raw.iter_mut() {
        if *c == b'\n' {
            *c = 0;
        }
    }
    raw
}

/// Converts NUL-separated raw headers back to HTTP wire format, one CRLF per
/// NUL, followed by a blank line. This is for DevTools. The continuation
/// information is already lost by then.
pub fn convert_headers_back_to_http_response(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + 2);
    let mut tokens = Tokenizer::new(raw, &[0u8], None, false);
    while let Some((b, e)) = tokens.next_token() {
        out.extend_from_slice(&raw[b..e]);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out
}

// ---------------------------------------------------------------------------
// Language lists and accept headers.
// ---------------------------------------------------------------------------

/// Collects language tags for `Accept-Language`, dropping duplicates. The
/// first tag is stored as-is. Later tags are joined with ','.
#[derive(Default)]
struct AcceptLanguageBuilder {
    out: Vec<u8>,
    seen: HashSet<Vec<u8>>,
}

impl AcceptLanguageBuilder {
    fn add_language_code(&mut self, language: &[u8]) {
        if !self.seen.insert(language.to_vec()) {
            return;
        }
        if self.out.is_empty() {
            self.out = language.to_vec();
        } else {
            self.out.push(b',');
            self.out.extend_from_slice(language);
        }
    }
}

/// The primary subtag: the part before the first '-', with whitespace trimmed.
fn base_language_code(language: &[u8]) -> &[u8] {
    let prefix = match language.iter().position(|&c| c == b'-') {
        Some(dash) => &language[..dash],
        None => language,
    };
    trim_ascii_ws(prefix)
}

/// Expands a comma-separated language list by adding each primary language
/// subtag, which helps language matching (see
/// https://www.w3.org/International/questions/qa-lang-priorities). The
/// primary subtag is added after the last tag of its family. Duplicates are
/// removed. 'x' and 'i' are not treated as primary subtags. Q values are not
/// supported.
pub fn expand_language_list(language_prefs: &[u8]) -> Vec<u8> {
    let languages: Vec<&[u8]> = language_prefs
        .split(|&c| c == b',')
        .map(trim_ascii_ws)
        .collect();

    let mut builder = AcceptLanguageBuilder::default();
    let size = languages.len();
    for (i, &language) in languages.iter().enumerate() {
        builder.add_language_code(language);

        let base_language = base_language_code(language);
        if base_language == b"x" || base_language == b"i" {
            continue;
        }

        // Add the primary subtag if the next tag is not in the same family.
        // An input of "en-US,fr,en" yields "en-US,en,fr,en", so the later
        // "en" outranks "fr". This is a known imperfection of the C++ code.
        let next_differs = i + 1 >= size || base_language_code(languages[i + 1]) != base_language;
        if next_differs {
            builder.add_language_code(base_language);
        }
    }
    builder.out
}

/// Turns a comma-separated language list into an `Accept-Language` value by
/// giving each tag after the first a descending q value. The q values start at
/// 0.9 and fall by 0.1 per tag, to a floor of 0.1. The first tag has no q
/// value, so it is q=1.0 by default. Empty tags are skipped and whitespace is
/// not trimmed.
pub fn generate_accept_language_header(raw_language_list: &[u8]) -> Vec<u8> {
    let mut qvalue10: u32 = 10;
    let mut out: Vec<u8> = Vec::new();
    let mut tokens = Tokenizer::new(raw_language_list, b",", None, false);
    while let Some((b, e)) = tokens.next_token() {
        let language = &raw_language_list[b..e];
        if qvalue10 == 10 {
            // q=1.0 is implicit.
            out = language.to_vec();
        } else {
            out.push(b',');
            out.extend_from_slice(language);
            out.extend_from_slice(format!(";q=0.{qvalue10}").as_bytes());
        }
        // q=0 would be meaningless, so the value stops at 0.1.
        if qvalue10 > 1 {
            qvalue10 -= 1;
        }
    }
    out
}

/// Parses an `Accept-Encoding` value. Returns the lowercased set of accepted
/// encodings, or `None` if the value is malformed. Encodings with q=0 are
/// excluded. An empty value yields {"*"}. Otherwise "identity" is always
/// added, and gzip and x-gzip (and compress and x-compress) are added as a
/// pair.
pub fn parse_accept_encoding(accept_encoding: &[u8]) -> Option<BTreeSet<Vec<u8>>> {
    if accept_encoding.contains(&b'"') {
        return None;
    }
    let mut allowed: BTreeSet<Vec<u8>> = BTreeSet::new();

    let mut tokens = Tokenizer::new(accept_encoding, b",", None, false);
    while let Some((b, e)) = tokens.next_token() {
        let entry = trim_lws(&accept_encoding[b..e]);
        let Some(semicolon_pos) = entry.iter().position(|&c| c == b';') else {
            if entry.iter().any(|&c| is_lws(c)) {
                return None;
            }
            allowed.insert(entry.to_ascii_lowercase());
            continue;
        };

        let encoding = trim_lws(&entry[..semicolon_pos]);
        if encoding.iter().any(|&c| is_lws(c)) {
            return None;
        }
        let params = trim_lws(&entry[semicolon_pos + 1..]);
        let equals_pos = params.iter().position(|&c| c == b'=')?;
        let param_name = trim_lws(&params[..equals_pos]);
        if !param_name.eq_ignore_ascii_case(b"q") {
            return None;
        }
        let qvalue = trim_lws(&params[equals_pos + 1..]);
        if qvalue.is_empty() {
            return None;
        }
        if qvalue[0] == b'1' {
            if b"1.000".starts_with(qvalue) {
                allowed.insert(encoding.to_ascii_lowercase());
                continue;
            }
            return None;
        }
        if qvalue[0] != b'0' {
            return None;
        }
        if qvalue.len() == 1 {
            continue;
        }
        if qvalue.len() <= 2 || qvalue.len() > 5 || qvalue[1] != b'.' {
            return None;
        }
        let mut nonzero_number = false;
        for &c in &qvalue[2..] {
            if !c.is_ascii_digit() {
                return None;
            }
            if c != b'0' {
                nonzero_number = true;
            }
        }
        if nonzero_number {
            allowed.insert(encoding.to_ascii_lowercase());
        }
    }

    // RFC 7231 5.3.4: no Accept-Encoding means no preference.
    if allowed.is_empty() {
        allowed.insert(b"*".to_vec());
        return Some(allowed);
    }

    // Every browser must support "identity".
    allowed.insert(b"identity".to_vec());
    mirror_pair(&mut allowed, b"gzip", b"x-gzip");
    mirror_pair(&mut allowed, b"compress", b"x-compress");
    Some(allowed)
}

/// If either `a` or `b` is in `set`, adds both.
fn mirror_pair(set: &mut BTreeSet<Vec<u8>>, a: &[u8], b: &[u8]) {
    if set.contains(a) {
        set.insert(b.to_vec());
    }
    if set.contains(b) {
        set.insert(a.to_vec());
    }
}

/// Parses a `Content-Encoding` value into a lowercased set. Returns `None` if
/// the value has quotes, '=', ';' or '*', or if an item contains LWS.
pub fn parse_content_encoding(content_encoding: &[u8]) -> Option<BTreeSet<Vec<u8>>> {
    if content_encoding
        .iter()
        .any(|&c| matches!(c, b'"' | b'=' | b';' | b'*'))
    {
        return None;
    }
    let mut used: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut tokens = Tokenizer::new(content_encoding, b",", None, false);
    while let Some((b, e)) = tokens.next_token() {
        let encoding = trim_lws(&content_encoding[b..e]);
        if encoding.iter().any(|&c| is_lws(c)) {
            return None;
        }
        used.insert(encoding.to_ascii_lowercase());
    }
    Some(used)
}

// ---------------------------------------------------------------------------
// Dates and validators.
// ---------------------------------------------------------------------------

/// Parses a `Retry-After` value. It is either a non-negative number of seconds
/// or an HTTP-date. A date is measured from `now`. Returns the delay, or `None`
/// if the value is invalid or the delay is negative.
pub fn parse_retry_after_header(retry_after: &[u8], now: Time) -> Option<TimeDelta> {
    let interval = match parse_uint32_non_negative(retry_after) {
        Some(seconds) => TimeDelta::seconds(seconds as i64),
        None => Time::from_utc_string(truncate_at_nul(retry_after))? - now,
    };
    if interval < TimeDelta::ZERO {
        return None;
    }
    Some(interval)
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Formats `time` as an IMF-fixdate (RFC 7231 section 7.1.1.1), such as
/// "Sat, 30 Apr 2011 22:42:07 GMT". Same output as the C++ version, which
/// lives in base/i18n.
pub fn time_format_http(time: Time) -> String {
    let e = time.to_utc_exploded();
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        WEEKDAY_NAMES[e.day_of_week as usize],
        e.day_of_month,
        MONTH_NAMES[(e.month - 1) as usize],
        e.year,
        e.hour,
        e.minute,
        e.second
    )
}

fn header_time(value: &[u8]) -> Option<Time> {
    Time::from_string(truncate_at_nul(value))
}

/// Returns true if the response has a validator (an ETag or Last-Modified). A
/// weak validator counts too. See RFC 2616 section 13.3.3.
pub fn has_validators(
    version: HttpVersion,
    etag_header: Option<&[u8]>,
    last_modified_header: Option<&[u8]>,
) -> bool {
    if version < HttpVersion::new(1, 0) {
        return false;
    }
    if last_modified_header.and_then(header_time).is_some() {
        return true;
    }
    // An empty ETag is treated as missing. A valid ETag is always a quoted
    // string, so `""` is not empty.
    version >= HttpVersion::new(1, 1) && etag_header.is_some_and(|etag| !etag.is_empty())
}

/// Returns true if the response has a strong validator. A strong ETag counts.
/// Otherwise, a Last-Modified counts if it is at least 60 seconds before
/// Date. See RFC 2616 section 13.3.3.
pub fn has_strong_validators(
    version: HttpVersion,
    etag_header: Option<&[u8]>,
    last_modified_header: Option<&[u8]>,
    date_header: Option<&[u8]>,
) -> bool {
    if version < HttpVersion::new(1, 1) {
        return false;
    }
    if let Some(etag) = etag_header.filter(|etag| !etag.is_empty()) {
        match etag.iter().position(|&c| c == b'/') {
            None | Some(0) => return true,
            Some(slash) => {
                if !trim_lws(&etag[..slash]).eq_ignore_ascii_case(b"w") {
                    return true;
                }
            }
        }
    }

    let Some(last_modified) = last_modified_header.and_then(header_time) else {
        return false;
    };
    let Some(date) = date_header.and_then(header_time) else {
        return false;
    };
    // Last-Modified is weak unless it is at least 60 seconds before Date.
    (date - last_modified).in_seconds() >= 60
}

// ---------------------------------------------------------------------------
// Status codes for histograms.
// ---------------------------------------------------------------------------

const HISTOGRAM_MIN_HTTP_STATUS_CODE: i32 = 100;
const HISTOGRAM_MAX_HTTP_STATUS_CODE: i32 = 599;

const STATUS_CODES_FOR_HISTOGRAM: [i32; 501] = build_status_codes_for_histogram();

const fn build_status_codes_for_histogram() -> [i32; 501] {
    let mut codes = [0i32; 501];
    let mut i = 0;
    // codes[0] is 0, for invalid status codes.
    while i < 500 {
        codes[i + 1] = HISTOGRAM_MIN_HTTP_STATUS_CODE + i as i32;
        i += 1;
    }
    codes
}

/// Returns the status codes that get their own histogram bucket: 0 for
/// invalid codes, then 100 through 599.
pub fn status_codes_for_histogram() -> &'static [i32] {
    &STATUS_CODES_FOR_HISTOGRAM
}

/// Maps a status code to its histogram bucket. Codes outside 100..=599 map
/// to 0.
pub fn map_status_code_for_histogram(code: i32) -> i32 {
    if (HISTOGRAM_MIN_HTTP_STATUS_CODE..=HISTOGRAM_MAX_HTTP_STATUS_CODE).contains(&code) {
        code
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Range headers.
// ---------------------------------------------------------------------------

/// The `Content-Range` value of a 206 response (RFC 2616 section 14.16):
/// `bytes first-last/length`. Returns `None` unless the unit is "bytes" and
/// `0 <= first <= last < length`, with all numbers parsing as int64.
pub fn parse_content_range_header_for_206(content_range_spec: &[u8]) -> Option<ContentRange> {
    let spec = trim_lws(content_range_spec);

    let space_position = spec.iter().position(|&c| c == b' ')?;
    if !trim_lws(&spec[..space_position]).eq_ignore_ascii_case(b"bytes") {
        return None;
    }

    let minus_position = space_position + 1 + spec[space_position + 1..].iter().position(|&c| c == b'-')?;
    let slash_position = minus_position + 1 + spec[minus_position + 1..].iter().position(|&c| c == b'/')?;

    let first_byte_position = string_to_int64(trim_lws(&spec[space_position + 1..minus_position]))?;
    if first_byte_position < 0 {
        return None;
    }
    let last_byte_position = string_to_int64(trim_lws(&spec[minus_position + 1..slash_position]))?;
    if last_byte_position < first_byte_position {
        return None;
    }
    let instance_length = string_to_int64(trim_lws(&spec[slash_position + 1..]))?;
    if instance_length <= last_byte_position {
        return None;
    }
    Some(ContentRange {
        first_byte_position,
        last_byte_position,
        instance_length,
    })
}

/// Parses a `Range` header value of the form `bytes=spec,spec,...`. Returns
/// `None` if any part is invalid or there are no ranges. Each part is an
/// `int64` pair as in `first-last`, `first-` or `-suffix`, with LWS allowed
/// around the parts. Each range must be valid by `HttpByteRange::is_valid`.
pub fn parse_range_header(ranges_specifier: &[u8]) -> Option<Vec<HttpByteRange>> {
    let equal_char_offset = ranges_specifier.iter().position(|&c| c == b'=')?;

    let bytes_unit = trim_lws(&ranges_specifier[..equal_char_offset]);
    if !bytes_unit.eq_ignore_ascii_case(b"bytes") {
        return None;
    }

    let mut ranges = Vec::new();
    let mut byte_range_set = ValuesIterator::new(&ranges_specifier[equal_char_offset + 1..], b',', true);
    while byte_range_set.get_next() {
        let value = byte_range_set.value();
        let minus_char_offset = value.iter().position(|&c| c == b'-')?;

        let first_byte_pos = trim_lws(&value[..minus_char_offset]);
        let last_byte_pos = trim_lws(&value[minus_char_offset + 1..]);

        let first = if first_byte_pos.is_empty() {
            None
        } else {
            Some(string_to_int64(first_byte_pos)?)
        };

        let range = if !last_byte_pos.is_empty() {
            let last = string_to_int64(last_byte_pos)?;
            match first {
                Some(first) => HttpByteRange::bounded(first, last),
                None => HttpByteRange::suffix(last),
            }
        } else {
            HttpByteRange::right_unbounded(first?)
        };

        if !range.is_valid() {
            return None;
        }
        ranges.push(range);
    }
    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}

/// Parses one range from a Fetch "simple range header value" for the "bytes"
/// unit (https://fetch.spec.whatwg.org/#simple-range-header-value). Returns the
/// range without checking `is_valid()`. Values that overflow int64 saturate
/// to `i64::MAX`. Spaces and tabs around '=' and '-' are accepted only when
/// `allow_whitespace` is set. At least one side of the range needs digits.
pub fn parse_fetch_single_range(range_header_value: &[u8], allow_whitespace: bool) -> Option<HttpByteRange> {
    const UNIT: &[u8] = b"bytes";
    if !range_header_value.starts_with(UNIT) {
        return None;
    }
    let mut parser = FetchRangeParser {
        input: range_header_value,
        pos: UNIT.len(),
        allow_whitespace,
    };

    if !parser.consume_delimiter(b'=') {
        return None;
    }
    let range_start = parser.parse_decimal_number();
    if !parser.consume_delimiter(b'-') {
        return None;
    }
    let range_end = parser.parse_decimal_number();
    // Trailing input, and ranges with no digits at all, are rejected.
    if parser.in_bounds() || (range_start.is_none() && range_end.is_none()) {
        return None;
    }

    match (range_start, range_end) {
        (None, Some(end)) => Some(HttpByteRange::suffix(end)),
        (Some(start), None) => Some(HttpByteRange::right_unbounded(start)),
        (Some(start), Some(end)) => Some(HttpByteRange::bounded(start, end)),
        (None, None) => None,
    }
}

struct FetchRangeParser<'a> {
    input: &'a [u8],
    pos: usize,
    allow_whitespace: bool,
}

impl FetchRangeParser<'_> {
    fn in_bounds(&self) -> bool {
        self.pos < self.input.len()
    }

    fn skip_optional_whitespace(&mut self) {
        if !self.allow_whitespace {
            return;
        }
        while self.in_bounds() && (self.input[self.pos] == b' ' || self.input[self.pos] == b'\t') {
            self.pos += 1;
        }
    }

    /// Skips optional whitespace, consumes `delimiter`, then skips optional
    /// whitespace again.
    fn consume_delimiter(&mut self, delimiter: u8) -> bool {
        self.skip_optional_whitespace();
        if !self.in_bounds() || self.input[self.pos] != delimiter {
            return false;
        }
        self.pos += 1;
        self.skip_optional_whitespace();
        true
    }

    /// Reads a run of ASCII digits. Values that do not fit in int64 saturate
    /// to `i64::MAX`.
    fn parse_decimal_number(&mut self) -> Option<i64> {
        let start = self.pos;
        while self.in_bounds() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start {
            return None;
        }
        let mut value: i64 = 0;
        for &c in &self.input[start..self.pos] {
            match value
                .checked_mul(10)
                .and_then(|v| v.checked_add((c - b'0') as i64))
            {
                Some(v) => value = v,
                None => return Some(i64::MAX),
            }
        }
        Some(value)
    }
}

// ---------------------------------------------------------------------------
// Parsing Content-Type.
// ---------------------------------------------------------------------------

/// Parses one `Content-Type` header into `state`. Call it once per header to
/// handle several headers. The type and charset are lowercased. A type change
/// clears the charset, unless the new value also sets one. `*/*` is ignored
/// unless it has parameters. Reject values with no '/' before the first ';'.
///
/// `state.boundary` is set to the unquoted `boundary` parameter when one is
/// present. Parameter names must match exactly, so `charset =` does not count.
pub fn parse_content_type(content_type: &[u8], state: &mut ContentType) {
    let Some((mime_type_value, params)) = mime::parse_mime_type(content_type) else {
        return;
    };
    // A bare "*/*" is meaningless, so don't store it.
    if content_type == b"*/*" {
        return;
    }

    let mut charset_value: Vec<u8> = Vec::new();
    let mut type_has_charset = false;
    let mut type_has_boundary = false;
    for (name, value) in &params {
        if !type_has_charset && name.eq_ignore_ascii_case(b"charset") {
            type_has_charset = true;
            charset_value = trim_lws(value).to_vec();
            continue;
        }
        if !type_has_boundary && name.eq_ignore_ascii_case(b"boundary") {
            type_has_boundary = true;
            state.boundary = trim_lws(value).to_vec();
            continue;
        }
    }

    // If the type is unchanged, keep the existing charset unless this header
    // gives one.
    let same_type = mime_type_value.eq_ignore_ascii_case(&state.mime_type);
    if !same_type {
        state.mime_type = mime_type_value.to_ascii_lowercase();
    }
    if (!same_type && state.had_charset) || type_has_charset {
        state.had_charset = true;
        state.charset = charset_value.to_ascii_lowercase();
    }
}

// ---------------------------------------------------------------------------
// Iterators.
// ---------------------------------------------------------------------------

/// Iterates over the name/value pairs of a raw header block. Each line is one
/// header, split on any byte in `line_delimiter`. Lines without a valid
/// header name are skipped. Lines are not joined for continuations, so run
/// `assemble_raw_headers` first if continuations may be present.
#[derive(Debug, Clone)]
pub struct HeadersIterator<'a> {
    headers: &'a [u8],
    initial_lines: Tokenizer<'a>,
    lines: Tokenizer<'a>,
    name_begin: usize,
    name_end: usize,
    values_begin: usize,
    values_end: usize,
}

impl<'a> HeadersIterator<'a> {
    /// Creates an iterator. `line_delimiter` is a set of bytes that end lines,
    /// typically `b"\r\n"`.
    pub fn new(headers: &'a [u8], line_delimiter: &[u8]) -> Self {
        let lines = Tokenizer::new(headers, line_delimiter, None, false);
        HeadersIterator {
            headers,
            initial_lines: lines.clone(),
            lines,
            name_begin: 0,
            name_end: 0,
            values_begin: 0,
            values_end: 0,
        }
    }

    /// Advances to the next valid header. Returns false at the end.
    pub fn get_next(&mut self) -> bool {
        while let Some((line_begin, line_end)) = self.lines.next_token() {
            self.name_begin = line_begin;
            self.values_end = line_end;

            let line = &self.headers[line_begin..line_end];
            let Some(colon_in_line) = line.iter().position(|&c| c == b':') else {
                continue; // Malformed header.
            };
            let colon = line_begin + colon_in_line;
            self.name_end = colon;

            // A name that is empty or starts with LWS is malformed. Leading
            // LWS would mean a continuation, and those were joined already.
            if self.name_begin == self.name_end || is_lws(self.headers[self.name_begin]) {
                continue;
            }

            let (name_begin, name_end) = trim_lws_range(self.headers, self.name_begin, self.name_end);
            self.name_begin = name_begin;
            self.name_end = name_end;
            if !is_token(&self.headers[name_begin..name_end]) {
                continue; // Malformed header.
            }

            self.values_begin = colon + 1;
            let (values_begin, values_end) =
                trim_lws_range(self.headers, self.values_begin, self.values_end);
            self.values_begin = values_begin;
            self.values_end = values_end;
            return true;
        }
        false
    }

    /// Restarts iteration from the first line.
    pub fn reset(&mut self) {
        self.lines = self.initial_lines.clone();
    }

    /// Start offset of the current name in the input.
    pub fn name_begin(&self) -> usize {
        self.name_begin
    }

    /// End offset of the current name in the input.
    pub fn name_end(&self) -> usize {
        self.name_end
    }

    /// The current header name.
    pub fn name(&self) -> &'a [u8] {
        &self.headers[self.name_begin..self.name_end]
    }

    /// Start offset of the current value in the input.
    pub fn values_begin(&self) -> usize {
        self.values_begin
    }

    /// End offset of the current value in the input.
    pub fn values_end(&self) -> usize {
        self.values_end
    }

    /// The current header value, with LWS trimmed.
    pub fn values(&self) -> &'a [u8] {
        &self.headers[self.values_begin..self.values_end]
    }
}

/// Iterates over the delimited values of a header, such as the items of
/// `Cache-Control`. LWS is trimmed from each value. Delimiters inside a
/// quoted-string do not split values. Do not use this for non-coalescing
/// headers (see `is_non_coalescing_header`).
#[derive(Debug, Clone)]
pub struct ValuesIterator<'a> {
    values: &'a [u8],
    ignore_empty_values: bool,
    tokenizer: Tokenizer<'a>,
    value_begin: usize,
    value_end: usize,
}

impl<'a> ValuesIterator<'a> {
    /// Creates an iterator. With `ignore_empty_values`, values that are empty
    /// after trimming are skipped.
    pub fn new(values: &'a [u8], delimiter: u8, ignore_empty_values: bool) -> Self {
        ValuesIterator {
            values,
            ignore_empty_values,
            tokenizer: Tokenizer::new(values, &[delimiter], Some(b'"'), !ignore_empty_values),
            value_begin: 0,
            value_end: 0,
        }
    }

    /// Advances to the next value. Returns false at the end.
    pub fn get_next(&mut self) -> bool {
        while let Some((begin, end)) = self.tokenizer.next_token() {
            let (begin, end) = trim_lws_range(self.values, begin, end);
            self.value_begin = begin;
            self.value_end = end;
            if !self.ignore_empty_values || begin != end {
                return true;
            }
        }
        false
    }

    /// The current value, with LWS trimmed.
    pub fn value(&self) -> &'a [u8] {
        &self.values[self.value_begin..self.value_end]
    }

    /// Start offset of the current value in the input.
    pub fn value_begin(&self) -> usize {
        self.value_begin
    }

    /// End offset of the current value in the input.
    pub fn value_end(&self) -> usize {
        self.value_end
    }
}

/// Whether values in a name/value pair are required. `NotRequired` allows
/// `name1=value1;name2;name3=value3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Values {
    NotRequired,
    Required,
}

/// Whether unmatched quotes are an error. By default, the iterator recovers
/// from them. With `Strict`, a value with mismatched or invalid quotes is a
/// parse error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quotes {
    Strict,
    NotStrict,
}

/// Iterates over `name=value` pairs separated by a delimiter, such as the
/// parameters of `Content-Disposition`. LWS is allowed around names, values
/// and delimiters. A quoted value is unquoted, and `value()` returns the
/// unquoted text. `raw_value()` returns the text as it appeared.
#[derive(Debug, Clone)]
pub struct NameValuePairsIterator<'a> {
    props: ValuesIterator<'a>,
    valid: bool,
    name: &'a [u8],
    value: &'a [u8],
    unquoted_value: Vec<u8>,
    value_is_quoted: bool,
    values_optional: bool,
    strict_quotes: bool,
}

impl<'a> NameValuePairsIterator<'a> {
    /// Creates an iterator that requires values and tolerates bad quotes.
    pub fn new(value: &'a [u8], delimiter: u8) -> Self {
        Self::with_options(value, delimiter, Values::Required, Quotes::NotStrict)
    }

    /// Creates an iterator with the given options.
    pub fn with_options(value: &'a [u8], delimiter: u8, values: Values, quotes: Quotes) -> Self {
        NameValuePairsIterator {
            props: ValuesIterator::new(value, delimiter, true),
            valid: true,
            name: &[],
            value: &[],
            unquoted_value: Vec::new(),
            value_is_quoted: false,
            values_optional: values == Values::NotRequired,
            strict_quotes: quotes == Quotes::Strict,
        }
    }

    /// Advances to the next pair. Returns false at the end or after an error.
    /// Check `valid()` to tell which. The values from the previous call are
    /// cleared, whatever the result.
    pub fn get_next(&mut self) -> bool {
        // After an error the iterator stays failed.
        if !self.valid {
            return false;
        }
        if self.props.get_next() {
            let pair = self.props.value();
            self.valid = self.parse_name_value_pair(pair);
            if self.valid {
                return true;
            }
        }

        self.name = &[];
        self.value = &[];
        self.unquoted_value.clear();
        self.value_is_quoted = false;
        false
    }

    /// False if the last pair was malformed.
    pub fn valid(&self) -> bool {
        self.valid
    }

    /// The name of the current pair.
    pub fn name(&self) -> &'a [u8] {
        self.name
    }

    /// The value of the current pair, unquoted if it was quoted.
    pub fn value(&self) -> &[u8] {
        if self.value_is_quoted {
            &self.unquoted_value
        } else {
            self.value
        }
    }

    /// True if the value was a quoted-string.
    pub fn value_is_quoted(&self) -> bool {
        self.value_is_quoted
    }

    /// The value as it appeared, before unquoting.
    pub fn raw_value(&self) -> &'a [u8] {
        self.value
    }

    fn parse_name_value_pair(&mut self, name_value_pair: &'a [u8]) -> bool {
        let equals = name_value_pair.iter().position(|&c| c == b'=');
        if equals == Some(0) {
            return false; // No name.
        }
        let has_value = equals.is_some();
        if !has_value && !self.values_optional {
            return false; // Values are required.
        }

        let name_end = equals.unwrap_or(name_value_pair.len());
        self.name = trim_lws(&name_value_pair[..name_end]);
        self.value = &[];
        self.value_is_quoted = false;
        self.unquoted_value.clear();

        if let Some(equals) = equals {
            // A quote before the '=' is an error.
            if self.name.iter().any(|&c| is_quote(c)) {
                return false;
            }
            self.value = trim_lws(&name_value_pair[equals + 1..]);
            if self.value.is_empty() {
                return false; // Empty value.
            }
        }

        if has_value && is_quote(self.value[0]) {
            self.value_is_quoted = true;

            if self.strict_quotes {
                return match strict_unquote(self.value) {
                    Some(unquoted) => {
                        self.unquoted_value = unquoted;
                        true
                    }
                    None => false,
                };
            }

            let value = self.value;
            if value[0] != value[value.len() - 1] || value.len() == 1 {
                // Mismatched quotes: drop the leading quote and keep the rest.
                // Quoted-pairs are not unescaped in this case.
                self.value_is_quoted = false;
                self.value = &value[1..];
            } else {
                self.unquoted_value = unquote(value);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
