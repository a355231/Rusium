// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of base/strings/utf_string_conversions.{h,cc}.
//!
//! Every conversion has the same rule for ill-formed input: each ill-formed
//! unit is replaced by one U+FFFD, the number of input units it covers follows
//! the ICU rules in `icu_utf.rs`, and `success` is false if any replacement was
//! made. The `_lossy` functions return only the text, like the C++ overloads
//! that return a string.

use crate::conversion_utils::{
    read_unicode_character_utf16, read_unicode_character_utf32, read_unicode_character_utf8,
    write_unicode_character_utf16, write_unicode_character_utf32, write_unicode_character_utf8,
    REPLACEMENT_CHARACTER,
};

/// Result of a conversion: the converted text, and whether the input was
/// well-formed (`success == false` if any replacement character was written).
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct Converted<T> {
    pub text: T,
    pub success: bool,
}

/// Shared driver for every conversion. It mirrors `DoUTFConversion` and
/// `ConvertUnicode`: each step reads one character with `read` (which leaves the
/// index at the last unit consumed), writes it or U+FFFD with `write`, and calls
/// `record(original_index, units_consumed, units_written)` when the consumed and
/// written lengths differ. Only the offset code needs `record`.
pub(crate) fn transcode<S, D: Default>(
    src: &[S],
    mut read: impl FnMut(&[S], &mut usize) -> Option<char>,
    mut write: impl FnMut(char, &mut D) -> usize,
    mut record: impl FnMut(usize, usize, usize),
) -> Converted<D> {
    let mut output = D::default();
    let mut success = true;
    let mut i = 0;
    while i < src.len() {
        let original_i = i;
        let written = match read(src, &mut i) {
            Some(code_point) => write(code_point, &mut output),
            None => {
                success = false;
                write(REPLACEMENT_CHARACTER, &mut output)
            }
        };
        let consumed = i - original_i + 1;
        if consumed != written {
            record(original_i, consumed, written);
        }
        i += 1;
    }
    Converted { text: output, success }
}

// UTF-8 <-> UTF-16 ------------------------------------------------------------

/// Converts UTF-8 to UTF-16. See the module docs for the error rules.
pub fn utf8_to_utf16(src: &[u8]) -> Converted<Vec<u16>> {
    transcode(src, read_unicode_character_utf8, write_unicode_character_utf16, |_, _, _| {})
}

/// Like [`utf8_to_utf16`], but returns only the text.
pub fn utf8_to_utf16_lossy(src: &[u8]) -> Vec<u16> {
    utf8_to_utf16(src).text
}

/// Converts UTF-16 to UTF-8. A lone surrogate becomes U+FFFD. Surrogate pairs
/// are combined.
pub fn utf16_to_utf8(src: &[u16]) -> Converted<String> {
    transcode(src, read_unicode_character_utf16, write_unicode_character_utf8, |_, _, _| {})
}

/// Like [`utf16_to_utf8`], but returns only the text.
pub fn utf16_to_utf8_lossy(src: &[u16]) -> String {
    utf16_to_utf8(src).text
}

// Wide (UTF-32) <-> UTF-8 and UTF-16 ------------------------------------------
// "Wide" is `wchar_t` as a 32-bit UTF-32 unit (`u32` here). Values that are not
// Unicode scalar values (surrogates and values above U+10FFFF) become U+FFFD.

/// Converts wide (UTF-32) text to UTF-8.
pub fn wide_to_utf8(src: &[u32]) -> Converted<String> {
    transcode(src, read_unicode_character_utf32, write_unicode_character_utf8, |_, _, _| {})
}

/// Like [`wide_to_utf8`], but returns only the text.
pub fn wide_to_utf8_lossy(src: &[u32]) -> String {
    wide_to_utf8(src).text
}

/// Converts UTF-8 to wide (UTF-32) text.
pub fn utf8_to_wide(src: &[u8]) -> Converted<Vec<u32>> {
    transcode(src, read_unicode_character_utf8, write_unicode_character_utf32, |_, _, _| {})
}

/// Like [`utf8_to_wide`], but returns only the text.
pub fn utf8_to_wide_lossy(src: &[u8]) -> Vec<u32> {
    utf8_to_wide(src).text
}

