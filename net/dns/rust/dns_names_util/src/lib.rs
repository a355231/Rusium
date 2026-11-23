// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::dns_names_util`
//! (net/dns/dns_names_util.{h,cc}).
//!
//! Utilities for validating, converting and comparing DNS names in dotted
//! form (`www.example.com`) and in network wire form (length-prefixed labels
//! ending in a zero-length root label).
//!
//! Names are `&[u8]`, not `&str`, and dotted results are `Vec<u8>`, because
//! the C++ takes `std::string_view` and returns `std::string`, which can hold
//! arbitrary octets. A DNS label may contain any byte, and the C++ does not
//! validate label characters.
//!
//! Differences from the C++ version:
//! - `base::SpanReader` and `base::BigEndianReader` are replaced by the small
//!   [`SpanReader`] in this crate. Only the operations this file needs are
//!   provided.
//! - Functions that return a bool and an out-parameter return `Option` instead.
//!   For `ReadU8LengthPrefixed`/`ReadU16LengthPrefixed` the result is the
//!   length-prefixed slice, which borrows from the input. On failure the
//!   reader is left unchanged, as in C++.
//! - `NetworkToDottedName` has two entry points: [`network_to_dotted_name`]
//!   for a slice, and [`network_to_dotted_name_from_reader`] for a
//!   [`SpanReader`]. The C++ overloads are named the same.
//! - `DomainNameComparator` is [`compare_domain_names`]. Bytes are compared
//!   as unsigned octets after ASCII lowercasing. The C++ comparison for octets
//!   of 0x80 and above depends on the signedness of `char`.
//! - Host canonicalization for [`url_canonicalize_name_if_able`] is a reduced
//!   version of `url::CanonicalizeHostVerbose`, and this is the main gap in
//!   the port. It covers percent-decoding, ASCII lowercasing, the WHATWG
//!   forbidden-domain-code-point check, WHATWG IPv4 number parsing, and IPv6
//!   literals (through the `ip_address` crate). Non-ASCII labels get a Unicode
//!   `char::to_lowercase` and punycode (RFC 3492) with an `xn--` prefix. This
//!   is not full UTS #46 IDNA: there is no IDNA mapping table, no NFC
//!   normalization, and no ICU. Hosts that the real canonicalizer would
//!   normalize differently may therefore differ.
//! - Invalid UTF-8 is handled as the C++ handles a host that fails to
//!   canonicalize: the input is returned unchanged.
//! - `IsCanonicalizedHostCompliant` (net/base/url_util.cc) and
//!   `HostStringIsLocalhost` are reimplemented here, from the C++ source, since
//!   they are not in a sibling crate. The `ip_address` crate supplies the IP
//!   literal parsing that `IPAddress::AssignFromIPLiteral` and
//!   `ParseURLHostnameToAddress` use.
//! - The `DCHECK`s on the empty-label and length checks are dropped. The
//!   `require_valid_internet_hostname` path already rejects those inputs
//!   before the checks run, so the `DCHECK`s never fire.
//! - `DnsRecordParser::ReadName` and the other `DnsResponse` code are not part
//!   of this file and are not ported.
//!
//! Not ported: the `base::BigEndianReader` and `base::SpanReader` APIs beyond
//! the methods used here, and the `net::dns_protocol` header. The protocol
//! constants are defined in this crate.

#![forbid(unsafe_code)]

use ip_address::{parse_url_hostname_to_address, IPAddress};
use std::cmp::Ordering;

/// Same as `dns_protocol::kMaxNameLength`: the maximum length of a wire-format
/// name, including the length octets and the terminating zero-length label.
pub const MAX_NAME_LENGTH: usize = 255;
/// Same as `dns_protocol::kMaxLabelLength`.
pub const MAX_LABEL_LENGTH: usize = 63;
/// Same as `dns_protocol::kLabelMask`.
const LABEL_MASK: u8 = 0xc0;
/// Same as `dns_protocol::kLabelPointer`. A name compression pointer.
const LABEL_POINTER: u8 = 0xc0;

