// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `net::HttpRequestHeaders`
//! (net/http/http_request_headers.{h,cc}).
//!
//! Holds request headers as an ordered list of key/value pairs. Lookups are
//! linear and compare names ASCII case-insensitively. Header names and values
//! are bytes, as in C++, so non-UTF-8 values such as obs-text are kept.
//!
//! Differences from the C++ version:
//! - Inputs are `impl AsRef<[u8]>`, so `&str`, `String`, and `&[u8]` all work.
//!   Stored keys and values are `Vec<u8>`.
//! - `ToString` is `to_bytes()`, which returns `Vec<u8>`. A `String` would not
//!   hold non-UTF-8 header values.
//! - `GetHeader` is `get_header()` returning `Option<Vec<u8>>`, and
//!   `GetHeaderView` is `get_header_view()` returning `Option<&[u8]>`.
//! - Where C++ `CHECK`s that a name or value is valid, `set_header` and
//!   `set_header_if_missing` panic with the same condition. The
//!   `..._without_check_for_testing` variant skips the check, as in C++.
//! - `AddHeaderFromString` and `AddHeadersFromString` return `Result`. The C++
//!   logs at `DFATAL`, which aborts in DCHECK builds and only logs in release.
//!   Here the malformed line is skipped in every build. `add_headers_from_string`
//!   skips bad lines and keeps going, as C++ does, and returns the first error.
//! - A CRLF inside a single header line is a C++ DCHECK. Here it always makes
//!   the line invalid, because CR and LF are not valid header value bytes.
//! - `SourceStreamTypeSet` is replaced by `Option<&[SourceStreamType]>`, with
//!   `None` meaning "all stream types" (C++ `std::nullopt`). `SourceStreamType`
//!   is a local enum with only the four encodings this code queries. The C++
//!   `url` argument becomes the caller-computed `can_use_advanced_encodings`
//!   bool, which is `url.SchemeIsCryptographic() || IsLocalhost(url)`.
//! - Not ported: `NetLogParams`, which needs `base::DictValue` and NetLog
//!   capture modes, and `Swap`, which is `std::mem::swap` in Rust.
//! - The `Iterator` class is `HttpRequestHeadersIterator`, which has the same
//!   `get_next` / `name` / `value` protocol and also implements
//!   `std::iter::Iterator`.
//! - Header name and value validation uses `http_util`, which matches the C++
//!   `HttpUtil::IsValidHeaderName` and `IsValidHeaderValue`.

use std::fmt;

use http_util::{is_valid_header_name, is_valid_header_value, trim_lws};

/// One request header. Key case is kept from the first insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderKeyValuePair {
    /// The header name, as first set.
    pub key: Vec<u8>,
    /// The header value.
    pub value: Vec<u8>,
}

/// Stream types that can be advertised in `Accept-Encoding`. Mirrors the
/// subset of `net::SourceStreamType` that `SetAcceptEncodingIfMissing` uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStreamType {
    Gzip,
    Deflate,
    Brotli,
    Zstd,
}

/// Why a header line could not be parsed by `add_header_from_string`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderLineError {
    /// The line has no `:`.
    MissingColon,
    /// The line starts with `:`, so the name is empty.
    MissingKey,
    /// The name is not a valid HTTP token.
    InvalidKey,
    /// The trimmed value contains CR, LF, or NUL.
    InvalidValue,
}

impl fmt::Display for HeaderLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingColon => "header line is missing a colon",
            Self::MissingKey => "header line is missing a name",
            Self::InvalidKey => "header name is invalid",
            Self::InvalidValue => "header value is invalid",
        })
    }
}

impl std::error::Error for HeaderLineError {}

/// An ordered set of HTTP request headers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpRequestHeaders {
    headers: Vec<HeaderKeyValuePair>,
}

/// Iterates over headers in insertion order. Mirrors C++
/// `HttpRequestHeaders::Iterator`: call `get_next`, then read `name` and
/// `value`. Use `name` and `value` only after `get_next` returned true.
#[derive(Debug, Clone)]
pub struct HttpRequestHeadersIterator<'a> {
    rest: std::slice::Iter<'a, HeaderKeyValuePair>,
    current: Option<&'a HeaderKeyValuePair>,
}

impl<'a> HttpRequestHeadersIterator<'a> {
    /// Advances to the next header. Returns true if there is one.
    pub fn get_next(&mut self) -> bool {
        self.current = self.rest.next();
        self.current.is_some()
    }

