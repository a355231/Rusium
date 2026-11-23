// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::IPAddress` and its helpers
//! (net/base/ip_address.{h,cc}).
//!
//! Provides a fixed-size byte container for IPv4 (4 bytes) and IPv6 (16 bytes)
//! addresses, literal parsing and canonical formatting, classification
//! predicates (publicly routable, multicast, loopback, link-local, unique
//! local), IPv4-mapped IPv6 conversion, prefix matching, CIDR parsing, and
//! NAT64/DNS64 prefix helpers.
//!
//! The C++ code delegates literal parsing and formatting to
//! `url::IPv4AddressToNumber`, `url::IPv6AddressToNumber`,
//! `url::AppendIPv4Address` and `url::AppendIPv6Address` (url/url_canon_ip.h).
//! Those rules are reimplemented here. Literal parsing is `const fn`, which
//! mirrors the C++ `constexpr` constructors. The rules are the same ones the
//! url code uses, and not `std::net` parsing:
//! - IPv4 components may be decimal, octal (leading `0`) or hexadecimal
//!   (leading `0x`). A trailing `.` is allowed, and values must fit in their
//!   field, including a last component that fills the remaining bytes.
//! - IPv6 literals are hex groups with at most one `::`. An embedded IPv4 tail
//!   must have exactly four components and no trailing dot.
//! - Formatting uses RFC 5952 style: the longest run of at least two zero
//!   groups is compressed, and the first run wins a tie.
//!
//! Differences from the C++ version:
//! - Out-parameters become return values. `IPAddress::from_ip_literal`,
//!   `parse_cidr_block`, `parse_url_hostname_to_address`, `create_ipv4_mask`,
//!   etc. return `Option`. `IPAddress::assign_from_ip_literal` keeps the
//!   in-place form: it returns `bool` and clears the value on failure, as in C++.
//! - `ParseCIDRBlock` returns `None` on any failure. The C++ comment says
//!   `*ip_address` is cleared on failure, but the C++ code only clears it when
//!   the address part fails. When only the prefix length is invalid, C++ leaves
//!   the parsed address in the out-parameter. That state is not reproduced.
//! - C++ `CHECK`s are `assert!`s here, and `DCHECK`s are also `assert!`s, so
//!   they are checked in release builds too. Affected functions are
//!   `IPAddressBytes::from_slice`, `IPAddress::all_zeros`,
//!   `convert_ipv4_to_ipv4_mapped_ipv6`, `convert_ipv4_mapped_ipv6_to_ipv4`,
//!   `ip_address_matches_prefix`, `common_prefix_length` and
//!   `convert_ipv4_to_ipv4_embedded_ipv6`. The `try_*` variants return `None`
//!   instead of panicking where that is natural.
//! - `base::Value` is replaced by the small `Value` enum (string or integer),
//!   which is all `FromValue` and `ToValue` need.
//! - `ToValue` returns `None` for an invalid address. C++ `DCHECK`s and returns
//!   the empty string.
//! - `Dns64PrefixLength` variants are named `Bits32`..`Bits96`, and
//!   `ExtractPref64FromIpv4onlyArpaAAAA` returns `Invalid` for non-IPv6 input.
//!   C++ `DCHECK`s and then reads past the end of the address.
//! - `IPAddressBytes` has no `Resize`, `push_back` or `operator[]`. They were
//!   only used internally. `IPAddress` has no `CopyBytesToVector`. Use
//!   `bytes().as_slice().to_vec()`.
//! - `IPAddressStartsWith` takes a slice, not a fixed-size array reference.
//!   The behavior is the same, including the `false` result when the prefix is
//!   longer than the address.
//! - `IPAddress` implements `Hash`, `Ord` and `Display` (canonical string).
//!
//! Not ported:
//! - `EstimateMemoryUsage` (memory accounting is not part of this crate).
//! - `AbslHashValue` (use `std::hash::Hash`).
//! - The socket conversions in ip_address_util.h (`in_addr`, `in6_addr`,
//!   `sockaddr`). They are not in ip_address.h.
//! - `ParseIPLiteralToBytes` is internal and is only exposed through
//!   `IPAddress::from_ip_literal` and `assign_from_ip_literal`.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[cfg(test)]
mod tests;

/// Maximum number of bytes in an address (`IPAddressBytes` storage).
const MAX_LEN: usize = 16;
const IPV4_LEN: usize = 4;
const IPV6_LEN: usize = 16;

/// Prefix of IPv4-mapped IPv6 addresses (RFC 4291 section 2.5.5.2).
const IPV4_MAPPED_PREFIX: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF];

/// Stand-in for `base::Value`. Only the cases the IP helpers use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Integer(i64),
    String(String),
}

/// A sequence of 0 to 16 bytes, representing the bytes of an IP address.
///
/// Bytes past `len()` are always zero, so derived-style equality and hashing
/// only need to look at the first `len()` bytes.
#[derive(Clone, Copy, Default, Debug)]
pub struct IPAddressBytes {
    bytes: [u8; MAX_LEN],
    len: u8,
}

impl IPAddressBytes {
    /// Creates an empty sequence.
    pub const fn new() -> Self {
        Self { bytes: [0; MAX_LEN], len: 0 }
    }

    const fn from_array(bytes: [u8; MAX_LEN], len: usize) -> Self {
        Self { bytes, len: len as u8 }
    }

