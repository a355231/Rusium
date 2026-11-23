// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::HostPortPair` (net/base/host_port_pair.{h,cc}).
//!
//! A hostname or IP literal plus a port. IPv6 literals are stored without
//! brackets and get brackets when formatted with [`HostPortPair::to_string`]
//! or [`HostPortPair::host_for_url`].
//!
//! Differences from the C++ version:
//! - `FromString` returns `Option<HostPortPair>`, `None` where the C++ version
//!   returns an empty pair (`IsEmpty()` is true). [`HostPortPair::from_string_or_empty`]
//!   gives the C++ behaviour.
//! - The `host:port` parser is `net::ParseHostAndPort` (net/base/url_util.cc)
//!   reimplemented for the inputs `FromString` accepts. The C++ version uses
//!   `url::ParseAuthority`, `url::ParsePort` and `url::IPv6AddressToNumber`.
//!   Here, ports are ASCII digits no larger than 65535, and bracketed hosts
//!   must be valid IPv6 according to `std::net::Ipv6Addr`. A `@` is rejected
//!   because the C++ version rejects any username or password. The host is not
//!   canonicalized, as in C++. This was written from the C++ source and has not
//!   been compared against Chromium's output.
//! - `FromSchemeHostPort` is not ported as written, because it takes a
//!   `url::SchemeHostPort`. [`HostPortPair::from_scheme_host_port_parts`] does
//!   the same bracket stripping on an already split host and port. It does not
//!   validate the scheme or canonicalize the host.
//! - `HostForURL` on a host containing a NUL byte: the C++ version also logs
//!   `LOG(DFATAL)`. This port has no logging and returns the host unchanged.
//!   The C++ `DCHECK_NE(host_[0], '[')` is a `debug_assert!`.
//! - Ordering is by port first, then host, which is the C++ defaulted `<=>`
//!   order because `port_` is declared before `host_`.
//! - Not ported:
//!   - `FromURL(const GURL&)`: depends on GURL (`url::`). The C++ header has no
//!     `ToURL`; `HostForURL` is the only URL-related method and is ported.
//!   - `FromIPEndPoint`: depends on `net::IPEndPoint`.
//!   - `FromValue` and `ToValue`: depend on `base::Value`.
//!   - Unit tests that need those: ParsesFromUrl, ParsesFromUrlWithIpv6Brackets,
//!     RoundtripThroughValue, DeserializeGarbageValue, DeserializeMalformedValues.

#![forbid(unsafe_code)]

use std::net::Ipv6Addr;

/// A host and port. The host is stored without IPv6 brackets.
///
/// Field order is significant: the derived `Ord` compares `port` first, as
/// the C++ defaulted `operator<=>` does.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostPortPair {
    port: u16,
    // Without brackets, even for IPv6 literals.
    host: String,
}

