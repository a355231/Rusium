// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! UTF-8 validation and truncation from base/strings/string_util.h.

/// C++ `IsValidCodepoint` (utf_string_conversion_utils.h). Rejects surrogates
/// and values above U+10FFFF. Non-characters are allowed.
pub(crate) fn is_valid_codepoint(cp: u32) -> bool {
    cp < 0xD800 || (0xE000..=0x10FFFF).contains(&cp)
}

/// C++ `IsValidCharacter` (utf_string_conversion_utils.h). Like
/// [`is_valid_codepoint`], and also rejects non-characters: U+FDD0-U+FDEF and
/// every code point ending in FFFE or FFFF.
pub(crate) fn is_valid_character(cp: u32) -> bool {
    is_valid_codepoint(cp) && !(0xFDD0..=0xFDEF).contains(&cp) && (cp & 0xFFFE) != 0xFFFE
}

/// C++ `IsStringUTF8`: structurally valid UTF-8 with no non-characters.
pub fn is_string_utf8(s: &[u8]) -> bool {
    match std::str::from_utf8(s) {
        Ok(text) => text.chars().all(|c| is_valid_character(u32::from(c))),
        Err(_) => false,
    }
}

/// C++ `IsStringUTF8AllowingNoncharacters`: structurally valid UTF-8.
pub fn is_string_utf8_allowing_noncharacters(s: &[u8]) -> bool {
    std::str::from_utf8(s).is_ok()
}

/// Decodes the UTF-8 sequence at `start`, reading no bytes at or beyond
/// `end`. Returns the code point and the index just past the sequence, or
/// `None` if the sequence is ill-formed or does not fit before `end`. This
/// mirrors ICU's CBU8_NEXT, which the C++ uses with a truncated length.
/// Ill-formed input is reported as `None` instead of a sentinel.
fn decode_bounded(s: &[u8], start: usize, end: usize) -> Option<(u32, usize)> {
    let len = match s[start] {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    let next = start + len;
    if next > end {
        return None;
    }
    let text = std::str::from_utf8(&s[start..next]).ok()?;
    let c = text.chars().next()?;
    Some((u32::from(c), next))
}

/// C++ `TruncateUTF8ToByteSize(std::string_view, size_t)`: returns the longest
/// prefix of at most `byte_size` bytes that ends on a complete, valid
/// character. Non-characters count as invalid, and so do ill-formed sequences.
/// Returns the whole input when it is no longer than `byte_size`.
pub fn truncate_utf8_to_byte_size(input: &[u8], byte_size: usize) -> &[u8] {
    if byte_size > input.len() {
        return input;
    }
    let length = byte_size;
    // Walk backwards from the truncation point, looking for a valid character
    // that ends at or before it.
    let mut char_index: isize = length as isize - 1;
    while char_index >= 0 {
        let prev = char_index as usize;
        match decode_bounded(input, prev, length) {
            Some((cp, next)) if is_valid_character(cp) => {
                char_index = next as isize;
                break;
            }
            _ => char_index = prev as isize - 1,
        }
    }
    if char_index >= 0 {
        &input[..char_index as usize]
    } else {
        &input[..0]
    }
}

/// `&str` form of [`truncate_utf8_to_byte_size`]. The result is a prefix that
/// ends on a character boundary, so it is always valid UTF-8.
pub fn truncate_utf8_to_byte_size_str(input: &str, byte_size: usize) -> &str {
    let kept = truncate_utf8_to_byte_size(input.as_bytes(), byte_size).len();
    input.get(..kept).unwrap_or("")
}