    /// Copies `data` into a new sequence.
    ///
    /// Panics if `data` is longer than 16 bytes (C++ `CHECK_GE(16u, size)`).
    pub fn from_slice(data: &[u8]) -> Self {
        Self::try_from_slice(data).expect("IPAddressBytes holds at most 16 bytes")
    }

    /// Copies `data` into a new sequence. Returns `None` if it is longer than
    /// 16 bytes.
    pub fn try_from_slice(data: &[u8]) -> Option<Self> {
        if data.len() > MAX_LEN {
            return None;
        }
        let mut bytes = [0; MAX_LEN];
        bytes[..data.len()].copy_from_slice(data);
        Some(Self::from_array(bytes, data.len()))
    }

    /// Replaces the contents with a copy of `data`.
    ///
    /// Panics if `data` is longer than 16 bytes.
    pub fn assign(&mut self, data: &[u8]) {
        *self = Self::from_slice(data);
    }

    /// Returns the number of bytes.
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns true if there are no bytes.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the bytes as a slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len()]
    }

    /// Returns the bytes as a mutable slice. The length does not change.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        let n = self.len();
        &mut self.bytes[..n]
    }

    /// Appends `data`. Returns false, and changes nothing, if the result would
    /// be longer than 16 bytes.
    #[must_use]
    pub fn try_append(&mut self, data: &[u8]) -> bool {
        let start = self.len();
        if data.len() > MAX_LEN - start {
            return false;
        }
        self.bytes[start..start + data.len()].copy_from_slice(data);
        self.len += data.len() as u8;
        true
    }

    /// Appends `data`. Panics if the result would be longer than 16 bytes
    /// (C++ `CHECK_LE`).
    pub fn append(&mut self, data: &[u8]) {
        assert!(self.try_append(data), "IPAddressBytes holds at most 16 bytes");
    }
}

impl PartialEq for IPAddressBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for IPAddressBytes {}

impl PartialOrd for IPAddressBytes {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IPAddressBytes {
    // Shorter sequences sort first. Sequences of the same length compare
    // lexicographically, as C++ `IPAddressBytes::operator<` does.
    fn cmp(&self, other: &Self) -> Ordering {
        self.len().cmp(&other.len()).then_with(|| self.as_slice().cmp(other.as_slice()))
    }
}

impl Hash for IPAddressBytes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

/// Lookup for the DNS64 / NAT64 prefix length, see RFC 6052 section 2.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dns64PrefixLength {
    Bits32,
    Bits40,
    Bits48,
    Bits56,
    Bits64,
    Bits96,
    Invalid,
}

/// Represents an IPv4 or IPv6 address, or an address of another length.
///
/// An address of length 4 is IPv4 and length 16 is IPv6. Other lengths are
/// accepted but are not valid addresses.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IPAddress {
    ip_address: IPAddressBytes,
}

impl IPAddress {
    /// Size in bytes of an IPv4 address.
    pub const IPV4_ADDRESS_SIZE: usize = IPV4_LEN;
    /// Size in bytes of an IPv6 address.
    pub const IPV6_ADDRESS_SIZE: usize = IPV6_LEN;

    /// Parses an IPv4 or IPv6 literal, with no brackets and no surrounding
    /// whitespace. Returns `None` if the literal is not valid.
    pub const fn from_ip_literal(literal: &str) -> Option<Self> {
        match parse_ip_literal(literal.as_bytes()) {
            Some(ip_address) => Some(Self { ip_address }),
            None => None,
        }
    }