/// A reader over a byte slice, used in place of `base::SpanReader`.
///
/// All methods leave the reader unchanged when they fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SpanReader<'a> {
    /// Creates a reader positioned at the start of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        SpanReader { data, pos: 0 }
    }

    /// Number of bytes not yet read.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// The bytes not yet read.
    pub fn remaining_span(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    /// Number of bytes read so far.
    pub fn num_read(&self) -> usize {
        self.pos
    }

    /// Advances past `n` bytes. Returns false, and does not advance, if fewer
    /// than `n` bytes remain.
    #[must_use]
    pub fn skip(&mut self, n: usize) -> bool {
        self.read(n).is_some()
    }

    /// Reads the next `n` bytes. Returns `None`, and does not advance, if fewer
    /// than `n` bytes remain.
    pub fn read(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.remaining() < n {
            return None;
        }
        let bytes = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(bytes)
    }

    /// Reads one byte.
    pub fn read_u8(&mut self) -> Option<u8> {
        self.read(1).map(|b| b[0])
    }

    /// Reads a big-endian `u16`.
    pub fn read_u16_be(&mut self) -> Option<u16> {
        self.read(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
}

/// Returns true iff `dotted` is acceptable to be encoded as a DNS name: it is
/// non-empty and fits the size limits, and it has the structure of
/// dot-separated labels, each non-empty and within the label limit, with an
/// optional dot at the end. See RFCs 1035 and 2181.
///
/// No validation is performed for the characters within a label. See
/// [`is_canonicalized_host_compliant`] for that.
pub fn is_valid_dns_name(dotted: &[u8]) -> bool {
    dotted_name_to_network(dotted, false).is_some()
}

/// Like [`is_valid_dns_name`], but also returns false if `dotted` is an IP
/// address (with or without `[]`) or `localhost`. Such names are not suitable
/// for DNS queries or as record or alias-target names.
pub fn is_valid_dns_record_name(dotted: &[u8]) -> bool {
    is_valid_dns_name(dotted)
        && !host_string_is_localhost(dotted)
        && ip_literal(dotted).is_none()
        && url_hostname_to_address(dotted).is_none()
}

/// Converts a dotted-form DNS name to network wire format. Returns `None` if
/// the input is not valid for conversion (see [`is_valid_dns_name`]). If
/// `require_valid_internet_hostname` is true, also returns `None` unless the
/// input passes [`is_canonicalized_host_compliant`].
pub fn dotted_name_to_network(
    dotted: &[u8],
    require_valid_internet_hostname: bool,
) -> Option<Vec<u8>> {
    // Full validation first, when requested. Every check below is looser than
    // this one.
    if require_valid_internet_hostname && !is_canonicalized_host_compliant(dotted) {
        return None;
    }

    let mut name: Vec<u8> = Vec::with_capacity(MAX_NAME_LENGTH);
    let mut start = 0usize;
    while start < dotted.len() {
        let pos = dotted[start..]
            .iter()
            .position(|&c| c == b'.')
            .map_or(dotted.len(), |p| start + p);
        let label = &dotted[start..pos];

        // Empty labels are not allowed. See http://crbug.com/456391.
        if label.is_empty() {
            return None;
        }
        // The `+ 2` counts the length octet and the terminating zero octet.
        if name.len() + label.len() + 2 > MAX_NAME_LENGTH || label.len() > MAX_LABEL_LENGTH {
            return None;
        }
        // The cast is safe because MAX_LABEL_LENGTH < 255.
        name.push(label.len() as u8);
        name.extend_from_slice(label);

        if pos == dotted.len() {
            break;
        }
        start = pos + 1;
    }

    // Empty names such as "" and "." are not valid.
    if name.is_empty() {
        return None;
    }
    // The root label has length 0.
    name.push(0);
    Some(name)
}

/// Converts a domain in wire format to a dotted string, without the dot at
/// the end. Returns `None` on malformed input.
///
/// If `require_complete` is true, input without a terminating zero-length
/// label is malformed. If false, the standard terminating label is assumed.
///
/// DNS name compression (RFC 1035, section 4.1.4) is disallowed and treated
/// as malformed. To handle a possibly compressed name in a DNS response, use
/// the response parser's name reader instead.
pub fn network_to_dotted_name(span: &[u8], require_complete: bool) -> Option<Vec<u8>> {
    let mut reader = SpanReader::new(span);
    network_to_dotted_name_from_reader(&mut reader, require_complete)
}

/// Like [`network_to_dotted_name`], but reads from `reader` and advances it
/// past the name. The reader is advanced past the terminating zero-length
/// label, if one is present; any bytes after it are not consumed.
pub fn network_to_dotted_name_from_reader(
    reader: &mut SpanReader<'_>,
    require_complete: bool,
) -> Option<Vec<u8>> {
    let mut ret: Vec<u8> = Vec::new();
    let mut octets_read = 0usize;
    while reader.remaining() > 0 {
        // Name compression is not allowed, because it has no meaning without
        // the rest of the DNS message.
        if (reader.remaining_span()[0] & LABEL_MASK) == LABEL_POINTER {
            return None;
        }

        let label = read_u8_length_prefixed(reader)?;

        // The final zero-length label is not counted against the size limit.
        if !label.is_empty() {
            octets_read += label.len() + 1;
        }

        if label.len() > MAX_LABEL_LENGTH {
            return None;
        }
        if octets_read > MAX_NAME_LENGTH {
            return None;
        }

        if label.is_empty() {
            return Some(ret);
        }

        if !ret.is_empty() {
            ret.push(b'.');
        }
        ret.extend_from_slice(label);
    }

    if require_complete {
        return None;
    }

    // If the terminating zero-length label was absent, the max-length check
    // needs no recheck, because that label does not count against the limit.
    Some(ret)
}

/// Reads a length-prefixed region with an 8-bit big-endian length `L`. On
/// success returns the next `L` bytes and advances the reader past them. Fails
/// if the length cannot be read, or if `L` exceeds the bytes remaining. On
/// failure the reader is unchanged.
pub fn read_u8_length_prefixed<'a>(reader: &mut SpanReader<'a>) -> Option<&'a [u8]> {
    let mut inner = *reader;
    let len = inner.read_u8()? as usize;
    let bytes = inner.read(len)?;
    *reader = inner;
    Some(bytes)
}

