// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of net/http/http_util_unittest.cc, plus seeded randomized and
//! reference-model tests.
//!
//! `RequestUrlSanitize` is not ported: it needs GURL.

use super::*;

/// A small xorshift64 PRNG, so the randomized tests need no crates.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    fn bytes_from(&mut self, alphabet: &[u8], max_len: usize) -> Vec<u8> {
        let len = self.below(max_len as u64 + 1) as usize;
        (0..len).map(|_| *self.pick(alphabet)).collect()
    }
}

fn some(s: &[u8]) -> Option<Vec<u8>> {
    Some(s.to_vec())
}

/// Maps NUL bytes to '|' so expected strings can be written readably.
fn pipes(v: &[u8]) -> String {
    v.iter()
        .map(|&c| if c == 0 { '|' } else { c as char })
        .collect()
}

/// Maps '|' to NUL, the inverse of `pipes` for test inputs.
fn nul_for_pipes(s: &str) -> Vec<u8> {
    s.bytes().map(|c| if c == b'|' { 0 } else { c }).collect()
}

fn reference_values(s: &[u8], ignore_empty: bool) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut cur = Vec::new();
    let mut in_quote = false;
    let mut escaped = false;
    for &c in s {
        if in_quote {
            cur.push(c);
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_quote = false;
            }
        } else if c == b',' {
            out.push(std::mem::take(&mut cur));
        } else {
            if c == b'"' {
                in_quote = true;
            }
            cur.push(c);
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|t| trim_lws(&t).to_vec())
        .filter(|t| !ignore_empty || !t.is_empty())
        .collect()
}

fn accept_encoding_repr(value: &str) -> String {
    match parse_accept_encoding(value.as_bytes()) {
        None => "INVALID".to_string(),
        Some(set) => set
            .iter()
            .map(|e| String::from_utf8(e.clone()).unwrap())
            .collect::<Vec<_>>()
            .join("|"),
    }
}

fn content_encoding_repr(value: &str) -> String {
    match parse_content_encoding(value.as_bytes()) {
        None => "INVALID".to_string(),
        Some(set) => set
            .iter()
            .map(|e| String::from_utf8(e.clone()).unwrap())
            .collect::<Vec<_>>()
            .join("|"),
    }
}

fn date(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Time {
    Time::from_utc_exploded(&Exploded {
        year,
        month,
        day_of_month: day,
        hour,
        minute,
        second,
        ..Default::default()
    })
    .expect("valid test date")
}

// ---------------------------------------------------------------------------
// Ported unit tests.
// ---------------------------------------------------------------------------

#[test]
fn is_safe_header() {
    let unsafe_headers: &[&str] = &[
        "sec-",
        "sEc-",
        "sec-foo",
        "sEc-FoO",
        "proxy-",
        "pRoXy-",
        "proxy-foo",
        "pRoXy-FoO",
        "accept-charset",
        "accept-encoding",
        "access-control-request-headers",
        "access-control-request-method",
        "connection",
        "content-length",
        "cookie",
        "cookie2",
        "date",
        "dnt",
        "expect",
        "host",
        "keep-alive",
        "origin",
        "referer",
        "set-cookie",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "user-agent",
        "via",
    ];
    for header in unsafe_headers {
        assert!(!is_safe_header(header.as_bytes(), b""), "{header}");
        assert!(
            !is_safe_header(header.to_ascii_uppercase().as_bytes(), b""),
            "{header}"
        );
    }

    let safe_headers: &[&str] = &[
        "foo",
        "x-",
        "x-foo",
        "content-disposition",
        "update",
        "accept-charseta",
        "accept_charset",
        "accept-encodinga",
        "accept_encoding",
        "access-control-request-headersa",
        "access-control-request-header",
        "access_control_request_header",
        "access-control-request-methoda",
        "access_control_request_method",
        "connectiona",
        "content-lengtha",
        "content_length",
        "content-transfer-encoding",
        "cookiea",
        "cookie2a",
        "cookie3",
        "content-transfer-encodinga",
        "content_transfer_encoding",
        "datea",
        "expecta",
        "hosta",
        "keep-alivea",
        "keep_alive",
        "origina",
        "referera",
        "referrer",
        "tea",
        "trailera",
        "transfer-encodinga",
        "transfer_encoding",
        "upgradea",
        "user-agenta",
        "user_agent",
        "viaa",
        // The next three are safe unless a forbidden method appears in the value.
        "x-http-method",
        "x-http-method-override",
        "x-method-override",
    ];
    for header in safe_headers {
        assert!(is_safe_header(header.as_bytes(), b""), "{header}");
        assert!(
            is_safe_header(header.to_ascii_uppercase().as_bytes(), b""),
            "{header}"
        );
    }

    let disallowed_with_forbidden_methods_headers: &[&str] =
        &["x-http-method", "x-http-method-override", "x-method-override"];
    let disallowed_values: &[(&str, bool)] = &[
        ("connect", false),
        ("trace", false),
        ("track", false),
        ("CONNECT", false),
        ("cOnnEcT", false),
        ("get", true),
        ("get,post", true),
        ("get,connect", false),
        ("get, connect", false),
        ("get,connect ", false),
        ("get,connect ,post", false),
        ("get,,,,connect", false),
        ("trace,get,PUT", false),
    ];
    for header in disallowed_with_forbidden_methods_headers {
        for (value, is_safe) in disallowed_values {
            assert_eq!(
                *is_safe,
                is_safe_header(header.as_bytes(), value.as_bytes()),
                "{header}: {value}"
            );
        }
    }
}

#[test]
fn headers_iterator() {
    let headers = b"foo: 1\t\r\nbar: hello world\r\nbaz: 3 \r\n";
    let mut it = HeadersIterator::new(headers, b"\r\n");

    assert!(it.get_next());
    assert_eq!(it.name(), b"foo");
    assert_eq!(it.values(), b"1");
    assert_eq!(it.name_begin(), 0);
    assert_eq!(it.name_end(), 3);
    assert_eq!(it.values_begin(), 5);
    assert_eq!(it.values_end(), 6);

    assert!(it.get_next());
    assert_eq!(it.name(), b"bar");
    assert_eq!(it.values(), b"hello world");
    assert_eq!(it.name_begin(), 9);
    assert_eq!(it.name_end(), 12);
    assert_eq!(it.values_begin(), 14);
    assert_eq!(it.values_end(), 25);

    assert!(it.get_next());
    assert_eq!(it.name(), b"baz");
    assert_eq!(it.values(), b"3");
    assert_eq!(it.name_begin(), 27);
    assert_eq!(it.name_end(), 30);
    assert_eq!(it.values_begin(), 32);
    assert_eq!(it.values_end(), 33);

    assert!(!it.get_next());
}

#[test]
fn headers_iterator_malformed_line() {
    let headers = b"foo: 1\n: 2\n3\nbar: 4";
    let mut it = HeadersIterator::new(headers, b"\n");

    assert!(it.get_next());
    assert_eq!(it.name(), b"foo");
    assert_eq!(it.values(), b"1");
    assert_eq!(it.name_begin(), 0);
    assert_eq!(it.name_end(), 3);
    assert_eq!(it.values_begin(), 5);
    assert_eq!(it.values_end(), 6);

    assert!(it.get_next());
    assert_eq!(it.name(), b"bar");
    assert_eq!(it.values(), b"4");
    assert_eq!(it.name_begin(), 13);
    assert_eq!(it.name_end(), 16);
    assert_eq!(it.values_begin(), 18);
    assert_eq!(it.values_end(), 19);

    assert!(!it.get_next());
}

#[test]
fn headers_iterator_malformed_name() {
    let headers = b"[ignore me] /: 3\r\n";
    let mut it = HeadersIterator::new(headers, b"\r\n");
    assert!(!it.get_next());
}

#[test]
fn headers_iterator_malformed_name_followed_by_valid_line() {
    let headers = b"[ignore me] /: 3\r\nbar: 4\n";
    let mut it = HeadersIterator::new(headers, b"\r\n");

    assert!(it.get_next());
    assert_eq!(it.name(), b"bar");
    assert_eq!(it.values(), b"4");
    assert_eq!(it.name_begin(), 18);
    assert_eq!(it.name_end(), 21);
    assert_eq!(it.values_begin(), 23);
    assert_eq!(it.values_end(), 24);

    assert!(!it.get_next());
}

#[test]
fn values_iterator() {
    let values = b" must-revalidate,   no-cache=\"foo, bar\"\t, private ";
    let mut it = ValuesIterator::new(values, b',', true);

    assert!(it.get_next());
    assert_eq!(it.value(), b"must-revalidate");
    assert_eq!(it.value_begin(), 1);
    assert_eq!(it.value_end(), 16);

    assert!(it.get_next());
    assert_eq!(it.value(), b"no-cache=\"foo, bar\"");
    assert_eq!(it.value_begin(), 20);
    assert_eq!(it.value_end(), 39);

    assert!(it.get_next());
    assert_eq!(it.value(), b"private");
    assert_eq!(it.value_begin(), 42);
    assert_eq!(it.value_end(), 49);

    assert!(!it.get_next());
}

#[test]
fn values_iterator_empty_values() {
    let values = b", foopy , \t ,,,";

    let mut it = ValuesIterator::new(values, b',', true);
    assert!(it.get_next());
    assert_eq!(it.value(), b"foopy");
    assert_eq!(it.value_begin(), 2);
    assert_eq!(it.value_end(), 7);
    assert!(!it.get_next());

    let mut it_with_empty_values = ValuesIterator::new(values, b',', false);
    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 0);
    assert_eq!(it_with_empty_values.value_end(), 0);

    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"foopy");
    assert_eq!(it_with_empty_values.value_begin(), 2);
    assert_eq!(it_with_empty_values.value_end(), 7);

    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 12);
    assert_eq!(it_with_empty_values.value_end(), 12);

    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 13);
    assert_eq!(it_with_empty_values.value_end(), 13);

    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 14);
    assert_eq!(it_with_empty_values.value_end(), 14);

    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 15);
    assert_eq!(it_with_empty_values.value_end(), 15);

    assert!(!it_with_empty_values.get_next());
}