impl HostPortPair {
    /// Creates a pair. `host` should not be bracketed if it is an IPv6 literal.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self { port, host: host.into() }
    }

    /// Parses `"host:port"`, with IPv6 literals written as `"[addr]:port"`.
    /// Returns `None` if the input is not in that form, or if the port is
    /// missing or not a valid `u16`.
    pub fn from_string(input: &str) -> Option<Self> {
        // An input with more than one ':' is ambiguous unless it is a bracketed
        // IPv6 literal. Reject it here, as the C++ version does, to catch the
        // common mistake of writing an IPv6 address without brackets.
        let colons = input.bytes().filter(|&b| b == b':').count();
        if colons > 1 && !input.starts_with('[') {
            return None;
        }
        let (host, port) = parse_host_and_port(input)?;
        Some(Self::new(host, port))
    }

    /// Like [`HostPortPair::from_string`], but returns the empty pair on
    /// failure, as the C++ `FromString` does.
    pub fn from_string_or_empty(input: &str) -> Self {
        Self::from_string(input).unwrap_or_default()
    }

    /// The host and port part of `FromSchemeHostPort`. Removes the brackets
    /// from an IPv6 host such as `"[::1]"`, but does not validate the scheme
    /// or canonicalize the host.
    pub fn from_scheme_host_port_parts(host: &str, port: u16) -> Self {
        let stripped = if host.len() >= 2 && host.starts_with('[') && host.ends_with(']') {
            &host[1..host.len() - 1]
        } else {
            host
        };
        Self::new(stripped, port)
    }

    /// True if both the host is empty and the port is 0.
    pub fn is_empty(&self) -> bool {
        self.host.is_empty() && self.port == 0
    }

    /// The host, without IPv6 brackets.
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_host(&mut self, host: impl Into<String>) {
        self.host = host.into();
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    /// Returns the host, adding brackets if it is an IPv6 literal (contains
    /// a `:`).
    pub fn host_for_url(&self) -> String {
        if self.host.contains(':') {
            debug_assert!(
                !self.host.starts_with('['),
                "host must not already be bracketed: {}",
                self.host
            );
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }
}

impl std::fmt::Display for HostPortPair {
    /// Same output as the C++ `ToString()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host_for_url(), self.port)
    }
}