/// Like [`read_u8_length_prefixed`], but with a 16-bit big-endian length.
pub fn read_u16_length_prefixed<'a>(reader: &mut SpanReader<'a>) -> Option<&'a [u8]> {
    let mut inner = *reader;
    let len = inner.read_u16_be()? as usize;
    let bytes = inner.read(len)?;
    *reader = inner;
    Some(bytes)
}

/// Canonicalizes `name` as a URL hostname, if able. If the name cannot be
/// canonicalized (for example, it is not a valid URL hostname), returns it
/// unchanged, because it could still be a valid DNS name.
///
/// See the crate documentation for how the host canonicalization differs from
/// `url::CanonicalizeHostVerbose`.
pub fn url_canonicalize_name_if_able(name: &[u8]) -> Vec<u8> {
    match canonicalize_host(name) {
        Some(canonical) => canonical.into_bytes(),
        None => name.to_vec(),
    }
}

/// Compares two domain names in dotted or wire format, ignoring ASCII case.
/// This is a strict weak ordering, suitable for an ordered map.
///
/// For valid wire-format names this gives the same result as for dotted
/// names, because a label length octet is at most 63 and so never falls in
/// the uppercase ASCII range.
pub fn compare_domain_names(lhs: &[u8], rhs: &[u8]) -> Ordering {
    lhs.iter()
        .map(u8::to_ascii_lowercase)
        .cmp(rhs.iter().map(u8::to_ascii_lowercase))
}

/// Reimplementation of `net::IsCanonicalizedHostCompliant` (net/base/url_util.cc).
///
/// Returns true iff `host` is a canonicalized internet hostname: labels of
/// lowercase ASCII letters, digits, `-` or `_`, where each label starts with a
/// letter or digit, has at most 63 characters, and the total is at most 253
/// characters (254 with a trailing dot). Uppercase letters are rejected, as in
/// the C++ code, which expects canonicalized input.
pub fn is_canonicalized_host_compliant(host: &[u8]) -> bool {
    if host.is_empty() || host.len() > 254 || (host.last() != Some(&b'.') && host.len() == 254) {
        return false;
    }

    let mut in_component = false;
    let mut most_recent_component_started_alphanumeric = false;
    let mut label_size = 0usize;

    for &c in host {
        label_size += 1;
        if !in_component {
            most_recent_component_started_alphanumeric = is_host_char_alphanumeric(c);
            if !most_recent_component_started_alphanumeric && c != b'-' && c != b'_' {
                return false;
            }
            in_component = true;
        } else if c == b'.' {
            in_component = false;
            // The label must be non-empty and at most 63 characters. The count
            // includes the '.', hence 64 here.
            if label_size > 64 || label_size == 1 {
                return false;
            }
            label_size = 0;
        } else if !is_host_char_alphanumeric(c) && c != b'-' && c != b'_' {
            return false;
        }
    }

    // Check for a too-long final label when the name does not end in a dot.
    if label_size > 63 {
        return false;
    }

    most_recent_component_started_alphanumeric
}

