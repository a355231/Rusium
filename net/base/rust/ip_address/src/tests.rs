// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of net/base/ip_address_unittest.cc, plus differential and extra tests.
//!
//! Each `mod` below corresponds to a C++ test suite. Test names are the C++
//! names in snake_case. The `differential` module compares against simple
//! bit-by-bit references and against std::net formatting. std::net parsing is
//! only used as a one-way check (std accepts => this crate accepts), never as
//! the definition of what is valid.

use super::*;

fn dump(ip: &IPAddress) -> String {
    ip.bytes().as_slice().iter().map(|b| b.to_string()).collect::<Vec<_>>().join(",")
}

fn ip(literal: &str) -> IPAddress {
    IPAddress::from_ip_literal(literal).expect("valid IP literal")
}

/// Small deterministic PRNG (xorshift64), used instead of the rand crate.
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

    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }

    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        std::array::from_fn(|_| self.byte())
    }
}

mod ip_address_bytes_test {
    use super::*;

    #[test]
    fn construct_empty() {
        let bytes = IPAddressBytes::new();
        assert_eq!(0, bytes.len());
    }

    #[test]
    fn construct_ipv4() {
        let data = [192u8, 168, 1, 1];
        let bytes = IPAddressBytes::from_slice(&data);
        assert_eq!(data.len(), bytes.len());
        let mut i = 0;
        for &byte in bytes.as_slice() {
            assert_eq!(data[i], byte);
            i += 1;
        }
        assert_eq!(data.len(), i);
    }

    #[test]
    fn construct_ipv6() {
        let data: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let bytes = IPAddressBytes::from_slice(&data);
        assert_eq!(data.len(), bytes.len());
        let mut i = 0;
        for &byte in bytes.as_slice() {
            assert_eq!(data[i], byte);
            i += 1;
        }
        assert_eq!(data.len(), i);
    }

    #[test]
    fn assign() {
        let data = [192u8, 168, 1, 1];
        let mut copy = IPAddressBytes::new();
        copy.assign(&data);
        assert_eq!(IPAddressBytes::from_slice(&data), copy);
    }

    #[test]
    fn rejects_more_than_16_bytes() {
        assert!(IPAddressBytes::try_from_slice(&[0u8; 17]).is_none());
        let mut full = IPAddressBytes::from_slice(&[0u8; 16]);
        assert!(!full.try_append(&[1]));
        assert_eq!(16, full.len());
    }
}

mod ip_address_test {
    use super::*;

    const RESERVED: bool = true;
    const NOT_RESERVED: bool = false;

    #[test]
    fn construct_ipv4() {
        assert_eq!("127.0.0.1", IPAddress::ipv4_localhost().to_string());

        let ipv4_ctor = IPAddress::from([192, 168, 1, 1]);
        assert_eq!("192.168.1.1", ipv4_ctor.to_string());
    }

    #[test]
    fn is_ip_version() {
        let ip_address1 = IPAddress::from_bytes(&[192, 168, 0, 1]);
        assert!(ip_address1.is_ipv4());
        assert!(!ip_address1.is_ipv6());

        let mut addr2 = [0u8; 16];
        addr2[..4].copy_from_slice(&[0xFE, 0xDC, 0xBA, 0x98]);
        let ip_address2 = IPAddress::from(addr2);
        assert!(ip_address2.is_ipv6());
        assert!(!ip_address2.is_ipv4());

        let ip_address3 = IPAddress::default();
        assert!(!ip_address3.is_ipv6());
        assert!(!ip_address3.is_ipv4());
    }

    #[test]
    fn is_valid() {
        let ip_address1 = IPAddress::from_bytes(&[192, 168, 0, 1]);
        assert!(ip_address1.is_valid());
        assert!(!ip_address1.is_empty());

        let mut addr2 = [0u8; 16];
        addr2[..4].copy_from_slice(&[0xFE, 0xDC, 0xBA, 0x98]);
        let ip_address2 = IPAddress::from(addr2);
        assert!(ip_address2.is_valid());
        assert!(!ip_address2.is_empty());

        let ip_address3 = IPAddress::from_bytes(&[0xFE, 0xDC, 0xBA, 0x98, 0]);
        assert!(!ip_address3.is_valid());
        assert!(!ip_address3.is_empty());

        let ip_address4 = IPAddress::default();
        assert!(!ip_address4.is_valid());
        assert!(ip_address4.is_empty());
    }

    #[test]
    fn is_publicly_routable_ipv4() {
        let tests = [
            // 0.0.0.0/8
            ("0.0.0.0", RESERVED),
            ("0.255.255.255", RESERVED),
            // Unreserved block(s)
            ("1.0.0.0", NOT_RESERVED),
            ("9.255.255.255", NOT_RESERVED),
            // 10.0.0.0/8
            ("10.0.0.0", RESERVED),
            ("10.255.255.255", RESERVED),
            // Unreserved block(s)
            ("11.0.0.0", NOT_RESERVED),
            ("100.63.255.255", NOT_RESERVED),
            // 100.64.0.0/10
            ("100.64.0.0", RESERVED),
            ("100.127.255.255", RESERVED),
            // Unreserved block(s)
            ("100.128.0.0", NOT_RESERVED),
            ("126.255.255.255", NOT_RESERVED),
            // 127.0.0.0/8
            ("127.0.0.0", RESERVED),
            ("127.255.255.255", RESERVED),
            // Unreserved block(s)
            ("128.0.0.0", NOT_RESERVED),
            ("169.253.255.255", NOT_RESERVED),
            // 169.254.0.0/16
            ("169.254.0.0", RESERVED),
            ("169.254.255.255", RESERVED),
            // Unreserved block(s)
            ("169.255.0.0", NOT_RESERVED),
            ("172.15.255.255", NOT_RESERVED),
            // 172.16.0.0/12
            ("172.16.0.0", RESERVED),
            ("172.31.255.255", RESERVED),
            // Unreserved block(s)
            ("172.32.0.0", NOT_RESERVED),
            ("191.255.255.255", NOT_RESERVED),
            // 192.0.0.0/24 (including sub ranges)
            ("192.0.0.0", RESERVED),
            ("192.0.0.255", RESERVED),
            // Unreserved block(s)
            ("192.0.1.0", NOT_RESERVED),
            ("192.0.1.255", NOT_RESERVED),
            // 192.0.2.0/24
            ("192.0.2.0", RESERVED),
            ("192.0.2.255", RESERVED),
            // Unreserved block(s)
            ("192.0.3.0", NOT_RESERVED),
            ("192.31.195.255", NOT_RESERVED),
            // 192.31.196.0/24
            ("192.31.196.0", NOT_RESERVED),
            ("192.31.196.255", NOT_RESERVED),
            // Unreserved block(s)
            ("192.32.197.0", NOT_RESERVED),
            ("192.52.192.255", NOT_RESERVED),
            // 192.52.193.0/24
            ("192.52.193.0", NOT_RESERVED),
            ("192.52.193.255", NOT_RESERVED),
            // Unreserved block(s)
            ("192.52.194.0", NOT_RESERVED),
            ("192.88.98.255", NOT_RESERVED),
            // 192.88.99.0/24
            ("192.88.99.0", RESERVED),
            ("192.88.99.255", RESERVED),
            // Unreserved block(s)
            ("192.88.100.0", NOT_RESERVED),
            ("192.167.255.255", NOT_RESERVED),
            // 192.168.0.0/16
            ("192.168.0.0", RESERVED),
            ("192.168.255.255", RESERVED),
            // Unreserved block(s)
            ("192.169.0.0", NOT_RESERVED),
            ("192.175.47.255", NOT_RESERVED),
            // 192.175.48.0/24
            ("192.175.48.0", NOT_RESERVED),
            ("192.175.48.255", NOT_RESERVED),
            // Unreserved block(s)
            ("192.175.49.0", NOT_RESERVED),
            ("198.17.255.255", NOT_RESERVED),
            // 198.18.0.0/15
            ("198.18.0.0", RESERVED),
            ("198.19.255.255", RESERVED),
            // Unreserved block(s)
            ("198.20.0.0", NOT_RESERVED),
            ("198.51.99.255", NOT_RESERVED),
            // 198.51.100.0/24
            ("198.51.100.0", RESERVED),
            ("198.51.100.255", RESERVED),
            // Unreserved block(s)
            ("198.51.101.0", NOT_RESERVED),
            ("203.0.112.255", NOT_RESERVED),
            // 203.0.113.0/24
            ("203.0.113.0", RESERVED),
            ("203.0.113.255", RESERVED),
            // Unreserved block(s)
            ("203.0.114.0", NOT_RESERVED),
            ("223.255.255.255", NOT_RESERVED),
            // 224.0.0.0/8 - 255.0.0.0/8
            ("224.0.0.0", RESERVED),
            ("255.255.255.255", RESERVED),
        ];

        for (literal, is_reserved) in tests {
            let mut address = IPAddress::default();
            assert!(address.assign_from_ip_literal(literal), "{literal}");
            assert!(address.is_valid());
            assert_eq!(!is_reserved, address.is_publicly_routable(), "{literal}");

            // Check these IPv4 addresses when mapped to IPv6. This verifies we're
            // properly unpacking mapped addresses.
            let mapped_address = super::super::convert_ipv4_to_ipv4_mapped_ipv6(&address);
            assert_eq!(!is_reserved, mapped_address.is_publicly_routable(), "{literal}");
        }
    }