    /// The name of the current header, or `None` before the first
    /// successful `get_next` or after the end.
    pub fn name(&self) -> Option<&'a [u8]> {
        self.current.map(|h| h.key.as_slice())
    }

    /// The value of the current header, or `None` before the first
    /// successful `get_next` or after the end.
    pub fn value(&self) -> Option<&'a [u8]> {
        self.current.map(|h| h.value.as_slice())
    }
}

impl<'a> Iterator for HttpRequestHeadersIterator<'a> {
    type Item = &'a HeaderKeyValuePair;

    fn next(&mut self) -> Option<Self::Item> {
        self.rest.next()
    }
}

impl HttpRequestHeaders {
    // Request methods.
    pub const CONNECT_METHOD: &'static str = "CONNECT";
    pub const DELETE_METHOD: &'static str = "DELETE";
    pub const GET_METHOD: &'static str = "GET";
    pub const HEAD_METHOD: &'static str = "HEAD";
    pub const OPTIONS_METHOD: &'static str = "OPTIONS";
    pub const PATCH_METHOD: &'static str = "PATCH";
    pub const POST_METHOD: &'static str = "POST";
    pub const PUT_METHOD: &'static str = "PUT";
    pub const TRACE_METHOD: &'static str = "TRACE";
    pub const TRACK_METHOD: &'static str = "TRACK";

    // Header names.
    pub const ACCEPT: &'static str = "Accept";
    pub const ACCEPT_CHARSET: &'static str = "Accept-Charset";
    pub const ACCEPT_ENCODING: &'static str = "Accept-Encoding";
    pub const ACCEPT_LANGUAGE: &'static str = "Accept-Language";
    pub const AUTHORIZATION: &'static str = "Authorization";
    pub const CACHE_CONTROL: &'static str = "Cache-Control";
    pub const CONNECTION: &'static str = "Connection";
    pub const CONTENT_LENGTH: &'static str = "Content-Length";
    pub const CONTENT_TYPE: &'static str = "Content-Type";
    pub const COOKIE: &'static str = "Cookie";
    pub const DNT: &'static str = "DNT";
    pub const HOST: &'static str = "Host";
    pub const IF_MATCH: &'static str = "If-Match";
    pub const IF_MODIFIED_SINCE: &'static str = "If-Modified-Since";
    pub const IF_NONE_MATCH: &'static str = "If-None-Match";
    pub const IF_RANGE: &'static str = "If-Range";
    pub const IF_UNMODIFIED_SINCE: &'static str = "If-Unmodified-Since";
    pub const ORIGIN: &'static str = "Origin";
    pub const PRAGMA: &'static str = "Pragma";
    pub const PRIORITY: &'static str = "Priority";
    pub const PROXY_AUTHORIZATION: &'static str = "Proxy-Authorization";
    pub const PROXY_CONNECTION: &'static str = "Proxy-Connection";
    pub const RANGE: &'static str = "Range";
    pub const REFERER: &'static str = "Referer";
    pub const SEC_GPC: &'static str = "Sec-GPC";
    pub const SEC_PRIVATE_VERIFICATION_TOKEN: &'static str = "Sec-Private-Verification-Token";
    pub const SEC_PURPOSE: &'static str = "Sec-Purpose";
    pub const TRANSFER_ENCODING: &'static str = "Transfer-Encoding";
    pub const UPGRADE_INSECURE_REQUESTS: &'static str = "Upgrade-Insecure-Requests";
    pub const USER_AGENT: &'static str = "User-Agent";

    /// Creates an empty header set.
    pub fn new() -> Self {
        Self::default()
    }

    /// True if there are no headers.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// True if a header named `key` exists, ignoring ASCII case.
    pub fn has_header(&self, key: impl AsRef<[u8]>) -> bool {
        self.find_header(key.as_ref()).is_some()
    }

    /// The value of the first header named `key`, borrowed.
    pub fn get_header_view(&self, key: impl AsRef<[u8]>) -> Option<&[u8]> {
        self.find_header(key.as_ref())
            .map(|i| self.headers[i].value.as_slice())
    }

    /// A copy of the value of the first header named `key`.
    pub fn get_header(&self, key: impl AsRef<[u8]>) -> Option<Vec<u8>> {
        self.get_header_view(key).map(<[u8]>::to_vec)
    }