    /// Parses an address from a string `Value`. Returns `None` for other
    /// value types and for malformed strings.
    pub fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Self::from_ip_literal(s),
            Value::Integer(_) => None,
        }
    }

    /// Copies `bytes` into a new address. Panics if there are more than 16
    /// bytes (C++ `CHECK_GE(16u, size)`).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self { ip_address: IPAddressBytes::from_slice(bytes) }
    }

    /// Copies `bytes` into a new address. Returns `None` if there are more than
    /// 16 bytes.
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        IPAddressBytes::try_from_slice(bytes).map(|ip_address| Self { ip_address })
    }

    /// Returns true if the address has 4 bytes.
    pub const fn is_ipv4(&self) -> bool {
        self.ip_address.len() == IPV4_LEN
    }

    /// Returns true if the address has 16 bytes.
    pub const fn is_ipv6(&self) -> bool {
        self.ip_address.len() == IPV6_LEN
    }

    /// Returns true if the address is IPv4 or IPv6. This only checks the length.
    pub const fn is_valid(&self) -> bool {
        self.is_ipv4() || self.is_ipv6()
    }

    /// Returns true if the address is not in a range reserved by IANA for
    /// local or special use. IPv4-mapped IPv6 addresses are checked against
    /// the IPv4 ranges. Addresses of other lengths return true, as in C++.
    pub fn is_publicly_routable(&self) -> bool {
        if self.is_ipv4() {
            publicly_routable_ipv4(self.raw())
        } else if self.is_ipv6() {
            publicly_routable_ipv6(self.raw())
        } else {
            true
        }
    }

    /// Returns true for 224.0.0.0/4, IPv4-mapped 224.0.0.0/4, and ff00::/8.
    pub fn is_multicast(&self) -> bool {
        if self.is_ipv4() {
            return (224..=239).contains(&self.raw()[0]);
        }
        if self.is_ipv4_mapped_ipv6() {
            return (224..=239).contains(&self.raw()[12]);
        }
        if self.is_ipv6() {
            return self.raw()[0] == 0xFF;
        }
        false
    }

    /// Returns true if every byte is zero. An empty address returns false.
    pub fn is_zero(&self) -> bool {
        !self.is_empty() && self.raw().iter().all(|&x| x == 0)
    }

    /// Returns true if the address is IPv6 and starts with `::ffff:0:0/96`.
    pub fn is_ipv4_mapped_ipv6(&self) -> bool {
        self.is_ipv6() && ip_address_starts_with(self, &IPV4_MAPPED_PREFIX)
    }

    /// Returns true for 127.0.0.0/8 and ::1/128.
    pub fn is_loopback(&self) -> bool {
        if self.is_ipv4() {
            return self.raw()[0] == 127;
        }
        if self.is_ipv6() {
            let raw = self.raw();
            return raw[..IPV6_LEN - 1].iter().all(|&b| b == 0) && raw[IPV6_LEN - 1] == 1;
        }
        false
    }

    /// Returns true for 169.254.0.0/16, ::ffff:169.254.0.0/112 and fe80::/10.
    pub fn is_link_local(&self) -> bool {
        let raw = self.raw();
        if self.is_ipv4() {
            return raw[0] == 169 && raw[1] == 254;
        }
        if self.is_ipv4_mapped_ipv6() {
            return raw[12] == 169 && raw[13] == 254;
        }
        if self.is_ipv6() {
            return raw[0] == 0xFE && (raw[1] & 0xC0) == 0x80;
        }
        false
    }

    /// Returns true for fc00::/7.
    pub fn is_unique_local_ipv6(&self) -> bool {
        self.is_ipv6() && (self.raw()[0] & 0xFE) == 0xFC
    }

    /// Returns the number of bytes in the address.
    pub const fn size(&self) -> usize {
        self.ip_address.len()
    }

    /// Returns true if the address has no bytes.
    pub const fn is_empty(&self) -> bool {
        self.ip_address.is_empty()
    }

    /// Returns the underlying bytes.
    pub const fn bytes(&self) -> &IPAddressBytes {
        &self.ip_address
    }

    fn raw(&self) -> &[u8] {
        self.ip_address.as_slice()
    }

    /// Serializes to a string value. Returns `None` for an invalid address.
    pub fn to_value(&self) -> Option<Value> {
        if self.is_valid() {
            Some(Value::String(self.to_string()))
        } else {
            None
        }
    }

    /// Parses `literal` in place. On failure the address is cleared to empty,
    /// and the method returns false.
    #[must_use]
    pub fn assign_from_ip_literal(&mut self, literal: &str) -> bool {
        match parse_ip_literal(literal.as_bytes()) {
            Some(ip_address) => {
                self.ip_address = ip_address;
                true
            }
            None => {
                self.ip_address = IPAddressBytes::new();
                false
            }
        }
    }

    /// Returns an address made of `num_zero_bytes` zero bytes. Panics if
    /// `num_zero_bytes` is greater than 16.
    pub fn all_zeros(num_zero_bytes: usize) -> Self {
        assert!(num_zero_bytes <= MAX_LEN, "at most 16 bytes");
        Self::from_bytes(&[0u8; MAX_LEN][..num_zero_bytes])
    }

    /// Returns 127.0.0.1.
    pub fn ipv4_localhost() -> Self {
        Self::from([127, 0, 0, 1])
    }

    /// Returns ::1.
    pub fn ipv6_localhost() -> Self {
        let mut bytes = [0u8; IPV6_LEN];
        bytes[IPV6_LEN - 1] = 1;
        Self::from(bytes)
    }

    /// Returns 0.0.0.0.
    pub fn ipv4_all_zeros() -> Self {
        Self::all_zeros(IPV4_LEN)
    }

    /// Returns ::.
    pub fn ipv6_all_zeros() -> Self {
        Self::all_zeros(IPV6_LEN)
    }

    /// Returns an IPv4 mask with `mask_prefix_length` leading one bits, or
    /// `None` if the prefix is longer than 32 bits.
    pub fn create_ipv4_mask(mask_prefix_length: usize) -> Option<Self> {
        create_mask(mask_prefix_length, IPV4_LEN)
    }

    /// Returns an IPv6 mask with `mask_prefix_length` leading one bits, or
    /// `None` if the prefix is longer than 128 bits.
    pub fn create_ipv6_mask(mask_prefix_length: usize) -> Option<Self> {
        create_mask(mask_prefix_length, IPV6_LEN)
    }
}

impl From<[u8; IPV4_LEN]> for IPAddress {
    fn from(bytes: [u8; IPV4_LEN]) -> Self {
        Self::from_bytes(&bytes)
    }
}

impl From<[u8; IPV6_LEN]> for IPAddress {
    fn from(bytes: [u8; IPV6_LEN]) -> Self {
        Self::from_bytes(&bytes)
    }
}

impl From<IPAddressBytes> for IPAddress {
    fn from(ip_address: IPAddressBytes) -> Self {
        Self { ip_address }
    }
}

/// Canonical string form: dotted decimal for IPv4, RFC 5952 style for IPv6.
/// Returns the empty string for addresses that are neither.
impl fmt::Display for IPAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_ipv4() {
            f.write_str(&format_ipv4(self.raw()))
        } else if self.is_ipv6() {
            f.write_str(&format_ipv6(self.raw()))
        } else {
            Ok(())
        }
    }
}

/// A list of addresses.
pub type IPAddressList = Vec<IPAddress>;

