// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of base/strings/string_util_unittest.cc. Each C++ TEST/TEST_F has a
//! Rust test with the same name in snake_case and the same inputs and expected
//! outputs. Differences from the C++ are noted on the tests they affect.

use std::cmp::Ordering;

use string_util::*;
use utf_string_conversions::{ascii_to_utf16, utf16_to_ascii};

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Maps a `Ordering` to the -1/0/1 the C++ tests compare against.
fn cmp_i(a: &[u8], b: &[u8]) -> i32 {
    match compare_case_insensitive_ascii(a, b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// C++ `trim_cases` (wide input; the C++ WideToUTF16 is `encode_utf16`).
const TRIM_CASES: &[(&str, TrimPositions, &str, TrimPositions)] = &[
    (" Google Video ", TrimPositions::Leading, "Google Video ", TrimPositions::Leading),
    (" Google Video ", TrimPositions::Trailing, " Google Video", TrimPositions::Trailing),
    (" Google Video ", TrimPositions::All, "Google Video", TrimPositions::All),
    ("Google Video", TrimPositions::All, "Google Video", TrimPositions::None),
    ("", TrimPositions::All, "", TrimPositions::None),
    ("  ", TrimPositions::Leading, "", TrimPositions::Leading),
    ("  ", TrimPositions::Trailing, "", TrimPositions::Trailing),
    ("  ", TrimPositions::All, "", TrimPositions::All),
    ("\t\rTest String\n", TrimPositions::All, "Test String", TrimPositions::All),
    ("\u{2002}Test String\u{00A0}\u{3000}", TrimPositions::All, "Test String", TrimPositions::All),
];

const TRIM_CASES_ASCII: &[(&str, TrimPositions, &str, TrimPositions)] = &[
    (" Google Video ", TrimPositions::Leading, "Google Video ", TrimPositions::Leading),
    (" Google Video ", TrimPositions::Trailing, " Google Video", TrimPositions::Trailing),
    (" Google Video ", TrimPositions::All, "Google Video", TrimPositions::All),
    ("Google Video", TrimPositions::All, "Google Video", TrimPositions::None),
    ("", TrimPositions::All, "", TrimPositions::None),
    ("  ", TrimPositions::Leading, "", TrimPositions::Leading),
    ("  ", TrimPositions::Trailing, "", TrimPositions::Trailing),
    ("  ", TrimPositions::All, "", TrimPositions::All),
    ("\t\rTest String\n", TrimPositions::All, "Test String", TrimPositions::All),
];

/// Helper used to test IsStringUTF8[AllowingNoncharacters].
fn test_structurally_valid_utf8(f: fn(&[u8]) -> bool) {
    assert!(f(b"abc"));
    assert!(f(b"\xC2\x81"));
    assert!(f(b"\xE1\x80\xBF"));
    assert!(f(b"\xF1\x80\xA0\xBF"));
    assert!(f(b"\xF1\x80\xA0\xBF"));
    assert!(f(b"a\xC2\x81\xE1\x80\xBF\xF1\x80\xA0\xBF"));

    // U+FEFF used as UTF-8 BOM.
    assert!(f(b"\xEF\xBB\xBFabc"));

    // Embedded nulls in canonical UTF-8 representation.
    assert!(f(b"embedded\0null"));
}

/// Helper used to test IsStringUTF8[AllowingNoncharacters].
fn test_structurally_invalid_utf8(f: fn(&[u8]) -> bool) {
    // Invalid encoding of U+1FFFE (0x8F instead of 0x9F).
    assert!(!f(b"\xF0\x8F\xBF\xBE"));

    // Surrogate code points.
    assert!(!f(b"\xED\xA0\x80\xED\xBF\xBF"));
    assert!(!f(b"\xED\xA0\x8F"));
    assert!(!f(b"\xED\xBF\xBF"));

    // Overlong sequences.
    assert!(!f(b"\xC0\x80")); // U+0000
    assert!(!f(b"\xC1\x80\xC1\x81")); // "AB"
    assert!(!f(b"\xE0\x80\x80")); // U+0000
    assert!(!f(b"\xE0\x82\x80")); // U+0080
    assert!(!f(b"\xE0\x9F\xBF")); // U+07FF
    assert!(!f(b"\xF0\x80\x80\x8D")); // U+000D
    assert!(!f(b"\xF0\x80\x82\x91")); // U+0091
    assert!(!f(b"\xF0\x80\xA0\x80")); // U+0800
    assert!(!f(b"\xF0\x8F\xBB\xBF")); // U+FEFF (BOM)
    assert!(!f(b"\xF8\x80\x80\x80\xBF")); // U+003F
    assert!(!f(b"\xFC\x80\x80\x80\xA0\xA5")); // U+00A5

    // Beyond U+10FFFF (the upper limit of Unicode codespace).
    assert!(!f(b"\xF4\x90\x80\x80")); // U+110000
    assert!(!f(b"\xF8\xA0\xBF\x80\xBF")); // 5 bytes
    assert!(!f(b"\xFC\x9C\xBF\x80\xBF\x80")); // 6 bytes

    // BOM in UTF-16(BE|LE).
    assert!(!f(b"\xFE\xFF"));
    assert!(!f(b"\xFF\xFE"));

    // Strings in legacy encodings. We can make up strings in a legacy encoding
    // that are valid in UTF-8, but in real data most of them are invalid.

    // cafe with U+00E9 in ISO-8859-1.
    assert!(!f(b"caf\xE9"));
    // U+AC00, U+AC001 in EUC-KR.
    assert!(!f(b"\xB0\xA1\xB0\xA2"));
    // U+4F60 U+597D in Big5.
    assert!(!f(b"\xA7\x41\xA6\x6E"));
    // "abc" with U+201[CD] in windows-125[0-8].
    assert!(!f(b"\x93abc\x94"));
    // U+0639 U+064E U+0644 U+064E in ISO-8859-6.
    assert!(!f(b"\xD9\xEE\xE4\xEE"));
    // U+03B3 U+03B5 U+03B9 U+03AC in ISO-8859-7.
    assert!(!f(b"\xE3\xE5\xE9\xDC"));

    // BOM in UTF-32(BE|LE).
    assert!(!f(b"\x00\x00\xFE\xFF"));
    assert!(!f(b"\xFF\xFE\x00\x00"));
}

/// Helper used to test IsStringUTF8[AllowingNoncharacters].
fn test_noncharacters(f: fn(&[u8]) -> bool, expected_result: bool) {
    assert_eq!(f(b"\xEF\xB7\x90"), expected_result); // U+FDD0
    assert_eq!(f(b"\xEF\xB7\x9F"), expected_result); // U+FDDF
    assert_eq!(f(b"\xEF\xB7\xAF"), expected_result); // U+FDEF
    assert_eq!(f(b"\xEF\xBF\xBE"), expected_result); // U+FFFE
    assert_eq!(f(b"\xEF\xBF\xBF"), expected_result); // U+FFFF
    assert_eq!(f(b"\xF0\x9F\xBF\xBE"), expected_result); // U+01FFFE
    assert_eq!(f(b"\xF0\x9F\xBF\xBF"), expected_result); // U+01FFFF
    assert_eq!(f(b"\xF0\xAF\xBF\xBE"), expected_result); // U+02FFFE
    assert_eq!(f(b"\xF0\xAF\xBF\xBF"), expected_result); // U+02FFFF
    assert_eq!(f(b"\xF0\xBF\xBF\xBE"), expected_result); // U+03FFFE
    assert_eq!(f(b"\xF0\xBF\xBF\xBF"), expected_result); // U+03FFFF
    assert_eq!(f(b"\xF1\x8F\xBF\xBE"), expected_result); // U+04FFFE
    assert_eq!(f(b"\xF1\x8F\xBF\xBF"), expected_result); // U+04FFFF
    assert_eq!(f(b"\xF1\x9F\xBF\xBE"), expected_result); // U+05FFFE
    assert_eq!(f(b"\xF1\x9F\xBF\xBF"), expected_result); // U+05FFFF
    assert_eq!(f(b"\xF1\xAF\xBF\xBE"), expected_result); // U+06FFFE
    assert_eq!(f(b"\xF1\xAF\xBF\xBF"), expected_result); // U+06FFFF
    assert_eq!(f(b"\xF1\xBF\xBF\xBE"), expected_result); // U+07FFFE
    assert_eq!(f(b"\xF1\xBF\xBF\xBF"), expected_result); // U+07FFFF
    assert_eq!(f(b"\xF2\x8F\xBF\xBE"), expected_result); // U+08FFFE
    assert_eq!(f(b"\xF2\x8F\xBF\xBF"), expected_result); // U+08FFFF
    assert_eq!(f(b"\xF2\x9F\xBF\xBE"), expected_result); // U+09FFFE
    assert_eq!(f(b"\xF2\x9F\xBF\xBF"), expected_result); // U+09FFFF
    assert_eq!(f(b"\xF2\xAF\xBF\xBE"), expected_result); // U+0AFFFE
    assert_eq!(f(b"\xF2\xAF\xBF\xBF"), expected_result); // U+0AFFFF
    assert_eq!(f(b"\xF2\xBF\xBF\xBE"), expected_result); // U+0BFFFE
    assert_eq!(f(b"\xF2\xBF\xBF\xBF"), expected_result); // U+0BFFFF
    assert_eq!(f(b"\xF3\x8F\xBF\xBE"), expected_result); // U+0CFFFE
    assert_eq!(f(b"\xF3\x8F\xBF\xBF"), expected_result); // U+0CFFFF
    assert_eq!(f(b"\xF3\x9F\xBF\xBE"), expected_result); // U+0DFFFE
    assert_eq!(f(b"\xF3\x9F\xBF\xBF"), expected_result); // U+0DFFFF
    assert_eq!(f(b"\xF3\xAF\xBF\xBE"), expected_result); // U+0EFFFE
    assert_eq!(f(b"\xF3\xAF\xBF\xBF"), expected_result); // U+0EFFFF
    assert_eq!(f(b"\xF3\xBF\xBF\xBE"), expected_result); // U+0FFFFE
    assert_eq!(f(b"\xF3\xBF\xBF\xBF"), expected_result); // U+0FFFFF
    assert_eq!(f(b"\xF4\x8F\xBF\xBE"), expected_result); // U+10FFFE
    assert_eq!(f(b"\xF4\x8F\xBF\xBF"), expected_result); // U+10FFFF
}

#[test]
fn truncate_utf8_to_byte_size() {
    // The C++ out-parameter form is covered by the slice-returning form here.
    assert_eq!(truncate_utf8_to_byte_size(b"test", 3), b"tes");
    assert_eq!(truncate_utf8_to_byte_size(b"test", 0), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"", 3), b"");

    // Empty strings and invalid byte_size arguments.
    assert_eq!(truncate_utf8_to_byte_size(b"", 0), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe1\x80\xbf", 0), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe1\x80\xbf", usize::MAX), b"\xe1\x80\xbf");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe1\x80\xbf", 4), b"\xe1\x80\xbf");

    // Testing the truncation of valid UTF8 correctly.
    assert_eq!(truncate_utf8_to_byte_size(b"abc", 2), b"ab");
    assert_eq!(truncate_utf8_to_byte_size(b"\xc2\x81\xc2\x81", 2), b"\xc2\x81");
    assert_eq!(truncate_utf8_to_byte_size(b"\xc2\x81\xc2\x81", 3), b"\xc2\x81");
    assert_eq!(truncate_utf8_to_byte_size(b"\xc2\x81\xc2\x81", 4), b"\xc2\x81\xc2\x81");

    {
        let array = b"\x00\x00\xc2\x81\xc2\x81";
        assert_eq!(truncate_utf8_to_byte_size(array, 4), b"\x00\x00\xc2\x81");
    }
    {
        let array = b"\x00\xc2\x81\xc2\x81";
        assert_eq!(truncate_utf8_to_byte_size(array, 4), b"\x00\xc2\x81");
    }

    // Testing invalid UTF8.
    assert_eq!(truncate_utf8_to_byte_size(b"\xed\xa0\x80\xed\xbf\xbf", 6), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xed\xa0\x8f", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xed\xbf\xbf", 3), b"");

    // Testing invalid UTF8 mixed with valid UTF8.
    assert_eq!(truncate_utf8_to_byte_size(b"\xe1\x80\xbf", 3), b"\xe1\x80\xbf");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf1\x80\xa0\xbf", 4), b"\xf1\x80\xa0\xbf");
    assert_eq!(
        truncate_utf8_to_byte_size(b"a\xc2\x81\xe1\x80\xbf\xf1\x80\xa0\xbf", 10),
        b"a\xc2\x81\xe1\x80\xbf\xf1\x80\xa0\xbf"
    );
    assert_eq!(
        truncate_utf8_to_byte_size(b"a\xc2\x81\xe1\x80\xbf\xf1a\x80\xa0", 10),
        b"a\xc2\x81\xe1\x80\xbf\xf1a"
    );
    assert_eq!(truncate_utf8_to_byte_size(b"\xef\xbb\xbfabc", 6), b"\xef\xbb\xbfabc");

    // Overlong sequences.
    assert_eq!(truncate_utf8_to_byte_size(b"\xc0\x80", 2), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xc1\x80\xc1\x81", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe0\x80\x80", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe0\x82\x80", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xe0\x9f\xbf", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf0\x80\x80\x8D", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf0\x80\x82\x91", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf0\x80\xa0\x80", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf0\x8f\xbb\xbf", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf8\x80\x80\x80\xbf", 5), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xfc\x80\x80\x80\xa0\xa5", 6), b"");

    // Beyond U+10FFFF (the upper limit of Unicode codespace).
    assert_eq!(truncate_utf8_to_byte_size(b"\xf4\x90\x80\x80", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf8\xa0\xbf\x80\xbf", 5), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xfc\x9c\xbf\x80\xbf\x80", 6), b"");

    // BOMs in UTF-16(BE|LE) and UTF-32(BE|LE).
    assert_eq!(truncate_utf8_to_byte_size(b"\xfe\xff", 2), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xff\xfe", 2), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\x00\x00\xfe\xff", 4), b"\x00\x00");

    // Variants on the previous test.
    assert_eq!(truncate_utf8_to_byte_size(b"\xff\xfe\x00\x00", 4), b"\xff\xfe\x00\x00");
    assert_eq!(truncate_utf8_to_byte_size(b"\xff\x00\x00\xfe", 4), b"\xff\x00\x00");

    // Non-characters: U+xxFFF[EF] where xx is 0x00 through 0x10 and <FDD0,FDEF>.
    assert_eq!(truncate_utf8_to_byte_size(b"\xef\xbf\xbe", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf0\x8f\xbf\xbe", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xf3\xbf\xbf\xbf", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xef\xb7\x90", 3), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xef\xb7\xaf", 3), b"");

    // Strings in legacy encodings that are valid in UTF-8, but are invalid as
    // UTF-8 in real data.
    assert_eq!(truncate_utf8_to_byte_size(b"caf\xe9", 4), b"caf");
    assert_eq!(truncate_utf8_to_byte_size(b"\xb0\xa1\xb0\xa2", 4), b"");
    assert_eq!(truncate_utf8_to_byte_size(b"\xa7\x41\xa6\x6e", 4), b"\xa7\x41\xa6\x6e");
    assert_eq!(
        truncate_utf8_to_byte_size(b"\xa7\x41\xa6\x6e\xd9\xee\xe4\xee", 7),
        b"\xa7\x41\xa6\x6e"
    );

    // "abc" with U+201[CD] in windows-125[0-8].
    assert_eq!(truncate_utf8_to_byte_size(b"\x93abc\x94", 5), b"\x93abc");

    // U+0639 U+064E U+0644 U+064E in ISO-8859-6.
    assert_eq!(truncate_utf8_to_byte_size(b"\xd9\xee\xe4\xee", 4), b"");

    // U+03B3 U+03B5 U+03B9 U+03AC in ISO-8859-7.
    assert_eq!(truncate_utf8_to_byte_size(b"\xe3\xe5\xe9\xdc", 4), b"");

    // The &str form agrees on valid input. Non-characters are dropped there too.
    assert_eq!(truncate_utf8_to_byte_size_str("ab\u{FFFE}", 5), "ab");
    assert_eq!(truncate_utf8_to_byte_size_str("h\u{e9}llo", 3), "h\u{e9}");
}