/// Splits `input` into `(host, port)` the way `net::ParseHostAndPort` does for
/// the inputs `from_string` accepts. Returns `None` if there is no port.
fn parse_host_and_port(input: &str) -> Option<(&str, u16)> {
    if input.is_empty() || input.contains('@') {
        return None;
    }

    // The port separator is the last ':' after any closing ']'. This matches
    // how url::ParseServerInfo scans backwards and stops at ']'.
    let separator = match input.rfind(']') {
        Some(bracket) => input[bracket + 1..].rfind(':').map(|i| bracket + 1 + i),
        None => input.rfind(':'),
    }?;
    let host = &input[..separator];
    let port_str = &input[separator + 1..];

    // Reject "foo:" as well as non-digits such as "+1" and "-1".
    if port_str.is_empty() || !port_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port = u16::try_from(port_str.parse::<u32>().ok()?).ok()?;

    if host.is_empty() {
        return None;
    }
    if host.starts_with('[') {
        // A bracketed host must be a complete, valid IPv6 literal.
        let inner = host.strip_prefix('[')?.strip_suffix(']')?;
        inner.parse::<Ipv6Addr>().ok()?;
        return Some((inner, port));
    }
    Some((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic PRNG (xorshift64), so the tests need no crates.
    struct XorShift64(u64);

    impl XorShift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
    }

    // (host, port, to_string, host_for_url) from the C++ `tests` table.
    const TESTS: &[(&str, u16, &str, &str)] = &[
        ("www.google.com", 80, "www.google.com:80", "www.google.com"),
        ("www.google.com", 443, "www.google.com:443", "www.google.com"),
        ("127.0.0.1", 80, "127.0.0.1:80", "127.0.0.1"),
        ("192.168.1.1", 80, "192.168.1.1:80", "192.168.1.1"),
        ("::1", 80, "[::1]:80", "[::1]"),
        ("2001:db8::42", 80, "[2001:db8::42]:80", "[2001:db8::42]"),
    ];

    #[test]
    fn parsing() {
        let foo = HostPortPair::new("foo.com", 10);
        let foo_str = foo.to_string();
        assert_eq!("foo.com:10", foo_str);
        let bar = HostPortPair::from_string(&foo_str).unwrap();
        assert_eq!(foo, bar);
    }

    #[test]
    fn parsing_ipv6() {
        let foo = HostPortPair::new("2001:db8::42", 100);
        let foo_str = foo.to_string();
        assert_eq!("[2001:db8::42]:100", foo_str);
        let bar = HostPortPair::from_string(&foo_str).unwrap();
        assert_eq!(foo, bar);
    }

    #[test]
    fn bad_string() {
        let bad_strings = [
            "foo.com",
            "foo.com:",
            "foo.com:2:3",
            "bar.com:two",
            "www.google.com:-1",
            "www.google.com:+1",
            "127.0.0.1:65536",
            "[2001:db8::42]:65536",
            "[2001:db8::42",
            "2001:db8::42",
            "2001:db8::42:100",
            "[2001:db8::42]",
        ];

        for test in bad_strings {
            assert_eq!(None, HostPortPair::from_string(test), "{test}");
            let foo = HostPortPair::from_string_or_empty(test);
            assert!(foo.host().is_empty(), "{test}");
            assert_eq!(0, foo.port(), "{test}");
        }
    }

    #[test]
    fn emptiness() {
        let foo = HostPortPair::default();
        assert!(foo.is_empty());
        let foo = HostPortPair::from_string("foo.com:8080").unwrap();
        assert!(!foo.is_empty());
    }

    #[test]
    fn to_string() {
        for &(host, port, to_string, _) in TESTS {
            let foo = HostPortPair::new(host, port);
            assert_eq!(to_string, foo.to_string());
        }

        // Empty hostname. The C++ test has no expectation here; this is the
        // value the C++ code produces.
        let foo = HostPortPair::new("", 10);
        assert_eq!(":10", foo.to_string());
    }

    #[test]
    fn host_for_url() {
        for &(host, port, _, host_for_url) in TESTS {
            let foo = HostPortPair::new(host, port);
            assert_eq!(host_for_url, foo.host_for_url());
        }

        // The C++ test expects a DFATAL here, which this port does not log.
        // It checks the returned value only: the NUL byte is kept as is.
        let bar_hostname = "a\0.\0com";
        let bar = HostPortPair::new(bar_hostname, 80);
        assert_eq!(bar_hostname, bar.host_for_url());
    }

    #[test]
    fn less_than() {
        let a_10 = HostPortPair::new("a.com", 10);
        let a_11 = HostPortPair::new("a.com", 11);
        let b_10 = HostPortPair::new("b.com", 10);
        let b_11 = HostPortPair::new("b.com", 11);

        assert!(!(a_10 < a_10));
        assert!(a_10 < a_11);
        assert!(a_10 < b_10);
        assert!(a_10 < b_11);

        assert!(!(a_11 < a_10));
        assert!(!(a_11 < b_10));

        assert!(!(b_10 < a_10));
        assert!(b_10 < a_11);

        assert!(!(b_11 < a_10));
    }

    #[test]
    fn equals() {
        let a_10 = HostPortPair::new("a.com", 10);
        let a_11 = HostPortPair::new("a.com", 11);
        let b_10 = HostPortPair::new("b.com", 10);
        let b_11 = HostPortPair::new("b.com", 11);

        let new_a_10 = HostPortPair::new("a.com", 10);

        assert_eq!(new_a_10, a_10);
        assert_ne!(new_a_10, a_11);
        assert_ne!(new_a_10, b_10);
        assert_ne!(new_a_10, b_11);
    }

    #[test]
    fn parses_from_scheme_host_port() {
        // C++ uses url::SchemeHostPort("ws", "bar.test", 111); the parts are
        // the host and port.
        let parsed = HostPortPair::from_scheme_host_port_parts("bar.test", 111);
        let expected = HostPortPair::new("bar.test", 111);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_from_scheme_host_port_with_ipv6_brackets() {
        // C++ uses url::SchemeHostPort("wss", "[::1022]", 112).
        let parsed = HostPortPair::from_scheme_host_port_parts("[::1022]", 112);
        let expected = HostPortPair::new("::1022", 112);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_host_and_port_edge_cases() {
        // Accepted forms.
        assert_eq!(HostPortPair::from_string("foo.com:0"), Some(HostPortPair::new("foo.com", 0)));
        assert_eq!(
            HostPortPair::from_string("foo.com:65535"),
            Some(HostPortPair::new("foo.com", 65535))
        );
        assert_eq!(
            HostPortPair::from_string("foo.com:0080"),
            Some(HostPortPair::new("foo.com", 80))
        );
        assert_eq!(HostPortPair::from_string("[::1]:80"), Some(HostPortPair::new("::1", 80)));
        assert_eq!(
            HostPortPair::from_string("[::ffff:1.2.3.4]:443"),
            Some(HostPortPair::new("::ffff:1.2.3.4", 443))
        );
        // Rejected forms.
        for input in [
            "",
            ":80",
            "[]:80",
            "[::1:80",
            "[::1]x:80",
            "[1.2.3.4]:80",
            "user@foo.com:80",
            "foo.com:65536",
            "foo.com:99999999999999999999",
            "foo.com: 80",
        ] {
            assert_eq!(None, HostPortPair::from_string(input), "{input}");
        }
    }

    #[test]
    fn display_matches_to_string() {
        let foo = HostPortPair::new("2001:db8::42", 100);
        assert_eq!(foo.to_string(), "[2001:db8::42]:100");
    }

    #[test]
    fn setters_and_accessors() {
        let mut foo = HostPortPair::default();
        foo.set_host("example.test");
        foo.set_port(8443);
        assert_eq!("example.test", foo.host());
        assert_eq!(8443, foo.port());
        assert!(!foo.is_empty());
    }

    #[test]
    fn differential_roundtrip_ipv6() {
        let mut rng = XorShift64(0x2545_f491_4f6c_dd1d);
        for _ in 0..5_000 {
            let addr = Ipv6Addr::from(((rng.next_u64() as u128) << 64) | rng.next_u64() as u128);
            let port = rng.next_u64() as u16;
            let pair = HostPortPair::new(addr.to_string(), port);
            let text = pair.to_string();
            assert_eq!(Some(pair), HostPortPair::from_string(&text), "{text}");
        }
    }

    #[test]
    fn differential_roundtrip_hostnames() {
        const ALPHABET: &[u8] = b"abcXYZ0123.-_ ]";
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..5_000 {
            let len = 1 + rng.below(12);
            let host: String =
                (0..len).map(|_| ALPHABET[rng.below(ALPHABET.len())] as char).collect();
            let port = rng.next_u64() as u16;
            let pair = HostPortPair::new(host.clone(), port);
            let text = pair.to_string();
            let parsed = HostPortPair::from_string(&text);
            assert_eq!(Some(pair), parsed, "host={host:?} text={text:?}");
        }
    }

    #[test]
    fn fuzz_parse_is_idempotent() {
        const ALPHABET: &[u8] = b"[]:0123456789ab.@+ -";
        let mut rng = XorShift64(0x0123_4567_89ab_cdef);
        let mut accepted = 0;
        for _ in 0..50_000 {
            let len = rng.below(16);
            let input: String =
                (0..len).map(|_| ALPHABET[rng.below(ALPHABET.len())] as char).collect();
            if let Some(pair) = HostPortPair::from_string(&input) {
                accepted += 1;
                // Anything accepted must format back to something that parses
                // to the same pair.
                let text = pair.to_string();
                assert_eq!(Some(pair.clone()), HostPortPair::from_string(&text), "{input:?}");
                assert!(!pair.host().contains('@'), "{input:?}");
            }
        }
        // Make sure the fuzzer actually exercised the accepting path.
        assert!(accepted > 0);
    }

    #[test]
    fn ordering_matches_port_then_host() {
        let mut rng = XorShift64(0xdead_beef_cafe_f00d);
        let hosts = ["a", "ab", "b", "a.com", ""];
        for _ in 0..2_000 {
            let p1 = HostPortPair::new(hosts[rng.below(hosts.len())], rng.below(4) as u16);
            let p2 = HostPortPair::new(hosts[rng.below(hosts.len())], rng.below(4) as u16);
            let expected = (p1.port(), p1.host()).cmp(&(p2.port(), p2.host()));
            assert_eq!(expected, p1.cmp(&p2));
        }
    }
}