/// Formats `address` with its port: "192.168.0.1:99" or "[::1]:80". Returns the
/// empty string for an invalid address.
pub fn ip_address_to_string_with_port(address: &IPAddress, port: u16) -> String {
    let address_str = address.to_string();
    if address_str.is_empty() {
        return address_str;
    }
    if address.is_ipv6() {
        format!("[{address_str}]:{port}")
    } else {
        format!("{address_str}:{port}")
    }
}

/// Converts an IPv4 address to an IPv4-mapped IPv6 address, such as
/// 192.168.0.1 to ::ffff:192.168.0.1. Panics if `address` is not IPv4.
pub fn convert_ipv4_to_ipv4_mapped_ipv6(address: &IPAddress) -> IPAddress {
    assert!(address.is_ipv4(), "address must be IPv4");
    let mut bytes = IPAddressBytes::new();
    bytes.append(&IPV4_MAPPED_PREFIX);
    bytes.append(address.raw());
    IPAddress::from(bytes)
}

/// Converts an IPv4-mapped IPv6 address to IPv4. Panics if `address` is not
/// IPv4-mapped.
pub fn convert_ipv4_mapped_ipv6_to_ipv4(address: &IPAddress) -> IPAddress {
    assert!(address.is_ipv4_mapped_ipv6(), "address must be IPv4-mapped");
    IPAddress::from_bytes(&address.raw()[IPV4_MAPPED_PREFIX.len()..])
}

/// Returns true if `ip_address` matches the block (`ip_prefix`,
/// `prefix_length_in_bits`).
///
/// IPv4 and IPv6 addresses are compared by converting the IPv4 side to
/// IPv4-mapped IPv6. Panics if either address is invalid or if the prefix is
/// longer than `ip_prefix`.
pub fn ip_address_matches_prefix(
    ip_address: &IPAddress,
    ip_prefix: &IPAddress,
    prefix_length_in_bits: usize,
) -> bool {
    assert!(ip_address.is_valid(), "ip_address must be valid");
    assert!(ip_prefix.is_valid(), "ip_prefix must be valid");
    assert!(prefix_length_in_bits <= ip_prefix.size() * 8, "prefix longer than the address");

    if ip_address.size() != ip_prefix.size() {
        return if ip_address.is_ipv4() {
            ip_address_matches_prefix(
                &convert_ipv4_to_ipv4_mapped_ipv6(ip_address),
                ip_prefix,
                prefix_length_in_bits,
            )
        } else {
            ip_address_matches_prefix(
                ip_address,
                &convert_ipv4_to_ipv4_mapped_ipv6(ip_prefix),
                96 + prefix_length_in_bits,
            )
        };
    }
    prefix_check(ip_address.raw(), ip_prefix.raw(), prefix_length_in_bits)
}

/// Parses CIDR notation, for example "10.10.3.1/20", "a:b:c::/46" or
/// "::1/128". Whitespace around each part is ignored, and the prefix must be
/// plain decimal digits no larger than the address size in bits.
///
/// Returns the address and prefix length on success.
#[must_use]
pub fn parse_cidr_block(cidr_literal: &str) -> Option<(IPAddress, usize)> {
    let (address_part, prefix_part) = split_cidr(cidr_literal)?;
    let ip_address = IPAddress::from_ip_literal(address_part)?;
    let prefix_length = parse_prefix_length(prefix_part, ip_address.size() * 8)?;
    Some((ip_address, prefix_length))
}

/// Like `parse_cidr_block`, but the address part must be a URL hostname form:
/// an IPv4 literal, or an IPv6 literal in square brackets, such as
/// "[::ffff:192.168.0.1]/112".
#[must_use]
pub fn parse_cidr_block_non_standard_url_format(cidr_literal: &str) -> Option<(IPAddress, usize)> {
    let (address_part, prefix_part) = split_cidr(cidr_literal)?;
    let ip_address = parse_url_hostname_to_address(address_part)?;
    let prefix_length = parse_prefix_length(prefix_part, ip_address.size() * 8)?;
    Some((ip_address, prefix_length))
}

/// Parses a URL hostname that is an IP literal (RFC 3986 section 3.2.2). The
/// input must be an IPv4 literal, or an IPv6 literal in square brackets.
#[must_use]
pub fn parse_url_hostname_to_address(hostname: &str) -> Option<IPAddress> {
    let bytes = hostname.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'[' && bytes[bytes.len() - 1] == b']' {
        let ip_address = IPAddress::from_ip_literal(&hostname[1..hostname.len() - 1])?;
        return if ip_address.is_ipv6() { Some(ip_address) } else { None };
    }

    let ip_address = IPAddress::from_ip_literal(hostname)?;
    if ip_address.is_ipv4() {
        Some(ip_address)
    } else {
        None
    }
}

/// Returns the number of leading bits that `a1` and `a2` have in common.
/// Panics if the two addresses have different sizes.
pub fn common_prefix_length(a1: &IPAddress, a2: &IPAddress) -> usize {
    assert_eq!(a1.size(), a2.size(), "addresses must have the same size");
    for (i, (x, y)) in a1.raw().iter().zip(a2.raw()).enumerate() {
        let diff = x ^ y;
        if diff != 0 {
            return i * 8 + diff.leading_zeros() as usize;
        }
    }
    a1.size() * 8
}

/// Returns the number of leading one bits in `mask`.
pub fn mask_prefix_length(mask: &IPAddress) -> usize {
    let ones = IPAddress::from_bytes(&vec![0xFF; mask.size()]);
    common_prefix_length(mask, &ones)
}