#[test]
fn trim_whitespace() {
    for &(input, positions, output, ret) in TRIM_CASES {
        let (returned, trimmed) = trim_whitespace_to_vec(&u16s(input), positions);
        assert_eq!(returned, ret, "input {input:?}");
        assert_eq!(trimmed, u16s(output), "input {input:?}");
    }

    // The C++ TrimWhitespace() can take the same string for input and output.
    // The Rust form returns a new Vec, so that aliasing has no analog.
    let (pos, out) = trim_whitespace_to_vec(&u16s("  This is a test \r\n"), TrimPositions::All);
    assert_eq!(pos, TrimPositions::All);
    assert_eq!(out, u16s("This is a test"));

    // Once more, but with a string of whitespace.
    let (pos, out) = trim_whitespace_to_vec(&u16s("  \r\n"), TrimPositions::All);
    assert_eq!(pos, TrimPositions::All);
    assert_eq!(out, u16s(""));

    for &(input, positions, output, ret) in TRIM_CASES_ASCII {
        let (returned, trimmed) = trim_whitespace_ascii_to_vec(input.as_bytes(), positions);
        assert_eq!(returned, ret, "input {input:?}");
        assert_eq!(trimmed, output.as_bytes(), "input {input:?}");
    }
}

/// C++ `collapse_cases` (wide). Each row is (input, trim, output).
const COLLAPSE_CASES: &[(&str, bool, &str)] = &[
    (" Google Video ", false, "Google Video"),
    ("Google Video", false, "Google Video"),
    ("", false, ""),
    ("  ", false, ""),
    ("\t\rTest String\n", false, "Test String"),
    ("\u{2002}Test String\u{00A0}\u{3000}", false, "Test String"),
    ("    Test     \n  \t String    ", false, "Test String"),
    ("\u{2002}Test\u{1680} \u{2028} \tString\u{00A0}\u{3000}", false, "Test String"),
    ("   Test String", false, "Test String"),
    ("Test String    ", false, "Test String"),
    ("Test String", false, "Test String"),
    ("", true, ""),
    ("\n", true, ""),
    ("  \r  ", true, ""),
    ("\nFoo", true, "Foo"),
    ("\r  Foo  ", true, "Foo"),
    (" Foo bar ", true, "Foo bar"),
    ("  \tFoo  bar  \n", true, "Foo bar"),
    (" a \r b\n c \r\n d \t\re \t f \n ", true, "abcde f"),
];