/// Reimplementation of `net::HostStringIsLocalhost` (net/base/url_util.cc).
///
/// An IP literal is localhost if it is a loopback address. Otherwise the name
/// is localhost if it is `localhost` or `*.localhost`, ignoring case and one
/// trailing dot.
pub fn host_string_is_localhost(host: &[u8]) -> bool {
    if let Some(ip) = ip_literal(host) {
        return ip.is_loopback();
    }
    is_local_hostname(host)
}

fn is_host_char_alphanumeric(c: u8) -> bool {
    // Uppercase letters are already normalized away in canonicalized input.
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

fn is_local_hostname(host: &[u8]) -> bool {
    // Remove one trailing '.'.
    let host = host.strip_suffix(b".").unwrap_or(host);
    host.eq_ignore_ascii_case(b"localhost") || is_normalized_localhost_tld(host)
}

fn is_normalized_localhost_tld(host: &[u8]) -> bool {
    let suffix = b".localhost";
    host.len() >= suffix.len() && host[host.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// Parses `host` as an IP literal (no brackets). Returns `None` if the bytes
/// are not UTF-8 or not an IP literal.
fn ip_literal(host: &[u8]) -> Option<IPAddress> {
    IPAddress::from_ip_literal(std::str::from_utf8(host).ok()?)
}

/// Mirrors `net::ParseURLHostnameToAddress`: an IPv4 literal, or an IPv6
/// literal in square brackets.
fn url_hostname_to_address(host: &[u8]) -> Option<IPAddress> {
    parse_url_hostname_to_address(std::str::from_utf8(host).ok()?)
}

/// Percent-decodes `input`. A `%` not followed by two hex digits is kept
/// as-is.
fn percent_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let (Some(hi), Some(lo)) = (hex_val(input[i + 1]), hex_val(input[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Returns true if `s` contains a forbidden domain code point (WHATWG URL
/// standard): C0 controls, DEL, space, and `# % / : < > ? @ [ \ ] ^ |`.
fn has_forbidden_domain_code_point(s: &str) -> bool {
    s.chars()
        .any(|c| (c as u32) <= 0x1f || c == '\u{7f}' || " #%/:<>?@[\\]^|".contains(c))
}

/// Returns true if the last label of `host` is numeric, which means the host
/// must be parsed as an IPv4 address (WHATWG URL standard "ends in a number").
fn ends_in_number(host: &str) -> bool {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    let Some(last) = parts.last() else {
        return false;
    };
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    parse_ipv4_number(last).is_some()
}

/// Parses one IPv4 component (decimal, `0x` hex, or leading-zero octal), as in
/// the WHATWG URL standard. Values that overflow saturate.
fn parse_ipv4_number(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let (digits, radix) = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (hex, 16)
    } else if s.len() >= 2 && s.starts_with('0') {
        (&s[1..], 8)
    } else {
        (s, 10)
    };
    if digits.is_empty() {
        return Some(0);
    }
    let mut value: u64 = 0;
    for c in digits.chars() {
        let d = c.to_digit(radix)?;
        value = value.saturating_mul(u64::from(radix)).saturating_add(u64::from(d));
    }
    Some(value)
}

/// Canonical dotted-decimal form of an IPv4 host, per the WHATWG URL standard.
/// Returns `None` if the host is not a valid IPv4 address.
fn canonical_ipv4(host: &str) -> Option<String> {
    let mut parts: Vec<&str> = host.split('.').collect();
    if parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() > 4 {
        return None;
    }
    let numbers: Vec<u64> = parts
        .iter()
        .map(|p| parse_ipv4_number(p))
        .collect::<Option<Vec<u64>>>()?;
    let (last, leading) = numbers.split_last()?;
    if leading.iter().any(|&n| n > 255) {
        return None;
    }
    // The last component fills all remaining octets.
    let last_limit = 256u64.pow((5 - numbers.len()) as u32);
    if *last >= last_limit {
        return None;
    }
    let mut ipv4 = *last;
    for (i, &n) in leading.iter().enumerate() {
        ipv4 += n * 256u64.pow((3 - i) as u32);
    }
    Some(format!(
        "{}.{}.{}.{}",
        ipv4 >> 24,
        (ipv4 >> 16) & 0xff,
        (ipv4 >> 8) & 0xff,
        ipv4 & 0xff
    ))
}

/// Simplified `url::CanonicalizeHostVerbose`. Returns `None` where the C++
/// reports a broken host (the caller then returns the input unchanged).
fn canonicalize_host(name: &[u8]) -> Option<String> {
    let text = String::from_utf8(percent_decode(name)).ok()?;

    if let Some(inner) = text.strip_prefix('[') {
        let inner = inner.strip_suffix(']')?;
        let ip = IPAddress::from_ip_literal(inner)?;
        return if ip.is_ipv6() {
            Some(format!("[{ip}]"))
        } else {
            None
        };
    }

    if ends_in_number(&text) {
        return canonical_ipv4(&text);
    }

    if text.is_ascii() {
        let lower = text.to_ascii_lowercase();
        return if has_forbidden_domain_code_point(&lower) {
            None
        } else {
            Some(lower)
        };
    }

    // Non-ASCII: lowercase each label and punycode-encode the ones that are
    // still non-ASCII.
    let mut out = String::with_capacity(text.len());
    for (i, label) in text.split('.').enumerate() {
        if i > 0 {
            out.push('.');
        }
        let lower: String = label.chars().flat_map(char::to_lowercase).collect();
        if lower.is_ascii() {
            out.push_str(&lower);
        } else {
            out.push_str("xn--");
            out.push_str(&punycode_encode(&lower)?);
        }
    }
    if has_forbidden_domain_code_point(&out) {
        return None;
    }
    Some(out)
}

const PUNY_BASE: u32 = 36;
const PUNY_TMIN: u32 = 1;
const PUNY_TMAX: u32 = 26;
const PUNY_SKEW: u32 = 38;
const PUNY_DAMP: u32 = 700;
const PUNY_INITIAL_BIAS: u32 = 72;
const PUNY_INITIAL_N: u32 = 128;

/// RFC 3492 punycode encoding of one label (without the `xn--` prefix).
/// Returns `None` on arithmetic overflow.
fn punycode_encode(input: &str) -> Option<String> {
    let code_points: Vec<u32> = input.chars().map(|c| c as u32).collect();
    let mut output: String = input.chars().filter(char::is_ascii).collect();
    let basic_count = output.len() as u32;
    let mut handled = basic_count;
    if basic_count > 0 {
        output.push('-');
    }

    let mut n = PUNY_INITIAL_N;
    let mut delta: u32 = 0;
    let mut bias = PUNY_INITIAL_BIAS;

    while (handled as usize) < code_points.len() {
        let m = code_points.iter().copied().filter(|&c| c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(handled + 1)?)?;
        n = m;

        for &c in &code_points {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = PUNY_BASE;
                loop {
                    let t = if k <= bias {
                        PUNY_TMIN
                    } else if k >= bias + PUNY_TMAX {
                        PUNY_TMAX
                    } else {
                        k - bias
                    };
                    if q < t {
                        break;
                    }
                    output.push(punycode_digit(t + (q - t) % (PUNY_BASE - t)));
                    q = (q - t) / (PUNY_BASE - t);
                    k += PUNY_BASE;
                }
                output.push(punycode_digit(q));
                bias = punycode_adapt(delta, handled + 1, handled == basic_count);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(output)
}

fn punycode_digit(d: u32) -> char {
    if d < 26 {
        (b'a' + d as u8) as char
    } else {
        (b'0' + (d - 26) as u8) as char
    }
}

fn punycode_adapt(mut delta: u32, num_points: u32, first_time: bool) -> u32 {
    delta /= if first_time { PUNY_DAMP } else { 2 };
    delta += delta / num_points;
    let mut k = 0;
    while delta > ((PUNY_BASE - PUNY_TMIN) * PUNY_TMAX) / 2 {
        delta /= PUNY_BASE - PUNY_TMIN;
        k += PUNY_BASE;
    }
    k + (((PUNY_BASE - PUNY_TMIN + 1) * delta) / (delta + PUNY_SKEW))
}

#[cfg(test)]
mod tests;