    /// Removes all headers.
    pub fn clear(&mut self) {
        self.headers.clear();
    }

    /// Sets the value for `key`. If the header exists, only its value changes,
    /// and its position and original key spelling stay the same. Otherwise the
    /// header is appended.
    ///
    /// # Panics
    ///
    /// Panics if `key` is not a valid header name or `value` is not a valid
    /// header value. The C++ code `CHECK`s the same conditions, because an
    /// invalid header could carry browser-internal data.
    pub fn set_header(&mut self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) {
        let (key, value) = (key.as_ref(), value.as_ref());
        assert!(
            is_valid_header_name(key),
            "invalid header name: {key:?}"
        );
        assert!(
            is_valid_header_value(value),
            "invalid header value for {key:?}"
        );
        self.set_header_internal(key, value);
    }

    /// Like [`set_header`](Self::set_header), but skips the validity checks.
    /// Only for tests, as in C++.
    pub fn set_header_without_check_for_testing(
        &mut self,
        key: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) {
        self.set_header_internal(key.as_ref(), value.as_ref());
    }

    /// Sets the header only if no header named `key` exists. Otherwise does
    /// nothing.
    ///
    /// # Panics
    ///
    /// Panics on an invalid name or value, as [`set_header`](Self::set_header)
    /// does, even if the header already exists.
    pub fn set_header_if_missing(&mut self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) {
        let (key, value) = (key.as_ref(), value.as_ref());
        assert!(is_valid_header_name(key), "invalid header name: {key:?}");
        assert!(is_valid_header_value(value), "invalid header value for {key:?}");
        if self.find_header(key).is_none() {
            self.headers.push(HeaderKeyValuePair {
                key: key.to_vec(),
                value: value.to_vec(),
            });
        }
    }

    /// Removes the first header named `key`, ignoring ASCII case.
    pub fn remove_header(&mut self, key: impl AsRef<[u8]>) {
        if let Some(i) = self.find_header(key.as_ref()) {
            self.headers.remove(i);
        }
    }

    /// Parses one header line of the form `name: value` and sets it, with
    /// `SetHeader` semantics. The line must not contain CRLF.
    ///
    /// LWS around the value is removed. A missing or empty value sets the
    /// header to the empty string. On error the headers are not changed.
    pub fn add_header_from_string(
        &mut self,
        header_line: impl AsRef<[u8]>,
    ) -> Result<(), HeaderLineError> {
        let line = header_line.as_ref();
        let colon = line
            .iter()
            .position(|&c| c == b':')
            .ok_or(HeaderLineError::MissingColon)?;
        if colon == 0 {
            return Err(HeaderLineError::MissingKey);
        }
        let key = &line[..colon];
        if !is_valid_header_name(key) {
            return Err(HeaderLineError::InvalidKey);
        }
        let value = trim_lws(&line[colon + 1..]);
        if !is_valid_header_value(value) {
            return Err(HeaderLineError::InvalidValue);
        }
        self.set_header_internal(key, value);
        Ok(())
    }