#[test]
fn collapse_whitespace() {
    for &(input, trim, output) in COLLAPSE_CASES {
        assert_eq!(collapse_whitespace(&u16s(input), trim), u16s(output), "input {input:?}");
    }
}

/// C++ `collapse_cases_ascii`.
const COLLAPSE_CASES_ASCII: &[(&str, bool, &str)] = &[
    (" Google Video ", false, "Google Video"),
    ("Google Video", false, "Google Video"),
    ("", false, ""),
    ("  ", false, ""),
    ("\t\rTest String\n", false, "Test String"),
    ("    Test     \n  \t String    ", false, "Test String"),
    ("   Test String", false, "Test String"),
    ("Test String    ", false, "Test String"),
    ("Test String", false, "Test String"),
    ("", true, ""),
    ("\n", true, ""),
    ("  \r  ", true, ""),
    ("\nFoo", true, "Foo"),
    ("\r  Foo  ", true, "Foo"),
    (" Foo bar ", true, "Foo bar"),
    //   is whitespace, but not ASCII whitespace, so it is not collapsed.
    ("Foo\u{00A0}bar", true, "Foo\u{00A0}bar"),
    ("  \tFoo  bar  \n", true, "Foo bar"),
    (" a \r b\n c \r\n d \t\re \t f \n ", true, "abcde f"),
];