#[test]
fn values_iterator_blanks() {
    let values = b" \t ";

    let mut it = ValuesIterator::new(values, b',', true);
    assert!(!it.get_next());

    let mut it_with_empty_values = ValuesIterator::new(values, b',', false);
    assert!(it_with_empty_values.get_next());
    assert_eq!(it_with_empty_values.value(), b"");
    assert_eq!(it_with_empty_values.value_begin(), 3);
    assert_eq!(it_with_empty_values.value_end(), 3);
    assert!(!it_with_empty_values.get_next());
}

#[test]
fn unquote() {
    // Replace <backslash> " with ".
    assert_eq!(super::unquote(b"\"xyz\\\"abc\""), b"xyz\"abc");

    // Replace <backslash> <backslash> with <backslash>.
    assert_eq!(super::unquote(b"\"xyz\\\\abc\""), b"xyz\\abc");
    assert_eq!(super::unquote(b"\"xyz\\\\\\\\\\\\abc\""), b"xyz\\\\\\abc");

    // Replace <backslash> X with X.
    assert_eq!(super::unquote(b"\"xyz\\Xabc\""), b"xyzXabc");

    // Act as the identity on unquoted input.
    assert_eq!(super::unquote(b"X"), b"X");
    assert_eq!(super::unquote(b"\""), b"\"");

    // Quotes in the middle of the input are allowed.
    assert_eq!(super::unquote(b"\"foo\"bar\""), b"foo\"bar");

    // The final quote may be escaped.
    assert_eq!(super::unquote(b"\"foo\\\""), b"foo");
}

#[test]
fn strict_unquote() {
    // Replace <backslash> " with ".
    assert_eq!(strict_unquote(b"\"xyz\\\"abc\""), some(b"xyz\"abc"));

    // Replace <backslash> <backslash> with <backslash>.
    assert_eq!(strict_unquote(b"\"xyz\\\\abc\""), some(b"xyz\\abc"));
    assert_eq!(
        strict_unquote(b"\"xyz\\\\\\\\\\\\abc\""),
        some(b"xyz\\\\\\abc")
    );

    // Replace <backslash> X with X.
    assert_eq!(strict_unquote(b"\"xyz\\Xabc\""), some(b"xyzXabc"));

    // Empty quoted string.
    assert_eq!(strict_unquote(b"\"\""), some(b""));

    // Unquoted input and empty input are rejected.
    assert_eq!(strict_unquote(b"X"), None);
    assert_eq!(strict_unquote(b""), None);

    // Mismatched quotes are rejected.
    assert_eq!(strict_unquote(b"\""), None);
    assert_eq!(strict_unquote(b"\"xyz"), None);
    assert_eq!(strict_unquote(b"\"abc'"), None);

    // An escaped terminal quote is rejected.
    assert_eq!(strict_unquote(b"\"abc\\\""), None);
    assert_eq!(strict_unquote(b"\"\\\""), None);

    // An escaped backslash before the terminal quote is allowed.
    assert_eq!(strict_unquote(b"\"\\\\\""), some(b"\\"));

    // Single quotes are not quote marks.
    assert_eq!(strict_unquote(b"'x\"'"), None);
    assert_eq!(strict_unquote(b"\"x'\""), some(b"x'"));
    assert_eq!(strict_unquote(b"''"), None);
}

#[test]
fn quote() {
    assert_eq!(super::quote(b"xyz\"abc"), b"\"xyz\\\"abc\"");

    // Replace <backslash> <backslash> with <backslash>.
    assert_eq!(super::quote(b"xyz\\abc"), b"\"xyz\\\\abc\"");

    // Replace <backslash> X with X.
    assert_eq!(super::quote(b"xyzXabc"), b"\"xyzXabc\"");
}

#[test]
fn locate_end_of_headers() {
    let tests: &[(&str, Option<usize>)] = &[
        ("\r\n", None),
        ("\n", None),
        ("\r", None),
        ("foo", None),
        ("\r\n\r\n", Some(4)),
        ("foo\r\nbar\r\n\r\n", Some(12)),
        ("foo\nbar\n\n", Some(9)),
        ("foo\r\nbar\r\n\r\njunk", Some(12)),
        ("foo\nbar\n\njunk", Some(9)),
        ("foo\nbar\n\r\njunk", Some(10)),
        ("foo\nbar\r\n\njunk", Some(10)),
    ];
    for (input, expected) in tests {
        assert_eq!(
            super::locate_end_of_headers(input.as_bytes(), 0),
            *expected,
            "{input:?}"
        );
    }
}

#[test]
fn locate_end_of_additional_headers() {
    let tests: &[(&str, Option<usize>)] = &[
        ("\r\n", Some(2)),
        ("\n", Some(1)),
        ("\r", None),
        ("foo", None),
        ("\r\n\r\n", Some(2)),
        ("foo\r\nbar\r\n\r\n", Some(12)),
        ("foo\nbar\n\n", Some(9)),
        ("foo\r\nbar\r\n\r\njunk", Some(12)),
        ("foo\nbar\n\njunk", Some(9)),
        ("foo\nbar\n\r\njunk", Some(10)),
        ("foo\nbar\r\n\njunk", Some(10)),
    ];
    for (input, expected) in tests {
        assert_eq!(
            super::locate_end_of_additional_headers(input.as_bytes(), 0),
            *expected,
            "{input:?}"
        );
    }
}

#[test]
fn locate_start_of_status_line() {
    assert_eq!(locate_start_of_status_line(b"HTTP"), Some(0));
    assert_eq!(locate_start_of_status_line(b"HTTP/1.1 200 OK"), Some(0));
    assert_eq!(locate_start_of_status_line(b"xxxxHTTP/1.1 200 OK"), Some(4));
    assert_eq!(locate_start_of_status_line(b"xxxxxHTTP/1.1 200 OK"), None);
    assert_eq!(locate_start_of_status_line(b"HTT"), None);
}

#[test]
fn assemble_raw_headers() {
    // Inputs use '|' for NUL, and expected outputs use '|' for NUL.
    let tests: &[(&str, &str)] = &[
        (
            "HTTP/1.0 200 OK\r\nFoo: 1\r\nBar: 2\r\n\r\n",
            "HTTP/1.0 200 OK|Foo: 1|Bar: 2||",
        ),
        (
            "HTTP/1.0 200 OK\nFoo: 1\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1|Bar: 2||",
        ),
        // Valid line continuation (single SP).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n continuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation|Bar: 2||",
        ),
        // Valid line continuation (single HT).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n\tcontinuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation|Bar: 2||",
        ),
        // Valid line continuation (multiple SP).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n   continuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation|Bar: 2||",
        ),
        // Valid line continuation (multiple HT).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n\t\t\tcontinuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation|Bar: 2||",
        ),
        // Valid line continuation (mixed HT, SP).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n \t \t continuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation|Bar: 2||",
        ),
        // Valid multi-line continuation.
        (
            "HTTP/1.0 200 OK\nFoo: 1\n continuation1\n\tcontinuation2\n  continuation3\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1 continuation1 continuation2 continuation3|Bar: 2||",
        ),
        // Continuation of a quoted value. This differs from Firefox, which keeps the LWS.
        (
            "HTTP/1.0 200 OK\nEtag: \"34534-d3\n    134q\"\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Etag: \"34534-d3 134q\"|Bar: 2||",
        ),
        // Valid multi-line continuation, full LWS lines: one SP per continued line.
        (
            "HTTP/1.0 200 OK\nFoo: 1\n         \n\t\t\t\t\n\t  continuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1   continuation|Bar: 2||",
        ),
        // Valid multi-line continuation, all LWS.
        (
            "HTTP/1.0 200 OK\nFoo: 1\n         \n\t\t\t\t\n\t  \nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1   |Bar: 2||",
        ),
        // Valid line continuation with no value bytes on the first line.
        (
            "HTTP/1.0 200 OK\nFoo:\n value\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: value|Bar: 2||",
        ),
        // Not a line continuation: can't continue the status line.
        (
            "HTTP/1.0 200 OK\n Foo: 1\nBar: 2\n\n",
            "HTTP/1.0 200 OK| Foo: 1|Bar: 2||",
        ),
        // Not a line continuation: can't continue the status line.
        (
            "HTTP/1.0\n 200 OK\nFoo: 1\nBar: 2\n\n",
            "HTTP/1.0| 200 OK|Foo: 1|Bar: 2||",
        ),
        // Not a line continuation: can't continue the status line.
        (
            "HTTP/1.0 404\n Not Found\nFoo: 1\nBar: 2\n\n",
            "HTTP/1.0 404| Not Found|Foo: 1|Bar: 2||",
        ),
        // Unterminated status line.
        ("HTTP/1.0 200 OK", "HTTP/1.0 200 OK||"),
        // Single terminated, with headers.
        (
            "HTTP/1.0 200 OK\nFoo: 1\nBar: 2\n",
            "HTTP/1.0 200 OK|Foo: 1|Bar: 2||",
        ),
        // Not terminated, with headers.
        ("HTTP/1.0 200 OK\nFoo: 1\nBar: 2", "HTTP/1.0 200 OK|Foo: 1|Bar: 2||"),
        // Not a line continuation (VT).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n\x0bInvalidContinuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1|\x0bInvalidContinuation|Bar: 2||",
        ),
        // Not a line continuation (form feed).
        (
            "HTTP/1.0 200 OK\nFoo: 1\n\x0cInvalidContinuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1|\x0cInvalidContinuation|Bar: 2||",
        ),
        // Not a line continuation: can't continue header names.
        (
            "HTTP/1.0 200 OK\nServ\n er: Apache\n\tInvalidContinuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Serv| er: Apache|\tInvalidContinuation|Bar: 2||",
        ),
        // Not a line continuation: no value to continue.
        (
            "HTTP/1.0 200 OK\nFoo: 1\ngarbage\n  not-a-continuation\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1|garbage|  not-a-continuation|Bar: 2||",
        ),
        // Not a line continuation: no valid name.
        (
            "HTTP/1.0 200 OK\n: 1\n  garbage\nBar: 2\n\n",
            "HTTP/1.0 200 OK|: 1|  garbage|Bar: 2||",
        ),
        // Not a line continuation: no valid name (whitespace).
        (
            "HTTP/1.0 200 OK\n   : 1\n  garbage\nBar: 2\n\n",
            "HTTP/1.0 200 OK|   : 1|  garbage|Bar: 2||",
        ),
        // Embedded NULs in the status line are not line separators.
        (
            "HTTP/1.0 200 OK|Bar2:0|Baz2:1\r\nFoo: 1\r\nBar: 2\r\n\r\n",
            "HTTP/1.0 200 OKBar2:0Baz2:1|Foo: 1|Bar: 2||",
        ),
        // Embedded NULs in a header line are not line separators.
        (
            "HTTP/1.0 200 OK\nFoo: 1|Foo2: 3\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1Foo2: 3|Bar: 2||",
        ),
        // A NUL at the start of a line is not LWS, so it is not a continuation.
        (
            "HTTP/1.0 200 OK\nFoo: 1\n|Blah: 3\nBar: 2\n\n",
            "HTTP/1.0 200 OK|Foo: 1|Blah: 3|Bar: 2||",
        ),
        // Leading slop before the status line.
        ("xxxHTTP/1.0 200 OK\r\nFoo: 1\r\n\r\n", "HTTP/1.0 200 OK|Foo: 1||"),
    ];
    for (input, expected) in tests {
        let raw = super::assemble_raw_headers(&nul_for_pipes(input));
        assert_eq!(pipes(&raw), *expected, "input={input:?}");
    }
}