/// Returns true if `address` starts with the bytes in `prefix`. Unlike
/// `ip_address_matches_prefix`, this does no IPv4-to-IPv6 conversion and only
/// compares whole bytes. A prefix longer than the address never matches.
pub fn ip_address_starts_with(address: &IPAddress, prefix: &[u8]) -> bool {
    let raw = address.raw();
    raw.len() >= prefix.len() && &raw[..prefix.len()] == prefix
}

/// Extracts the NAT64 translation prefix length from an IPv6 address that
/// holds the well-known IPv4-only address (192.0.0.170 or 192.0.0.171) from
/// ipv4only.arpa, as defined in RFC 7050. Returns `Invalid` if no such
/// address is found, or if `address` is not IPv6.
pub fn extract_pref64_from_ipv4only_arpa_aaaa(address: &IPAddress) -> Dns64PrefixLength {
    const ARPA0: [u8; IPV4_LEN] = [192, 0, 0, 170];
    const ARPA1: [u8; IPV4_LEN] = [192, 0, 0, 171];
    fn is_arpa(candidate: &[u8]) -> bool {
        candidate == &ARPA0[..] || candidate == &ARPA1[..]
    }

    if !address.is_ipv6() {
        return Dns64PrefixLength::Invalid;
    }
    let s = address.raw();
    if is_arpa(&s[12..16]) {
        return Dns64PrefixLength::Bits96;
    }
    if is_arpa(&s[9..13]) {
        return Dns64PrefixLength::Bits64;
    }
    if is_arpa(&[s[7], s[9], s[10], s[11]]) {
        return Dns64PrefixLength::Bits56;
    }
    if is_arpa(&[s[6], s[7], s[9], s[10]]) {
        return Dns64PrefixLength::Bits48;
    }
    if is_arpa(&[s[5], s[6], s[7], s[9]]) {
        return Dns64PrefixLength::Bits40;
    }
    if is_arpa(&s[4..8]) {
        return Dns64PrefixLength::Bits32;
    }
    Dns64PrefixLength::Invalid
}

/// Embeds `ipv4_address` into `ipv6_address` at the given NAT64 prefix length.
/// For example, 192.168.0.1 with 64:ff9b::/96 gives 64:ff9b::192.168.0.1.
/// Returns a copy of `ipv4_address` when `prefix_length` is `Invalid`.
/// Panics if the address families are not IPv4 and IPv6.
pub fn convert_ipv4_to_ipv4_embedded_ipv6(
    ipv4_address: &IPAddress,
    ipv6_address: &IPAddress,
    prefix_length: Dns64PrefixLength,
) -> IPAddress {
    assert!(ipv4_address.is_ipv4(), "ipv4_address must be IPv4");
    assert!(ipv6_address.is_ipv6(), "ipv6_address must be IPv6");

    let v6 = ipv6_address.raw();
    let v4 = ipv4_address.raw();
    let mut out: Vec<u8> = Vec::with_capacity(MAX_LEN);
    match prefix_length {
        Dns64PrefixLength::Bits96 => {
            out.extend_from_slice(&v6[..12]);
            out.extend_from_slice(v4);
        }
        Dns64PrefixLength::Bits64 => {
            out.extend_from_slice(&v6[..8]);
            out.push(0);
            out.extend_from_slice(v4);
            out.extend_from_slice(&[0; 3]);
        }
        Dns64PrefixLength::Bits56 => {
            out.extend_from_slice(&v6[..7]);
            out.extend_from_slice(&v4[..1]);
            out.push(0);
            out.extend_from_slice(&v4[1..]);
            out.extend_from_slice(&[0; 4]);
        }
        Dns64PrefixLength::Bits48 => {
            out.extend_from_slice(&v6[..6]);
            out.extend_from_slice(&v4[..2]);
            out.push(0);
            out.extend_from_slice(&v4[2..]);
            out.extend_from_slice(&[0; 5]);
        }
        Dns64PrefixLength::Bits40 => {
            out.extend_from_slice(&v6[..5]);
            out.extend_from_slice(&v4[..3]);
            out.push(0);
            out.extend_from_slice(&v4[3..]);
            out.extend_from_slice(&[0; 6]);
        }
        Dns64PrefixLength::Bits32 => {
            out.extend_from_slice(&v6[..4]);
            out.extend_from_slice(v4);
            out.extend_from_slice(&[0; 8]);
        }
        Dns64PrefixLength::Invalid => return *ipv4_address,
    }
    IPAddress::from_bytes(&out)
}

// ---------------------------------------------------------------------------
// Internals.
// ---------------------------------------------------------------------------

/// Checks whether the first `bits` bits of `addr` and `prefix` are equal.
/// Both slices must hold at least `ceil(bits / 8)` bytes.
fn prefix_check(addr: &[u8], prefix: &[u8], bits: usize) -> bool {
    let whole = bits / 8;
    if addr[..whole] != prefix[..whole] {
        return false;
    }
    let rem = bits % 8;
    if rem != 0 {
        let mask = 0xFFu8 << (8 - rem);
        if (addr[whole] & mask) != (prefix[whole] & mask) {
            return false;
        }
    }
    true
}

fn create_mask(prefix_length: usize, len: usize) -> Option<IPAddress> {
    if len != IPV4_LEN && len != IPV6_LEN {
        return None;
    }
    if prefix_length > len * 8 {
        return None;
    }
    let mut bytes = [0u8; MAX_LEN];
    let full = prefix_length / 8;
    bytes[..full].fill(0xFF);
    let rem = prefix_length % 8;
    if rem != 0 {
        bytes[full] = 0xFF << (8 - rem);
    }
    Some(IPAddress::from_bytes(&bytes[..len]))
}

