// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of net/dns/dns_names_util_unittest.cc, plus randomized and
//! differential tests.
//!
//! The C++ `ToBytes("...")` helper includes the terminating NUL, so the
//! expected wire-format values below end in `\0` where that applied. Octal
//! escapes in the C++ are written here as the equivalent `\x` escapes.

use super::*;

fn some(b: &[u8]) -> Option<Vec<u8>> {
    Some(b.to_vec())
}

// ---- DottedNameToNetwork ----------------------------------------------------

#[test]
fn dotted_name_to_network_with_validation() {
    assert_eq!(
        dotted_name_to_network(b"com", true),
        some(b"\x03com\x00")
    );
    assert_eq!(
        dotted_name_to_network(b"google.com", true),
        some(b"\x06google\x03com\x00")
    );
    assert_eq!(
        dotted_name_to_network(b"www.google.com", true),
        some(b"\x03www\x06google\x03com\x00")
    );
}

#[test]
fn dotted_name_to_network_basic() {
    assert_eq!(
        dotted_name_to_network(b"com", false),
        some(b"\x03com\x00")
    );
    assert_eq!(
        dotted_name_to_network(b"google.com", false),
        some(b"\x06google\x03com\x00")
    );
    assert_eq!(
        dotted_name_to_network(b"www.google.com", false),
        some(b"\x03www\x06google\x03com\x00")
    );
}

#[test]
fn dotted_name_to_network_with_validation_rejects_empty_labels() {
    assert_eq!(dotted_name_to_network(b"", true), None);
    assert_eq!(dotted_name_to_network(b".", true), None);
    assert_eq!(dotted_name_to_network(b"..", true), None);
    assert_eq!(dotted_name_to_network(b".google.com", true), None);
    assert_eq!(dotted_name_to_network(b"www..google.com", true), None);
}

#[test]
fn dotted_name_to_network_rejects_empty_labels() {
    assert_eq!(dotted_name_to_network(b"", false), None);
    assert_eq!(dotted_name_to_network(b".", false), None);
    assert_eq!(dotted_name_to_network(b"..", false), None);
    assert_eq!(dotted_name_to_network(b".google.com", false), None);
    assert_eq!(dotted_name_to_network(b"www..google.com", false), None);
}

#[test]
fn dotted_name_to_network_with_validation_accepts_empty_label_at_end() {
    assert_eq!(
        dotted_name_to_network(b"www.google.com.", true),
        some(b"\x03www\x06google\x03com\x00")
    );
}

#[test]
fn dotted_name_to_network_accepts_empty_label_at_end() {
    assert_eq!(
        dotted_name_to_network(b"www.google.com.", false),
        some(b"\x03www\x06google\x03com\x00")
    );
}

#[test]
fn dotted_name_to_network_with_validation_allows_long_names() {
    // Label is 63 chars: still valid.
    assert_eq!(
        dotted_name_to_network(
            b"z23456789a123456789a123456789a123456789a123456789a123456789a123",
            true
        ),
        some(
            concat!(
                "\x3f",
                "z23456789a123456789a123456789a123456789a123456",
                "789a123456789a123",
                "\x00"
            )
            .as_bytes()
        )
    );
    assert_eq!(
        dotted_name_to_network(
            b"z23456789a123456789a123456789a123456789a123456789a123456789a123.",
            true
        ),
        some(
            concat!(
                "\x3f",
                "z23456789a123456789a123456789a123456789a123456",
                "789a123456789a123",
                "\x00"
            )
            .as_bytes()
        )
    );

    // 253 characters in the name: still valid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abc"
            )
            .as_bytes(),
            true
        ),
        some(
            concat!(
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x03abc",
                "\x00"
            )
            .as_bytes()
        )
    );

    // 253 characters in the name plus final dot: still valid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abc."
            )
            .as_bytes(),
            true
        ),
        some(
            concat!(
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x03abc",
                "\x00"
            )
            .as_bytes()
        )
    );
}