/// Converts wide (UTF-32) text to UTF-16.
pub fn wide_to_utf16(src: &[u32]) -> Converted<Vec<u16>> {
    transcode(src, read_unicode_character_utf32, write_unicode_character_utf16, |_, _, _| {})
}

/// Like [`wide_to_utf16`], but returns only the text.
pub fn wide_to_utf16_lossy(src: &[u32]) -> Vec<u16> {
    wide_to_utf16(src).text
}

/// Converts UTF-16 to wide (UTF-32) text.
pub fn utf16_to_wide(src: &[u16]) -> Converted<Vec<u32>> {
    transcode(src, read_unicode_character_utf16, write_unicode_character_utf32, |_, _, _| {})
}

/// Like [`utf16_to_wide`], but returns only the text.
pub fn utf16_to_wide_lossy(src: &[u16]) -> Vec<u32> {
    utf16_to_wide(src).text
}

// ASCII helpers ----------------------------------------------------------------

/// Widens an ASCII byte string to UTF-16. The C++ `ASCIIToUTF16` DCHECKs that
/// the input is ASCII; that check is not ported. Non-ASCII bytes are widened
/// one-to-one, as in a release build of the C++.
pub fn ascii_to_utf16(ascii: &[u8]) -> Vec<u16> {
    ascii.iter().map(|&b| u16::from(b)).collect()
}