#[test]
fn collapse_whitespace_ascii() {
    for &(input, trim, output) in COLLAPSE_CASES_ASCII {
        assert_eq!(collapse_whitespace_ascii(input.as_bytes(), trim), output.as_bytes());
        assert_eq!(collapse_whitespace_ascii_str(input, trim), output);
    }
}

#[test]
fn is_string_utf8() {
    test_structurally_valid_utf8(string_util::is_string_utf8_allowing_noncharacters);
    test_structurally_invalid_utf8(string_util::is_string_utf8_allowing_noncharacters);
    test_noncharacters(string_util::is_string_utf8_allowing_noncharacters, true);

    test_structurally_valid_utf8(string_util::is_string_utf8);
    test_structurally_invalid_utf8(string_util::is_string_utf8);
    test_noncharacters(string_util::is_string_utf8, false);
}

#[test]
fn is_string_ascii() {
    // Test a variety of the fragment start positions and lengths, so the
    // ASCII check is exercised at every alignment. Also test that a non-ASCII
    // unit is detected wherever it sits in the string.
    let mut char_ascii: Vec<u8> = b"0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF".to_vec();
    let string_length = char_ascii.len();
    for offset in 0..8 {
        for len in 0..(string_length - offset) {
            assert!(is_string_ascii(&char_ascii[offset..offset + len]));
            for char_pos in offset..len {
                char_ascii[char_pos] |= 0x80;
                assert!(!is_string_ascii(&char_ascii[offset..offset + len]));
                char_ascii[char_pos] &= !0x80;
            }
        }
    }

    let mut char16_ascii = u16s("01234567890ABCDEF01234567890ABCDEF");
    let string_length = char16_ascii.len();
    for offset in 0..4 {
        for len in 0..(string_length - offset) {
            assert!(is_string_ascii(&char16_ascii[offset..offset + len]));
            for char_pos in offset..len {
                char16_ascii[char_pos] |= 0x80;
                assert!(!is_string_ascii(&char16_ascii[offset..offset + len]));
                char16_ascii[char_pos] &= !0x80;
                // Also test when the upper half is non-zero.
                char16_ascii[char_pos] |= 0x100;
                assert!(!is_string_ascii(&char16_ascii[offset..offset + len]));
                char16_ascii[char_pos] &= !0x100;
            }
        }
    }
}