#[test]
fn dotted_name_to_network_allows_long_names() {
    // Label is 63 chars: still valid.
    assert_eq!(
        dotted_name_to_network(
            b"z23456789a123456789a123456789a123456789a123456789a123456789a123",
            false
        ),
        some(
            concat!(
                "\x3f",
                "z23456789a123456789a123456789a123456789a123456",
                "789a123456789a123",
                "\x00"
            )
            .as_bytes()
        )
    );
    // Label is 63 chars: still valid.
    assert_eq!(
        dotted_name_to_network(
            b"z23456789a123456789a123456789a123456789a123456789a123456789a123.",
            false
        ),
        some(
            concat!(
                "\x3f",
                "z23456789a123456789a123456789a123456789a123456",
                "789a123456789a123",
                "\x00"
            )
            .as_bytes()
        )
    );

    // 253 characters in the name: still valid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abc"
            )
            .as_bytes(),
            false
        ),
        some(
            concat!(
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x03abc",
                "\x00"
            )
            .as_bytes()
        )
    );

    // 253 characters in the name plus final dot: still valid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
                "abcdefghi.abc."
            )
            .as_bytes(),
            false
        ),
        some(
            concat!(
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x09abcdefghi\x09abcdefghi\x09abcdefghi",
                "\x09abcdefghi\x03abc",
                "\x00"
            )
            .as_bytes()
        )
    );
}

#[test]
fn dotted_name_to_network_with_validation_rejects_too_long_names() {
    // Label is too long: invalid.
    assert_eq!(
        dotted_name_to_network(
            b"123456789a123456789a123456789a123456789a123456789a123456789a1234",
            true
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            b"123456789a123456789a123456789a123456789a123456789a123456789a1234.",
            true
        ),
        None
    );

    // 254 characters in the name: invalid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.1234"
            )
            .as_bytes(),
            true
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.1234."
            )
            .as_bytes(),
            true
        ),
        None
    );

    // 255 characters in the name: invalid before even trying to add a final
    // zero-length termination.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.12345"
            )
            .as_bytes(),
            true
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.12345."
            )
            .as_bytes(),
            true
        ),
        None
    );
}

#[test]
fn dotted_name_to_network_rejects_too_long_names() {
    // Label is too long: invalid.
    assert_eq!(
        dotted_name_to_network(
            b"123456789a123456789a123456789a123456789a123456789a123456789a1234",
            false
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            b"123456789a123456789a123456789a123456789a123456789a123456789a1234.",
            false
        ),
        None
    );

    // 254 characters in the name: invalid.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.1234"
            )
            .as_bytes(),
            false
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.1234."
            )
            .as_bytes(),
            false
        ),
        None
    );

    // 255 characters in the name: invalid before even trying to add a final
    // zero-length termination.
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.12345"
            )
            .as_bytes(),
            false
        ),
        None
    );
    assert_eq!(
        dotted_name_to_network(
            concat!(
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.123456789.123456789.123456789.123456789.123456789.",
                "123456789.12345."
            )
            .as_bytes(),
            false
        ),
        None
    );
}

#[test]
fn dotted_name_to_network_with_validation_rejects_restricted_characters() {
    assert_eq!(dotted_name_to_network(b"foo,bar.com", true), None);
    assert_eq!(
        dotted_name_to_network(b"_ipp._tcp.local.foo printer (bar)", true),
        None
    );
}

#[test]
fn dotted_name_to_network_accepts_restricted_characters() {
    assert_eq!(
        dotted_name_to_network(b"foo,bar.com", false),
        some(b"\x07foo,bar\x03com\x00")
    );
    assert_eq!(
        dotted_name_to_network(b"_ipp._tcp.local.foo printer (bar)", false),
        some(b"\x04_ipp\x04_tcp\x05local\x11foo printer (bar)\x00")
    );
}

// ---- NetworkToDottedName ----------------------------------------------------