#[test]
fn generate_accept_language_header() {
    assert!(generate_accept_language_header(b"").is_empty());
    assert_eq!(generate_accept_language_header(b"es"), b"es");
    assert_eq!(
        generate_accept_language_header(b"en-US,fr,de"),
        b"en-US,fr;q=0.9,de;q=0.8"
    );
    assert_eq!(
        generate_accept_language_header(b"en-US,fr,de,ko,zh-CN,ja"),
        b"en-US,fr;q=0.9,de;q=0.8,ko;q=0.7,zh-CN;q=0.6,ja;q=0.5"
    );
    assert_eq!(
        generate_accept_language_header(b"en,fr,de,ko,zh-CN,ja,es,it,pt,nl,sv"),
        b"en,fr;q=0.9,de;q=0.8,ko;q=0.7,zh-CN;q=0.6,ja;q=0.5,es;q=0.4,it;q=0.3,pt;q=0.2,nl;q=0.1,sv;q=0.1"
    );
}

#[test]
fn parse_content_type() {
    // (content_type, mime_type, charset, had_charset, boundary)
    let tests: &[(&str, &str, &str, bool, &str)] = &[
        ("text/html", "text/html", "", false, ""),
        ("text/html;", "text/html", "", false, ""),
        ("text/html; charset=utf-8", "text/html", "utf-8", true, ""),
        // The parameter name is "charset ", not "charset". See crbug.com/772834.
        ("text/html; charset =utf-8", "text/html", "", false, ""),
        ("text/html; charset= utf-8", "text/html", "utf-8", true, ""),
        ("text/html; charset=utf-8 ", "text/html", "utf-8", true, ""),
        (
            "text/html; boundary=\"WebKit-ada-df-dsf-adsfadsfs\"",
            "text/html",
            "",
            false,
            "WebKit-ada-df-dsf-adsfadsfs",
        ),
        // The parameter name is "boundary ", not "boundary". See crbug.com/772834.
        (
            "text/html; boundary =\"WebKit-ada-df-dsf-adsfadsfs\"",
            "text/html",
            "",
            false,
            "",
        ),
        // The parameter value includes a leading space. See crbug.com/772834.
        (
            "text/html; boundary= \"WebKit-ada-df-dsf-adsfadsfs\"",
            "text/html",
            "",
            false,
            "WebKit-ada-df-dsf-adsfadsfs",
        ),
        (
            "text/html; boundary= \"WebKit-ada-df-dsf-adsfadsfs\"   ",
            "text/html",
            "",
            false,
            "WebKit-ada-df-dsf-adsfadsfs",
        ),
        (
            "text/html; boundary=\"WebKit-ada-df-dsf-adsfadsfs  \"",
            "text/html",
            "",
            false,
            "WebKit-ada-df-dsf-adsfadsfs",
        ),
        (
            "text/html; boundary=WebKit-ada-df-dsf-adsfadsfs",
            "text/html",
            "",
            false,
            "WebKit-ada-df-dsf-adsfadsfs",
        ),
        ("text/html; charset", "text/html", "", false, ""),
        ("text/html; charset=", "text/html", "", false, ""),
        ("text/html; charset= ", "text/html", "", false, ""),
        ("text/html; charset= ;", "text/html", "", false, ""),
        // Empty quoted strings are allowed.
        ("text/html; charset=\"\"", "text/html", "", true, ""),
        // Leading and trailing whitespace in quotes is trimmed.
        ("text/html; charset=\" \"", "text/html", "", true, ""),
        ("text/html; charset=\" foo \"", "text/html", "foo", true, ""),
        // With multiple values, the first one is used.
        ("text/html; charset=foo; charset=utf-8", "text/html", "foo", true, ""),
        ("text/html; charset; charset=; charset=utf-8", "text/html", "utf-8", true, ""),
        ("text/html; charset=utf-8; charset=; charset", "text/html", "utf-8", true, ""),
        ("text/html; boundary=foo; boundary=bar", "text/html", "", false, "foo"),
        // Stray quotes are ignored.
        ("text/html; \"; \"\"; charset=utf-8", "text/html", "utf-8", true, ""),
        // Non-leading quotes are kept as-is.
        ("text/html; charset=u\"tf-8\"", "text/html", "u\"tf-8\"", true, ""),
        ("text/html; charset=\"utf-8\"", "text/html", "utf-8", true, ""),
        // No closing quote.
        ("text/html; charset=\"utf-8", "text/html", "utf-8", true, ""),
        // \ is treated as an escape character.
        ("text/html; charset=\"\\utf\\-\\8\"", "text/html", "utf-8", true, ""),
        // Escaped backslash, escaped quote, and a backslash at the end of an
        // unterminated quoted string.
        ("text/html; charset=\"\\\\\\\"\\", "text/html", "\\\"\\", true, ""),
        // A quoted semicolon.
        ("text/html; charset=\";charset=utf-8;\"", "text/html", ";charset=utf-8;", true, ""),
        // Whitespace before a quoted value.
        ("text/html; charset= \"utf-8\"", "text/html", "utf-8", true, ""),
        // Regression test for crbug.com/772350: single quotes are part of the charset.
        ("text/html; charset='utf-8'", "text/html", "'utf-8'", true, ""),
        // An empty subtype is accepted.
        ("text/", "text/", "", false, ""),
        // "*/*" is ignored unless it has parameters or is not an exact match.
        ("*/*", "", "", false, ""),
        ("*/*; charset=utf-8", "*/*", "utf-8", true, ""),
        ("*/* ", "*/*", "", false, ""),
        // Regression test for crbug.com/1326529.
        ("teXT/html", "text/html", "", false, ""),
    ];
    for (content_type, mime, charset, had_charset, boundary) in tests {
        let mut state = ContentType::default();
        super::parse_content_type(content_type.as_bytes(), &mut state);
        assert_eq!(state.mime_type, mime.as_bytes(), "content_type={content_type:?}");
        assert_eq!(state.charset, charset.as_bytes(), "content_type={content_type:?}");
        assert_eq!(state.had_charset, *had_charset, "content_type={content_type:?}");
        assert_eq!(state.boundary, boundary.as_bytes(), "content_type={content_type:?}");
    }
}

#[test]
fn parse_content_reset_charset() {
    let mut state = ContentType::default();

    // Set the MIME type (case is ignored) but not the charset.
    super::parse_content_type(b"Text/Html", &mut state);
    assert_eq!(state.mime_type, b"text/html");
    assert_eq!(state.charset, b"");
    assert!(!state.had_charset);

    // The same type, with a charset.
    super::parse_content_type(b"tExt/hTml;charset=utf-8", &mut state);
    assert_eq!(state.mime_type, b"text/html");
    assert_eq!(state.charset, b"utf-8");
    assert!(state.had_charset);

    // The same type with different case and no charset. The charset is kept.
    super::parse_content_type(b"teXt/htMl", &mut state);
    assert_eq!(state.mime_type, b"text/html");
    assert_eq!(state.charset, b"utf-8");
    assert!(state.had_charset);

    // A different type clears the charset.
    super::parse_content_type(b"texT/plaiN", &mut state);
    assert_eq!(state.mime_type, b"text/plain");
    assert_eq!(state.charset, b"");
    assert!(state.had_charset);
}

