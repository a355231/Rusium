// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Low-level code point helpers. Port of
//! base/strings/utf_string_conversion_utils.{h,cc}.
//!
//! Code points are `u32` for the validity predicates, because the C++ takes a
//! signed `UChar32` and every negative value is invalid. The readers return
//! `char`, which is a valid code point by construction; `None` replaces the C++
//! `false` return (the rejected value itself is not exposed).

use crate::icu_utf::decode_utf8;

/// U+FFFD REPLACEMENT CHARACTER, substituted for every ill-formed input unit.
pub const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

/// Returns true for Unicode scalar values: everything except surrogates
/// (U+D800..U+DFFF) and values above U+10FFFF. Non-characters are allowed.
pub fn is_valid_codepoint(code_point: u32) -> bool {
    code_point < 0xD800 || (0xE000..=0x10FFFF).contains(&code_point)
}

/// Returns true for valid code points that are also not non-characters:
/// U+FDD0..U+FDEF and every code point ending in U+FFFE or U+FFFF are excluded.
pub fn is_valid_character(code_point: u32) -> bool {
    (code_point < 0xD800)
        || ((0xE000..0xFDD0).contains(&code_point))
        || (code_point > 0xFDEF && code_point <= 0x10FFFF && (code_point & 0xFFFE) != 0xFFFE)
}

/// Returns the number of characters in `text`, stopping once `limit` is
/// reached. Returns `None` if the text contains an ill-formed sequence before
/// that point. Pass `usize::MAX` for no limit.
pub fn count_unicode_characters(text: &[u8], limit: usize) -> Option<usize> {
    let mut count = 0;
    let mut index = 0;
    while count < limit && index < text.len() {
        read_unicode_character_utf8(text, &mut index)?;
        count += 1;
        index += 1;
    }
    Some(count)
}

/// Reads the UTF-8 character at `*char_index`. On return `*char_index` is the
/// index of the last byte consumed, so `+1` gives the next character. This is
/// also true when the sequence is ill-formed, where `None` is returned.
///
/// If `*char_index` is out of range, returns `None` and leaves it unchanged.
pub fn read_unicode_character_utf8(src: &[u8], char_index: &mut usize) -> Option<char> {
    let start = *char_index;
    if start >= src.len() {
        return None;
    }
    let (code_point, next) = decode_utf8(src, start);
    *char_index = next - 1;
    code_point
}

/// Reads the UTF-16 character at `*char_index` (a surrogate pair counts as one
/// character). A lone surrogate returns `None` and leaves `*char_index`
/// unchanged, so the caller advances by one unit. On success for a pair,
/// `*char_index` moves to the trail unit, as in the C++ version.
pub fn read_unicode_character_utf16(src: &[u16], char_index: &mut usize) -> Option<char> {
    let start = *char_index;
    let unit = *src.get(start)?;
    if !(0xD800..=0xDFFF).contains(&unit) {
        return char::from_u32(u32::from(unit));
    }
    // Only a lead surrogate followed by a trail surrogate is valid.
    if !(0xD800..=0xDBFF).contains(&unit) {
        return None;
    }
    let trail = *src.get(start + 1)?;
    if !(0xDC00..=0xDFFF).contains(&trail) {
        return None;
    }
    *char_index = start + 1;
    let code_point = 0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(trail) - 0xDC00);
    char::from_u32(code_point)
}

/// Reads the UTF-32 (wide) character at `*char_index`. Values that are not
/// Unicode scalar values return `None`. `*char_index` is never changed.
pub fn read_unicode_character_utf32(src: &[u32], char_index: &mut usize) -> Option<char> {
    char::from_u32(*src.get(*char_index)?)
}

/// Appends `code_point` as UTF-8 to `output`. Returns the number of bytes
/// written. Taking `char` means surrogates and out-of-range values cannot be
/// passed in; the C++ version would write ill-formed bytes for them.
pub fn write_unicode_character_utf8(code_point: char, output: &mut String) -> usize {
    output.push(code_point);
    code_point.len_utf8()
}

/// Appends `code_point` as UTF-16 to `output`. Returns the number of 16-bit
/// units written (1 or 2).
pub fn write_unicode_character_utf16(code_point: char, output: &mut Vec<u16>) -> usize {
    let mut buffer = [0u16; 2];
    let units = code_point.encode_utf16(&mut buffer);
    output.extend_from_slice(units);
    units.len()
}