#[test]
fn network_to_dotted_name_advances_reader() {
    {
        let mut reader = SpanReader::new(b"\x03com");
        assert_eq!(
            network_to_dotted_name_from_reader(&mut reader, false),
            some(b"com")
        );
        assert_eq!(reader.remaining(), 0);
    }
    {
        let mut reader = SpanReader::new(b"\x03com\x00ZZZ");
        assert_eq!(
            network_to_dotted_name_from_reader(&mut reader, false),
            some(b"com")
        );
        assert_eq!(reader.remaining(), 3);
    }
}

#[test]
fn network_to_dotted_name_should_handle_simple_names() {
    let mut dns_name = b"\x03foo".to_vec();
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b"foo"));

    dns_name.extend_from_slice(b"\x03bar");
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b"foo.bar"));

    dns_name.extend_from_slice(b"\x02uk");
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b"foo.bar.uk"));

    dns_name.push(0);
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b"foo.bar.uk"));
}

#[test]
fn network_to_dotted_name_should_handle_empty() {
    let mut dns_name: Vec<u8> = Vec::new();
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b""));

    dns_name.push(0);
    assert_eq!(network_to_dotted_name(&dns_name, false), some(b""));
}

#[test]
fn network_to_dotted_name_should_reject_empty_incomplete() {
    let dns_name: Vec<u8> = Vec::new();

    assert_eq!(network_to_dotted_name(&dns_name, false), some(b""));
    assert_eq!(network_to_dotted_name(&dns_name, true), None);
}

// Tests `require_complete` given an input with a terminating zero-length label.
#[test]
fn network_to_dotted_name_complete() {
    let dns_name = b"\x03foo\x04test\x00";

    assert_eq!(network_to_dotted_name(dns_name, false), some(b"foo.test"));
    assert_eq!(network_to_dotted_name(dns_name, true), some(b"foo.test"));
}

// Tests `require_complete` given an input without a terminating zero-length
// label.
#[test]
fn network_to_dotted_name_not_complete() {
    let dns_name = b"\x03boo\x04test";

    assert_eq!(network_to_dotted_name(dns_name, false), some(b"boo.test"));
    assert_eq!(network_to_dotted_name(dns_name, true), None);
}

#[test]
fn network_to_dotted_name_should_reject_empty_when_requiring_complete() {
    let mut dns_name: Vec<u8> = Vec::new();

    assert_eq!(network_to_dotted_name(&dns_name, false), some(b""));
    assert_eq!(network_to_dotted_name(&dns_name, true), None);

    dns_name.push(0);
    assert_eq!(network_to_dotted_name(&dns_name, true), some(b""));
}

