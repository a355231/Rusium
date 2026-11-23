// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of the escaping half of `base/strings/escape.{h,cc}`.

use crate::utf8::is_hex_digit;

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// A set of byte values, stored as 256 bits. Mirrors the C++ `Charmap`.
#[derive(Clone, Copy)]
struct Charmap([u32; 8]);

impl Charmap {
    fn contains(self, c: u8) -> bool {
        self.0[usize::from(c >> 5)] & (1 << (c & 31)) != 0
    }
}

// Everything except alphanumerics and -._~ (RFC 3986 unreserved).
const UNRESERVED: Charmap = Charmap([
    0xffff_ffff,
    0xfc00_9fff,
    0x7800_0001,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Everything except alphanumerics and !'()*-._~.
const QUERY: Charmap = Charmap([
    0xffff_ffff,
    0xfc00_987d,
    0x7800_0001,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Non-printable, non-7bit, and (including space) "#%:<>?[\]^`{|}.
const PATH: Charmap = Charmap([
    0xffff_ffff,
    0xd400_002d,
    0x7800_0000,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Non-printable, non-7bit, and (including space) "#%<>[\]^`{|}.
const NS_URL: Charmap = Charmap([
    0xffff_ffff,
    0x5000_002d,
    0x7800_0000,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Form-urlencoded: non-printable, non-7bit, and (including space) and the
// characters the C++ table lists as special.
const URL_ESCAPE: Charmap = Charmap([
    0xffff_ffff,
    0xf800_08fd,
    0x7800_0001,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Non-7bit bytes, and '%'.
const NON_ASCII_AND_PERCENT: Charmap = Charmap([
    0x0000_0000,
    0x0000_0020,
    0x0000_0000,
    0x0000_0000,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Non-7bit bytes.
const NON_ASCII: Charmap = Charmap([
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

// Everything except alphanumerics, the reserved characters ;/?:@&=+$,, and
// !'()*-._~#[].
const EXTERNAL_HANDLER: Charmap = Charmap([
    0xffff_ffff,
    0x5000_0025,
    0x5000_0000,
    0xb800_0001,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
]);

fn push_percent_encoded(out: &mut String, c: u8) {
    out.push('%');
    out.push(char::from(HEX_UPPER[usize::from(c >> 4)]));
    out.push(char::from(HEX_UPPER[usize::from(c & 0x0f)]));
}

/// Escapes `text` using `charmap`. With `use_plus`, a space becomes `+`. With
/// `keep_escaped`, a `%` that begins a valid `%XX` sequence is left as is.
///
/// Every charmap here contains all bytes >= 0x80, so bytes that are not
/// escaped are ASCII and the output is valid UTF-8.
fn escape(text: &[u8], charmap: Charmap, use_plus: bool, keep_escaped: bool) -> String {
    let mut escaped = String::with_capacity(text.len() * 3);
    for (i, &c) in text.iter().enumerate() {
        if use_plus && c == b' ' {
            escaped.push('+');
        } else if keep_escaped
            && c == b'%'
            && i + 2 < text.len()
            && is_hex_digit(text[i + 1])
            && is_hex_digit(text[i + 2])
        {
            escaped.push('%');
        } else if charmap.contains(c) {
            push_percent_encoded(&mut escaped, c);
        } else {
            debug_assert!(c.is_ascii());
            escaped.push(char::from(c));
        }
    }
    escaped
}

/// Escapes everything except the RFC 3986 unreserved characters
/// (alphanumerics and `-._~`).
pub fn escape_all_except_unreserved(text: &[u8]) -> String {
    escape(text, UNRESERVED, false, false)
}

/// Escapes text for use as a query parameter value. Everything except
/// alphanumerics and `-_.!~*'()` is percent-encoded. With `use_plus`, a space
/// becomes `+`; otherwise it becomes `%20`. Matches JavaScript's
/// `encodeURIComponent`.
pub fn escape_query_param_value(text: &[u8], use_plus: bool) -> String {
    escape(text, QUERY, use_plus, false)
}

/// Escapes a partial or complete file or path name. Escapes non-printable and
/// non-7-bit bytes, space, and `"#%:<>?[\]^`{|}`.
pub fn escape_path(path: &[u8]) -> String {
    escape(path, PATH, false, false)
}

/// Escapes a string as NSURL expects. Escapes non-printable and non-7-bit
/// bytes, space, and `"#%<>[\]^`{|}`. Apple-only in the C++; compiled here
/// on all platforms.
pub fn escape_ns_url_precursor(precursor: &[u8]) -> String {
    escape(precursor, NS_URL, false, true)
}

/// Escapes `application/x-www-form-urlencoded` content. Escapes
/// non-printable and non-7-bit bytes, space, and the form-special characters.
/// With `use_plus`, a space becomes `+`; otherwise it becomes `%20`.
pub fn escape_url_encoded_data(path: &[u8], use_plus: bool) -> String {
    escape(path, URL_ESCAPE, use_plus, false)
}

/// Escapes every non-ASCII byte, and also `%` (as `%25`). Unescaping the
/// result gives back the original input.
pub fn escape_non_ascii_and_percent(input: &[u8]) -> String {
    escape(input, NON_ASCII_AND_PERCENT, false, false)
}

/// Escapes every non-ASCII byte. A `%` is left as is, so unescaping the result
/// does not give back the original input.
pub fn escape_non_ascii(input: &[u8]) -> String {
    escape(input, NON_ASCII, false, false)
}

/// Escapes text for use as an external protocol handler command. Everything
/// except alphanumerics, `-_.!~*'()`, the reserved characters `;/?:@&=+$,`,
/// `#[]`, and a valid `%XX` sequence is percent-encoded.
pub fn escape_external_handler_value(text: &[u8]) -> String {
    escape(text, EXTERNAL_HANDLER, false, true)
}

/// Appends `c` to `output`, replacing it with an entity if it would be read as
/// HTML markup: `<`, `>`, `&`, `"`, `'`.
pub fn append_escaped_char_for_html(c: u8, output: &mut Vec<u8>) {
    let replacement: &[u8] = match c {
        b'<' => b"&lt;",
        b'>' => b"&gt;",
        b'&' => b"&amp;",
        b'"' => b"&quot;",
        b'\'' => b"&#39;",
        _ => {
            output.push(c);
            return;
        }
    };
    output.extend_from_slice(replacement);
}

/// Escapes the characters in `input` that could be read as HTML markup. Works
/// on bytes, so the output is the same as the C++ `std::string` overload. Any
/// non-ASCII bytes pass through unchanged.
pub fn escape_for_html(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(input.len());
    for &c in input {
        append_escaped_char_for_html(c, &mut result);
    }
    result
}

#[cfg(test)]
mod charmap_tests {
    use super::*;

    // Each bitmap is checked against a reference set written out by hand.
    // The reference does not come from the bitmap.

    #[test]
    fn unreserved_bitmap_matches_rfc3986() {
        for c in 0..=u8::MAX {
            let unreserved = c.is_ascii_alphanumeric() || b"-._~".contains(&c);
            assert_eq!(!UNRESERVED.contains(c), unreserved, "byte {c:#04x}");
        }
    }

    #[test]
    fn query_bitmap_matches_documented_set() {
        for c in 0..=u8::MAX {
            let unescaped = c.is_ascii_alphanumeric() || b"!'()*-._~".contains(&c);
            assert_eq!(!QUERY.contains(c), unescaped, "byte {c:#04x}");
        }
    }

    #[test]
    fn path_bitmap_matches_documented_set() {
        for c in 0..=u8::MAX {
            let escaped = c <= 0x20 || c >= 0x7F || b"\"#%:<>?[\\]^`{|}".contains(&c);
            assert_eq!(PATH.contains(c), escaped, "byte {c:#04x}");
        }
    }

    #[test]
    fn ns_url_bitmap_matches_documented_set() {
        for c in 0..=u8::MAX {
            let escaped = c <= 0x20 || c >= 0x7F || b"\"#%<>[\\]^`{|}".contains(&c);
            assert_eq!(NS_URL.contains(c), escaped, "byte {c:#04x}");
        }
    }

    #[test]
    fn url_escape_bitmap_matches_observed_set() {
        // '!', '_', ',', '-', '.', '/', ':' are not escaped, as in the C++
        // bitmap. The header comment lists '!' but the bitmap and the unit test
        // both leave it alone.
        for c in 0..=u8::MAX {
            let escaped = c <= 0x20 || c >= 0x7F || b"\"#$%&'+;<=>?@[\\]^`{|}".contains(&c);
            assert_eq!(URL_ESCAPE.contains(c), escaped, "byte {c:#04x}");
        }
    }

    #[test]
    fn external_handler_bitmap_matches_documented_set() {
        for c in 0..=u8::MAX {
            let escaped = c <= 0x20 || c >= 0x7F || b"\"%<>\\^`{|}".contains(&c);
            assert_eq!(EXTERNAL_HANDLER.contains(c), escaped, "byte {c:#04x}");
        }
    }

    #[test]
    fn non_ascii_bitmaps() {
        for c in 0..=u8::MAX {
            assert_eq!(NON_ASCII.contains(c), c >= 0x80, "byte {c:#04x}");
            assert_eq!(NON_ASCII_AND_PERCENT.contains(c), c >= 0x80 || c == b'%', "byte {c:#04x}");
        }
    }
}