#[test]
fn find_first_non_ascii() {
    // Empty string.
    assert_eq!(find_first_non_ascii(b""), 0);
    assert_eq!(find_first_non_ascii(&u16s("")), 0);

    // All ASCII strings (returns length).
    assert_eq!(find_first_non_ascii(b"Hello"), 5);
    assert_eq!(find_first_non_ascii(b"Hello World!"), 12);
    assert_eq!(find_first_non_ascii(&u16s("Hello World")), 11);

    let long_ascii = vec![b'a'; 200];
    assert_eq!(find_first_non_ascii(&long_ascii), 200);

    // Non-ASCII at the start, the middle, and the end.
    assert_eq!(find_first_non_ascii(b"\x80World"), 0);
    assert_eq!(find_first_non_ascii(b"Hello\x80World"), 5);
    assert_eq!(find_first_non_ascii(&u16s("Hello\u{0080}World")), 5);

    // Non-ASCII after an offset that leaves the start unaligned.
    let prologue_str = b"   \x80Hello";
    assert_eq!(find_first_non_ascii(&prologue_str[3..]), 0);

    // Non-ASCII inside a 128-byte batch, and in the second batch.
    let mut batch_str = vec![b'a'; 200];
    batch_str[50] = 0x80;
    assert_eq!(find_first_non_ascii(&batch_str), 50);

    batch_str[50] = b'a';
    batch_str[127] = 0x80;
    assert_eq!(find_first_non_ascii(&batch_str), 127);

    batch_str[127] = b'a';
    batch_str[150] = 0x80;
    assert_eq!(find_first_non_ascii(&batch_str), 150);

    // Non-ASCII in the trailing bytes after the full batches.
    let mut remainder_str = vec![b'a'; 135];
    remainder_str[129] = 0x80;
    assert_eq!(find_first_non_ascii(&remainder_str), 129);
}