#[test]
fn parse_content_range_header() {
    // (spec, expected_return_value, first, last, instance_length)
    let tests: &[(&str, bool, i64, i64, i64)] = &[
        ("", false, -1, -1, -1),
        ("megabytes 0-10/50", false, -1, -1, -1),
        ("0-10/50", false, -1, -1, -1),
        ("Bytes 0-50/51", true, 0, 50, 51),
        ("bytes 0-50/51", true, 0, 50, 51),
        ("bytes\t0-50/51", false, -1, -1, -1),
        ("    bytes 0-50/51", true, 0, 50, 51),
        ("    bytes    0    -   50  \t / \t51", true, 0, 50, 51),
        ("bytes 0\t-\t50\t/\t51\t", true, 0, 50, 51),
        ("  \tbytes\t\t\t 0\t-\t50\t/\t51\t", true, 0, 50, 51),
        ("\t   bytes \t  0    -   50   /   5   1", false, -1, -1, -1),
        ("\t   bytes \t  0    -   5 0   /   51", false, -1, -1, -1),
        ("bytes 50-0/51", false, -1, -1, -1),
        ("bytes * /*", false, -1, -1, -1),
        ("bytes *   /    *   ", false, -1, -1, -1),
        ("bytes 0-50/*", false, -1, -1, -1),
        ("bytes 0-50  /    * ", false, -1, -1, -1),
        ("bytes 0-10000000000/10000000001", true, 0, 10_000_000_000, 10_000_000_001),
        ("bytes 0-10000000000/10000000000", false, -1, -1, -1),
        // 64-bit wraparound.
        ("bytes 0 - 9223372036854775807 / 100", false, -1, -1, -1),
        // 64-bit wraparound.
        ("bytes 0 - 100 / -9223372036854775808", false, -1, -1, -1),
        ("bytes */50", false, -1, -1, -1),
        ("bytes 0-50/10", false, -1, -1, -1),
        ("bytes 40-50/45", false, -1, -1, -1),
        ("bytes 0-50/-10", false, -1, -1, -1),
        ("bytes 0-0/1", true, 0, 0, 1),
        ("bytes 0-40000000000000000000/40000000000000000001", false, -1, -1, -1),
        ("bytes 1-/100", false, -1, -1, -1),
        ("bytes -/100", false, -1, -1, -1),
        ("bytes -1/100", false, -1, -1, -1),
        ("bytes 0-1233/*", false, -1, -1, -1),
        ("bytes -123 - -1/100", false, -1, -1, -1),
    ];
    for (spec, expected_ok, first, last, length) in tests {
        let expected = expected_ok.then_some(ContentRange {
            first_byte_position: *first,
            last_byte_position: *last,
            instance_length: *length,
        });
        assert_eq!(
            parse_content_range_header_for_206(spec.as_bytes()),
            expected,
            "{spec:?}"
        );
    }
}

#[test]
fn parse_range_header() {
    let ranges = super::parse_range_header(b"bytes=0-99").expect("valid");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].first_byte_position(), Some(0));
    assert_eq!(ranges[0].last_byte_position(), Some(99));

    let ranges = super::parse_range_header(b"bytes=100-").expect("valid");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].first_byte_position(), Some(100));
    assert!(!ranges[0].has_last_byte_position());

    let ranges = super::parse_range_header(b"bytes=-50").expect("valid");
    assert_eq!(ranges.len(), 1);
    assert!(ranges[0].is_suffix_byte_range());
    assert_eq!(ranges[0].suffix_length(), Some(50));

    assert_eq!(super::parse_range_header(b"bytes"), None);
    assert_eq!(super::parse_range_header(b"bytes=100"), None);
}

/// Invalid Fetch single-range values must fail, including malformed syntax,
/// unsupported formats, missing digits, reverse ranges, and out-of-range values.
#[test]
fn parse_fetch_single_range_rejects_invalid() {
    let cases: &[&[u8]] = &[
        b"",
        b"byte=0-",
        b"bytes",
        b"bytes\t \t",
        b"bytes=0-5,15-",
        b"bytes=0-5, 15-",
        b"bytes=0-5,",
        b"bytes=-",
        b"bytes=x-5",
        b"bytes=5",
        b"bytes=5-x",
        b"bytes=x",
        b"5-",
        b"Bytes=0-5",
        b"bytes=-5-10",
        // Trailing whitespace after the range is not allowed by the spec.
        b"bytes=5-10 ",
        // Multiple ranges are not a single range.
        b"bytes=0-10,15-20",
        // Extra '=' or '-' delimiters are rejected.
        b"bytes==0-5",
        b"bytes=0--5",
        b"bytes=0-5-",
        b"bytes=-0-5",
    ];
    for value in cases {
        assert!(
            parse_fetch_single_range(value, true).is_none(),
            "{:?}",
            String::from_utf8_lossy(value)
        );
    }
}

/// Syntactically valid but semantically invalid ranges parse. Callers must
/// check `is_valid()`.
#[test]
fn parse_fetch_single_range_parses_semantic_invalid() {
    // "bytes=-0" is a zero-length suffix range. It parses, but is invalid.
    let range = parse_fetch_single_range(b"bytes=-0", true).expect("parses");
    assert!(range.is_suffix_byte_range());
    assert_eq!(range.suffix_length(), Some(0));
    assert!(!range.is_valid());

    // "bytes=10-5" has start > end. It parses, but is invalid.
    let range = parse_fetch_single_range(b"bytes=10-5", true).expect("parses");
    assert!(!range.is_suffix_byte_range());
    assert_eq!(range.first_byte_position(), Some(10));
    assert_eq!(range.last_byte_position(), Some(5));
    assert!(!range.is_valid());

    // An overflowing start saturates to i64::MAX, so start > end.
    let range = parse_fetch_single_range(b"bytes=999999999999999999999-5", true).expect("parses");
    assert!(!range.is_valid());
}

#[test]
fn parse_fetch_single_range_accepts_normal() {
    let cases: &[(&[u8], i64, i64)] = &[
        (b"bytes=5-10", 5, 10),
        (b"bytes=00005-00010", 5, 10),
        (b"bytes=0-0", 0, 0),
    ];
    for (value, first, last) in cases {
        let range = parse_fetch_single_range(value, true).expect("parses");
        assert!(!range.is_suffix_byte_range());
        assert_eq!(range.first_byte_position(), Some(*first));
        assert_eq!(range.last_byte_position(), Some(*last));
    }
}

#[test]
fn parse_fetch_single_range_accepts_suffix() {
    // The second case has whitespace after '-', which Fetch allows.
    for value in [&b"bytes=-5"[..], b"bytes=-\t 5"] {
        let range = parse_fetch_single_range(value, true).expect("parses");
        assert!(range.is_suffix_byte_range());
        assert_eq!(range.suffix_length(), Some(5));
    }
}

#[test]
fn parse_fetch_single_range_accepts_open_ended() {
    // The later cases have whitespace around '=' and '-', which Fetch allows.
    for value in [&b"bytes=5-"[..], b"bytes= 5 - ", b"bytes \t =\t 5-"] {
        let range = parse_fetch_single_range(value, true).expect("parses");
        assert!(!range.is_suffix_byte_range());
        assert_eq!(range.first_byte_position(), Some(5));
        assert!(!range.has_last_byte_position());
    }
}

#[test]
fn parse_fetch_single_range_accepts_whitespace_when_allowed() {
    let cases: &[&[u8]] = &[
        b"bytes =5-10",
        b"bytes= 5-10",
        b"bytes=5 -10",
        b"bytes=5- 10",
        b"bytes=5\t-\t10",
    ];
    for value in cases {
        let range = parse_fetch_single_range(value, true).expect("parses");
        assert_eq!(range.first_byte_position(), Some(5));
        assert_eq!(range.last_byte_position(), Some(10));
        assert!(
            parse_fetch_single_range(value, false).is_none(),
            "{:?}",
            String::from_utf8_lossy(value)
        );
    }
}

#[test]
fn parse_fetch_single_range_accepts_boundary_values() {
    // "bytes=0-" is an open-ended range that starts at byte 0.
    let open_ended = parse_fetch_single_range(b"bytes=0-", true).expect("parses");
    assert!(!open_ended.is_suffix_byte_range());
    assert_eq!(open_ended.first_byte_position(), Some(0));
    assert!(!open_ended.has_last_byte_position());

    // "bytes=-1" is a suffix range of the last byte.
    let suffix = parse_fetch_single_range(b"bytes=-1", true).expect("parses");
    assert!(suffix.is_suffix_byte_range());
    assert_eq!(suffix.suffix_length(), Some(1));
}

#[test]
fn parse_fetch_single_range_accepts_valid_ranges_in_strict_mode() {
    let cases: &[&[u8]] = &[b"bytes=5-10", b"bytes=5-", b"bytes=-5"];
    for value in cases {
        assert!(parse_fetch_single_range(value, false).is_some());
    }
}

/// Overflowing values saturate to i64::MAX instead of being rejected.
#[test]
fn parse_fetch_single_range_saturates_overflow() {
    let max = i64::MAX;

    // Overflowing end with a valid start: bounded range [5, max].
    let bounded = parse_fetch_single_range(b"bytes=5-999999999999999999999", true).expect("parses");
    assert!(!bounded.is_suffix_byte_range());
    assert_eq!(bounded.first_byte_position(), Some(5));
    assert_eq!(bounded.last_byte_position(), Some(max));

    // Overflowing start, open-ended: range [max, ).
    let open_ended = parse_fetch_single_range(b"bytes=999999999999999999999-", true).expect("parses");
    assert!(!open_ended.is_suffix_byte_range());
    assert_eq!(open_ended.first_byte_position(), Some(max));
    assert!(!open_ended.has_last_byte_position());

    // Overflowing suffix: suffix of max length.
    let suffix = parse_fetch_single_range(b"bytes=-999999999999999999999", true).expect("parses");
    assert!(suffix.is_suffix_byte_range());
    assert_eq!(suffix.suffix_length(), Some(max));
}