/// Returns false if `addr` is in an IPv4 range reserved by IANA.
/// Sources: www.iana.org/assignments/iana-ipv4-special-registry/
fn publicly_routable_ipv4(addr: &[u8]) -> bool {
    const RESERVED_IPV4_RANGES: [([u8; 4], usize); 14] = [
        ([0, 0, 0, 0], 8),
        ([10, 0, 0, 0], 8),
        ([100, 64, 0, 0], 10),
        ([127, 0, 0, 0], 8),
        ([169, 254, 0, 0], 16),
        ([172, 16, 0, 0], 12),
        ([192, 0, 0, 0], 24),
        ([192, 0, 2, 0], 24),
        ([192, 88, 99, 0], 24),
        ([192, 168, 0, 0], 16),
        ([198, 18, 0, 0], 15),
        ([198, 51, 100, 0], 24),
        ([203, 0, 113, 0], 24),
        ([224, 0, 0, 0], 3),
    ];
    !RESERVED_IPV4_RANGES.iter().any(|(prefix, bits)| prefix_check(addr, prefix, *bits))
}

/// Returns true if `addr` is in an IPv6 global unicast or multicast range, or
/// is an IPv4-mapped address whose IPv4 part is publicly routable.
/// Sources: www.iana.org/assignments/ipv6-address-space/
fn publicly_routable_ipv6(addr: &[u8]) -> bool {
    const PUBLIC_IPV6_RANGES: [([u8; 2], usize); 2] = [
        // 2000::/3 -- Global Unicast
        ([0x20, 0], 3),
        // ff00::/8 -- Multicast
        ([0xFF, 0], 8),
    ];
    if PUBLIC_IPV6_RANGES.iter().any(|(prefix, bits)| prefix_check(addr, prefix, *bits)) {
        return true;
    }
    if addr.len() == IPV6_LEN && addr[..IPV4_MAPPED_PREFIX.len()] == IPV4_MAPPED_PREFIX {
        return publicly_routable_ipv4(&addr[IPV4_MAPPED_PREFIX.len()..]);
    }
    false
}

fn split_cidr(literal: &str) -> Option<(&str, &str)> {
    let mut parts = literal.split('/').map(trim_ascii_whitespace);
    let first = parts.next()?;
    let second = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((first, second))
}

/// Parses a non-negative decimal prefix length no larger than `max_bits`.
fn parse_prefix_length(digits: &str, max_bits: usize) -> Option<usize> {
    let bytes = digits.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut value: u64 = 0;
    for &b in bytes {
        value = value * 10 + u64::from(b - b'0');
        if value > u64::from(u32::MAX) {
            return None;
        }
    }
    if value > max_bits as u64 {
        None
    } else {
        Some(value as usize)
    }
}

/// Trims the ASCII whitespace characters that `base::kWhitespaceASCII` lists:
/// tab, line feed, vertical tab, form feed, carriage return and space.
fn trim_ascii_whitespace(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' '))
}

// Formatting.

fn format_ipv4(addr: &[u8]) -> String {
    format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
}

/// Returns the longest run of zero 16-bit groups as (byte_begin, byte_len),
/// with a length of 0 meaning there is none. Only runs of at least two groups
/// qualify, and ties go to the first run.
fn choose_ipv6_contraction(addr: &[u8]) -> (usize, usize) {
    let mut best = (0usize, 0usize);
    let mut cur: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < IPV6_LEN {
        let is_zero = addr[i] == 0 && addr[i + 1] == 0;
        if is_zero {
            let (begin, len) = cur.unwrap_or((i, 0));
            cur = Some((begin, len + 2));
        }
        if !is_zero || i == IPV6_LEN - 2 {
            if let Some((begin, len)) = cur {
                if len > 2 && len > best.1 {
                    best = (begin, len);
                }
            }
            cur = None;
        }
        i += 2;
    }
    best
}

fn format_ipv6(addr: &[u8]) -> String {
    let (contraction_begin, contraction_len) = choose_ipv6_contraction(addr);
    let mut out = String::new();
    let mut i = 0;
    while i < IPV6_LEN - 1 {
        if contraction_len > 0 && i == contraction_begin {
            if i == 0 {
                out.push(':');
            }
            out.push(':');
            i = contraction_begin + contraction_len;
        } else {
            let x = (u16::from(addr[i]) << 8) | u16::from(addr[i + 1]);
            i += 2;
            out.push_str(&format!("{x:x}"));
            if i < IPV6_LEN {
                out.push(':');
            }
        }
    }
    out
}

// Literal parsing. Follows url::IPv4AddressToNumber / IPv6AddressToNumber.
//
// The parsers take byte slices and index ranges, not string slices, because
// they must be usable in `const` context. Range indexing of slices is not
// `const`, so each range is passed as (begin, end) instead.

#[derive(Clone, Copy)]
enum Kind {
    Dec,
    Oct,
    Hex,
}

#[derive(Clone, Copy)]
enum ComponentParse {
    Value(u32),
    /// Numeric but too large, or has a leading zero and an 8 or 9 digit.
    Broken,
    /// Not numeric at all.
    Neutral,
}

enum Ipv4Parse {
    /// A valid address and the number of components it was written with.
    Ok([u8; IPV4_LEN], usize),
    Broken,
    Neutral,
}

const fn is_dec(c: u8) -> bool {
    c.is_ascii_digit()
}