#[test]
fn network_to_dotted_name_should_reject_compression() {
    // CreateNamePointer(152): the 16-bit pointer with the 0xc0 marker set.
    let mut dns_name = vec![0xc0u8, 0x98];

    assert_eq!(network_to_dotted_name(&dns_name, false), None);

    dns_name.splice(0..0, *b"\x05hello");
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

// Extra input past the terminating zero-length label is ignored.
#[test]
fn network_to_dotted_name_should_handle_excess_input() {
    let dns_name = b"\x04cool\x04name\x04test\x00blargh!";
    assert_eq!(network_to_dotted_name(dns_name, false), some(b"cool.name.test"));

    let dns_name = b"\x02hi\x00goodbye";
    assert_eq!(network_to_dotted_name(dns_name, false), some(b"hi"));
}

// Input is malformed if it ends mid-label.
#[test]
fn network_to_dotted_name_should_reject_truncated_names() {
    assert_eq!(network_to_dotted_name(b"\x07cheese", false), None);
    assert_eq!(network_to_dotted_name(b"\x06cheesy\x05test", false), None);
}

#[test]
fn network_to_dotted_name_should_handle_long_single_label() {
    let mut dns_name = vec![MAX_LABEL_LENGTH as u8];
    for _ in 0..MAX_LABEL_LENGTH {
        dns_name.push(b'a');
    }
    assert!(network_to_dotted_name(&dns_name, false).is_some());
}

#[test]
fn network_to_dotted_name_should_handle_long_second_label() {
    let mut dns_name = b"\x03foo".to_vec();
    dns_name.push(MAX_LABEL_LENGTH as u8);
    for _ in 0..MAX_LABEL_LENGTH {
        dns_name.push(b'a');
    }
    assert!(network_to_dotted_name(&dns_name, false).is_some());
}

#[test]
fn network_to_dotted_name_should_reject_too_long_single_label() {
    let mut dns_name = vec![MAX_LABEL_LENGTH as u8];
    for _ in 0..(MAX_LABEL_LENGTH + 1) {
        dns_name.push(b'a');
    }
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

#[test]
fn network_to_dotted_name_should_reject_too_long_second_label() {
    let mut dns_name = b"\x03foo".to_vec();
    dns_name.push(MAX_LABEL_LENGTH as u8);
    for _ in 0..(MAX_LABEL_LENGTH + 1) {
        dns_name.push(b'a');
    }
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

// The C++ test is conditional on CHAR_MIN < 0, which holds on the platforms
// where it runs. The Rust port always runs it, using the same byte value.
#[test]
fn network_to_dotted_name_should_reject_char_min_labels() {
    // CHAR_MIN as a uint8_t is 0x80 == 128, which is above the label limit.
    let char_min_as_u8: u8 = 128;
    assert!(char_min_as_u8 > MAX_LABEL_LENGTH as u8);

    let mut dns_name = vec![char_min_as_u8];
    // Make the rest otherwise valid: 128 bytes of 'a'.
    for _ in 0..char_min_as_u8 {
        dns_name.push(b'a');
    }
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

/// Builds the labels with the same loops as the C++ tests. `loop_bound` is the
/// C++ loop limit and `label_bound` the value that `label_size` is computed
/// from (`min(label_bound - i, 63)`). The "should handle" tests use
/// (255, 254) and the "should reject" tests use (256, 255).
fn build_long_labels(loop_bound: usize, label_bound: usize) -> Vec<u8> {
    let mut dns_name = Vec::new();
    let mut i = 0usize;
    while i < loop_bound {
        let label_size = std::cmp::min(label_bound - i, MAX_LABEL_LENGTH);
        dns_name.push(label_size as u8);
        for _ in 0..label_size {
            dns_name.push(b'a');
        }
        i += MAX_LABEL_LENGTH + 1;
    }
    dns_name
}

#[test]
fn network_to_dotted_name_should_handle_long_name() {
    let dns_name = build_long_labels(255, 254);
    assert_eq!(dns_name.len(), MAX_NAME_LENGTH);
    assert!(network_to_dotted_name(&dns_name, false).is_some());
}

#[test]
fn network_to_dotted_name_should_reject_too_long_name() {
    let dns_name = build_long_labels(256, 255);
    assert_eq!(dns_name.len(), MAX_NAME_LENGTH + 1);
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

#[test]
fn network_to_dotted_name_should_handle_long_complete_name() {
    let mut dns_name = build_long_labels(255, 254);
    dns_name.push(0);
    assert_eq!(dns_name.len(), MAX_NAME_LENGTH + 1);
    assert!(network_to_dotted_name(&dns_name, false).is_some());
}

#[test]
fn network_to_dotted_name_should_reject_too_long_complete_name() {
    let mut dns_name = build_long_labels(256, 255);
    dns_name.push(0);
    assert_eq!(dns_name.len(), MAX_NAME_LENGTH + 2);
    assert_eq!(network_to_dotted_name(&dns_name, false), None);
}

// ---- IsValidDnsName / IsValidDnsRecordName -------------------------------

#[test]
fn valid_dns_names() {
    let good_hostnames: [&[u8]; 14] = [
        b"www.noodles.blorg",
        b"1www.noodles.blorg",
        b"www.2noodles.blorg",
        b"www.n--oodles.blorg",
        b"www.noodl_es.blorg",
        b"www.no-_odles.blorg",
        b"www_.noodles.blorg",
        b"www.noodles.blorg.",
        b"_privet._tcp.local",
        b"%20%20noodles.blorg",
        b"noo dles.blorg ",
        b"noo dles_ipp._tcp.local",
        b"www.nood(les).blorg",
        b"noo dl(es)._tcp.local",
    ];

    for good_hostname in good_hostnames {
        assert!(is_valid_dns_name(good_hostname), "{good_hostname:?}");
        assert!(is_valid_dns_record_name(good_hostname), "{good_hostname:?}");
    }
}

#[test]
fn empty_not_valid_dns_name() {
    assert!(!is_valid_dns_name(b""));
    assert!(!is_valid_dns_record_name(b""));
}

#[test]
fn empty_label_not_valid_dns_name() {
    assert!(!is_valid_dns_name(b"www..test"));
    assert!(!is_valid_dns_name(b".foo.test"));

    assert!(!is_valid_dns_record_name(b"www..test"));
    assert!(!is_valid_dns_record_name(b".foo.test"));
}

#[test]
fn long_labels_in_valid_dns_names() {
    assert!(is_valid_dns_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a123"
    ));
    assert!(is_valid_dns_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a123."
    ));

    assert!(is_valid_dns_record_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a123"
    ));
    assert!(is_valid_dns_record_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a123."
    ));
}

#[test]
fn too_long_labels_in_invalid_dns_names() {
    assert!(!is_valid_dns_name(
        b"123456789a123456789a123456789a123456789a123456789a123456789a1234"
    ));
    assert!(!is_valid_dns_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a1234."
    ));

    assert!(!is_valid_dns_record_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a1234"
    ));
    assert!(!is_valid_dns_record_name(
        b"z23456789a123456789a123456789a123456789a123456789a123456789a1234."
    ));
}