    /// Parses a CRLF-separated list of header lines. Each line is trimmed of
    /// ASCII whitespace, empty lines are skipped, and each remaining line goes
    /// to [`add_header_from_string`](Self::add_header_from_string).
    ///
    /// A bad line is skipped and the rest are still added. The first error is
    /// returned after all lines are processed, which matches the C++ behavior.
    pub fn add_headers_from_string(
        &mut self,
        headers: impl AsRef<[u8]>,
    ) -> Result<(), HeaderLineError> {
        let mut first_error = None;
        for piece in split_on_crlf(headers.as_ref()) {
            let trimmed = trim_ascii_whitespace(piece);
            if trimmed.is_empty() {
                continue;
            }
            if let Err(e) = self.add_header_from_string(trimmed) {
                first_error.get_or_insert(e);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Calls `set_header` for each header of `other`, in order.
    pub fn merge_from(&mut self, other: &HttpRequestHeaders) {
        for header in &other.headers {
            self.set_header(&header.key, &header.value);
        }
    }

    /// Serializes the headers as `Name: value\r\n` lines followed by a final
    /// `\r\n`. This is `ToString()` in C++.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for header in &self.headers {
            out.extend_from_slice(&header.key);
            out.extend_from_slice(b": ");
            out.extend_from_slice(&header.value);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out
    }

    /// The headers in insertion order.
    pub fn header_vector(&self) -> &[HeaderKeyValuePair] {
        &self.headers
    }

    /// An iterator over the headers in insertion order.
    pub fn iter(&self) -> HttpRequestHeadersIterator<'_> {
        HttpRequestHeadersIterator {
            rest: self.headers.iter(),
            current: None,
        }
    }

    /// Sets `Accept-Encoding` if it is not already set and there is no
    /// `Range` header. The C++ method takes a `GURL`; the URL check is passed
    /// in as a bool, as described in the crate docs.
    ///
    /// - If `Range` is set, `Accept-Encoding` becomes `identity`.
    /// - Otherwise `gzip` and `deflate` are advertised when accepted. `br` and
    ///   `zstd` are advertised only when enabled, accepted, and
    ///   `can_use_advanced_encodings` is true. That flag is the caller's
    ///   `url.SchemeIsCryptographic() || IsLocalhost(url)`.
    /// - `accepted_stream_types` of `None` accepts every type. `Some(&[])`
    ///   accepts none, which means no header is set.
    pub fn set_accept_encoding_if_missing(
        &mut self,
        can_use_advanced_encodings: bool,
        accepted_stream_types: Option<&[SourceStreamType]>,
        enable_brotli: bool,
        enable_zstd: bool,
    ) {
        if self.has_header(Self::ACCEPT_ENCODING) {
            return;
        }

        // A range request is served as-is, so compression is not wanted.
        if self.has_header(Self::RANGE) {
            self.set_header(Self::ACCEPT_ENCODING, "identity");
            return;
        }

        let supports = |t: SourceStreamType| match accepted_stream_types {
            None => true,
            Some(types) => types.contains(&t),
        };

        // gzip and deflate are always safe to advertise. br and zstd are
        // only advertised where the transfer is opaque to proxies.
        let mut names: Vec<&str> = Vec::with_capacity(4);
        if supports(SourceStreamType::Gzip) {
            names.push("gzip");
        }
        if supports(SourceStreamType::Deflate) {
            names.push("deflate");
        }
        if enable_brotli && supports(SourceStreamType::Brotli) && can_use_advanced_encodings {
            names.push("br");
        }
        if enable_zstd && supports(SourceStreamType::Zstd) && can_use_advanced_encodings {
            names.push("zstd");
        }
        if !names.is_empty() {
            self.set_header(Self::ACCEPT_ENCODING, names.join(", "));
        }
    }

    fn find_header(&self, key: &[u8]) -> Option<usize> {
        self.headers
            .iter()
            .position(|h| h.key.eq_ignore_ascii_case(key))
    }

    fn set_header_internal(&mut self, key: &[u8], value: &[u8]) {
        match self.find_header(key) {
            Some(i) => self.headers[i].value = value.to_vec(),
            None => self.headers.push(HeaderKeyValuePair {
                key: key.to_vec(),
                value: value.to_vec(),
            }),
        }
    }
}

impl<'a> IntoIterator for &'a HttpRequestHeaders {
    type Item = &'a HeaderKeyValuePair;
    type IntoIter = std::slice::Iter<'a, HeaderKeyValuePair>;

    fn into_iter(self) -> Self::IntoIter {
        self.headers.iter()
    }
}

/// Splits on every `\r\n`. Equivalent to `base::SplitStringPieceUsingSubstr`
/// with `"\r\n"`, before trimming.
fn split_on_crlf(s: &[u8]) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + 1 < s.len() {
        if s[i] == b'\r' && s[i + 1] == b'\n' {
            pieces.push(&s[start..i]);
            i += 2;
            start = i;
        } else {
            i += 1;
        }
    }
    pieces.push(&s[start..]);
    pieces
}