#[test]
fn convert_ascii() {
    let char_cases: [&str; 3] = ["Google Video", "Hello, world\n", "0123ABCDwxyz \x07\x08\t\r\n!+,.~"];
    let wchar_cases: [&str; 3] = char_cases;

    for (i, &case) in char_cases.iter().enumerate() {
        assert!(is_string_ascii(case.as_bytes()));
        let utf16 = ascii_to_utf16(case.as_bytes());
        assert_eq!(u16s(wchar_cases[i]), utf16);

        let ascii = utf16_to_ascii(&u16s(wchar_cases[i]));
        assert_eq!(case.as_bytes(), ascii.as_slice());
    }

    assert!(!is_string_ascii(b"Google \x80Video"));

    // Convert empty strings.
    let empty16: Vec<u16> = Vec::new();
    let empty: Vec<u8> = Vec::new();
    assert_eq!(empty, utf16_to_ascii(&empty16));
    assert_eq!(empty16, ascii_to_utf16(&empty));

    // Convert strings with an embedded NUL character.
    let chars_with_nul = b"test\0string";
    let string_with_nul: &[u8] = &chars_with_nul[..];
    let string16_with_nul = ascii_to_utf16(string_with_nul);
    assert_eq!(string_with_nul.len(), string16_with_nul.len());
    let narrow_with_nul = utf16_to_ascii(&string16_with_nul);
    assert_eq!(string_with_nul.len(), narrow_with_nul.len());
    assert_eq!(string_with_nul, narrow_with_nul.as_slice());
}