#[test]
fn parse_retry_after_header() {
    let now = date(2014, 11, 4, 5, 22, 39);
    let later = date(2015, 1, 5, 1, 12, 34);
    // Each case: (value, expected seconds or None).
    let tests: Vec<(&str, Option<TimeDelta>)> = vec![
        ("", None),
        ("-3", None),
        ("-2", None),
        ("-1", None),
        ("+0", None),
        ("+1", None),
        ("0", Some(TimeDelta::seconds(0))),
        ("1", Some(TimeDelta::seconds(1))),
        ("2", Some(TimeDelta::seconds(2))),
        ("3", Some(TimeDelta::seconds(3))),
        ("60", Some(TimeDelta::seconds(60))),
        ("3600", Some(TimeDelta::seconds(3600))),
        ("86400", Some(TimeDelta::seconds(86400))),
        ("Thu, 1 Jan 2015 12:34:56 GMT", Some(later - now)),
        ("Mon, 1 Jan 1900 12:34:56 GMT", None),
    ];
    for (i, (value, expected)) in tests.iter().enumerate() {
        assert_eq!(
            super::parse_retry_after_header(value.as_bytes(), now),
            *expected,
            "Test case {i}: {value:?}"
        );
    }
}

#[test]
fn time_format_http() {
    let exploded = Exploded {
        year: 2011,
        month: 4,
        day_of_week: 6,
        day_of_month: 30,
        hour: 22,
        minute: 42,
        second: 7,
        millisecond: 0,
    };
    let time = Time::from_utc_exploded(&exploded).expect("valid");
    assert_eq!(super::time_format_http(time), "Sat, 30 Apr 2011 22:42:07 GMT");
}

#[test]
fn name_value_pairs_iterator_copy_and_assign() {
    let data = "alpha=\"\\\"a\\\"\"; beta=\" b \"; cappa=\"c;\"; delta=\"d\"";
    let mut parser_a = NameValuePairsIterator::new(data.as_bytes(), b';');

    assert!(parser_a.valid());
    check_next(&mut parser_a, true, true, "alpha", "\"a\"");

    let mut parser_b = parser_a.clone();
    // a and b now point to the same location.
    check_current(&parser_b, true, "alpha", "\"a\"");
    check_current(&parser_a, true, "alpha", "\"a\"");

    // Advancing a does not affect b.
    check_next(&mut parser_a, true, true, "beta", " b ");
    check_current(&parser_b, true, "alpha", "\"a\"");

    // Assign b the current state of a. This does not affect a.
    parser_b = parser_a.clone();
    check_current(&parser_b, true, "beta", " b ");
    check_current(&parser_a, true, "beta", " b ");

    // Advancing b does not affect a.
    check_next(&mut parser_b, true, true, "cappa", "c;");
    check_current(&parser_a, true, "beta", " b ");
}

#[test]
fn name_value_pairs_iterator_empty_input() {
    let mut parser = NameValuePairsIterator::new(b"", b';');
    assert!(parser.valid());
    check_next(&mut parser, false, true, "", "");
}

#[test]
fn name_value_pairs_iterator() {
    let data = "alpha=1; beta= 2 ;\
        cappa =' 3; foo=';\
        cappa =\" 3; foo=\";\
        delta= \" \\\"4\\\" \"; e= \" '5'\"; e=6;\
        f=\"\\\"\\h\\e\\l\\l\\o\\ \\w\\o\\r\\l\\d\\\"\";\
        g=\"\"; h=\"hello\"";
    let mut parser = NameValuePairsIterator::new(data.as_bytes(), b';');
    assert!(parser.valid());

    check_next(&mut parser, true, true, "alpha", "1");
    check_next(&mut parser, true, true, "beta", "2");

    // Single quotes are not quotes.
    check_next(&mut parser, true, true, "cappa", "' 3");
    check_next(&mut parser, true, true, "foo", "'");

    // Double quotes are, and can contain semicolons and equals signs.
    check_next(&mut parser, true, true, "cappa", " 3; foo=");

    check_next(&mut parser, true, true, "delta", " \"4\" ");
    check_next(&mut parser, true, true, "e", " '5'");
    check_next(&mut parser, true, true, "e", "6");
    check_next(&mut parser, true, true, "f", "\"hello world\"");
    check_next(&mut parser, true, true, "g", "");
    check_next(&mut parser, true, true, "h", "hello");
    check_next(&mut parser, false, true, "", "");
}

#[test]
fn name_value_pairs_iterator_optional_values() {
    let data = "alpha=1; beta;cappa ;  delta; e    ; f=1";

    // The default parser requires values.
    let mut default_parser = NameValuePairsIterator::new(data.as_bytes(), b';');
    assert!(default_parser.valid());
    check_next(&mut default_parser, true, true, "alpha", "1");
    check_next(&mut default_parser, false, false, "", "");

    let mut values_required_parser = NameValuePairsIterator::with_options(
        data.as_bytes(),
        b';',
        Values::Required,
        Quotes::NotStrict,
    );
    assert!(values_required_parser.valid());
    check_next(&mut values_required_parser, true, true, "alpha", "1");
    check_next(&mut values_required_parser, false, false, "", "");

    let mut parser = NameValuePairsIterator::with_options(
        data.as_bytes(),
        b';',
        Values::NotRequired,
        Quotes::NotStrict,
    );
    assert!(parser.valid());

    check_next(&mut parser, true, true, "alpha", "1");
    check_next(&mut parser, true, true, "beta", "");
    check_next(&mut parser, true, true, "cappa", "");
    check_next(&mut parser, true, true, "delta", "");
    check_next(&mut parser, true, true, "e", "");
    check_next(&mut parser, true, true, "f", "1");
    check_next(&mut parser, false, true, "", "");
    assert!(parser.valid());
}

#[test]
fn name_value_pairs_iterator_illegal_inputs() {
    check_invalid_name_value_pair("alpha=1", "; beta");
    check_invalid_name_value_pair("", "beta");

    check_invalid_name_value_pair("alpha=1", "; \"beta\"=2");
    check_invalid_name_value_pair("", "\"beta\"=2");
    check_invalid_name_value_pair("alpha=1", ";beta=");
    check_invalid_name_value_pair("alpha=1", ";beta=;cappa=2");

    // The spec treats ";; beta=2" as an error, but this parser accepts it.
    // See the ExtraSeparators test.
}

/// Extra separators are accepted and handled sensibly.
#[test]
fn name_value_pairs_iterator_extra_separators() {
    let data = " ; ;;alpha=1; ;; ; beta= 2;cappa=3;;; ; ";
    let mut parser = NameValuePairsIterator::new(data.as_bytes(), b';');
    assert!(parser.valid());

    check_next(&mut parser, true, true, "alpha", "1");
    check_next(&mut parser, true, true, "beta", "2");
    check_next(&mut parser, true, true, "cappa", "3");
    check_next(&mut parser, false, true, "", "");
}

/// A missing closing quote is tolerated. See the comments on
/// `NameValuePairsIterator::get_next` for why this differs from the spec.
#[test]
fn name_value_pairs_iterator_missing_end_quote() {
    let mut parser = NameValuePairsIterator::new(b"name=\"value", b';');
    assert!(parser.valid());

    check_next(&mut parser, true, true, "name", "value");
    check_next(&mut parser, false, true, "", "");
}

#[test]
fn name_value_pairs_iterator_strict_quotes_escaped_end_quote() {
    let mut parser = NameValuePairsIterator::with_options(
        b"foo=bar; name=\"value\\\"",
        b';',
        Values::Required,
        Quotes::Strict,
    );
    assert!(parser.valid());

    check_next(&mut parser, true, true, "foo", "bar");
    check_next(&mut parser, false, false, "", "");
}

#[test]
fn name_value_pairs_iterator_strict_quotes_quote_in_value() {
    let mut parser = NameValuePairsIterator::with_options(
        b"foo=\"bar\"; name=\"va\"lue\"",
        b';',
        Values::Required,
        Quotes::Strict,
    );
    assert!(parser.valid());

    check_next(&mut parser, true, true, "foo", "bar");
    check_next(&mut parser, false, false, "", "");
}

#[test]
fn name_value_pairs_iterator_strict_quotes_missing_end_quote() {
    let mut parser = NameValuePairsIterator::with_options(
        b"foo=\"bar\"; name=\"value",
        b';',
        Values::Required,
        Quotes::Strict,
    );
    assert!(parser.valid());

    check_next(&mut parser, true, true, "foo", "bar");
    check_next(&mut parser, false, false, "", "");
}

#[test]
fn name_value_pairs_iterator_strict_quotes_single_quotes() {
    let mut parser = NameValuePairsIterator::with_options(
        b"foo=\"bar\"; name='value; ok=it'",
        b';',
        Values::Required,
        Quotes::Strict,
    );
    assert!(parser.valid());

    check_next(&mut parser, true, true, "foo", "bar");
    check_next(&mut parser, true, true, "name", "'value");
    check_next(&mut parser, true, true, "ok", "it'");
}

/// Checks the current pair, with `valid()` and the name/value text.
fn check_current(parser: &NameValuePairsIterator, expect_valid: bool, name: &str, value: &str) {
    assert_eq!(parser.valid(), expect_valid);
    if !expect_valid {
        return;
    }
    // A quoted value is unquoted once, and the same storage is returned later.
    let first_ptr = parser.value().as_ptr();
    let first_len = parser.value().len();

    assert_eq!(parser.name(), name.as_bytes());
    assert_eq!(parser.value(), value.as_bytes());

    assert_eq!(first_ptr, parser.value().as_ptr());
    assert_eq!(first_len, parser.value().len());
}

fn check_next(
    parser: &mut NameValuePairsIterator,
    expect_next: bool,
    expect_valid: bool,
    name: &str,
    value: &str,
) {
    assert_eq!(parser.get_next(), expect_next);
    assert_eq!(parser.valid(), expect_valid);
    if !expect_next || !expect_valid {
        return;
    }
    check_current(parser, expect_valid, name, value);
}