/// Truncates each UTF-16 unit to its low 8 bits. The C++ `UTF16ToASCII`
/// DCHECKs that the input is ASCII. That check is not ported, and the result is
/// bytes rather than a `String` because truncation can produce non-ASCII bytes.
pub fn utf16_to_ascii(utf16: &[u16]) -> Vec<u8> {
    utf16.iter().map(|&unit| unit as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::XorShift64;

    /// Reference for UTF-8 -> UTF-16: std's lossy decoder uses the same maximal
    /// subpart replacement as ICU, and `success` is true exactly for valid UTF-8.
    fn reference_utf8_to_utf16(src: &[u8]) -> (Vec<u16>, bool) {
        (String::from_utf8_lossy(src).encode_utf16().collect(), std::str::from_utf8(src).is_ok())
    }

    fn reference_utf16_to_utf8(src: &[u16]) -> (String, bool) {
        let mut success = true;
        let text = char::decode_utf16(src.iter().copied())
            .map(|r| {
                r.unwrap_or_else(|_| {
                    success = false;
                    REPLACEMENT_CHARACTER
                })
            })
            .collect();
        (text, success)
    }

    fn reference_wide_to_utf8(src: &[u32]) -> (String, bool) {
        let mut success = true;
        let text = src
            .iter()
            .map(|&c| {
                char::from_u32(c).unwrap_or_else(|| {
                    success = false;
                    REPLACEMENT_CHARACTER
                })
            })
            .collect();
        (text, success)
    }

    // Port of UTFStringConversionsTest.ConvertUTF8AndWide. The 16-bit non-BMP
    // case is included as UTF-16 because it is compiled out on 32-bit wchar_t.
    #[test]
    fn convert_utf8_and_wide() {
        let cases: &[&str] = &[
            "Google Video",
            "\u{7f51}\u{9875}\u{0020}\u{56fe}\u{7247}\u{0020}\u{8d44}\u{8baf}\u{66f4}\u{591a}\u{0020}\u{00bb}",
            "\u{03a0}\u{03b1}\u{03b3}\u{03ba}\u{03cc}\u{03c3}\u{03bc}\u{03b9}\u{03bf}\u{03c2}\u{0020}\u{0399}\u{03c3}\u{03c4}\u{03cc}\u{03c2}",
            "\u{041f}\u{043e}\u{0438}\u{0441}\u{043a}\u{0020}\u{0441}\u{0442}\u{0440}\u{0430}\u{043d}\u{0438}\u{0446}\u{0020}\u{043d}\u{0430}\u{0020}\u{0440}\u{0443}\u{0441}\u{0441}\u{043a}\u{043e}\u{043c}",
            "\u{c804}\u{ccb4}\u{c11c}\u{be44}\u{c2a4}",
            "\u{10300}",
            "\u{11d40}\u{11d41}\u{11d42}\u{11d43}\u{11d44}",
        ];
        for case in cases {
            let wide: Vec<u32> = case.chars().map(u32::from).collect();
            let utf8 = wide_to_utf8(&wide);
            assert!(utf8.success);
            assert_eq!(utf8.text, *case);
            let back = utf8_to_wide(utf8.text.as_bytes());
            assert!(back.success);
            assert_eq!(back.text, wide);
        }
        // Non-BMP (16-bit wchar_t case), via UTF-16.
        let utf16: Vec<u16> = "\u{10300}".encode_utf16().collect();
        assert_eq!(utf16, [0xD800, 0xDF00]);
        assert_eq!(utf16_to_utf8(&utf16).text, "\u{10300}");
    }

    // Port of UTFStringConversionsTest.ConvertUTF8AndWideEmptyString.
    #[test]
    fn convert_utf8_and_wide_empty_string() {
        assert_eq!(wide_to_utf8_lossy(&[]), "");
        assert_eq!(utf8_to_wide_lossy(b""), Vec::<u32>::new());
    }

    // Port of UTFStringConversionsTest.ConvertUTF8ToWide.
    #[test]
    fn convert_utf8_to_wide() {
        struct Case {
            utf8: &'static [u8],
            wide: &'static [u32],
            success: bool,
        }
        let cases = [
            // Regular UTF-8 input.
            Case { utf8: b"\xe4\xbd\xa0\xe5\xa5\xbd", wide: &[0x4f60, 0x597d], success: true },
            // Non-character is passed through.
            Case {
                utf8: b"\xef\xbf\xbfHello",
                wide: &[0xffff, 'H' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32],
                success: true,
            },
            // Truncated UTF-8 sequence.
            Case { utf8: b"\xe4\xa0\xe5\xa5\xbd", wide: &[0xfffd, 0x597d], success: false },
            // Truncated off the end.
            Case { utf8: b"\xe5\xa5\xbd\xe4\xa0", wide: &[0x597d, 0xfffd], success: false },
            // Non-shortest-form UTF-8.
            Case {
                utf8: b"\xf0\x84\xbd\xa0\xe5\xa5\xbd",
                wide: &[0xfffd, 0xfffd, 0xfffd, 0xfffd, 0x597d],
                success: false,
            },
            // This UTF-8 character decodes to a UTF-16 surrogate, which is illegal.
            Case { utf8: b"\xed\xb0\x80", wide: &[0xfffd, 0xfffd, 0xfffd], success: false },
            // Non-BMP characters, as UTF-32 (32-bit wchar_t).
            Case { utf8: b"A\xF0\x90\x8C\x80z", wide: &[0x41, 0x10300, 0x7a], success: true },
            Case { utf8: b"A\xF4\x8F\xBF\xBEz", wide: &[0x41, 0x10fffe, 0x7a], success: true },
        ];
        for case in &cases {
            let converted = utf8_to_wide(case.utf8);
            assert_eq!(converted.success, case.success);
            assert_eq!(converted.text, case.wide);
        }

        // The same non-BMP cases as UTF-16 (16-bit wchar_t expectations).
        let utf16_cases: [(&[u8], &[u16], bool); 2] = [
            (b"A\xF0\x90\x8C\x80z", &[0x41, 0xd800, 0xdf00, 0x7a], true),
            (b"A\xF4\x8F\xBF\xBEz", &[0x41, 0xdbff, 0xdffe, 0x7a], true),
        ];
        for (utf8, expected, success) in utf16_cases {
            let converted = utf8_to_utf16(utf8);
            assert_eq!(converted.success, success);
            assert_eq!(converted.text, expected);
        }

        // Manually test an embedded NULL.
        let converted = utf8_to_wide(b"\0Z\t");
        assert!(converted.success);
        assert_eq!(converted.text, [0, 'Z' as u32, '\t' as u32]);

        // Conversion returns a new value; it does not append to an old one.
        let converted = utf8_to_wide(b"B");
        assert!(converted.success);
        assert_eq!(converted.text, ['B' as u32]);
    }

    // Port of UTFStringConversionsTest.ConvertUTF16ToUTF8 (the case set that is
    // compiled only when wchar_t is 16-bit, applied to UTF-16 directly).
    #[test]
    fn convert_utf16_to_utf8() {
        struct Case {
            utf16: &'static [u16],
            utf8: &'static [u8],
            success: bool,
        }
        let cases = [
            // Regular UTF-16 input.
            Case { utf16: &[0x4f60, 0x597d], utf8: b"\xe4\xbd\xa0\xe5\xa5\xbd", success: true },
            // Test a non-BMP character.
            Case { utf16: &[0xd800, 0xdf00], utf8: b"\xF0\x90\x8C\x80", success: true },
            // Non-characters are passed through.
            Case {
                utf16: &[0xffff, 'H' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16],
                utf8: b"\xEF\xBF\xBFHello",
                success: true,
            },
            Case {
                utf16: &[
                    0xdbff, 0xdffe, 'H' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16,
                ],
                utf8: b"\xF4\x8F\xBF\xBEHello",
                success: true,
            },
            // The first character is a truncated UTF-16 character.
            Case { utf16: &[0xd800, 0x597d], utf8: b"\xef\xbf\xbd\xe5\xa5\xbd", success: false },
            // Truncated at the end.
            Case { utf16: &[0x597d, 0xd800], utf8: b"\xe5\xa5\xbd\xef\xbf\xbd", success: false },
        ];
        for case in &cases {
            let converted = utf16_to_utf8(case.utf16);
            assert_eq!(converted.success, case.success);
            assert_eq!(converted.text.as_bytes(), case.utf8);
        }
    }

    // Port of UTFStringConversionsTest.ConvertUTF32ToUTF8 (active on 32-bit wchar_t).
    #[test]
    fn convert_utf32_to_utf8() {
        struct Case {
            utf32: &'static [u32],
            utf8: &'static [u8],
            success: bool,
        }
        let cases = [
            // Regular 16-bit input.
            Case { utf32: &[0x4f60, 0x597d], utf8: b"\xe4\xbd\xa0\xe5\xa5\xbd", success: true },
            // Test a non-BMP character.
            Case { utf32: &[0x41, 0x10300, 0x7a], utf8: b"A\xF0\x90\x8C\x80z", success: true },
            // Non-characters are passed through.
            Case {
                utf32: &[0xffff, 'H' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32],
                utf8: b"\xEF\xBF\xBFHello",
                success: true,
            },
            Case {
                utf32: &[0x10fffe, 'H' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32],
                utf8: b"\xF4\x8F\xBF\xBEHello",
                success: true,
            },
            // Invalid Unicode code points.
            Case {
                utf32: &[0x0fff_ffff, 'H' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32],
                utf8: b"\xEF\xBF\xBDHello",
                success: false,
            },
            // The first character is a truncated UTF-16 character.
            Case { utf32: &[0xd800, 0x597d], utf8: b"\xef\xbf\xbd\xe5\xa5\xbd", success: false },
            Case {
                utf32: &[0xdc01, 'H' as u32, 'e' as u32, 'l' as u32, 'l' as u32, 'o' as u32],
                utf8: b"\xef\xbf\xbdHello",
                success: false,
            },
        ];
        for case in &cases {
            let converted = wide_to_utf8(case.utf32);
            assert_eq!(converted.success, case.success);
            assert_eq!(converted.text.as_bytes(), case.utf8);
        }
    }

    // Port of UTFStringConversionsTest.ConvertMultiString.
    #[test]
    fn convert_multi_string() {
        let multistring16: Vec<u16> = "foo\0bar\0baz\0".encode_utf16().collect();
        assert_eq!(multistring16.len(), 12);
        let expected = "foo\0bar\0baz\0";
        assert_eq!(expected.len(), 12);
        assert_eq!(utf16_to_utf8_lossy(&multistring16), expected);
    }

    // Port of UTFStringConversionsTest.UTFConversionASCIISequenceGapsCoverage.
    // The C++ test compares the ASCII fast path with the legacy path. This port
    // has one path, so each result is compared with std's reference decoder
    // instead, and `success` with `std::str::from_utf8`.
    #[test]
    fn utf_conversion_ascii_sequence_gaps_coverage() {
        let mut test_strings: Vec<Vec<u8>> = vec![
            b"a".to_vec(),
            b"main-container".to_vec(),
            b"id_caf\xc3\xa9".to_vec(),
            b"\xe6\xa4\x9c\xe7\xb4\xa2".to_vec(),
            b"Hello \xf0\x9f\x91\x8b World \xf0\x9f\x8c\x8d Chromium \xf0\x9f\x9a\x80".to_vec(),
        ];
        let mut long_json = b"{\"name\": \"Jos\xc3\xa9\", \"payload\": \"".to_vec();
        long_json.extend(std::iter::repeat_n(b'x', 1000));
        long_json.extend_from_slice(b"\"}");
        test_strings.push(long_json);
        test_strings.push(vec![b'x'; 10000]);
        let mut with_cent = vec![b'x'; 10000];
        with_cent.extend_from_slice(b"\xc2\xa2");
        with_cent.extend(std::iter::repeat_n(b'y', 500));
        test_strings.push(with_cent);
        let mut with_gap = vec![b'z'; 50000];
        with_gap.extend_from_slice(b"\xe6\xa4\x9c");
        test_strings.push(with_gap);
        let mut japanese = Vec::new();
        while japanese.len() < 10000 {
            japanese.extend_from_slice(b"\xe6\x97\xa5\xe6\x9c\xac\xe8\xaa\x9e");
        }
        test_strings.push(japanese);
        test_strings.push(b"\xff".to_vec());
        test_strings.push(b"Hello\xffWorld".to_vec());
        let mut ff_127 = vec![b'A'; 127];
        ff_127.push(0xff);
        ff_127.extend(std::iter::repeat_n(b'B', 100));
        test_strings.push(ff_127);
        let mut ff_1023 = vec![b'A'; 1023];
        ff_1023.push(0xff);
        ff_1023.extend(std::iter::repeat_n(b'B', 100));
        test_strings.push(ff_1023);

        for input in &test_strings {
            let converted = utf8_to_utf16(input);
            let (expected, expected_success) = reference_utf8_to_utf16(input);
            assert_eq!(converted.success, expected_success);
            assert_eq!(converted.text, expected);

            let back = utf16_to_utf8(&converted.text);
            let (expected_back, back_success) = reference_utf16_to_utf8(&converted.text);
            assert_eq!(back.success, back_success);
            assert_eq!(back.text, expected_back);
        }
    }

    fn random_bytes(rng: &mut XorShift64) -> Vec<u8> {
        const INTERESTING: [u8; 12] =
            [0x00, 0x41, 0x7f, 0x80, 0xa0, 0xbf, 0xc0, 0xc2, 0xdf, 0xe0, 0xed, 0xf0];
        let len = rng.below(48);
        (0..len)
            .map(|_| {
                if rng.below(2) == 0 {
                    INTERESTING[rng.below(INTERESTING.len())]
                } else {
                    rng.next_u8()
                }
            })
            .collect()
    }

    fn random_utf16(rng: &mut XorShift64) -> Vec<u16> {
        let len = rng.below(32);
        (0..len)
            .map(|_| match rng.below(4) {
                0 => 0xd800 + rng.below(0x400) as u16,
                1 => 0xdc00 + rng.below(0x400) as u16,
                _ => rng.next_u16(),
            })
            .collect()
    }

    fn random_wide(rng: &mut XorShift64) -> Vec<u32> {
        let len = rng.below(32);
        (0..len)
            .map(|_| match rng.below(4) {
                0 => 0xd800 + rng.below(0x800) as u32,
                1 => 0x110000 + rng.below(0x1000) as u32,
                _ => rng.next_u32() % 0x11_0000,
            })
            .collect()
    }

    // Port of the FUZZ_TEST UTF8ToUTF16FastPathMatchesLegacy: fixed corpus plus
    // random inputs, compared with the std reference.
    #[test]
    fn utf8_to_utf16_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0001);
        for _ in 0..20000 {
            let input = random_bytes(&mut rng);
            let converted = utf8_to_utf16(&input);
            let (expected, success) = reference_utf8_to_utf16(&input);
            assert_eq!((converted.text, converted.success), (expected, success), "{input:02x?}");
        }
    }

    // Port of the FUZZ_TEST UTF16ToUTF8FastPathMatchesLegacy.
    #[test]
    fn utf16_to_utf8_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0002);
        for _ in 0..20000 {
            let input = random_utf16(&mut rng);
            let converted = utf16_to_utf8(&input);
            let (expected, success) = reference_utf16_to_utf8(&input);
            assert_eq!((converted.text, converted.success), (expected, success), "{input:x?}");
        }
    }

    // Port of the FUZZ_TEST UTF8ToWideFastPathMatchesLegacy.
    #[test]
    fn utf8_to_wide_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0003);
        for _ in 0..20000 {
            let input = random_bytes(&mut rng);
            let converted = utf8_to_wide(&input);
            let expected_wide: Vec<u32> =
                String::from_utf8_lossy(&input).chars().map(u32::from).collect();
            assert_eq!(converted.text, expected_wide, "{input:02x?}");
            assert_eq!(converted.success, std::str::from_utf8(&input).is_ok());
        }
    }

    // Port of the FUZZ_TEST WideToUTF8FastPathMatchesLegacy.
    #[test]
    fn wide_to_utf8_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0004);
        for _ in 0..20000 {
            let input = random_wide(&mut rng);
            let converted = wide_to_utf8(&input);
            let (expected, success) = reference_wide_to_utf8(&input);
            assert_eq!((converted.text, converted.success), (expected, success), "{input:x?}");
        }
    }

    // Port of the FUZZ_TEST WideToUTF16FastPathMatchesLegacy.
    #[test]
    fn wide_to_utf16_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0005);
        for _ in 0..20000 {
            let input = random_wide(&mut rng);
            let converted = wide_to_utf16(&input);
            let (text, success) = reference_wide_to_utf8(&input);
            let expected: Vec<u16> = text.encode_utf16().collect();
            assert_eq!((converted.text, converted.success), (expected, success), "{input:x?}");
        }
    }

    // Port of the FUZZ_TEST UTF16ToWideFastPathMatchesLegacy.
    #[test]
    fn utf16_to_wide_matches_reference() {
        let mut rng = XorShift64::new(0x5eed_0006);
        for _ in 0..20000 {
            let input = random_utf16(&mut rng);
            let converted = utf16_to_wide(&input);
            let (text, success) = reference_utf16_to_utf8(&input);
            let expected: Vec<u32> = text.chars().map(u32::from).collect();
            assert_eq!((converted.text, converted.success), (expected, success), "{input:x?}");
        }
    }

    // Port of UTFStringConversionsTest.ValidatingViewOverloads.
    #[test]
    fn validating_view_overloads() {
        let converted = utf8_to_utf16(b"\xe4\xbd\xa0\xe5\xa5\xbd");
        assert!(converted.success);
        assert_eq!(converted.text, [0x4f60, 0x597d]);
        let converted = utf8_to_utf16(b"\xe4\xa0\xe5\xa5\xbd");
        assert!(!converted.success);
        assert_eq!(converted.text, [0xfffd, 0x597d]);

        let converted = utf16_to_utf8(&[0x4f60, 0x597d]);
        assert!(converted.success);
        assert_eq!(converted.text.as_bytes(), b"\xe4\xbd\xa0\xe5\xa5\xbd");
        let converted = utf16_to_utf8(&[0xd800, 0x597d]);
        assert!(!converted.success);
        assert_eq!(converted.text.as_bytes(), b"\xef\xbf\xbd\xe5\xa5\xbd");

        let converted = utf16_to_wide(&[0x4f60, 0x597d]);
        assert!(converted.success);
        assert_eq!(converted.text, [0x4f60, 0x597d]);
        let converted = wide_to_utf16(&[0x4f60, 0x597d]);
        assert!(converted.success);
        assert_eq!(converted.text, [0x4f60, 0x597d]);

        let converted = utf16_to_wide(&[0xd800, 0x597d]);
        assert!(!converted.success);
        assert_eq!(converted.text, [0xfffd, 0x597d]);
        let converted = wide_to_utf16(&[0xd800, 0x597d]);
        assert!(!converted.success);
        assert_eq!(converted.text, [0xfffd, 0x597d]);
    }

    // Exhaustive over all sequences of up to 4 bytes drawn from representative
    // lead, continuation, and boundary bytes (22^4 cases). Compared with std.
    #[test]
    fn utf8_representative_sequences_match_std_exhaustively() {
        const BYTES: [u8; 22] = [
            0x00, 0x41, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xc1, 0xc2, 0xdf, 0xe0,
            0xe1, 0xed, 0xef, 0xf0, 0xf1, 0xf4, 0xf5, 0xff,
        ];
        let mut sequence = Vec::with_capacity(4);
        for len in 0..=4u32 {
            for mut code in 0..BYTES.len().pow(len) {
                sequence.clear();
                for _ in 0..len {
                    sequence.push(BYTES[code % BYTES.len()]);
                    code /= BYTES.len();
                }
                let converted = utf8_to_utf16(&sequence);
                let (expected, success) = reference_utf8_to_utf16(&sequence);
                assert_eq!(
                    (converted.text, converted.success),
                    (expected, success),
                    "{sequence:02x?}"
                );
            }
        }
    }

    #[test]
    fn success_matches_std_validity_for_random_utf8() {
        let mut rng = XorShift64::new(0x5eed_0007);
        for _ in 0..2000 {
            let input = random_bytes(&mut rng);
            assert_eq!(
                utf8_to_utf16(&input).success,
                std::str::from_utf8(&input).is_ok(),
                "{input:02x?}"
            );
        }
    }

    #[test]
    fn utf8_surrogate_and_overlong_forms_are_rejected() {
        // Encoded surrogate: one U+FFFD per byte.
        assert_eq!(utf8_to_utf16_lossy(b"\xed\xa0\x80"), [0xfffd, 0xfffd, 0xfffd]);
        // Overlong encodings of '/' and U+0000 (C0 AF, E0 80 80, F0 80 80 80).
        assert_eq!(utf8_to_utf16_lossy(b"\xc0\xaf"), [0xfffd, 0xfffd]);
        assert_eq!(utf8_to_utf16_lossy(b"\xe0\x80\x80"), [0xfffd, 0xfffd, 0xfffd]);
        assert_eq!(utf8_to_utf16_lossy(b"\xf0\x80\x80\x80"), [0xfffd, 0xfffd, 0xfffd, 0xfffd]);
        // Above U+10FFFF: F4 90 80 80 is rejected at the second byte.
        assert_eq!(utf8_to_utf16_lossy(b"\xf4\x90\x80\x80"), [0xfffd, 0xfffd, 0xfffd, 0xfffd]);
        // A truncated 4-byte sequence at the end is one U+FFFD.
        let converted = utf8_to_utf16(b"\xf0\x90\x80");
        assert!(!converted.success);
        assert_eq!(converted.text, [0xfffd]);
        // A valid prefix followed by a non-continuation byte.
        assert_eq!(utf8_to_utf16_lossy(b"\xf0\x90A"), [0xfffd, 0x41]);
    }

    #[test]
    fn lone_surrogates_and_out_of_range_wide_values() {
        assert_eq!(utf16_to_utf8_lossy(&[0xdc00, 0x41]), "\u{fffd}A");
        assert_eq!(utf16_to_utf8_lossy(&[0xd800, 0xd800, 0xdc00]), "\u{fffd}\u{10000}");
        assert_eq!(wide_to_utf8_lossy(&[0x110000, 0x41]), "\u{fffd}A");
        assert!(!wide_to_utf8(&[0x8000_0000]).success);
    }

    #[test]
    fn ascii_helpers() {
        assert_eq!(ascii_to_utf16(b"abc"), [0x61, 0x62, 0x63]);
        assert_eq!(ascii_to_utf16(b""), Vec::<u16>::new());
        // Truncation keeps the low byte.
        assert_eq!(utf16_to_ascii(&[0x61, 0x0162, 0x00e9]), [0x61, 0x62, 0xe9]);
    }
}