#[test]
fn long_valid_dns_names() {
    let name_253 = concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abc"
    );
    assert!(is_valid_dns_name(name_253.as_bytes()));
    assert!(is_valid_dns_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abc."
    )
    .as_bytes()));

    assert!(is_valid_dns_record_name(name_253.as_bytes()));
    assert!(is_valid_dns_record_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abc."
    )
    .as_bytes()));
}

#[test]
fn too_long_invalid_dns_names() {
    assert!(!is_valid_dns_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcd"
    )
    .as_bytes()));
    assert!(!is_valid_dns_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcd."
    )
    .as_bytes()));

    assert!(!is_valid_dns_record_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcd"
    )
    .as_bytes()));
    assert!(!is_valid_dns_record_name(concat!(
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.abcdefghi.",
        "abcdefghi.abcd."
    )
    .as_bytes()));
}

#[test]
fn localhost_not_valid_dns_record_name() {
    assert!(is_valid_dns_name(b"localhost"));
    assert!(!is_valid_dns_record_name(b"localhost"));
}

#[test]
fn ip_address_not_valid_dns_record_name() {
    assert!(is_valid_dns_name(b"1.2.3.4"));
    assert!(!is_valid_dns_record_name(b"1.2.3.4"));

    assert!(is_valid_dns_name(b"[2001:4860:4860::8888]"));
    assert!(!is_valid_dns_record_name(b"[2001:4860:4860::8888]"));

    assert!(is_valid_dns_name(b"2001:4860:4860::8888"));
    assert!(!is_valid_dns_record_name(b"2001:4860:4860::8888"));
}

// ---- UrlCanonicalizeNameIfAble ---------------------------------------------