fn check_invalid_name_value_pair(valid_part: &str, invalid_part: &str) {
    let whole = format!("{valid_part}{invalid_part}");

    let mut valid_parser = NameValuePairsIterator::new(valid_part.as_bytes(), b';');
    let mut invalid_parser = NameValuePairsIterator::new(whole.as_bytes(), b';');
    assert!(valid_parser.valid());
    assert!(invalid_parser.valid());

    // Both parsers return the same pairs until valid_parser is exhausted.
    while valid_parser.get_next() {
        assert!(invalid_parser.get_next());
        assert!(valid_parser.valid());
        assert!(invalid_parser.valid());
        assert_eq!(valid_parser.name(), invalid_parser.name());
        assert_eq!(valid_parser.value(), invalid_parser.value());
    }

    // valid_parser is exhausted, but still valid, and its data is cleared.
    assert!(valid_parser.valid());
    assert!(valid_parser.name().is_empty());
    assert!(valid_parser.value().is_empty());
    assert!(valid_parser.raw_value().is_empty());
    assert!(!valid_parser.value_is_quoted());

    // The corresponding call on invalid_parser returns false, and it is
    // invalid.
    assert!(!invalid_parser.get_next());
    assert!(!invalid_parser.valid());

    // All values in an invalid parser are cleared.
    assert!(invalid_parser.name().is_empty());
    assert!(invalid_parser.value().is_empty());
    assert!(invalid_parser.raw_value().is_empty());
    assert!(!invalid_parser.value_is_quoted());
}

#[test]
fn has_validators() {
    // An empty header value is passed as Some(""), as in the C++ test.
    let missing = Some(&b""[..]);
    let etag_empty = Some(&b"\"\""[..]);
    let etag_strong = Some(&b"\"strong\""[..]);
    let etag_weak = Some(&b"W/\"weak\""[..]);
    let last_modified = Some(&b"Tue, 15 Nov 1994 12:45:26 GMT"[..]);
    let last_modified_invalid = Some(&b"invalid"[..]);

    let v0_9 = HttpVersion::new(0, 9);
    assert!(!super::has_validators(v0_9, missing, missing));
    assert!(!super::has_validators(v0_9, etag_strong, missing));
    assert!(!super::has_validators(v0_9, etag_weak, missing));
    assert!(!super::has_validators(v0_9, etag_empty, missing));

    assert!(!super::has_validators(v0_9, missing, last_modified));
    assert!(!super::has_validators(v0_9, etag_strong, last_modified));
    assert!(!super::has_validators(v0_9, etag_weak, last_modified));
    assert!(!super::has_validators(v0_9, etag_empty, last_modified));

    assert!(!super::has_validators(v0_9, missing, last_modified_invalid));
    assert!(!super::has_validators(v0_9, etag_strong, last_modified_invalid));
    assert!(!super::has_validators(v0_9, etag_weak, last_modified_invalid));
    assert!(!super::has_validators(v0_9, etag_empty, last_modified_invalid));

    let v1_0 = HttpVersion::new(1, 0);
    assert!(!super::has_validators(v1_0, missing, missing));
    assert!(!super::has_validators(v1_0, etag_strong, missing));
    assert!(!super::has_validators(v1_0, etag_weak, missing));
    assert!(!super::has_validators(v1_0, etag_empty, missing));

    assert!(super::has_validators(v1_0, missing, last_modified));
    assert!(super::has_validators(v1_0, etag_strong, last_modified));
    assert!(super::has_validators(v1_0, etag_weak, last_modified));
    assert!(super::has_validators(v1_0, etag_empty, last_modified));

    assert!(!super::has_validators(v1_0, missing, last_modified_invalid));
    assert!(!super::has_validators(v1_0, etag_strong, last_modified_invalid));
    assert!(!super::has_validators(v1_0, etag_weak, last_modified_invalid));
    assert!(!super::has_validators(v1_0, etag_empty, last_modified_invalid));

    let v1_1 = HttpVersion::new(1, 1);
    assert!(!super::has_validators(v1_1, missing, missing));
    assert!(super::has_validators(v1_1, etag_strong, missing));
    assert!(super::has_validators(v1_1, etag_weak, missing));
    assert!(super::has_validators(v1_1, etag_empty, missing));

    assert!(super::has_validators(v1_1, missing, last_modified));
    assert!(super::has_validators(v1_1, etag_strong, last_modified));
    assert!(super::has_validators(v1_1, etag_weak, last_modified));
    assert!(super::has_validators(v1_1, etag_empty, last_modified));

    assert!(!super::has_validators(v1_1, missing, last_modified_invalid));
    assert!(super::has_validators(v1_1, etag_strong, last_modified_invalid));
    assert!(super::has_validators(v1_1, etag_weak, last_modified_invalid));
    assert!(super::has_validators(v1_1, etag_empty, last_modified_invalid));
}

#[test]
fn has_strong_validators() {
    let v1_1 = HttpVersion::new(1, 1);
    let weak_etag = Some(&b"W/\"weak\""[..]);
    let strong_etag = Some(&b"\"strong\""[..]);
    let last_modified = Some(&b"Tue, 15 Nov 1994 12:45:26 GMT"[..]);
    let date_after_59_seconds = Some(&b"Tue, 15 Nov 1994 12:46:25 GMT"[..]);
    let date_after_60_seconds = Some(&b"Tue, 15 Nov 1994 12:46:26 GMT"[..]);

    assert!(super::has_strong_validators(v1_1, strong_etag, None, None));
    assert!(!super::has_strong_validators(v1_1, weak_etag, None, None));
    assert!(!super::has_strong_validators(v1_1, None, last_modified, date_after_59_seconds));
    assert!(super::has_strong_validators(v1_1, None, last_modified, date_after_60_seconds));
}

#[test]
fn is_valid_header_value() {
    let invalid_values: &[&[u8]] = &[
        b"X-Requested-With: chrome\0Sec-Unsafe: injected",
        b"X-Requested-With: chrome\r\nSec-Unsafe: injected",
        b"X-Requested-With: chrome\nSec-Unsafe: injected",
        b"X-Requested-With: chrome\rSec-Unsafe: injected",
    ];
    for value in invalid_values {
        assert!(!super::is_valid_header_value(value), "{value:?}");
    }

    // Every character RFC 7230 section 3.2.6 permits is allowed, including
    // the obs-text bytes 0x80..=0xFF.
    let mut allowed = vec![b'\t'];
    allowed.extend(0x20u8..0x7f);
    allowed.extend(0x80u8..=0xff);
    assert!(super::is_valid_header_value(&allowed));
}

#[test]
fn is_token_char() {
    for i in 0..256u32 {
        let c = i as u8;
        let expected = !(i >= 0x7f
            || i <= 0x20
            || c == b'('
            || c == b')'
            || c == b'<'
            || c == b'>'
            || c == b'@'
            || c == b','
            || c == b';'
            || c == b':'
            || c == b'\\'
            || c == b'"'
            || c == b'/'
            || c == b'['
            || c == b']'
            || c == b'?'
            || c == b'='
            || c == b'{'
            || c == b'}');
        assert_eq!(expected, super::is_token_char(c), "Failed for i={i}");
    }
}

#[test]
fn is_token() {
    assert!(super::is_token(b"valid"));
    assert!(super::is_token(b"!"));
    assert!(super::is_token(b"~"));

    assert!(!super::is_token(b""));
    assert!(!super::is_token(b"hello, world"));
    assert!(!super::is_token(b" "));
    assert!(!super::is_token(b"\0"));
    assert!(!super::is_token(b"\x01"));
    assert!(!super::is_token(b"\x7f"));
    assert!(!super::is_token(b"\x80"));
    assert!(!super::is_token(b"\xff"));
}

#[test]
fn is_param_name() {
    assert!(super::is_param_name(b"filename"));
    assert!(super::is_param_name(b"filename.ext"));

    assert!(!super::is_param_name(b""));
    assert!(!super::is_param_name(b"file*"));
    assert!(!super::is_param_name(b"file'name"));
    assert!(!super::is_param_name(b"file%20name"));
    assert!(!super::is_param_name(b"file name"));
}

#[test]
fn is_lws() {
    assert!(!super::is_lws(b'\x0b'));
    assert!(!super::is_lws(b'\0'));
    assert!(!super::is_lws(b'1'));
    assert!(!super::is_lws(b'a'));
    assert!(!super::is_lws(b'.'));
    assert!(!super::is_lws(b'\n'));
    assert!(!super::is_lws(b'\r'));

    assert!(super::is_lws(b'\t'));
    assert!(super::is_lws(b' '));
}

#[test]
fn trim_lws() {
    // (input, begin, end, expected_begin, expected_end)
    let cases: &[(&str, usize, usize, usize, usize)] = &[
        ("", 0, 0, 0, 0),
        (" a ", 0, 3, 1, 2),
        ("\ta\t", 0, 3, 1, 2),
        (" a ", 1, 2, 1, 2),
        (" a ", 1, 3, 1, 2),
        (" a ", 0, 2, 1, 2),
        (" a ", 0, 1, 1, 1),
        (" a ", 2, 3, 3, 3),
        (" \x01z\u{ff} ", 0, 5, 1, 4),
        (" a b ", 0, 5, 1, 4),
        ("\ra\n", 0, 3, 0, 3),
        (" \t a \t b\t \t", 0, 11, 3, 8),
    ];
    for (input, begin, end, expected_begin, expected_end) in cases {
        let bytes = input.as_bytes();
        assert_eq!(
            trim_lws_range(bytes, *begin, *end),
            (*expected_begin, *expected_end),
            "{input:?}"
        );
        // The slice overload trims the same way.
        let slice = &bytes[*begin..*end];
        assert_eq!(trim_lws(slice), &bytes[*expected_begin..*expected_end], "{input:?}");
    }
}