const fn is_hex(c: u8) -> bool {
    c.is_ascii_hexdigit()
}

const fn is_ipv4_char(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'.' | b'x' | b'X')
}

const fn char_is_of_kind(c: u8, kind: Kind) -> bool {
    match kind {
        Kind::Dec => is_dec(c),
        Kind::Oct => matches!(c, b'0'..=b'7'),
        Kind::Hex => is_hex(c),
    }
}

const fn digit_value(c: u8) -> u64 {
    match c {
        b'0'..=b'9' => (c - b'0') as u64,
        b'a'..=b'f' => (c - b'a' + 10) as u64,
        b'A'..=b'F' => (c - b'A' + 10) as u64,
        _ => 0,
    }
}

/// Converts one IPv4 component in `host[begin..end]` to a 32-bit number.
const fn ipv4_component_to_number(host: &[u8], begin: usize, end: usize) -> ComponentParse {
    let len = end - begin;
    if len == 0 {
        return ComponentParse::Neutral;
    }

    // Decimal unless there is a leading zero: "0x" is hex, another leading
    // zero is octal, and a lone "0" is decimal.
    let (kind, mut prefix) = if host[begin] != b'0' || len == 1 {
        (Kind::Dec, 0usize)
    } else if host[begin + 1] == b'x' || host[begin + 1] == b'X' {
        (Kind::Hex, 2)
    } else {
        (Kind::Oct, 1)
    };
    // Skip the rest of the leading zeros.
    while prefix < len && host[begin + prefix] == b'0' {
        prefix += 1;
    }

    let base: u64 = match kind {
        Kind::Dec => 10,
        Kind::Oct => 8,
        Kind::Hex => 16,
    };
    let mut value: u64 = 0;
    let mut may_be_broken_octal = false;
    let mut i = prefix;
    while i < len {
        let c = host[begin + i];
        if c >= 0x80 {
            return ComponentParse::Neutral;
        }
        if !char_is_of_kind(c, kind) {
            if is_dec(c) {
                // A decimal digit that is not valid in this base, such as an
                // 8 in an octal component.
                may_be_broken_octal = true;
            } else {
                return ComponentParse::Neutral;
            }
        }
        // Once the value is past u32::MAX it stays past it, so stop growing.
        if value <= u32::MAX as u64 {
            value = value * base + digit_value(c);
        }
        i += 1;
    }

    if may_be_broken_octal || value > u32::MAX as u64 {
        return ComponentParse::Broken;
    }
    ComponentParse::Value(value as u32)
}

/// Parses an IPv4 address written in `host[begin..end]`. A trailing dot is
/// ignored. Values are checked against the field width: the final component
/// fills the remaining bytes, and each earlier component must fit in a byte.
const fn parse_ipv4_range(host: &[u8], begin: usize, end: usize) -> Ipv4Parse {
    let mut end = end;
    if end > begin && host[end - 1] == b'.' {
        end -= 1;
    }
    if end == begin {
        return Ipv4Parse::Neutral;
    }

    // Components are read right to left, so that an early exit is possible
    // when the last one is not numeric. values[0] is the last component.
    let mut values = [0u32; 4];
    let mut existing = 0usize;
    let mut comp_end = end;
    let mut pos = end;
    loop {
        if pos != begin && host[pos - 1] != b'.' {
            pos -= 1;
            continue;
        }
        let value = match ipv4_component_to_number(host, pos, comp_end) {
            ComponentParse::Value(v) => v,
            ComponentParse::Neutral => {
                if existing == 0 {
                    return Ipv4Parse::Neutral;
                }
                return Ipv4Parse::Broken;
            }
            ComponentParse::Broken => return Ipv4Parse::Broken,
        };
        values[existing] = value;
        existing += 1;

        if pos == begin {
            break;
        }
        if existing == 4 {
            // More than four components.
            return Ipv4Parse::Broken;
        }
        comp_end = pos - 1;
        pos -= 1;
    }

    let mut address = [0u8; IPV4_LEN];
    // Every component except the last must fit in one byte.
    let mut i = existing - 1;
    while i > 0 {
        if values[i] > u8::MAX as u32 {
            return Ipv4Parse::Broken;
        }
        address[existing - i - 1] = values[i] as u8;
        i -= 1;
    }

    // The last component fills the remaining bytes, and must not have bits
    // left over.
    let mut last = values[0];
    let mut k = IPV4_LEN;
    while k >= existing {
        address[k - 1] = last as u8;
        last >>= 8;
        k -= 1;
    }
    if last != 0 {
        return Ipv4Parse::Broken;
    }

    Ipv4Parse::Ok(address, existing)
}

const fn parse_ipv4_literal(literal: &[u8]) -> Option<[u8; IPV4_LEN]> {
    match parse_ipv4_range(literal, 0, literal.len()) {
        Ipv4Parse::Ok(address, _) => Some(address),
        Ipv4Parse::Broken | Ipv4Parse::Neutral => None,
    }
}

/// Parsed layout of an IPv6 literal. Positions are indices into the literal,
/// without the brackets.
#[derive(Clone, Copy)]
struct Ipv6Parsed {
    hex_begin: [usize; 8],
    hex_len: [usize; 8],
    num_hex: usize,
    /// Index into the hex groups where the "::" sits, if any.
    contraction: Option<usize>,
    /// Byte range of an embedded IPv4 address, if any.
    ipv4: Option<(usize, usize)>,
}