#[test]
fn canonicalize_names() {
    assert_eq!(url_canonicalize_name_if_able(b"GOOGLE.test"), b"google.test");

    assert_eq!(url_canonicalize_name_if_able(b"g{oo}gle.test"), b"g{oo}gle.test");
    assert_eq!(url_canonicalize_name_if_able(b"G{OO}GLE.test"), b"g{oo}gle.test");

    // g\u{fc}gle.test
    assert_eq!(
        url_canonicalize_name_if_able("g\u{00FC}gle.test".as_bytes()),
        b"xn--ggle-0ra.test"
    );
    assert_eq!(
        url_canonicalize_name_if_able("G\u{00fc}GLE.test".as_bytes()),
        b"xn--ggle-0ra.test"
    );
}

#[test]
fn ignore_uncanonicalizeable_names() {
    assert_eq!(url_canonicalize_name_if_able(b""), b"");

    // Forbidden domain code point.
    // https://url.spec.whatwg.org/#forbidden-domain-code-point
    assert_eq!(url_canonicalize_name_if_able(b"g<oo>gle.test"), b"g<oo>gle.test");
    assert_eq!(url_canonicalize_name_if_able(b"G<OO>GLE.test"), b"G<OO>GLE.test");

    // Invalid UTF-8. C++ input "g\x00FCgle.test" is the byte 0xFC.
    assert_eq!(url_canonicalize_name_if_able(b"g\xfcgle.test"), b"g\xfcgle.test");
    assert_eq!(url_canonicalize_name_if_able(b"G\xfcGLE.test"), b"G\xfcGLE.test");

    // Disallowed ASCII character.
    assert_eq!(url_canonicalize_name_if_able(b"google\n.test"), b"google\n.test");
    assert_eq!(url_canonicalize_name_if_able(b"GOOGLE\n.test"), b"GOOGLE\n.test");
}

// ---- ReadU8LengthPrefixed / ReadU16LengthPrefixed --------------------------

#[test]
fn read_u8_length_prefixed_basic() {
    let array: [u8; 7] = [b'b', b'4', 3, b'a', b'b', b'c', b'd'];
    let mut reader = SpanReader::new(&array);
    assert!(reader.skip(2));
    assert_eq!(reader.remaining(), 5);
    assert_eq!(reader.num_read(), 2);
    let s = read_u8_length_prefixed(&mut reader);
    assert_eq!(s, Some(&array[3..6]));
    assert_eq!(reader.remaining(), 1);
    assert_eq!(reader.num_read(), 6);
}

#[test]
fn read_u16_length_prefixed_basic() {
    let array: [u8; 8] = [b'b', b'4', 0, 3, b'a', b'b', b'c', b'd'];
    let mut reader = SpanReader::new(&array);
    assert!(reader.skip(2));
    assert_eq!(reader.remaining(), 6);
    assert_eq!(reader.num_read(), 2);
    let s = read_u16_length_prefixed(&mut reader);
    assert_eq!(s, Some(&array[4..7]));
    assert_eq!(reader.remaining(), 1);
    assert_eq!(reader.num_read(), 7);
}

// ---- Additional tests -------------------------------------------------------

#[test]
fn length_prefixed_failure_leaves_reader_unchanged() {
    // Length says 5 but only 2 bytes follow.
    let data = [5u8, b'a', b'b'];
    let mut reader = SpanReader::new(&data);
    let before = reader;
    assert_eq!(read_u8_length_prefixed(&mut reader), None);
    assert_eq!(reader, before);

    // The 16-bit length itself is truncated.
    let data = [0u8, 3];
    let mut reader = SpanReader::new(&data[..1]);
    let before = reader;
    assert_eq!(read_u16_length_prefixed(&mut reader), None);
    assert_eq!(reader, before);
}

#[test]
fn network_to_dotted_name_from_reader_stops_at_root_label() {
    let data = b"\x01a\x00\x01b";
    let mut reader = SpanReader::new(data);
    assert_eq!(network_to_dotted_name_from_reader(&mut reader, false), some(b"a"));
    // The root label was read, the following bytes were not.
    assert_eq!(reader.remaining(), 2);
}