/// Trims the ASCII whitespace set used by `base::TrimWhitespace` for narrow
/// strings: space, tab, LF, VT, FF, and CR. Rust's `trim_ascii` leaves VT
/// out, so this is a separate function.
fn trim_ascii_whitespace(s: &[u8]) -> &[u8] {
    let is_ws = |c: &u8| matches!(*c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let start = s.iter().position(|c| !is_ws(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|c| !is_ws(c)).map_or(start, |p| p + 1);
    &s[start..end]
}

#[cfg(test)]
mod tests {
    use super::{HeaderLineError, HttpRequestHeaders, SourceStreamType};

    #[test]
    fn has_header() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        assert!(headers.has_header("foo"));
        assert!(headers.has_header("Foo"));
        assert!(!headers.has_header("Fo"));

        let headers_ref = &headers;
        assert!(headers_ref.has_header("foo"));
        assert!(headers_ref.has_header("Foo"));
        assert!(!headers_ref.has_header("Fo"));
    }

    #[test]
    fn set_header() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn set_multiple_headers() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Cookie-Monster", "Nom nom nom");
        headers.set_header("Domo-Kun", "Loves Chrome");
        assert_eq!(
            headers.to_bytes(),
            b"Cookie-Monster: Nom nom nom\r\nDomo-Kun: Loves Chrome\r\n\r\n"
        );
    }

    #[test]
    fn set_header_twice() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        headers.set_header("Foo", "bar");
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn set_header_twice_case_insensitive() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        headers.set_header("FoO", "Bar");
        assert_eq!(headers.to_bytes(), b"Foo: Bar\r\n\r\n");
    }

    #[test]
    fn set_header_twice_same_prefix() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("FooBar", "baz");
        headers.set_header("Foo", "qux");
        assert_eq!(headers.to_bytes(), b"FooBar: baz\r\nFoo: qux\r\n\r\n");
        let headers_ref = &headers;
        assert_eq!(headers_ref.to_bytes(), b"FooBar: baz\r\nFoo: qux\r\n\r\n");
    }

    #[test]
    fn set_empty_header() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "Bar");
        headers.set_header("Bar", "");
        assert_eq!(headers.to_bytes(), b"Foo: Bar\r\nBar: \r\n\r\n");
    }

    #[test]
    fn set_header_if_missing() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header_if_missing("Foo", "Bar");
        assert_eq!(headers.to_bytes(), b"Foo: Bar\r\n\r\n");
        headers.set_header_if_missing("Foo", "Baz");
        assert_eq!(headers.to_bytes(), b"Foo: Bar\r\n\r\n");
    }

    #[test]
    fn remove_header() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        headers.remove_header("Foo");
        assert_eq!(headers.to_bytes(), b"\r\n");
    }

    #[test]
    fn remove_header_missing_header() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        headers.remove_header("Bar");
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn remove_header_case_insensitive() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "bar");
        headers.set_header("All-Your-Base", "Belongs To Chrome");
        headers.remove_header("foo");
        assert_eq!(headers.to_bytes(), b"All-Your-Base: Belongs To Chrome\r\n\r\n");
    }

    #[test]
    fn add_header_from_string() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo: bar").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_no_leading_whitespace() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo:bar").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_more_leading_whitespace() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo: \t  \t  bar").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_trailing_whitespace() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo: bar  \t  \t   ").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_leading_trailing_whitespace() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo: \t    bar\t       ").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: bar\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_with_empty_value() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo:").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: \r\n\r\n");
    }

    #[test]
    fn add_header_from_string_with_whitespace_value() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Foo: ").unwrap();
        assert_eq!(headers.to_bytes(), b"Foo: \r\n\r\n");
    }

    #[test]
    fn merge_from() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("A", "A");
        headers.set_header("B", "B");

        let mut headers2 = HttpRequestHeaders::new();
        headers2.set_header("B", "b");
        headers2.set_header("C", "c");
        headers.merge_from(&headers2);
        assert_eq!(headers.to_bytes(), b"A: A\r\nB: b\r\nC: c\r\n\r\n");
    }

    #[test]
    fn assign() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("A", "A");
        headers.set_header("B", "B");

        let mut headers2 = HttpRequestHeaders::new();
        headers2.set_header("B", "b");
        headers2.set_header("C", "c");
        headers = headers2;
        assert_eq!(headers.to_bytes(), b"B: b\r\nC: c\r\n\r\n");
    }

    #[test]
    fn copy() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("A", "A");
        headers.set_header("B", "B");

        let headers2 = headers.clone();
        assert_eq!(headers.to_bytes(), headers2.to_bytes());
    }

    #[test]
    fn set_accept_encoding_if_missing() {
        // https: every encoding is available.
        {
            let mut headers = HttpRequestHeaders::new();
            headers.set_accept_encoding_if_missing(true, None, true, true);
            assert_eq!(
                headers.get_header(HttpRequestHeaders::ACCEPT_ENCODING),
                Some(b"gzip, deflate, br, zstd".to_vec())
            );
        }

        // http: br and zstd need a cryptographic scheme or localhost.
        {
            let mut headers = HttpRequestHeaders::new();
            headers.set_accept_encoding_if_missing(false, None, true, true);
            assert_eq!(
                headers.get_header(HttpRequestHeaders::ACCEPT_ENCODING),
                Some(b"gzip, deflate".to_vec())
            );
        }

        // Nothing accepted: no header.
        {
            let mut headers = HttpRequestHeaders::new();
            headers.set_accept_encoding_if_missing(true, Some(&[]), true, true);
            assert_eq!(headers.get_header(HttpRequestHeaders::ACCEPT_ENCODING), None);
        }

        // Only brotli and zstd accepted.
        {
            let mut headers = HttpRequestHeaders::new();
            headers.set_accept_encoding_if_missing(
                true,
                Some(&[SourceStreamType::Brotli, SourceStreamType::Zstd]),
                true,
                true,
            );
            assert_eq!(
                headers.get_header(HttpRequestHeaders::ACCEPT_ENCODING),
                Some(b"br, zstd".to_vec())
            );
        }
    }

    // ---- Tests beyond the C++ unit test. ----

    #[test]
    fn get_header_is_case_insensitive_and_returns_first_match_value() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Accept", "text/html");
        assert_eq!(headers.get_header("ACCEPT"), Some(b"text/html".to_vec()));
        assert_eq!(headers.get_header_view("accept"), Some(&b"text/html"[..]));
        assert_eq!(headers.get_header("Missing"), None);
        assert_eq!(headers.get_header_view("Missing"), None);
    }

    #[test]
    fn is_empty_and_clear() {
        let mut headers = HttpRequestHeaders::new();
        assert!(headers.is_empty());
        assert_eq!(headers.to_bytes(), b"\r\n");
        headers.set_header("Foo", "bar");
        assert!(!headers.is_empty());
        headers.clear();
        assert!(headers.is_empty());
        assert_eq!(headers.to_bytes(), b"\r\n");
    }

    #[test]
    fn set_header_keeps_key_spelling_from_first_insertion() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("content-type", "a");
        headers.set_header("Content-Type", "b");
        assert_eq!(headers.header_vector().len(), 1);
        assert_eq!(headers.header_vector()[0].key, b"content-type");
        assert_eq!(headers.header_vector()[0].value, b"b");
    }

    #[test]
    fn set_header_accepts_non_utf8_value() {
        // Obs-text (0x80-0xff) is allowed in header values.
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("X-Bin", [0x80u8, 0xff, b'a'].as_slice());
        assert_eq!(headers.get_header("X-Bin"), Some(vec![0x80, 0xff, b'a']));
    }

    #[test]
    #[should_panic(expected = "invalid header name")]
    fn set_header_panics_on_invalid_name() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Bad Name", "x");
    }

    #[test]
    #[should_panic(expected = "invalid header value")]
    fn set_header_panics_on_crlf_in_value() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Foo", "a\r\nInjected: 1");
    }

    #[test]
    #[should_panic(expected = "invalid header name")]
    fn set_header_if_missing_panics_on_invalid_name() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header_if_missing("", "x");
    }

    #[test]
    fn set_header_without_check_for_testing_skips_validation() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header_without_check_for_testing("Bad Name", "a\nb");
        assert_eq!(headers.to_bytes(), b"Bad Name: a\nb\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_errors_leave_headers_unchanged() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("Keep", "1");
        assert_eq!(
            headers.add_header_from_string("NoColon"),
            Err(HeaderLineError::MissingColon)
        );
        assert_eq!(
            headers.add_header_from_string(": value"),
            Err(HeaderLineError::MissingKey)
        );
        assert_eq!(
            headers.add_header_from_string("Bad Key: value"),
            Err(HeaderLineError::InvalidKey)
        );
        assert_eq!(
            headers.add_header_from_string("Key: a\x00b"),
            Err(HeaderLineError::InvalidValue)
        );
        assert_eq!(headers.to_bytes(), b"Keep: 1\r\n\r\n");
    }

    #[test]
    fn add_header_from_string_value_colons_are_kept() {
        // Only the first colon separates the name.
        let mut headers = HttpRequestHeaders::new();
        headers.add_header_from_string("Date: 12:30:00").unwrap();
        assert_eq!(headers.to_bytes(), b"Date: 12:30:00\r\n\r\n");
    }

    #[test]
    fn add_headers_from_string_splits_on_crlf_and_skips_bad_lines() {
        let mut headers = HttpRequestHeaders::new();
        let result = headers.add_headers_from_string(
            "A: 1\r\n  \r\nBad Line\r\nB: 2 \r\n\r\nC: 3",
        );
        // The bad line is skipped, the rest are added, and the error is returned.
        assert_eq!(result, Err(HeaderLineError::MissingColon));
        assert_eq!(headers.to_bytes(), b"A: 1\r\nB: 2\r\nC: 3\r\n\r\n");
    }

    #[test]
    fn add_headers_from_string_ok() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_headers_from_string("A: 1\r\nB: 2").unwrap();
        assert_eq!(headers.to_bytes(), b"A: 1\r\nB: 2\r\n\r\n");
    }

    #[test]
    fn add_headers_from_string_trims_vertical_tab_and_form_feed() {
        let mut headers = HttpRequestHeaders::new();
        headers.add_headers_from_string("\x0bA: 1\x0c\r\n").unwrap();
        assert_eq!(headers.to_bytes(), b"A: 1\r\n\r\n");
    }

    #[test]
    fn iterator_walks_headers_in_order() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("A", "1");
        headers.set_header("B", "2");

        let mut it = headers.iter();
        assert_eq!(it.name(), None);
        assert!(it.get_next());
        assert_eq!((it.name(), it.value()), (Some(&b"A"[..]), Some(&b"1"[..])));
        assert!(it.get_next());
        assert_eq!((it.name(), it.value()), (Some(&b"B"[..]), Some(&b"2"[..])));
        assert!(!it.get_next());
        assert_eq!(it.name(), None);
        // Stays at the end.
        assert!(!it.get_next());
    }

    #[test]
    fn iterator_on_empty_headers() {
        let headers = HttpRequestHeaders::new();
        let mut it = headers.iter();
        assert!(!it.get_next());
    }

    #[test]
    fn std_iterator_and_into_iterator_match_order() {
        let mut headers = HttpRequestHeaders::new();
        headers.set_header("A", "1");
        headers.set_header("B", "2");
        let names: Vec<&[u8]> = headers.iter().map(|h| h.key.as_slice()).collect();
        assert_eq!(names, vec![&b"A"[..], &b"B"[..]]);
        let mut count = 0;
        for _ in &headers {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    /// Reference model: a list of (name, value) with ASCII case-insensitive
    /// matching, the same behavior as the C++ linear scan.
    fn model_set(model: &mut Vec<(Vec<u8>, Vec<u8>)>, key: &[u8], value: &[u8]) {
        match model.iter().position(|(k, _)| k.eq_ignore_ascii_case(key)) {
            Some(i) => model[i].1 = value.to_vec(),
            None => model.push((key.to_vec(), value.to_vec())),
        }
    }

    fn model_bytes(model: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, v) in model {
            out.extend_from_slice(k);
            out.extend_from_slice(b": ");
            out.extend_from_slice(v);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out
    }

    #[test]
    fn randomized_matches_reference_model() {
        // Small xorshift PRNG; no external crates.
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        // Names differ only in case, so case-insensitive matching is tested.
        let names: [&[u8]; 5] = [b"Foo", b"foo", b"BAR", b"Bar", b"x-Y"];
        let values: [&[u8]; 4] = [b"", b"1", b"two words", b"\x80"];

        for _ in 0..300 {
            let mut headers = HttpRequestHeaders::new();
            let mut model: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
            for _ in 0..40 {
                let r = next();
                let name = names[(r % names.len() as u64) as usize];
                let value = values[((r >> 8) % values.len() as u64) as usize];
                match (r >> 16) % 3 {
                    0 => {
                        headers.set_header(name, value);
                        model_set(&mut model, name, value);
                    }
                    1 => {
                        headers.set_header_if_missing(name, value);
                        if !model.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) {
                            model.push((name.to_vec(), value.to_vec()));
                        }
                    }
                    _ => {
                        headers.remove_header(name);
                        if let Some(i) =
                            model.iter().position(|(k, _)| k.eq_ignore_ascii_case(name))
                        {
                            model.remove(i);
                        }
                    }
                }
                assert_eq!(headers.to_bytes(), model_bytes(&model));
                for probe in names {
                    let expected = model
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(probe))
                        .map(|(_, v)| v.clone());
                    assert_eq!(headers.get_header(probe), expected);
                    assert_eq!(headers.has_header(probe), expected.is_some());
                }
            }
        }
    }
}