#[test]
fn is_control_char() {
    assert!(!super::is_control_char(b'1'));
    assert!(!super::is_control_char(b'a'));
    assert!(!super::is_control_char(b'.'));
    assert!(!super::is_control_char(b'$'));
    assert!(!super::is_control_char(0x7e));
    assert!(!super::is_control_char(0x80));
    assert!(!super::is_control_char(0xff));

    assert!(super::is_control_char(0));
    assert!(super::is_control_char(b'\x0b'));
    assert!(super::is_control_char(b'\n'));
    assert!(super::is_control_char(b'\r'));
    assert!(super::is_control_char(b'\t'));
    assert!(super::is_control_char(1));
    assert!(super::is_control_char(0x7f));
}

#[test]
fn parse_accept_encoding() {
    let tests: &[(&str, &str)] = &[
        ("", "*"),
        ("identity;q=1, *;q=0", "identity"),
        ("identity", "identity"),
        ("FOO, Bar", "bar|foo|identity"),
        ("foo; q=1", "foo|identity"),
        ("abc, foo; Q=1.0", "abc|foo|identity"),
        ("abc, foo;q= 1.00 , bar", "abc|bar|foo|identity"),
        ("abc, foo; q=1.000, bar", "abc|bar|foo|identity"),
        ("abc, foo ; q = 0 , bar", "abc|bar|identity"),
        ("abc, foo; q=0.0, bar", "abc|bar|identity"),
        ("abc, foo; q=0.00, bar", "abc|bar|identity"),
        ("abc, foo; q=0.000, bar", "abc|bar|identity"),
        ("abc, foo; q=0.001, bar", "abc|bar|foo|identity"),
        ("gzip", "gzip|identity|x-gzip"),
        ("x-gzip", "gzip|identity|x-gzip"),
        ("compress", "compress|identity|x-compress"),
        ("x-compress", "compress|identity|x-compress"),
        ("x-compress", "compress|identity|x-compress"),
        ("foo bar", "INVALID"),
        ("foo;", "INVALID"),
        ("foo;w=1", "INVALID"),
        ("foo;q+1", "INVALID"),
        ("foo;q=2", "INVALID"),
        ("foo;q=1.001", "INVALID"),
        ("foo;q=0.", "INVALID"),
        ("foo,\"bar\"", "INVALID"),
    ];
    for (value, expected) in tests {
        assert_eq!(accept_encoding_repr(value), *expected, "value={value:?}");
    }
}

#[test]
fn parse_content_encoding() {
    let tests: &[(&str, &str)] = &[
        ("", ""),
        ("identity;q=1, *;q=0", "INVALID"),
        ("identity", "identity"),
        ("FOO, zergli , Bar", "bar|foo|zergli"),
        ("foo, *", "INVALID"),
        ("foo,\"bar\"", "INVALID"),
    ];
    for (value, expected) in tests {
        assert_eq!(content_encoding_repr(value), *expected, "value={value:?}");
    }
}

#[test]
fn expand_language_list() {
    let expand = |s: &str| String::from_utf8(super::expand_language_list(s.as_bytes())).unwrap();
    assert_eq!(expand(""), "");
    assert_eq!(expand("en-US"), "en-US,en");
    assert_eq!(expand("fr"), "fr");

    // The base language comes after all regional codes...
    assert_eq!(expand("en-US,en-CA"), "en-US,en-CA,en");

    // ...but before other language families.
    assert_eq!(expand("en-US,en-CA,fr"), "en-US,en-CA,en,fr");
    assert_eq!(expand("en-US,en-CA,fr,en-AU"), "en-US,en-CA,en,fr,en-AU");
    assert_eq!(expand("en-US,en-CA,fr-CA"), "en-US,en-CA,en,fr-CA,fr");

    // A base language is added even if it is already in the list.
    assert_eq!(
        expand("en-US,fr-CA,it,fr,es-AR,it-IT"),
        "en-US,en,fr-CA,fr,it,es-AR,es,it-IT"
    );
    // Whitespace is trimmed.
    assert_eq!(expand("en-US, fr"), "en-US,en,fr");

    // The single-character subtags 'x' and 'i' are not expanded.
    assert_eq!(expand("x-private-agreement-subtags"), "x-private-agreement-subtags");
    assert_eq!(expand("i-klingon"), "i-klingon");
}

#[test]
fn status_codes_for_histogram() {
    let codes = status_codes_for_histogram();
    assert_eq!(codes.len(), 501);
    assert_eq!(codes[0], 0);
    for i in 100..=599 {
        assert_eq!(codes[(i - 100 + 1) as usize], i);
        assert_eq!(map_status_code_for_histogram(i), i);
    }
    assert_eq!(map_status_code_for_histogram(-1), 0);
    assert_eq!(map_status_code_for_histogram(0), 0);
    assert_eq!(map_status_code_for_histogram(99), 0);
    assert_eq!(map_status_code_for_histogram(600), 0);
}

// ---------------------------------------------------------------------------
// Additional tests: reference models, round trips and robustness.
// ---------------------------------------------------------------------------

#[test]
fn values_iterator_matches_reference_model_on_random_input() {
    let mut rng = Rng::new(0x9e37_79b9_7f4a_7c15);
    let alphabet = b"ab, \"\\\t";
    for _ in 0..20_000 {
        let input = rng.bytes_from(alphabet, 24);
        for ignore_empty in [true, false] {
            let expected = reference_values(&input, ignore_empty);
            let mut it = ValuesIterator::new(&input, b',', ignore_empty);
            let mut actual = Vec::new();
            while it.get_next() {
                actual.push(it.value().to_vec());
            }
            assert_eq!(
                actual,
                expected,
                "input={:?} ignore_empty={ignore_empty}",
                String::from_utf8_lossy(&input)
            );
        }
    }
}

#[test]
fn trim_lws_matches_reference_on_random_input() {
    let mut rng = Rng::new(42);
    let alphabet = b" \t\x0ba";
    for _ in 0..5_000 {
        let input = rng.bytes_from(alphabet, 12);
        let expected: &[u8] = {
            let s = input
                .iter()
                .position(|&c| c != b' ' && c != b'\t')
                .unwrap_or(input.len());
            let e = input
                .iter()
                .rposition(|&c| c != b' ' && c != b'\t')
                .map_or(s, |i| i + 1);
            &input[s..e.max(s)]
        };
        assert_eq!(trim_lws(&input), expected);
    }
}

#[test]
fn quote_round_trips_on_random_bytes() {
    let mut rng = Rng::new(7);
    for _ in 0..5_000 {
        let mut input = Vec::new();
        for _ in 0..rng.below(16) {
            input.push(rng.next_u64() as u8);
        }
        let quoted = super::quote(&input);
        assert_eq!(strict_unquote(&quoted), Some(input.clone()));
        assert_eq!(super::unquote(&quoted), input);
    }
}

#[test]
fn content_range_round_trips_random_values() {
    let mut rng = Rng::new(11);
    for _ in 0..5_000 {
        let instance = 1 + rng.below(1 << 40) as i64;
        let first = rng.below(instance as u64) as i64;
        let last = first + rng.below((instance - first) as u64) as i64;
        if last >= instance {
            continue;
        }
        let spec = format!("bytes {first}-{last}/{instance}");
        let parsed = parse_content_range_header_for_206(spec.as_bytes()).expect("valid spec");
        assert_eq!(
            parsed,
            ContentRange {
                first_byte_position: first,
                last_byte_position: last,
                instance_length: instance,
            },
            "{spec}"
        );
        // Making the instance length equal to the last position must fail.
        let bad = format!("bytes {first}-{last}/{last}");
        assert_eq!(parse_content_range_header_for_206(bad.as_bytes()), None, "{bad}");
    }
}

#[test]
fn fetch_single_range_round_trips_random_values() {
    let mut rng = Rng::new(13);
    let spaces: &[&str] = &["", " ", "\t", "  "];
    for _ in 0..5_000 {
        let start = rng.below(1 << 50) as i64;
        let end = start + rng.below(1 << 20) as i64;
        let ws1 = *rng.pick(spaces);
        let ws2 = *rng.pick(spaces);
        let ws3 = *rng.pick(spaces);
        let value = format!("bytes{ws3}={ws1}{start}{ws2}-{end}");
        let range = parse_fetch_single_range(value.as_bytes(), true).expect("parses");
        assert_eq!(range.first_byte_position(), Some(start), "{value}");
        assert_eq!(range.last_byte_position(), Some(end), "{value}");
        assert!(range.is_valid());
        // Whitespace is rejected in strict mode unless there is none.
        assert_eq!(
            parse_fetch_single_range(value.as_bytes(), false).is_some(),
            ws1.is_empty() && ws2.is_empty() && ws3.is_empty(),
            "{value}"
        );

        let suffix = 1 + rng.below(1 << 30);
        let value = format!("bytes=-{suffix}");
        let range = parse_fetch_single_range(value.as_bytes(), false).expect("parses");
        assert_eq!(range.suffix_length(), Some(suffix as i64));
    }
}

#[test]
fn range_header_round_trips_random_lists() {
    let mut rng = Rng::new(17);
    for _ in 0..2_000 {
        let count = 1 + rng.below(4) as usize;
        let mut parts = Vec::new();
        let mut expected = Vec::new();
        for _ in 0..count {
            match rng.below(3) {
                0 => {
                    let a = rng.below(1 << 30) as i64;
                    let b = a + rng.below(1 << 20) as i64;
                    parts.push(format!("{a}-{b}"));
                    expected.push(HttpByteRange::bounded(a, b));
                }
                1 => {
                    let a = rng.below(1 << 30) as i64;
                    parts.push(format!("{a}-"));
                    expected.push(HttpByteRange::right_unbounded(a));
                }
                _ => {
                    let n = 1 + rng.below(1 << 30) as i64;
                    parts.push(format!("-{n}"));
                    expected.push(HttpByteRange::suffix(n));
                }
            }
        }
        let header = format!("bytes={}", parts.join(", "));
        assert_eq!(
            parse_range_header(header.as_bytes()),
            Some(expected),
            "{header}"
        );
    }
}