    #[test]
    fn is_publicly_routable_ipv6() {
        let tests = [
            // 0000::/8.
            // Skip testing ::ffff:/96 explicitly since it was tested
            // in IsPubliclyRoutableIPv4
            ("0:0:0:0:0:0:0:0", RESERVED),
            ("ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 0100::/8
            ("100:0:0:0:0:0:0:0", RESERVED),
            ("1ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 0200::/7
            ("200:0:0:0:0:0:0:0", RESERVED),
            ("3ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 0400::/6
            ("400:0:0:0:0:0:0:0", RESERVED),
            ("7ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 0800::/5
            ("800:0:0:0:0:0:0:0", RESERVED),
            ("fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 1000::/4
            ("1000:0:0:0:0:0:0:0", RESERVED),
            ("1fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 2000::/3 (Global Unicast)
            ("2000:0:0:0:0:0:0:0", NOT_RESERVED),
            ("3fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", NOT_RESERVED),
            // 4000::/3
            ("4000:0:0:0:0:0:0:0", RESERVED),
            ("5fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 6000::/3
            ("6000:0:0:0:0:0:0:0", RESERVED),
            ("7fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // 8000::/3
            ("8000:0:0:0:0:0:0:0", RESERVED),
            ("9fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // c000::/3
            ("c000:0:0:0:0:0:0:0", RESERVED),
            ("dfff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // e000::/4
            ("e000:0:0:0:0:0:0:0", RESERVED),
            ("efff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // f000::/5
            ("f000:0:0:0:0:0:0:0", RESERVED),
            ("f7ff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // f800::/6
            ("f800:0:0:0:0:0:0:0", RESERVED),
            ("fbff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // fc00::/7
            ("fc00:0:0:0:0:0:0:0", RESERVED),
            ("fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // fe00::/9
            ("fe00:0:0:0:0:0:0:0", RESERVED),
            ("fe7f:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // fe80::/10
            ("fe80:0:0:0:0:0:0:0", RESERVED),
            ("febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // fec0::/10
            ("fec0:0:0:0:0:0:0:0", RESERVED),
            ("feff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", RESERVED),
            // ff00::/8 (Multicast)
            ("ff00:0:0:0:0:0:0:0", NOT_RESERVED),
            ("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", NOT_RESERVED),
        ];

        for (literal, is_reserved) in tests {
            let mut address = IPAddress::default();
            assert!(address.assign_from_ip_literal(literal), "{literal}");
            assert_eq!(!is_reserved, address.is_publicly_routable(), "{literal}");
        }
    }

    #[test]
    fn is_multicast() {
        let mut ipv4_multicast = IPAddress::default();
        assert!(ipv4_multicast.assign_from_ip_literal("224.0.0.1"));
        assert!(ipv4_multicast.is_multicast());

        let mut ipv4_last_multicast = IPAddress::default();
        assert!(ipv4_last_multicast.assign_from_ip_literal("239.255.255.255"));
        assert!(ipv4_last_multicast.is_multicast());

        let mut ipv4_non_multicast = IPAddress::default();
        assert!(ipv4_non_multicast.assign_from_ip_literal("223.255.255.255"));
        assert!(!ipv4_non_multicast.is_multicast());

        let mut ipv4_after_multicast = IPAddress::default();
        assert!(ipv4_after_multicast.assign_from_ip_literal("240.0.0.0"));
        assert!(!ipv4_after_multicast.is_multicast());

        let mut ipv6_multicast = IPAddress::default();
        assert!(ipv6_multicast.assign_from_ip_literal("ff02::1"));
        assert!(ipv6_multicast.is_multicast());

        let mut ipv6_non_multicast = IPAddress::default();
        assert!(ipv6_non_multicast.assign_from_ip_literal("fe80::1"));
        assert!(!ipv6_non_multicast.is_multicast());

        let mut ipv4_mapped_multicast = IPAddress::default();
        assert!(ipv4_mapped_multicast.assign_from_ip_literal("::ffff:224.0.0.1"));
        assert!(ipv4_mapped_multicast.is_multicast());

        let mut ipv4_mapped_last_multicast = IPAddress::default();
        assert!(ipv4_mapped_last_multicast.assign_from_ip_literal("::ffff:239.255.255.255"));
        assert!(ipv4_mapped_last_multicast.is_multicast());

        let mut ipv4_mapped_non_multicast = IPAddress::default();
        assert!(ipv4_mapped_non_multicast.assign_from_ip_literal("::ffff:223.255.255.255"));
        assert!(!ipv4_mapped_non_multicast.is_multicast());

        let mut ipv4_mapped_after_multicast = IPAddress::default();
        assert!(ipv4_mapped_after_multicast.assign_from_ip_literal("::ffff:240.0.0.0"));
        assert!(!ipv4_mapped_after_multicast.is_multicast());

        let invalid = IPAddress::default();
        assert!(!invalid.is_multicast());
    }

    #[test]
    fn is_loopback() {
        let ipv4_loopback = ip("127.0.0.1");
        assert!(ipv4_loopback.is_loopback());

        let ipv4_non_loopback = ip("128.0.0.1");
        assert!(!ipv4_non_loopback.is_loopback());

        let ipv6_loopback = ip("::1");
        assert!(ipv6_loopback.is_loopback());

        let ipv6_not_loopback_last_byte = ip("::2");
        assert!(!ipv6_not_loopback_last_byte.is_loopback());

        let ipv6_not_loopback_prefix = ip("1::1");
        assert!(!ipv6_not_loopback_prefix.is_loopback());

        let invalid = IPAddress::default();
        assert!(!invalid.is_loopback());
    }

    #[test]
    fn is_zero() {
        let zero_ipv4_address = IPAddress::from_bytes(&[0, 0, 0, 0]);
        assert!(zero_ipv4_address.is_zero());

        let non_zero_ipv4_address = IPAddress::from_bytes(&[10, 0, 0, 0]);
        assert!(!non_zero_ipv4_address.is_zero());

        let zero_ipv6_address = IPAddress::from([0u8; 16]);
        assert!(zero_ipv6_address.is_zero());

        let mut addr4 = [0u8; 16];
        addr4[0] = 10;
        let non_zero_ipv6_address = IPAddress::from(addr4);
        assert!(!non_zero_ipv6_address.is_zero());

        let empty_address = IPAddress::default();
        assert!(!empty_address.is_zero());
    }

    #[test]
    fn is_ipv4_mapped() {
        let ipv4_address = IPAddress::from([192, 168, 0, 1]);
        assert!(!ipv4_address.is_ipv4_mapped_ipv6());
        let ipv6_address = IPAddress::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert!(!ipv6_address.is_ipv4_mapped_ipv6());
        let mapped_address = IPAddress::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 1, 1, 0, 1]);
        assert!(mapped_address.is_ipv4_mapped_ipv6());
    }

    #[test]
    fn all_zeros() {
        assert!(IPAddress::all_zeros(0).is_empty());

        assert_eq!(3, IPAddress::all_zeros(3).size());
        assert!(IPAddress::all_zeros(3).is_zero());

        assert_eq!("0.0.0.0", IPAddress::ipv4_all_zeros().to_string());
        assert_eq!("::", IPAddress::ipv6_all_zeros().to_string());
    }

    #[test]
    fn to_string() {
        assert_eq!("0.0.0.0", IPAddress::ipv4_all_zeros().to_string());

        let address = IPAddress::from([192, 168, 0, 1]);
        assert_eq!("192.168.0.1", address.to_string());

        let address2 =
            IPAddress::from([0xFE, 0xDC, 0xBA, 0x98, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!("fedc:ba98::", address2.to_string());

        // ToString() shouldn't crash on invalid addresses.
        let address4 = IPAddress::from_bytes(&[0, 0]);
        assert_eq!("", address4.to_string());

        let address5 = IPAddress::default();
        assert_eq!("", address5.to_string());
    }

    #[test]
    fn ip_address_to_string_with_port() {
        assert_eq!(
            "0.0.0.0:3",
            super::super::ip_address_to_string_with_port(&IPAddress::ipv4_all_zeros(), 3)
        );

        let address1 = IPAddress::from([192, 168, 0, 1]);
        assert_eq!("192.168.0.1:99", super::super::ip_address_to_string_with_port(&address1, 99));

        let address2 =
            IPAddress::from([0xFE, 0xDC, 0xBA, 0x98, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            "[fedc:ba98::]:8080",
            super::super::ip_address_to_string_with_port(&address2, 8080)
        );

        // Shouldn't crash on invalid addresses.
        let addr3 = IPAddress::from_bytes(&[0, 0]);
        assert_eq!("", super::super::ip_address_to_string_with_port(&addr3, 8080));
    }

    // Test that invalid IP literals fail to parse.
    #[test]
    fn assign_from_ip_literal_fail_parse() {
        let mut address = IPAddress::default();

        assert!(!address.assign_from_ip_literal("bad value"));
        assert!(!address.assign_from_ip_literal("bad:value"));
        assert!(!address.assign_from_ip_literal(""));
        assert!(!address.assign_from_ip_literal("192.168.0.1:30"));
        assert!(!address.assign_from_ip_literal("  192.168.0.1  "));
        assert!(!address.assign_from_ip_literal("[::1]"));
    }

    // Test that a failure calling AssignFromIPLiteral() has the sideffect of
    // clearing the current value.
    #[test]
    fn assign_from_ip_literal_reset_on_failure() {
        let mut address = IPAddress::ipv6_localhost();

        assert!(address.is_valid());
        assert!(!address.is_empty());

        assert!(!address.assign_from_ip_literal("bad value"));

        assert!(!address.is_valid());
        assert!(address.is_empty());
    }

    // Test parsing an IPv4 literal.
    #[test]
    fn assign_from_ip_literal_ipv4() {
        let mut address = IPAddress::default();
        assert!(address.assign_from_ip_literal("192.168.0.1"));
        assert_eq!("192,168,0,1", dump(&address));
        assert_eq!("192.168.0.1", address.to_string());
    }

    // Test parsing an IPv6 literal.
    #[test]
    fn assign_from_ip_literal_ipv6() {
        let mut address = IPAddress::default();
        assert!(address.assign_from_ip_literal("1:abcd::3:4:ff"));
        assert_eq!("0,1,171,205,0,0,0,0,0,0,0,3,0,4,0,255", dump(&address));
        assert_eq!("1:abcd::3:4:ff", address.to_string());
    }

    #[test]
    fn is_ipv4_mapped_ipv6() {
        let ipv4_address = IPAddress::from([192, 168, 0, 1]);
        assert!(!ipv4_address.is_ipv4_mapped_ipv6());
        let ipv6_address = IPAddress::ipv6_localhost();
        assert!(!ipv6_address.is_ipv4_mapped_ipv6());
        let mapped_address = IPAddress::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 1, 1, 0, 1]);
        assert!(mapped_address.is_ipv4_mapped_ipv6());
    }

    #[test]
    fn is_equal() {
        let mut ip_address1 = IPAddress::default();
        assert!(ip_address1.assign_from_ip_literal("127.0.0.1"));
        let mut ip_address2 = IPAddress::default();
        assert!(ip_address2.assign_from_ip_literal("2001:db8:0::42"));
        let mut ip_address3 = IPAddress::default();
        assert!(ip_address3.assign_from_ip_literal("127.0.0.1"));

        assert!(ip_address1 != ip_address2);
        assert!(ip_address1 == ip_address3);
    }

    #[test]
    fn less_than() {
        // IPv4 vs IPv6
        let mut ip_address1 = IPAddress::default();
        assert!(ip_address1.assign_from_ip_literal("127.0.0.1"));
        let mut ip_address2 = IPAddress::default();
        assert!(ip_address2.assign_from_ip_literal("2001:db8:0::42"));
        assert!(ip_address1 < ip_address2);
        assert!(!(ip_address2 < ip_address1));

        // Compare equivalent addresses.
        let mut ip_address3 = IPAddress::default();
        assert!(ip_address3.assign_from_ip_literal("127.0.0.1"));
        assert!(!(ip_address1 < ip_address3));
        assert!(!(ip_address3 < ip_address1));

        let mut ip_address4 = IPAddress::default();
        assert!(ip_address4.assign_from_ip_literal("128.0.0.0"));
        assert!(ip_address1 < ip_address4);
        assert!(!(ip_address4 < ip_address1));
    }

    // Test mapping an IPv4 address to an IPv6 address.
    #[test]
    fn convert_ipv4_to_ipv4_mapped_ipv6() {
        let ipv4_address = IPAddress::from([192, 168, 0, 1]);
        let ipv6_address = super::super::convert_ipv4_to_ipv4_mapped_ipv6(&ipv4_address);

        // ::ffff:192.168.0.1
        assert_eq!("0,0,0,0,0,0,0,0,0,0,255,255,192,168,0,1", dump(&ipv6_address));
        assert_eq!("::ffff:c0a8:1", ipv6_address.to_string());
    }

    // Test reversal of a IPv6 address mapping.
    #[test]
    fn convert_ipv4_mapped_ipv6_to_ipv4() {
        let mut ipv4mapped_address = IPAddress::default();
        assert!(ipv4mapped_address.assign_from_ip_literal("::ffff:c0a8:1"));

        let expected = IPAddress::from([192, 168, 0, 1]);

        let result = super::super::convert_ipv4_mapped_ipv6_to_ipv4(&ipv4mapped_address);
        assert_eq!(expected, result);
    }

    #[test]
    fn ip_address_matches_prefix() {
        let tests: [(&str, usize, &str, bool); 9] = [
            // IPv4 prefix with IPv4 inputs.
            ("10.10.1.32", 27, "10.10.1.44", true),
            ("10.10.1.32", 27, "10.10.1.90", false),
            ("10.10.1.32", 27, "10.10.1.90", false),
            // IPv6 prefix with IPv6 inputs.
            ("2001:db8::", 32, "2001:DB8:3:4::5", true),
            ("2001:db8::", 32, "2001:c8::", false),
            // IPv6 prefix with IPv4 inputs.
            ("2001:db8::", 33, "192.168.0.1", false),
            ("::ffff:192.168.0.1", 112, "192.168.33.77", true),
            // IPv4 prefix with IPv6 inputs.
            ("10.11.33.44", 16, "::ffff:0a0b:89", true),
            ("10.11.33.44", 16, "::ffff:10.12.33.44", false),
        ];
        for (cidr_literal, prefix_length_in_bits, ip_literal, expected_to_match) in tests {
            let ip_address = ip(ip_literal);
            let ip_prefix = ip(cidr_literal);
            assert_eq!(
                expected_to_match,
                super::super::ip_address_matches_prefix(
                    &ip_address,
                    &ip_prefix,
                    prefix_length_in_bits
                ),
                "{cidr_literal}, {ip_literal}"
            );
        }
    }

    // Test parsing invalid CIDR notation literals.
    #[test]
    fn parse_cidr_block_invalid() {
        let bad_literals = [
            "foobar",
            "",
            "192.168.0.1",
            "::1",
            "/",
            "/1",
            "1",
            "192.168.1.1/-1",
            "192.168.1.1/33",
            "::1/-3",
            "a::3/129",
            "::1/x",
            "192.168.0.1//11",
            "192.168.1.1/+1",
            "192.168.1.1/ +1",
            "192.168.1.1/",
        ];

        for bad_literal in bad_literals {
            assert!(parse_cidr_block(bad_literal).is_none(), "{bad_literal}");
        }
    }

    // Test parsing a valid CIDR notation literal.
    #[test]
    fn parse_cidr_block_valid() {
        let (ip_address, prefix_length_in_bits) = parse_cidr_block("192.168.0.1/11").unwrap();
        assert_eq!("192,168,0,1", dump(&ip_address));
        assert_eq!(11, prefix_length_in_bits);

        let (ip_address, prefix_length_in_bits) =
            parse_cidr_block("::ffff:192.168.0.1/112").unwrap();
        assert_eq!("0,0,0,0,0,0,0,0,0,0,255,255,192,168,0,1", dump(&ip_address));
        assert_eq!(112, prefix_length_in_bits);

        let (ip_address, prefix_length_in_bits) = parse_cidr_block("192.168.0.1/32").unwrap();
        assert_eq!("192,168,0,1", dump(&ip_address));
        assert_eq!(32, prefix_length_in_bits);

        let (ip_address, prefix_length_in_bits) = parse_cidr_block("::1/128").unwrap();
        assert_eq!("0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1", dump(&ip_address));
        assert_eq!(128, prefix_length_in_bits);
    }

    // Test parsing invalid CIDR notation literals specific to the URL-Hostname
    // version of this function.
    #[test]
    fn parse_cidr_block_non_standard_url_format_invalid() {
        let bad_literals = [
            "foobar",
            "",
            "192.168.0.1",
            "::1",
            "/",
            "/1",
            "1",
            "192.168.1.1/-1",
            "[192.168.1.1]/16",
            "::1/10",
        ];

        for bad_literal in bad_literals {
            assert!(
                parse_cidr_block_non_standard_url_format(bad_literal).is_none(),
                "{bad_literal}"
            );
        }
    }

    // Test parsing a valid CIDR notation literal using the URLHostnameIP version of
    // ParseCIDRBlock.
    #[test]
    fn parse_cidr_block_non_standard_url_format_valid() {
        let (ip_address, prefix_length_in_bits) =
            parse_cidr_block_non_standard_url_format("192.168.0.1/11").unwrap();
        assert_eq!("192,168,0,1", dump(&ip_address));
        assert_eq!(11, prefix_length_in_bits);

        let (ip_address, prefix_length_in_bits) =
            parse_cidr_block_non_standard_url_format("[::ffff:192.168.0.1]/112").unwrap();
        assert_eq!("0,0,0,0,0,0,0,0,0,0,255,255,192,168,0,1", dump(&ip_address));
        assert_eq!(112, prefix_length_in_bits);
    }

    #[test]
    fn parse_url_hostname_to_address_fail_parse() {
        assert!(parse_url_hostname_to_address("bad value").is_none());
        assert!(parse_url_hostname_to_address("bad:value").is_none());
        assert!(parse_url_hostname_to_address("").is_none());
        assert!(parse_url_hostname_to_address("192.168.0.1:30").is_none());
        assert!(parse_url_hostname_to_address("  192.168.0.1  ").is_none());
        assert!(parse_url_hostname_to_address("::1").is_none());
        assert!(parse_url_hostname_to_address("[192.169.0.1]").is_none());
        assert!(parse_url_hostname_to_address("[]").is_none());
    }

    #[test]
    fn parse_url_hostname_to_address_ipv4() {
        let address = parse_url_hostname_to_address("192.168.0.1").unwrap();
        assert_eq!("192,168,0,1", dump(&address));
        assert_eq!("192.168.0.1", address.to_string());
    }

    #[test]
    fn parse_url_hostname_to_address_ipv6() {
        let address = parse_url_hostname_to_address("[1:abcd::3:4:ff]").unwrap();
        assert_eq!("0,1,171,205,0,0,0,0,0,0,0,3,0,4,0,255", dump(&address));
        assert_eq!("1:abcd::3:4:ff", address.to_string());
    }

    #[test]
    fn ip_address_starts_with() {
        let ipv4_address = IPAddress::from([192, 168, 10, 5]);

        let ipv4_prefix1 = [192, 168, 10];
        assert!(super::super::ip_address_starts_with(&ipv4_address, &ipv4_prefix1));

        let ipv4_prefix3 = [192, 168, 10, 5];
        assert!(super::super::ip_address_starts_with(&ipv4_address, &ipv4_prefix3));

        let ipv4_prefix2 = [192, 168, 10, 10];
        assert!(!super::super::ip_address_starts_with(&ipv4_address, &ipv4_prefix2));

        // Prefix is longer than the address.
        let ipv4_prefix4 = [192, 168, 10, 10, 0];
        assert!(!super::super::ip_address_starts_with(&ipv4_address, &ipv4_prefix4));

        let ipv6_address = ip("2a00:1450:400c:c09::64");

        let ipv6_prefix1 = [42, 0, 20, 80, 64, 12, 12, 9];
        assert!(super::super::ip_address_starts_with(&ipv6_address, &ipv6_prefix1));

        let ipv6_prefix2 = [41, 0, 20, 80, 64, 12, 12, 9, 0, 0, 0, 0, 0, 0, 100];
        assert!(!super::super::ip_address_starts_with(&ipv6_address, &ipv6_prefix2));

        let ipv6_prefix3 = [42, 0, 20, 80, 64, 12, 12, 9, 0, 0, 0, 0, 0, 0, 0, 100];
        assert!(super::super::ip_address_starts_with(&ipv6_address, &ipv6_prefix3));

        let ipv6_prefix4 = [42, 0, 20, 80, 64, 12, 12, 9, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(!super::super::ip_address_starts_with(&ipv6_address, &ipv6_prefix4));

        // Prefix is longer than the address.
        let ipv6_prefix5 = [42, 0, 20, 80, 64, 12, 12, 9, 0, 0, 0, 0, 0, 0, 0, 0, 10];
        assert!(!super::super::ip_address_starts_with(&ipv6_address, &ipv6_prefix5));
    }

    #[test]
    fn is_link_local() {
        let positive = [
            "169.254.0.0",
            "169.254.100.1",
            "169.254.100.1",
            "::ffff:169.254.0.0",
            "::ffff:169.254.100.1",
            "fe80::1",
            "fe81::1",
        ];
        for literal in positive {
            assert!(ip(literal).is_link_local(), "{literal}");
        }

        let negative = [
            "170.254.0.0",
            "169.255.0.0",
            "::169.254.0.0",
            "::fffe:169.254.0.0",
            "::ffff:169.255.0.0",
            "fec0::1",
        ];
        for literal in negative {
            assert!(!ip(literal).is_link_local(), "{literal}");
        }
    }

    #[test]
    fn is_unique_local_ipv6() {
        let positive = ["fc00::1", "fc80::1", "fd00::1"];
        for literal in positive {
            assert!(ip(literal).is_unique_local_ipv6(), "{literal}");
        }

        let negative = ["fe00::1", "ff00::1", "252.0.0.1"];
        for literal in negative {
            assert!(!ip(literal).is_unique_local_ipv6(), "{literal}");
        }
    }

    // Tests extraction of the NAT64 translation prefix.
    #[test]
    fn extract_pref64_from_ipv4only_arpa_aaaa() {
        use super::super::{extract_pref64_from_ipv4only_arpa_aaaa as extract, Dns64PrefixLength};

        // Well Known Prefix 64:ff9b::/96.
        let wkp_0 = IPAddress::from([0, 100, 255, 155, 0, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 170]);
        let wkp_1 = IPAddress::from([0, 100, 255, 155, 0, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 171]);
        assert_eq!(Dns64PrefixLength::Bits96, extract(&wkp_0));
        assert_eq!(Dns64PrefixLength::Bits96, extract(&wkp_1));

        // Prefix length 96
        let ipv6_address_96_0 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 0, 0, 0, 192, 0, 0, 170]);
        let ipv6_address_96_1 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 0, 0, 0, 192, 0, 0, 171]);
        assert_eq!(Dns64PrefixLength::Bits96, extract(&ipv6_address_96_0));
        assert_eq!(Dns64PrefixLength::Bits96, extract(&ipv6_address_96_1));

        // Prefix length 64
        let ipv6_address_64_0 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 192, 0, 0, 170, 0, 0, 0]);
        let ipv6_address_64_1 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 192, 0, 0, 171, 0, 0, 0]);
        assert_eq!(Dns64PrefixLength::Bits64, extract(&ipv6_address_64_0));
        assert_eq!(Dns64PrefixLength::Bits64, extract(&ipv6_address_64_1));

        // Prefix length 56
        let ipv6_address_56_0 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 192, 0, 0, 0, 170, 0, 0, 0, 0]);
        let ipv6_address_56_1 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 192, 0, 0, 0, 171, 0, 0, 0, 0]);
        assert_eq!(Dns64PrefixLength::Bits56, extract(&ipv6_address_56_0));
        assert_eq!(Dns64PrefixLength::Bits56, extract(&ipv6_address_56_1));

        // Prefix length 48
        let ipv6_address_48_0 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 192, 0, 0, 0, 170, 0, 0, 0, 0, 0]);
        let ipv6_address_48_1 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 192, 0, 0, 0, 171, 0, 0, 0, 0, 0]);
        assert_eq!(Dns64PrefixLength::Bits48, extract(&ipv6_address_48_0));
        assert_eq!(Dns64PrefixLength::Bits48, extract(&ipv6_address_48_1));

        // Prefix length 40
        let ipv6_address_40_0 =
            IPAddress::from([32, 1, 13, 184, 1, 192, 0, 0, 0, 170, 0, 0, 0, 0, 0, 0]);
        let ipv6_address_40_1 =
            IPAddress::from([32, 1, 13, 184, 1, 192, 0, 0, 0, 171, 0, 0, 0, 0, 0, 0]);
        assert_eq!(Dns64PrefixLength::Bits40, extract(&ipv6_address_40_0));
        assert_eq!(Dns64PrefixLength::Bits40, extract(&ipv6_address_40_1));

        // Prefix length 32
        let ipv6_address_32_0 =
            IPAddress::from([32, 1, 13, 184, 192, 0, 0, 170, 0, 0, 0, 0, 0, 0, 0, 0]);
        let ipv6_address_32_1 =
            IPAddress::from([32, 1, 13, 184, 192, 0, 0, 171, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(Dns64PrefixLength::Bits32, extract(&ipv6_address_32_0));
        assert_eq!(Dns64PrefixLength::Bits32, extract(&ipv6_address_32_1));
    }

    // Tests mapping an IPv4 address to an IPv6 address.
    #[test]
    fn convert_ipv4_to_ipv4_embedded_ipv6() {
        use super::super::{convert_ipv4_to_ipv4_embedded_ipv6 as embed, Dns64PrefixLength};

        let ipv4_address = IPAddress::from([192, 0, 2, 33]);

        // Well Known Prefix 64:ff9b::/96.
        let ipv6_address_wkp =
            IPAddress::from([0, 100, 255, 155, 0, 0, 0, 0, 0, 0, 0, 0, 192, 0, 0, 170]);
        let converted_wkp = embed(&ipv4_address, &ipv6_address_wkp, Dns64PrefixLength::Bits96);
        assert_eq!("0,100,255,155,0,0,0,0,0,0,0,0,192,0,2,33", dump(&converted_wkp));
        assert_eq!("64:ff9b::c000:221", converted_wkp.to_string());

        // Prefix length 96
        let ipv6_address_96 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_96 = embed(&ipv4_address, &ipv6_address_96, Dns64PrefixLength::Bits96);
        assert_eq!("32,1,13,184,1,34,3,68,0,0,0,0,192,0,2,33", dump(&converted_96));
        assert_eq!("2001:db8:122:344::c000:221", converted_96.to_string());

        // Prefix length 64
        let ipv6_address_64 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 68, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_64 = embed(&ipv4_address, &ipv6_address_64, Dns64PrefixLength::Bits64);
        assert_eq!("32,1,13,184,1,34,3,68,0,192,0,2,33,0,0,0", dump(&converted_64));
        assert_eq!("2001:db8:122:344:c0:2:2100:0", converted_64.to_string());

        // Prefix length 56
        let ipv6_address_56 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_56 = embed(&ipv4_address, &ipv6_address_56, Dns64PrefixLength::Bits56);
        assert_eq!("32,1,13,184,1,34,3,192,0,0,2,33,0,0,0,0", dump(&converted_56));
        assert_eq!("2001:db8:122:3c0:0:221::", converted_56.to_string());

        // Prefix length 48
        let ipv6_address_48 =
            IPAddress::from([32, 1, 13, 184, 1, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_48 = embed(&ipv4_address, &ipv6_address_48, Dns64PrefixLength::Bits48);
        assert_eq!("32,1,13,184,1,34,192,0,0,2,33,0,0,0,0,0", dump(&converted_48));
        assert_eq!("2001:db8:122:c000:2:2100::", converted_48.to_string());

        // Prefix length 40
        let ipv6_address_40 = IPAddress::from([32, 1, 13, 184, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_40 = embed(&ipv4_address, &ipv6_address_40, Dns64PrefixLength::Bits40);
        assert_eq!("32,1,13,184,1,192,0,2,0,33,0,0,0,0,0,0", dump(&converted_40));
        assert_eq!("2001:db8:1c0:2:21::", converted_40.to_string());

        // Prefix length 32
        let ipv6_address_32 = IPAddress::from([32, 1, 13, 184, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let converted_32 = embed(&ipv4_address, &ipv6_address_32, Dns64PrefixLength::Bits32);
        assert_eq!("32,1,13,184,192,0,2,33,0,0,0,0,0,0,0,0", dump(&converted_32));
        assert_eq!("2001:db8:c000:221::", converted_32.to_string());
    }

    #[test]
    fn roundtrip_address_through_value() {
        let address = IPAddress::from([1, 2, 3, 4]);
        assert!(address.is_valid());

        let value = address.to_value().unwrap();
        assert_eq!(Some(address), IPAddress::from_value(&value));
    }

    #[test]
    fn from_garbage_value() {
        let value = Value::Integer(123);
        assert!(IPAddress::from_value(&value).is_none());
    }

    #[test]
    fn from_invalid_value() {
        let value = Value::String("1.2.3.4.5".to_string());
        assert!(IPAddress::from_value(&value).is_none());
    }

    #[test]
    fn ipv4_mask() {
        assert!(IPAddress::create_ipv4_mask(IPAddress::IPV6_ADDRESS_SIZE * 8).is_none());
        assert!(IPAddress::create_ipv4_mask((IPAddress::IPV4_ADDRESS_SIZE + 1) * 8).is_none());
        assert!(IPAddress::create_ipv4_mask(IPAddress::IPV4_ADDRESS_SIZE * 8 + 1).is_none());
        let mask_of = |n| IPAddress::create_ipv4_mask(n).unwrap().to_string();
        assert_eq!("255.255.255.255", mask_of(IPAddress::IPV4_ADDRESS_SIZE * 8));
        assert_eq!("255.255.255.254", mask_of(31));
        assert_eq!("255.255.255.0", mask_of(24));
        assert_eq!("255.255.254.0", mask_of(23));
        assert_eq!("255.255.192.0", mask_of(18));
        assert_eq!("255.255.0.0", mask_of(16));
        assert_eq!("255.0.0.0", mask_of(8));
        assert_eq!("128.0.0.0", mask_of(1));
        assert_eq!("0.0.0.0", mask_of(0));
    }

    #[test]
    fn ipv6_mask() {
        assert!(IPAddress::create_ipv6_mask(IPAddress::IPV6_ADDRESS_SIZE * 8 + 1).is_none());
        let mask_of = |n| IPAddress::create_ipv6_mask(n).unwrap().to_string();
        assert_eq!(
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            mask_of(IPAddress::IPV6_ADDRESS_SIZE * 8)
        );
        assert_eq!("ffff:ffff:ffff:ffff:ffff:ffff:ffff:0", mask_of(112));
        assert_eq!("ffff:ffff::", mask_of(32));
        assert_eq!("8000::", mask_of(1));
        assert_eq!("::", mask_of(0));
    }

    // Test that IPAddress can be created at compile time. The C++ test uses
    // static_asserts. These are the same checks as `const` assertions.
    const fn create_ip_address(literal: &str) -> IPAddress {
        match IPAddress::from_ip_literal(literal) {
            Some(address) => address,
            // C++ ignores the AssignFromIPLiteral result, leaving an empty address.
            None => IPAddress { ip_address: IPAddressBytes::new() },
        }
    }

    const fn bytes_equal(actual: &IPAddressBytes, expected: &[u8]) -> bool {
        if actual.len() != expected.len() {
            return false;
        }
        let mut i = 0;
        while i < expected.len() {
            if actual.bytes[i] != expected[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    const IPV4_ADDRESS: IPAddress = create_ip_address("192.168.2.3");
    const _: () = assert!(bytes_equal(IPV4_ADDRESS.bytes(), &[192, 168, 2, 3]));

    const IPV6_ADDRESS: IPAddress = create_ip_address("2001:0700:0300:1800::000f");
    const _: () = assert!(bytes_equal(
        IPV6_ADDRESS.bytes(),
        &[
            0x20, 0x01, 0x07, 0x00, 0x03, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x0f,
        ]
    ));

    // This test exists mainly to prevent the compiler from optimizing away
    // the compile-time checks above. All actual validation is done at compile time.
    #[test]
    fn verify_ip_address_created_at_compile_time() {
        assert_eq!(IPV4_ADDRESS.bytes().as_slice(), &[192, 168, 2, 3]);
        assert_eq!(
            IPV6_ADDRESS.bytes().as_slice(),
            &[
                0x20, 0x01, 0x07, 0x00, 0x03, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x0f,
            ]
        );
    }

    #[test]
    fn common_prefix_length() {
        let ipv4_1 = IPAddress::from([192, 168, 0, 1]);
        assert_eq!(32, super::super::common_prefix_length(&ipv4_1, &ipv4_1));

        let ipv4_2 = IPAddress::from([192, 168, 0, 2]);
        // First 3 bytes (192.168.0) match. Of the last byte, first 6 bits match.
        assert_eq!(30, super::super::common_prefix_length(&ipv4_1, &ipv4_2));

        let ipv6_1 = ip("2001:db8::1");
        assert_eq!(128, super::super::common_prefix_length(&ipv6_1, &ipv6_1));

        let ipv6_2 = ip("2001:db8::2");
        // First 15 bytes match, followed by 6 bits matching of the last byte.
        assert_eq!(126, super::super::common_prefix_length(&ipv6_1, &ipv6_2));
    }
}

mod extra_test {
    use super::*;

    #[test]
    fn ipv4_literal_formats() {
        // Accepted and rejected forms, following the url::IPv4AddressToNumber
        // rules: octal and hex components, a trailing dot, and field widths.
        let accepted: [(&str, [u8; 4]); 11] = [
            ("0x7f.0.0.1", [127, 0, 0, 1]),
            ("010.0.0.1", [8, 0, 0, 1]),
            ("01.2.3.4", [1, 2, 3, 4]),
            ("0x", [0, 0, 0, 0]),
            ("0XfF.0.0.1", [255, 0, 0, 1]),
            ("4294967295", [255, 255, 255, 255]),
            ("0xFFFFFFFF", [255, 255, 255, 255]),
            ("1.2.65535", [1, 2, 255, 255]),
            ("1.2.3", [1, 2, 0, 3]),
            ("1.2.3.4.", [1, 2, 3, 4]),
            ("0.0.0.0", [0, 0, 0, 0]),
        ];
        for (literal, expected) in accepted {
            assert_eq!(
                Some(expected),
                IPAddress::from_ip_literal(literal)
                    .map(|a| <[u8; 4]>::try_from(a.bytes().as_slice()).unwrap()),
                "{literal}"
            );
        }

        let rejected = [
            "08.0.0.1",
            "1.09.0.0",
            "4294967296",
            "0x100000000",
            "1.2.65536",
            "1..2.3",
            "1.2.3.4.5",
            "256.1.1.1",
            "1.a",
            "-1.0.0.0",
            "1.2.3.4 ",
            "..",
            "1..",
        ];
        for literal in rejected {
            assert!(IPAddress::from_ip_literal(literal).is_none(), "{literal}");
        }
    }

    #[test]
    fn ipv6_literal_formats() {
        let accepted: [(&str, [u8; 16]); 8] = [
            ("::", [0; 16]),
            ("1::", [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ("ABCD::", [0xAB, 0xCD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ("1:2:3:4:5:6:7:8", [0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8]),
            ("1:2:3:4:5:6:7::", [0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 0]),
            ("::ffff:1.2.3.4", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 1, 2, 3, 4]),
            ("1:2:3:4:5:6:1.2.3.4", [0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 1, 2, 3, 4]),
            ("::1.2.3.4", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4]),
        ];
        for (literal, expected) in accepted {
            assert_eq!(
                Some(IPAddress::from(expected)),
                IPAddress::from_ip_literal(literal),
                "{literal}"
            );
        }

        let rejected = [
            "1::2::3",
            "12345::",
            ":1:2:3:4:5:6:7",
            "1:2:3:4:5:6:7:8:9",
            "1::2:3:4:5:6:7:8",
            "1:2:3:4:5:6:7:1.2.3.4",
            "::1.2.3",
            "::1.2.3.4.",
            ":::",
            "[::1]",
            "::1.2.3.256",
            "fe80::1%eth0",
        ];
        for literal in rejected {
            assert!(IPAddress::from_ip_literal(literal).is_none(), "{literal}");
        }
    }

    #[test]
    fn ipv6_formatting_follows_rfc5952() {
        let cases = [
            ("1:0:0:1:0:0:0:1", "1:0:0:1::1"),
            ("1:0:1:1:1:1:1:1", "1:0:1:1:1:1:1:1"),
            ("2001:db8:0:0:1:0:0:1", "2001:db8::1:0:0:1"),
            ("1:0:0:0:1:0:0:0", "1::1:0:0:0"),
            ("0:0:0:0:0:0:0:1", "::1"),
            ("1:0:0:0:0:0:0:0", "1::"),
            ("0:0:0:0:0:0:0:0", "::"),
            ("ABCD:EF01::", "abcd:ef01::"),
        ];
        for (input, expected) in cases {
            assert_eq!(expected, ip(input).to_string(), "{input}");
        }
    }

    #[test]
    fn publicly_routable_mapped_uses_ipv4_ranges() {
        assert!(!ip("::ffff:10.0.0.1").is_publicly_routable());
        assert!(ip("::ffff:8.8.8.8").is_publicly_routable());
        assert!(IPAddress::from_bytes(&[]).is_publicly_routable());
    }

    #[test]
    fn cidr_extras() {
        let (address, bits) = parse_cidr_block(" 10.0.0.0 / 8 ").unwrap();
        assert_eq!("10.0.0.0", address.to_string());
        assert_eq!(8, bits);

        assert_eq!(Some(8), parse_cidr_block("10.0.0.0/08").map(|(_, b)| b));
        assert_eq!(Some(32), parse_cidr_block("10.0.0.0/0032").map(|(_, b)| b));
        assert!(parse_cidr_block("10.0.0.0/4294967297").is_none());
        assert!(parse_cidr_block("10.0.0.0/33").is_none());
        assert!(parse_cidr_block("10.0.0.0/").is_none());
    }

    #[test]
    fn url_hostname_extras() {
        assert_eq!(Some(ip("::1")), parse_url_hostname_to_address("[::1]"));
        assert!(parse_url_hostname_to_address("[192.168.0.1]").is_none());
        assert!(parse_url_hostname_to_address("[::1").is_none());
        assert!(parse_url_hostname_to_address("::1]").is_none());
        assert!(parse_url_hostname_to_address("[[::1]]").is_none());
    }

    #[test]
    fn matches_prefix_edge_cases() {
        assert!(ip_address_matches_prefix(&ip("10.0.0.1"), &ip("10.0.0.0"), 8));
        assert!(!ip_address_matches_prefix(&ip("11.0.0.1"), &ip("10.0.0.0"), 8));
        // A zero-length prefix matches everything of the same family.
        assert!(ip_address_matches_prefix(&ip("203.0.113.9"), &ip("0.0.0.0"), 0));
        assert!(ip_address_matches_prefix(&ip("2001:db8::1"), &ip("::"), 0));
        // IPv6 prefix that cannot match an IPv4 address (mapped prefix differs).
        assert!(!ip_address_matches_prefix(&ip("10.0.0.1"), &ip("::"), 128));
    }

    #[test]
    fn to_value_is_none_for_invalid_address() {
        assert!(IPAddress::default().to_value().is_none());
        assert_eq!(Some(Value::String("192.168.0.1".to_string())), ip("192.168.0.1").to_value());
    }

    #[test]
    fn ordering_is_by_size_then_bytes() {
        let v4_big = IPAddress::from([255, 255, 255, 255]);
        let v6_small = IPAddress::from([0u8; 16]);
        assert!(v4_big < v6_small);
        assert!(IPAddress::from([1, 2, 3, 4]) < IPAddress::from([1, 2, 3, 5]));
    }
}

mod differential {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn bit(bytes: &[u8], i: usize) -> u8 {
        (bytes[i / 8] >> (7 - i % 8)) & 1
    }

    /// Bit-by-bit reference for prefix matching.
    fn reference_prefix_match(addr: &[u8], prefix: &[u8], bits: usize) -> bool {
        (0..bits).all(|i| bit(addr, i) == bit(prefix, i))
    }

    /// Bit-by-bit reference for the common prefix length.
    fn reference_common_prefix(a: &[u8], b: &[u8]) -> usize {
        for i in 0..a.len() * 8 {
            if bit(a, i) != bit(b, i) {
                return i;
            }
        }
        a.len() * 8
    }

    /// Random IPv6 bytes where each 16-bit group is zero about half the time,
    /// so that the "::" compression is exercised.
    fn random_ipv6_bytes(rng: &mut XorShift64) -> [u8; 16] {
        let mut out = [0u8; 16];
        for group in 0..8 {
            if rng.below(2) == 0 {
                out[group * 2] = rng.byte();
                out[group * 2 + 1] = rng.byte();
            }
        }
        out
    }

    #[test]
    fn ipv4_format_and_parse_match_std() {
        let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
        for _ in 0..2000 {
            let bytes: [u8; 4] = rng.bytes();
            let address = IPAddress::from(bytes);
            let text = Ipv4Addr::from(bytes).to_string();
            assert_eq!(text, address.to_string());
            assert_eq!(Some(address), IPAddress::from_ip_literal(&text));
        }
    }

    #[test]
    fn ipv4_canonical_quads_are_accepted_like_std() {
        let mut rng = XorShift64(0x0123_4567_89AB_CDEF);
        for _ in 0..2000 {
            let parts: [u8; 4] = rng.bytes();
            let text = format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]);
            assert_eq!(Some(IPAddress::from(parts)), IPAddress::from_ip_literal(&text), "{text}");
            assert!(text.parse::<Ipv4Addr>().is_ok());
        }
    }

    #[test]
    fn ipv4_out_of_range_components_are_rejected_like_std() {
        let mut rng = XorShift64(0xDEAD_BEEF_CAFE_F00D);
        for _ in 0..500 {
            let big = 256 + rng.below(99_744);
            let text = format!("1.2.3.{big}");
            assert!("1.2.3.0".parse::<Ipv4Addr>().is_ok());
            assert!(text.parse::<Ipv4Addr>().is_err());
            assert!(IPAddress::from_ip_literal(&text).is_none(), "{text}");
        }
    }

    #[test]
    fn ipv6_format_matches_std() {
        let mut rng = XorShift64(0x5151_5151_0000_0001);
        for _ in 0..5000 {
            let bytes = random_ipv6_bytes(&mut rng);
            // std prints IPv4-mapped addresses in dotted form. url (and so C++)
            // prints hex, which the ipv4_mapping_round_trips test covers.
            if bytes[..12] == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF] {
                continue;
            }
            let address = IPAddress::from(bytes);
            let text = Ipv6Addr::from(bytes).to_string();
            assert_eq!(text, address.to_string());
            assert_eq!(Some(address), IPAddress::from_ip_literal(&text), "{text}");
        }
    }

    #[test]
    fn ipv6_uncompressed_form_is_accepted_like_std() {
        let mut rng = XorShift64(0x7777_0000_1111_2222);
        for _ in 0..2000 {
            let bytes = random_ipv6_bytes(&mut rng);
            let expanded = bytes
                .chunks(2)
                .map(|c| format!("{:x}", u16::from(c[0]) << 8 | u16::from(c[1])))
                .collect::<Vec<_>>()
                .join(":");
            assert!(expanded.parse::<Ipv6Addr>().is_ok());
            assert_eq!(
                Some(IPAddress::from(bytes)),
                IPAddress::from_ip_literal(&expanded),
                "{expanded}"
            );
        }
    }

    #[test]
    fn prefix_match_matches_bit_reference() {
        let mut rng = XorShift64(0xABCD_EF01_2345_6789);
        for _ in 0..5000 {
            let v6 = rng.below(2) == 0;
            let len = if v6 { 16 } else { 4 };
            let mut prefix = [0u8; 16];
            for b in prefix.iter_mut().take(len) {
                *b = rng.byte();
            }
            // Derive the address from the prefix, then flip one bit sometimes,
            // so that both matching and non-matching cases are common.
            let mut addr = prefix;
            if rng.below(2) == 0 {
                let flip = rng.below((len * 8) as u64) as usize;
                addr[flip / 8] ^= 0x80 >> (flip % 8);
            }
            let bits = rng.below((len * 8 + 1) as u64) as usize;
            let ip_address = IPAddress::from_bytes(&addr[..len]);
            let ip_prefix = IPAddress::from_bytes(&prefix[..len]);
            assert_eq!(
                reference_prefix_match(&addr[..len], &prefix[..len], bits),
                ip_address_matches_prefix(&ip_address, &ip_prefix, bits),
                "bits={bits} v6={v6}"
            );
        }
    }

    #[test]
    fn mixed_family_prefix_match_uses_mapped_bits() {
        let mut rng = XorShift64(0x1357_9BDF_2468_ACE0);
        for _ in 0..3000 {
            let v4: [u8; 4] = rng.bytes();
            let mut v6 = [0u8; 16];
            for b in v6.iter_mut() {
                *b = rng.byte();
            }
            let bits = rng.below(129) as usize;
            // IPv4 address against an IPv6 prefix: compare against the
            // IPv4-mapped form, using the same bit positions.
            let mut mapped = [0u8; 16];
            mapped[10] = 0xFF;
            mapped[11] = 0xFF;
            mapped[12..].copy_from_slice(&v4);
            assert_eq!(
                reference_prefix_match(&mapped, &v6, bits),
                ip_address_matches_prefix(&IPAddress::from(v4), &IPAddress::from(v6), bits),
                "bits={bits}"
            );
        }
    }

    #[test]
    fn common_prefix_length_matches_bit_reference() {
        let mut rng = XorShift64(0x0F0F_F0F0_3333_CCCC);
        for _ in 0..5000 {
            let v6 = rng.below(2) == 0;
            let len = if v6 { 16 } else { 4 };
            let a: [u8; 16] = {
                let mut x = [0u8; 16];
                for b in x.iter_mut().take(len) {
                    *b = rng.byte();
                }
                x
            };
            let mut b = a;
            if rng.below(4) != 0 {
                let flip = rng.below((len * 8) as u64) as usize;
                b[flip / 8] ^= 0x80 >> (flip % 8);
            }
            assert_eq!(
                reference_common_prefix(&a[..len], &b[..len]),
                super::super::common_prefix_length(
                    &IPAddress::from_bytes(&a[..len]),
                    &IPAddress::from_bytes(&b[..len])
                )
            );
        }
    }

    #[test]
    fn masks_match_bit_reference_and_round_trip() {
        for n in 0..=32usize {
            let mask = IPAddress::create_ipv4_mask(n).unwrap();
            assert_eq!(n, super::super::mask_prefix_length(&mask));
            for i in 0..32 {
                assert_eq!(u8::from(i < n), bit(mask.bytes().as_slice(), i), "n={n} i={i}");
            }
        }
        let mut rng = XorShift64(0xFACE_B00C_0000_0001);
        for _ in 0..1000 {
            let n = rng.below(129) as usize;
            let mask = IPAddress::create_ipv6_mask(n).unwrap();
            assert_eq!(n, super::super::mask_prefix_length(&mask));
        }
    }

    #[test]
    fn cidr_round_trip() {
        let mut rng = XorShift64(0x1122_3344_5566_7788);
        for _ in 0..2000 {
            let v6 = rng.below(2) == 0;
            let (address, max_bits) = if v6 {
                (IPAddress::from(random_ipv6_bytes(&mut rng)), 128)
            } else {
                (IPAddress::from(rng.bytes::<4>()), 32)
            };
            let bits = rng.below(max_bits as u64 + 1) as usize;
            let text = format!("{address}/{bits}");
            assert_eq!(Some((address, bits)), parse_cidr_block(&text), "{text}");
            let url_text = if v6 { format!("[{address}]/{bits}") } else { text.clone() };
            assert_eq!(
                Some((address, bits)),
                parse_cidr_block_non_standard_url_format(&url_text),
                "{url_text}"
            );
        }
    }

    #[test]
    fn ipv4_mapping_round_trips() {
        let mut rng = XorShift64(0x4242_4242_1010_1010);
        for _ in 0..1000 {
            let v4 = IPAddress::from(rng.bytes::<4>());
            let mapped = convert_ipv4_to_ipv4_mapped_ipv6(&v4);
            assert!(mapped.is_ipv4_mapped_ipv6());
            assert_eq!(v4, convert_ipv4_mapped_ipv6_to_ipv4(&mapped));
            // url formatting writes the IPv4 part as two hex groups (the C++
            // expectation is "::ffff:c0a8:1"), unlike std, which writes it dotted.
            let o = v4.bytes().as_slice();
            let expected = format!(
                "::ffff:{:x}:{:x}",
                u16::from_be_bytes([o[0], o[1]]),
                u16::from_be_bytes([o[2], o[3]])
            );
            assert_eq!(expected, mapped.to_string());
        }
    }

    #[test]
    fn dns64_embedding_round_trips_through_extraction() {
        use super::super::{
            convert_ipv4_to_ipv4_embedded_ipv6 as embed,
            extract_pref64_from_ipv4only_arpa_aaaa as extract, Dns64PrefixLength,
        };
        let arpa = IPAddress::from([192, 0, 0, 170]);
        let cases = [
            (Dns64PrefixLength::Bits96, 96),
            (Dns64PrefixLength::Bits64, 64),
            (Dns64PrefixLength::Bits56, 56),
            (Dns64PrefixLength::Bits48, 48),
            (Dns64PrefixLength::Bits40, 40),
            (Dns64PrefixLength::Bits32, 32),
        ];
        let mut rng = XorShift64(0x6464_6464_9696_9696);
        for _ in 0..200 {
            let prefix = IPAddress::from(random_ipv6_bytes(&mut rng));
            for (length, _) in cases {
                let embedded = embed(&arpa, &prefix, length);
                assert_eq!(length, extract(&embedded), "{embedded}");
            }
        }
    }

    #[test]
    fn parse_never_panics_and_accepted_inputs_round_trip() {
        const ALPHABET: &[u8] = b"0123456789abcdefABCDEFxX.:[]/ ";
        let mut rng = XorShift64(0xC0FF_EE00_BAD0_F00D);
        for _ in 0..20_000 {
            let len = rng.below(41) as usize;
            let text: String = (0..len)
                .map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize] as char)
                .collect();

            if let Some(address) = IPAddress::from_ip_literal(&text) {
                assert!(address.is_valid(), "{text}");
                let canonical = address.to_string();
                assert_eq!(Some(address), IPAddress::from_ip_literal(&canonical), "{text}");
            }
            let _ = parse_cidr_block(&text);
            let _ = parse_cidr_block_non_standard_url_format(&text);
            let _ = parse_url_hostname_to_address(&text);
        }
    }

    #[test]
    fn std_accepted_literals_are_accepted_here() {
        // One-way check: literals that std::net accepts must parse to the same
        // bytes here. The converse is not required, since url rules are wider
        // (octal, hex and short IPv4 forms).
        const ALPHABET: &[u8] = b"0123456789abcdef:.";
        let mut rng = XorShift64(0x0BAD_5EED_0000_0007);
        for _ in 0..20_000 {
            let len = rng.below(40) as usize;
            let text: String = (0..len)
                .map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize] as char)
                .collect();
            if let Ok(v4) = text.parse::<Ipv4Addr>() {
                assert_eq!(
                    Some(IPAddress::from(v4.octets())),
                    IPAddress::from_ip_literal(&text),
                    "{text}"
                );
            }
            if let Ok(v6) = text.parse::<Ipv6Addr>() {
                assert_eq!(
                    Some(IPAddress::from(v6.octets())),
                    IPAddress::from_ip_literal(&text),
                    "{text}"
                );
            }
        }
    }
}
