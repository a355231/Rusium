// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Internal UTF-8 and hex helpers. These re-implement the parts of
//! `base/strings/utf_string_conversion_utils` and `base/third_party/icu/icu_utf.h`
//! that the escape code uses.

use crate::offset::Adjustment;

/// Returns the value of an ASCII hex digit, as `string_util.h`'s `IsHexDigit`
/// and `HexDigitToInt` do. Other bytes, including non-ASCII ones, return `None`.
pub(crate) fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn is_hex_digit(c: u8) -> bool {
    hex_value(c).is_some()
}

/// Decodes one UTF-8 character that starts at `src[index]`. `index` must be
/// less than `src.len()`.
///
/// On success returns the character and the number of bytes it occupies. On
/// failure returns the number of bytes consumed by the ill-formed sequence,
/// which is at least 1. That count covers the lead byte plus every following
/// byte that is still a valid prefix of a well-formed sequence. It matches ICU's
/// `CBU8_NEXT`: for `E4 BD 41` it is 2, for `80` it is 1.
///
/// Well-formedness follows Unicode Table 3-7. Overlong forms, surrogates and
/// code points above U+10FFFF are rejected. Noncharacters are accepted, as
/// `IsValidCodepoint` in the C++ does.
pub(crate) fn decode_utf8_char(src: &[u8], index: usize) -> Result<(char, usize), usize> {
    let lead = src[index];
    if lead < 0x80 {
        return Ok((char::from(lead), 1));
    }
    // (sequence length, allowed range of the first continuation byte).
    let (len, first_lo, first_hi): (usize, u8, u8) = match lead {
        0xC2..=0xDF => (2, 0x80, 0xBF),
        0xE0 => (3, 0xA0, 0xBF),
        0xE1..=0xEC | 0xEE..=0xEF => (3, 0x80, 0xBF),
        0xED => (3, 0x80, 0x9F),
        0xF0 => (4, 0x90, 0xBF),
        0xF1..=0xF3 => (4, 0x80, 0xBF),
        0xF4 => (4, 0x80, 0x8F),
        // Continuation bytes, C0/C1 (overlong) and F5..=FF can never start a
        // sequence.
        _ => return Err(1),
    };
    let mut cp = u32::from(match len {
        2 => lead & 0x1F,
        3 => lead & 0x0F,
        _ => lead & 0x07,
    });
    for k in 1..len {
        let Some(&b) = src.get(index + k) else {
            return Err(k);
        };
        let (lo, hi) = if k == 1 {
            (first_lo, first_hi)
        } else {
            (0x80, 0xBF)
        };
        if !(lo..=hi).contains(&b) {
            return Err(k);
        }
        cp = (cp << 6) | u32::from(b & 0x3F);
    }
    match char::from_u32(cp) {
        Some(ch) => Ok((ch, len)),
        // Unreachable: the range checks above exclude surrogates and values
        // above U+10FFFF.
        None => Err(1),
    }
}

/// Converts UTF-8 to a `String`, recording the adjustments the C++
/// `UTF8ToUTF16WithAdjustments` records. `adjustments` are measured in UTF-16
/// code units. Each run of invalid bytes becomes one U+FFFD. The returned flag
/// is `false` if any input was invalid.
pub(crate) fn utf8_to_utf16_with_adjustments(src: &[u8]) -> (String, Vec<Adjustment>, bool) {
    let mut out = String::with_capacity(src.len());
    let mut adjustments = Vec::new();
    let mut success = true;
    let mut i = 0;
    while i < src.len() {
        let start = i;
        let (ch, consumed) = match decode_utf8_char(src, i) {
            Ok((ch, n)) => (ch, n),
            Err(n) => {
                success = false;
                ('\u{FFFD}', n)
            }
        };
        out.push(ch);
        i += consumed;
        // The C++ records an adjustment only when the number of input bytes
        // differs from the number of UTF-16 units written.
        let units = ch.len_utf16();
        if consumed != units {
            adjustments.push(Adjustment::new(start, consumed, units));
        }
    }
    (out, adjustments, success)
}