#[test]
fn retry_after_random_seconds() {
    let now = date(2020, 6, 1, 0, 0, 0);
    let mut rng = Rng::new(19);
    for _ in 0..5_000 {
        let seconds = rng.next_u64() as u32;
        let value = seconds.to_string();
        assert_eq!(
            super::parse_retry_after_header(value.as_bytes(), now),
            Some(TimeDelta::seconds(seconds as i64))
        );
        let signed = format!("+{seconds}");
        assert_eq!(super::parse_retry_after_header(signed.as_bytes(), now), None);
    }
}

#[test]
fn time_format_round_trips_random_times() {
    let mut rng = Rng::new(23);
    for _ in 0..5_000 {
        // Whole seconds between 1970 and 2096.
        let seconds = rng.below(4_000_000_000) as i64;
        let time = Time::from_unix_micros(seconds * 1_000_000);
        let text = super::time_format_http(time);
        assert_eq!(Time::from_utc_string(text.as_bytes()), Some(time), "{text}");
        assert!(text.ends_with(" GMT"));
    }
}

#[test]
fn time_format_known_weekdays() {
    // 1970-01-01 was a Thursday.
    assert_eq!(
        super::time_format_http(Time::UNIX_EPOCH),
        "Thu, 01 Jan 1970 00:00:00 GMT"
    );
    // The leap day of 2024 was a Thursday.
    assert_eq!(
        super::time_format_http(date(2024, 2, 29, 12, 0, 0)),
        "Thu, 29 Feb 2024 12:00:00 GMT"
    );
}

#[test]
fn date_parser_accepts_rfc850_and_asctime() {
    let expected = date(1994, 11, 6, 8, 49, 37);
    assert_eq!(
        Time::from_utc_string(b"Sunday, 06-Nov-94 08:49:37 GMT"),
        Some(expected)
    );
    assert_eq!(Time::from_utc_string(b"Sun Nov  6 08:49:37 1994"), Some(expected));
    assert_eq!(Time::from_utc_string(b"Sun, 06 Nov 1994 08:49:37 GMT"), Some(expected));
    assert_eq!(Time::from_utc_string(b"Sun, 31 Feb 1994 08:49:37 GMT"), None);
    assert_eq!(Time::from_utc_string(b"Sun, 06 Nov 1994 24:00:00 GMT"), None);
}

#[test]
fn name_value_pairs_iterator_matches_quoting_rules_on_random_input() {
    let mut rng = Rng::new(29);
    let alphabet = b"ab=\"\\; ;\t";
    for _ in 0..5_000 {
        let input = rng.bytes_from(alphabet, 20);
        let mut parser = NameValuePairsIterator::new(&input, b';');
        // Iteration must terminate, and once it stops it must stay stopped.
        let mut steps = 0;
        while parser.get_next() {
            steps += 1;
            assert!(steps <= input.len() + 1);
        }
        assert!(!parser.get_next());
        assert!(parser.name().is_empty());
    }
}

/// Robustness: the parsers must not panic on arbitrary bytes. This replaces
/// the C++ `FUZZ_TEST(ParseFetchSingleRangeNeverCrashes)` harness.
#[test]
fn parsers_do_not_panic_on_random_bytes() {
    let mut rng = Rng::new(31);
    let alphabet = b"bytesBYTES=-,/ \t0123456789\"\\:;'\r\n\x00\xff\x80abcxyz";
    let now = date(2014, 11, 4, 5, 22, 39);
    for _ in 0..20_000 {
        let input = rng.bytes_from(alphabet, 40);
        let _ = parse_fetch_single_range(&input, true);
        let _ = parse_fetch_single_range(&input, false);
        let _ = parse_range_header(&input);
        let _ = parse_content_range_header_for_206(&input);
        let mut state = ContentType::default();
        parse_content_type(&input, &mut state);
        let _ = parse_accept_encoding(&input);
        let _ = parse_content_encoding(&input);
        let _ = parse_retry_after_header(&input, now);
        let _ = assemble_raw_headers(&input);
        let _ = convert_headers_back_to_http_response(&input);
        let _ = locate_start_of_status_line(&input);
        let _ = locate_end_of_headers(&input, 0);
        let _ = locate_end_of_additional_headers(&input, 0);
        let _ = expand_language_list(&input);
        let _ = generate_accept_language_header(&input);
        let _ = has_strong_validators(HttpVersion::new(1, 1), Some(&input), Some(&input), Some(&input));
        let _ = has_validators(HttpVersion::new(1, 1), Some(&input), Some(&input));
        let _ = is_safe_header(&input, &input);
        let _ = is_valid_header_value(&input);
        let _ = is_token(&input);

        let mut headers = HeadersIterator::new(&input, b"\r\n");
        while headers.get_next() {
            let _ = headers.name();
            let _ = headers.values();
        }

        let mut values = ValuesIterator::new(&input, b',', false);
        while values.get_next() {
            let _ = values.value();
        }

        let mut pairs = NameValuePairsIterator::with_options(
            &input,
            b';',
            Values::NotRequired,
            Quotes::Strict,
        );
        while pairs.get_next() {
            let _ = pairs.value();
        }
    }
}

#[test]
fn assemble_raw_headers_output_has_only_nul_separators() {
    let mut rng = Rng::new(37);
    let alphabet = b"Ab: \r\n\t\x00x";
    for _ in 0..5_000 {
        let input = rng.bytes_from(alphabet, 40);
        let raw = assemble_raw_headers(&input);
        assert!(!raw.contains(&b'\r'));
        assert!(!raw.contains(&b'\n'));
        assert!(raw.ends_with(b"\0\0"));
    }
}

#[test]
fn generate_accept_language_header_has_descending_q_values() {
    let mut rng = Rng::new(41);
    let tags: &[&str] = &["en", "fr", "de", "ko", "zh-CN", "ja", "es", "it", "pt", "nl", "sv", "ru"];
    for _ in 0..2_000 {
        let count = 1 + rng.below(12) as usize;
        let list: Vec<&str> = (0..count).map(|_| *rng.pick(tags)).collect();
        let raw = list.join(",");
        let header = String::from_utf8(generate_accept_language_header(raw.as_bytes())).unwrap();
        let items: Vec<&str> = header.split(',').collect();
        assert_eq!(items.len(), count);
        assert_eq!(items[0], list[0]);
        for (i, item) in items.iter().enumerate().skip(1) {
            let q = (10 - i).max(1);
            assert_eq!(*item, format!("{};q=0.{q}", list[i]));
        }
    }
}

#[test]
fn expand_language_list_keeps_every_input_tag() {
    let mut rng = Rng::new(43);
    let tags: &[&str] = &["en-US", "en-CA", "fr", "fr-CA", "it", "x-foo", "i-bar"];
    for _ in 0..2_000 {
        let count = 1 + rng.below(6) as usize;
        let list: Vec<&str> = (0..count).map(|_| *rng.pick(tags)).collect();
        let input = list.join(",");
        let out = String::from_utf8(expand_language_list(input.as_bytes())).unwrap();
        let out_tags: Vec<&str> = out.split(',').collect();
        for tag in &list {
            assert!(out_tags.contains(tag), "{tag} missing from {out} (input {input})");
        }
    }
}

#[test]
fn has_conflicting_values_compares_each_to_first() {
    let a: &[u8] = b"a";
    let b: &[u8] = b"b";
    assert!(!has_conflicting_values(&[]));
    assert!(!has_conflicting_values(&[a]));
    assert!(!has_conflicting_values(&[a, a]));
    assert!(has_conflicting_values(&[a, b]));
    assert!(has_conflicting_values(&[a, a, b]));
    assert!(has_conflicting_values(&[a, b, b]));
}

#[test]
fn headers_iterator_reset_restarts_iteration() {
    let headers = b"a: 1\r\nb: 2\r\n";
    let mut it = HeadersIterator::new(headers, b"\r\n");
    assert!(it.get_next());
    assert!(it.get_next());
    assert!(!it.get_next());
    it.reset();
    assert!(it.get_next());
    assert_eq!(it.name(), b"a");
}

#[test]
fn name_value_pairs_iterator_stays_failed_after_error() {
    let mut parser = NameValuePairsIterator::new(b"alpha=1; beta=", b';');
    assert!(parser.get_next());
    assert!(!parser.get_next());
    assert!(!parser.valid());
    // Further calls return false and do not revive the iterator.
    assert!(!parser.get_next());
    assert!(!parser.valid());
}

#[test]
fn http_byte_range_validity() {
    assert!(HttpByteRange::suffix(1).is_valid());
    assert!(!HttpByteRange::suffix(0).is_valid());
    assert!(HttpByteRange::bounded(0, 0).is_valid());
    assert!(!HttpByteRange::bounded(5, 4).is_valid());
    assert!(!HttpByteRange::bounded(-1, 4).is_valid());
    assert!(HttpByteRange::right_unbounded(3).is_valid());
    // -1 is a real value here. It is not "not specified".
    assert!(!HttpByteRange::bounded(0, -1).is_valid());
}

#[test]
fn parse_range_header_rejects_negative_last_position() {
    // The C++ -1 sentinel would accept this as open-ended. Here -1 is a value.
    assert_eq!(parse_range_header(b"bytes=0--1"), None);
}