/// Appends `code_point` as one UTF-32 (wide) unit to `output`. Returns 1.
pub fn write_unicode_character_utf32(code_point: char, output: &mut Vec<u32>) -> usize {
    output.push(u32::from(code_point));
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_codepoint_boundaries() {
        assert!(is_valid_codepoint(0));
        assert!(is_valid_codepoint(0xD7FF));
        assert!(!is_valid_codepoint(0xD800));
        assert!(!is_valid_codepoint(0xDFFF));
        assert!(is_valid_codepoint(0xE000));
        assert!(is_valid_codepoint(0x10FFFF));
        assert!(!is_valid_codepoint(0x110000));
        assert!(!is_valid_codepoint(0x8000_0000));
        assert!(!is_valid_codepoint(u32::MAX));
    }

    #[test]
    fn valid_character_excludes_noncharacters() {
        assert!(is_valid_character(0xFDCF));
        assert!(!is_valid_character(0xFDD0));
        assert!(!is_valid_character(0xFDEF));
        assert!(is_valid_character(0xFDF0));
        assert!(!is_valid_character(0xFFFE));
        assert!(!is_valid_character(0xFFFF));
        assert!(!is_valid_character(0x1FFFE));
        assert!(!is_valid_character(0x10FFFF));
        assert!(is_valid_character(0x10FFFD));
        assert!(!is_valid_character(0xD800));
        assert!(!is_valid_character(0x110000));
    }

    #[test]
    fn count_unicode_characters_cases() {
        // Port of UtfStringConversionUtilsTest.CountUnicodeCharacters.
        let cases: &[(&[u8], usize, Option<usize>)] = &[
            (b"", 0, Some(0)),
            (b"abc", 1, Some(1)),
            (b"abc", 3, Some(3)),
            (b"abc", 0, Some(0)),
            (b"abc", 4, Some(3)),
            ("abc\u{1F4A9}".as_bytes(), 4, Some(4)),
            ("\u{1F4A9}".as_bytes(), 1, Some(1)),
            (&[1, 0xFF], 5, None),
        ];
        for &(text, limit, expected) in cases {
            assert_eq!(count_unicode_characters(text, limit), expected);
        }
    }

    #[test]
    fn count_unicode_characters_limit_stops_before_bad_bytes() {
        // The limit is reached before the ill-formed byte, so it is not checked.
        assert_eq!(count_unicode_characters(b"ab\xff", 2), Some(2));
        assert_eq!(count_unicode_characters(b"ab\xff", 3), None);
    }

    #[test]
    fn read_utf8_advances_to_last_byte() {
        let text = "a\u{4f60}b".as_bytes();
        let mut index = 1;
        assert_eq!(read_unicode_character_utf8(text, &mut index), Some('\u{4f60}'));
        assert_eq!(index, 3); // Last byte of the 3-byte sequence.
        index += 1;
        assert_eq!(read_unicode_character_utf8(text, &mut index), Some('b'));
        // Out of range: fails and leaves the index unchanged.
        let mut past_end = text.len();
        assert_eq!(read_unicode_character_utf8(text, &mut past_end), None);
        assert_eq!(past_end, text.len());
    }

    #[test]
    fn read_utf8_error_index_points_at_last_consumed_byte() {
        // E4 A0 is a truncated sequence before E5, so two bytes are consumed.
        let text = b"\xe4\xa0\xe5\xa5\xbd";
        let mut index = 0;
        assert_eq!(read_unicode_character_utf8(text, &mut index), None);
        assert_eq!(index, 1);
        index += 1;
        assert_eq!(read_unicode_character_utf8(text, &mut index), Some('\u{597d}'));
        assert_eq!(index, 4);

        // A lone continuation byte is consumed on its own.
        let mut lone = 0;
        assert_eq!(read_unicode_character_utf8(b"\xa0", &mut lone), None);
        assert_eq!(lone, 0);
    }

    #[test]
    fn read_utf16_pairs_and_lone_surrogates() {
        let text = [0x61, 0xD800, 0xDF00, 0xD800, 0x62, 0xDC00];
        let mut index = 0;
        assert_eq!(read_unicode_character_utf16(&text, &mut index), Some('a'));
        index += 1;
        assert_eq!(read_unicode_character_utf16(&text, &mut index), Some('\u{10300}'));
        assert_eq!(index, 2); // Moved to the trail unit.
        index += 1;
        // Lead followed by a non-trail: fails, index unchanged.
        let mut bad = 3;
        assert_eq!(read_unicode_character_utf16(&text, &mut bad), None);
        assert_eq!(bad, 3);
        // Trail on its own: fails.
        let mut trail = 5;
        assert_eq!(read_unicode_character_utf16(&text, &mut trail), None);
        // Lead at the end of input: fails.
        let mut at_end = 3;
        assert_eq!(read_unicode_character_utf16(&[0xD800u16], &mut at_end), None);
    }

    #[test]
    fn read_utf32_rejects_invalid_values() {
        let text = [0x41, 0xD800, 0x110000, 0x10FFFF];
        let mut index = 0;
        assert_eq!(read_unicode_character_utf32(&text, &mut index), Some('A'));
        assert_eq!(index, 0);
        index = 1;
        assert_eq!(read_unicode_character_utf32(&text, &mut index), None);
        index = 2;
        assert_eq!(read_unicode_character_utf32(&text, &mut index), None);
        index = 3;
        assert_eq!(read_unicode_character_utf32(&text, &mut index), Some('\u{10FFFF}'));
        index = 4;
        assert_eq!(read_unicode_character_utf32(&text, &mut index), None);
    }

    #[test]
    fn write_returns_units_written() {
        let mut s = String::new();
        assert_eq!(write_unicode_character_utf8('a', &mut s), 1);
        assert_eq!(write_unicode_character_utf8('\u{4f60}', &mut s), 3);
        assert_eq!(write_unicode_character_utf8('\u{10300}', &mut s), 4);
        assert_eq!(s, "a\u{4f60}\u{10300}");

        let mut u = Vec::new();
        assert_eq!(write_unicode_character_utf16('a', &mut u), 1);
        assert_eq!(write_unicode_character_utf16('\u{10300}', &mut u), 2);
        assert_eq!(u, [0x61, 0xD800, 0xDF00]);

        let mut w = Vec::new();
        assert_eq!(write_unicode_character_utf32('\u{10300}', &mut w), 1);
        assert_eq!(w, [0x10300]);
    }
}