#[test]
fn network_to_dotted_name_keeps_arbitrary_label_bytes() {
    // Labels can carry any octet. Non-UTF-8 bytes are preserved.
    assert_eq!(
        network_to_dotted_name(b"\x03\xff\x80a\x00", false),
        some(b"\xff\x80a")
    );
}

#[test]
fn dotted_name_to_network_round_trip_randomized() {
    let mut rng = Rng(0x0dd5_1a3e_5eed_0001);
    let alphabet = b"abcxyz019-_";
    for _ in 0..3000 {
        // Random label count and label lengths, within limits.
        let labels = 1 + rng.below(5);
        let mut dotted: Vec<u8> = Vec::new();
        for l in 0..labels {
            if l > 0 {
                dotted.push(b'.');
            }
            let len = 1 + rng.below(20);
            for _ in 0..len {
                dotted.push(alphabet[rng.below(alphabet.len())]);
            }
        }
        let wire = dotted_name_to_network(&dotted, false).expect("valid name");
        assert_eq!(
            network_to_dotted_name(&wire, true),
            Some(dotted.clone()),
            "dotted = {dotted:?}"
        );
    }
}

/// Reference: a direct transliteration of the wire-format validity rules,
/// written independently of `dotted_name_to_network`. Counts octets as the
/// spec does: one length octet per label, plus one for the root.
fn reference_is_valid_dotted(dotted: &[u8]) -> bool {
    let trimmed = dotted.strip_suffix(b".").unwrap_or(dotted);
    if dotted.is_empty() {
        return false;
    }
    let mut wire_len = 1usize; // root label
    for label in trimmed.split(|&c| c == b'.') {
        if label.is_empty() || label.len() > MAX_LABEL_LENGTH {
            return false;
        }
        wire_len += 1 + label.len();
    }
    wire_len <= MAX_NAME_LENGTH
}

#[test]
fn is_valid_dns_name_matches_reference_randomized() {
    let mut rng = Rng(0xfeed_face_cafe_beef);
    let alphabet = b"ab.";
    for _ in 0..20000 {
        let len = rng.below(300);
        let dotted: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        assert_eq!(
            is_valid_dns_name(&dotted),
            reference_is_valid_dotted(&dotted),
            "dotted = {:?}",
            String::from_utf8_lossy(&dotted)
        );
    }
}

#[test]
fn network_to_dotted_name_rejects_random_garbage_without_panicking() {
    let mut rng = Rng(7);
    for _ in 0..5000 {
        let len = rng.below(300);
        let bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
        // Only asserts that this terminates without panicking; the result
        // is checked against a round trip for the valid cases.
        if let Some(dotted) = network_to_dotted_name(&bytes, false) {
            if let Some(wire) = dotted_name_to_network(&dotted, false) {
                assert_eq!(network_to_dotted_name(&wire, true), Some(dotted));
            }
        }
    }
}

#[test]
fn require_valid_hostname_rejects_uppercase_as_in_cpp() {
    // IsCanonicalizedHostCompliant expects canonicalized input.
    assert!(!is_canonicalized_host_compliant(b"Google.com"));
    assert!(is_canonicalized_host_compliant(b"google.com"));
    assert_eq!(dotted_name_to_network(b"Google.com", true), None);
    assert!(dotted_name_to_network(b"Google.com", false).is_some());
}