#[test]
fn to_lower_ascii() {
    assert_eq!(to_lower_ascii_char(b'C'), b'c');
    assert_eq!(to_lower_ascii_char(b'c'), b'c');
    assert_eq!(to_lower_ascii_char(b'2'), b'2');

    assert_eq!(to_lower_ascii_char(u16::from(b'C')), u16::from(b'c'));
    assert_eq!(to_lower_ascii_char(u16::from(b'c')), u16::from(b'c'));
    assert_eq!(to_lower_ascii_char(u16::from(b'2')), u16::from(b'2'));

    assert_eq!(to_lower_ascii(b"Cc2"), b"cc2");
    assert_eq!(to_lower_ascii(&u16s("Cc2")), u16s("cc2"));
    assert_eq!(to_lower_ascii_str("Cc2"), "cc2");

    // Non-ASCII characters are unmodified. U+00C4 is LATIN CAPITAL LETTER A WITH
    // DIAERESIS.
    assert_eq!(to_lower_ascii_char(0xc4u8), 0xc4);
    assert_eq!(to_lower_ascii_char(0x00c4u16), 0x00c4);
}

#[test]
fn to_upper_ascii() {
    assert_eq!(to_upper_ascii_char(b'C'), b'C');
    assert_eq!(to_upper_ascii_char(b'c'), b'C');
    assert_eq!(to_upper_ascii_char(b'2'), b'2');

    assert_eq!(to_upper_ascii_char(u16::from(b'C')), u16::from(b'C'));
    assert_eq!(to_upper_ascii_char(u16::from(b'c')), u16::from(b'C'));
    assert_eq!(to_upper_ascii_char(u16::from(b'2')), u16::from(b'2'));

    assert_eq!(to_upper_ascii(b"Cc2"), b"CC2");
    assert_eq!(to_upper_ascii(&u16s("Cc2")), u16s("CC2"));
    assert_eq!(to_upper_ascii_str("Cc2"), "CC2");

    // Non-ASCII characters are unmodified. U+00E4 is LATIN SMALL LETTER A WITH
    // DIAERESIS.
    assert_eq!(to_upper_ascii_char(0xe4u8), 0xe4);
    assert_eq!(to_upper_ascii_char(0x00e4u16), 0x00e4);
}