/// Splits an IPv6 literal into its hex groups, "::" and IPv4 tail. Returns
/// `None` if the syntax is wrong.
const fn parse_ipv6_components(literal: &[u8]) -> Option<Ipv6Parsed> {
    let end = literal.len();
    if end == 0 {
        return None;
    }

    let mut parsed = Ipv6Parsed {
        hex_begin: [0; 8],
        hex_len: [0; 8],
        num_hex: 0,
        contraction: None,
        ipv4: None,
    };
    let mut cur_begin = 0usize;
    let mut i = 0usize;
    loop {
        let is_colon = i < end && literal[i] == b':';
        let is_contraction = is_colon && i + 1 < end && literal[i + 1] == b':';

        if is_colon || i == end {
            let comp_len = i - cur_begin;
            if comp_len > 4 {
                return None;
            }
            if comp_len == 0 {
                // Empty components are only allowed for a leading "::" or a
                // trailing one.
                let at_start = is_contraction && i == 0;
                let at_end = i == end
                    && match parsed.contraction {
                        Some(c) => c == parsed.num_hex,
                        None => false,
                    };
                if !(at_start || at_end) {
                    return None;
                }
            }
            if comp_len > 0 {
                if parsed.num_hex >= 8 {
                    return None;
                }
                parsed.hex_begin[parsed.num_hex] = cur_begin;
                parsed.hex_len[parsed.num_hex] = comp_len;
                parsed.num_hex += 1;
            }
        }

        if i == end {
            break;
        }

        if is_contraction {
            if parsed.contraction.is_some() {
                return None;
            }
            parsed.contraction = Some(parsed.num_hex);
            i += 1;
        }

        if is_colon {
            cur_begin = i + 1;
        } else {
            let c = literal[i];
            if c >= 0x80 {
                return None;
            }
            if !is_hex(c) {
                if is_ipv4_char(c) {
                    // An IPv4 tail must be at the end, so the rest of the
                    // literal is handed to the IPv4 parser.
                    parsed.ipv4 = Some((cur_begin, end));
                    break;
                }
                return None;
            }
        }
        i += 1;
    }
    Some(parsed)
}

const fn hex_component_value(literal: &[u8], begin: usize, len: usize) -> u16 {
    let mut value: u16 = 0;
    let mut k = 0;
    while k < len {
        value = value * 16 + digit_value(literal[begin + k]) as u16;
        k += 1;
    }
    value
}

/// Parses an IPv6 literal without brackets into 16 bytes, network order.
const fn parse_ipv6_address(literal: &[u8]) -> Option<[u8; MAX_LEN]> {
    let parsed = match parse_ipv6_components(literal) {
        Some(parsed) => parsed,
        None => return None,
    };

    // Each hex group is 16 bits, and an IPv4 tail is 32 bits. The "::" stands
    // for whatever is left, and must be at least 16 bits.
    let mut without_contraction = parsed.num_hex * 2;
    if parsed.ipv4.is_some() {
        without_contraction += IPV4_LEN;
    }
    let contraction_bytes = if parsed.contraction.is_some() {
        if without_contraction > MAX_LEN - 2 {
            2
        } else {
            MAX_LEN - without_contraction
        }
    } else {
        0
    };
    if without_contraction + contraction_bytes != MAX_LEN {
        return None;
    }

    let mut out = [0u8; MAX_LEN];
    let mut idx = 0usize;
    let mut i = 0usize;
    while i <= parsed.num_hex {
        let contraction_here = match parsed.contraction {
            Some(c) => c == i,
            None => false,
        };
        if contraction_here {
            // `out` is already zero, so only the index needs to move.
            idx += contraction_bytes;
        }
        if i != parsed.num_hex {
            let v = hex_component_value(literal, parsed.hex_begin[i], parsed.hex_len[i]);
            out[idx] = (v >> 8) as u8;
            out[idx + 1] = (v & 0xFF) as u8;
            idx += 2;
        }
        i += 1;
    }

    if let Some((begin, end)) = parsed.ipv4 {
        // The embedded address must be exactly four components with no
        // trailing dot.
        let trailing_dot = end > begin && literal[end - 1] == b'.';
        match parse_ipv4_range(literal, begin, end) {
            Ipv4Parse::Ok(address, count) => {
                if count != IPV4_LEN || trailing_dot {
                    return None;
                }
                let mut k = 0;
                while k < IPV4_LEN {
                    out[idx + k] = address[k];
                    k += 1;
                }
            }
            Ipv4Parse::Broken | Ipv4Parse::Neutral => return None,
        }
    }
    Some(out)
}

/// Parses an IP literal. A colon makes it IPv6, otherwise it is IPv4. Matches
/// `ParseIPLiteralToBytes` in the C++ header.
const fn parse_ip_literal(literal: &[u8]) -> Option<IPAddressBytes> {
    let mut has_colon = false;
    let mut i = 0;
    while i < literal.len() {
        if literal[i] == b':' {
            has_colon = true;
        }
        i += 1;
    }

    if has_colon {
        match parse_ipv6_address(literal) {
            Some(address) => Some(IPAddressBytes::from_array(address, IPV6_LEN)),
            None => None,
        }
    } else {
        match parse_ipv4_literal(literal) {
            Some(v4) => {
                let mut bytes = [0u8; MAX_LEN];
                let mut k = 0;
                while k < IPV4_LEN {
                    bytes[k] = v4[k];
                    k += 1;
                }
                Some(IPAddressBytes::from_array(bytes, IPV4_LEN))
            }
            None => None,
        }
    }
}