#[test]
fn canonicalized_host_compliant_edge_cases() {
    assert!(!is_canonicalized_host_compliant(b""));
    assert!(!is_canonicalized_host_compliant(b"-a"));
    assert!(!is_canonicalized_host_compliant(b"a..b"));
    // Only the first character of the final label is checked for alphanumeric.
    assert!(is_canonicalized_host_compliant(b"a.b-"));
    assert!(is_canonicalized_host_compliant(b"_a.b-c"));
    assert!(is_canonicalized_host_compliant(b"a.b."));
    assert!(is_canonicalized_host_compliant(b"a.b.c."));
    assert!(is_canonicalized_host_compliant(b"ab.cd"));
    // Uppercase, and any other character, is rejected.
    assert!(!is_canonicalized_host_compliant(b"a.B"));
    assert!(!is_canonicalized_host_compliant(b"a b"));
    // 63-character label ok, 64 not.
    let l63 = vec![b'a'; 63];
    let l64 = vec![b'a'; 64];
    assert!(is_canonicalized_host_compliant(&l63));
    assert!(!is_canonicalized_host_compliant(&l64));
    // 254 characters must end with a dot.
    let mut long: Vec<u8> = Vec::new();
    while long.len() + 64 < 253 {
        long.extend_from_slice(&[b'a'; 63]);
        long.push(b'.');
    }
    let fill = 254 - long.len() - 1;
    long.extend(std::iter::repeat_n(b'a', fill));
    long.push(b'.');
    assert_eq!(long.len(), 254);
    assert!(is_canonicalized_host_compliant(&long));
    long.pop();
    long.push(b'a');
    assert!(!is_canonicalized_host_compliant(&long));
}

#[test]
fn host_string_is_localhost_cases() {
    assert!(host_string_is_localhost(b"localhost"));
    assert!(host_string_is_localhost(b"LOCALHOST."));
    assert!(host_string_is_localhost(b"foo.localhost"));
    assert!(!host_string_is_localhost(b"notlocalhost"));
    assert!(host_string_is_localhost(b"127.0.0.1"));
    assert!(host_string_is_localhost(b"127.9.9.9"));
    assert!(host_string_is_localhost(b"::1"));
    assert!(!host_string_is_localhost(b"1.2.3.4"));
    assert!(!host_string_is_localhost(b"::2"));
}

#[test]
fn compare_domain_names_is_case_insensitive() {
    assert_eq!(compare_domain_names(b"Foo.COM", b"foo.com"), Ordering::Equal);
    assert_eq!(compare_domain_names(b"a.test", b"B.test"), Ordering::Less);
    assert_eq!(compare_domain_names(b"abc", b"ab"), Ordering::Greater);
    assert_eq!(compare_domain_names(b"", b""), Ordering::Equal);
}

#[test]
fn canonicalize_extra_cases() {
    // IPv4 numeric forms are canonicalized, as in the WHATWG parser.
    assert_eq!(url_canonicalize_name_if_able(b"0x7f.1"), b"127.0.0.1");
    assert_eq!(url_canonicalize_name_if_able(b"1.2.3.4"), b"1.2.3.4");
    assert_eq!(url_canonicalize_name_if_able(b"1.2.3.4.5"), b"1.2.3.4.5");
    // Percent-encoded ASCII is decoded then lowercased.
    assert_eq!(url_canonicalize_name_if_able(b"%41BC.test"), b"abc.test");
    // A lone '%' that does not start an escape is forbidden.
    assert_eq!(url_canonicalize_name_if_able(b"a%zzb"), b"a%zzb");
    // IPv6 literal is compressed and lowercased.
    assert_eq!(
        url_canonicalize_name_if_able(b"[2001:DB8:0:0::1]"),
        b"[2001:db8::1]"
    );
    // Non-ASCII uppercase in a label that is otherwise ASCII.
    assert_eq!(
        url_canonicalize_name_if_able("WWW.G\u{00dc}.test".as_bytes()),
        b"www.xn--g-eha.test"
    );
}

#[test]
fn punycode_known_values() {
    // RFC 3492 section 7.1 sample "(A) Arabic" is long; use simple known pairs.
    // "bücher" -> "bcher-kva" (RFC 3492 style basic-then-delta encoding).
    assert_eq!(
        url_canonicalize_name_if_able("b\u{00fc}cher.test".as_bytes()),
        b"xn--bcher-kva.test"
    );
    // "münchen" -> "mnchen-3ya".
    assert_eq!(
        url_canonicalize_name_if_able("m\u{00fc}nchen.test".as_bytes()),
        b"xn--mnchen-3ya.test"
    );
}

/// Small xorshift64* PRNG. The crate has no rand dependency.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}
