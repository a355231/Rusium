// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of the unescaping half of `base/strings/escape.{h,cc}`.

use crate::offset::{merge_sequential_adjustments, Adjustment};
use crate::utf8::{decode_utf8_char, hex_value, utf8_to_utf16_with_adjustments};

/// Flags for the unescaping functions. Mirrors `UnescapeRule` in escape.h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnescapeRule(u32);

impl UnescapeRule {
    /// Unescape nothing at all.
    pub const NONE: UnescapeRule = UnescapeRule(0);

    /// Normal unescaping. This is a placeholder: the other flags imply it.
    pub const NORMAL: UnescapeRule = UnescapeRule(1 << 0);

    /// Convert `%20` to a space.
    pub const SPACES: UnescapeRule = UnescapeRule(1 << 1);

    /// Unescape `/` and `\`. Dangerous for paths and file names.
    pub const PATH_SEPARATORS: UnescapeRule = UnescapeRule(1 << 2);

    /// Unescape characters that change URL meaning (`%`, `+`, `&`, `#`, and
    /// so on). Path separators are not included.
    pub const URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS: UnescapeRule = UnescapeRule(1 << 3);

    /// Convert `+` to a space, as URL query strings do.
    pub const REPLACE_PLUS_WITH_SPACE: UnescapeRule = UnescapeRule(1 << 4);

    /// The raw bit pattern.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether `self` and `other` share at least one bit. This is the C++
    /// `rules & flag` test.
    pub const fn intersects(self, other: UnescapeRule) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for UnescapeRule {
    type Output = UnescapeRule;

    fn bitor(self, rhs: UnescapeRule) -> UnescapeRule {
        UnescapeRule(self.0 | rhs.0)
    }
}

// Lookup for ASCII code points that are safe to unescape under the default
// rules. Each row is 16 entries, starting at the row's first code point.
// Derived from the C++ `kUrlUnescape` table.
// clang-format off
const URL_UNESCAPE: [u8; 128] = [
    // Control characters, 0x00..=0x1F.
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // ' ' ! " # $ % & ' ( ) * + , - . /
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, // 0 1 2 3 4 5 6 7 8 9 : ; < = > ?
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0, // @ A B C D E F G H I J K L M N O
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // P Q R S T U V W X Y Z [ \ ] ^ _
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, // ` a b c d e f g h i j k l m n o
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // p q r s t u v w x y z { | } ~ DEL
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0,
];
// clang-format on

/// Returns true if `code_point` (a non-ASCII scalar value) must stay escaped
/// under any rule. These are BiDi controls, spoofing symbols, invisible or
/// format characters, and spaces that could hide text.
fn is_banned_non_ascii(code_point: u32) -> bool {
    matches!(
        code_point,
        // BiDi controls (RFC 3987 section 4.1 and UAX #9).
        0x200E | 0x200F | 0x202A..=0x202E | 0x061C | 0x2066..=0x2069
        // Spoofable lock symbols.
        | 0x1F50F | 0x1F510 | 0x1F512 | 0x1F513
        // Spaces.
        | 0x0085 | 0x00A0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F
        | 0x205F | 0x3000 | 0x2800
        // Default ignorable and format characters.
        | 0x00AD | 0x034F | 0x0600..=0x0605 | 0x06DD | 0x070F | 0x08E2 | 0x115F
        | 0x1160 | 0x17B4 | 0x17B5 | 0x180B..=0x180E | 0x200B..=0x200D
        | 0x2060..=0x2065 | 0x206A..=0x206F | 0x3164 | 0xFFF0..=0xFFF8
        | 0xFE00..=0xFE0F | 0xFEFF | 0xFFA0 | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD
        | 0x13430..=0x13438 | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
        | 0xE0000..=0xE0FFF
    )
}

/// Whether a code point may be unescaped under `rules`.
fn should_unescape_code_point(rules: UnescapeRule, code_point: u32) -> bool {
    if code_point < 0x80 {
        let c = code_point as u8;
        return URL_UNESCAPE[usize::from(c)] != 0
            || (c == b' ' && rules.intersects(UnescapeRule::SPACES))
            || ((c == b'/' || c == b'\\') && rules.intersects(UnescapeRule::PATH_SEPARATORS))
            || (c > b' '
                && c != b'/'
                && c != b'\\'
                && rules.intersects(UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS));
    }
    !is_banned_non_ascii(code_point)
}

/// Returns the byte value of the `%XX` escape at `index`. `None` if there is
/// no complete escape there.
fn unescape_byte_at(escaped: &[u8], index: usize) -> Option<u8> {
    // The escape needs three bytes, index..index+3, which must all exist.
    if index + 2 >= escaped.len() || escaped[index] != b'%' {
        return None;
    }
    let hi = hex_value(escaped[index + 1])?;
    let lo = hex_value(escaped[index + 2])?;
    Some(hi * 16 + lo)
}

fn is_utf8_lead(c: u8) -> bool {
    (0xC2..=0xF4).contains(&c)
}

fn is_utf8_trail(c: u8) -> bool {
    (0x80..=0xBF).contains(&c)
}

/// Decodes the UTF-8 character formed by `%XX` escapes starting at `index`.
/// Returns the code point, the bytes of the whole sequence, and how many of
/// those bytes it uses. `None` if the escapes do not form one well-formed
/// character.
fn unescape_utf8_char_at(escaped: &[u8], index: usize) -> Option<(u32, [u8; 4], usize)> {
    let first = unescape_byte_at(escaped, index)?;
    let mut bytes = [0u8; 4];
    bytes[0] = first;
    let mut len = 1;
    // A lead byte takes up to three trail bytes, which are read from the
    // escapes that follow. Stop at the first escape that is not a trail byte.
    if is_utf8_lead(first) {
        while len < bytes.len() {
            match unescape_byte_at(escaped, index + len * 3) {
                Some(b) if is_utf8_trail(b) => {
                    bytes[len] = b;
                    len += 1;
                }
                _ => break,
            }
        }
    }
    let (ch, used) = decode_utf8_char(&bytes[..len], 0).ok()?;
    Some((u32::from(ch), bytes, used))
}

/// Core unescaping loop. Returns the output and the adjustments that map
/// escaped offsets to output offsets.
fn unescape_with_adjustments(escaped: &[u8], rules: UnescapeRule) -> (Vec<u8>, Vec<Adjustment>) {
    // With NONE, return the input unchanged and no adjustments.
    if rules == UnescapeRule::NONE {
        return (escaped.to_vec(), Vec::new());
    }

    let mut result = Vec::with_capacity(escaped.len());
    let mut adjustments = Vec::new();
    let mut i = 0;
    while i < escaped.len() {
        match unescape_utf8_char_at(escaped, i) {
            Some((code_point, bytes, used)) => {
                let span = used * 3;
                if !should_unescape_code_point(rules, code_point) {
                    // Valid, but unsafe to unescape: copy the escapes as they are.
                    result.extend_from_slice(&escaped[i..i + span]);
                } else {
                    result.extend_from_slice(&bytes[..used]);
                    for j in 0..used {
                        adjustments.push(Adjustment::new(i + j * 3, 3, 1));
                    }
                }
                i += span;
            }
            None => {
                // Not a valid character. A lone escape still unescapes to its
                // raw byte, because C++ does not check the rules here.
                if let Some(byte) = unescape_byte_at(escaped, i) {
                    result.push(byte);
                    adjustments.push(Adjustment::new(i, 3, 1));
                    i += 3;
                    continue;
                }
                // Not an escape. A '+' becomes a space if requested.
                if escaped[i] == b'+' && rules.intersects(UnescapeRule::REPLACE_PLUS_WITH_SPACE) {
                    result.push(b' ');
                } else {
                    result.push(escaped[i]);
                }
                i += 1;
            }
        }
    }
    (result, adjustments)
}

/// Unescapes `escaped_text` according to `rules`. Looks for `%XX` with two hex
/// digits and replaces it with the byte it names. Any other byte is kept.
///
/// Unescaping is done on whole UTF-8 characters, so characters that are unsafe
/// to show or to re-parse as URL syntax stay escaped (see `rules`). Invalid
/// UTF-8 escapes are unescaped to raw bytes. The result is not necessarily
/// valid UTF-8.
pub fn unescape_url_component(escaped_text: &[u8], rules: UnescapeRule) -> Vec<u8> {
    unescape_with_adjustments(escaped_text, rules).0
}

/// Unescapes `text` as a URL component, then decodes it as UTF-8. If the
/// unescaped bytes are not valid UTF-8, returns the escaped `text` decoded
/// with U+FFFD for invalid bytes, as the C++ does.
///
/// Also returns the adjustments from the escaped `text` to the returned string,
/// measured in UTF-16 units. The C++ version fills these only when given a
/// non-null pointer; the string result is the same either way.
pub fn unescape_and_decode_utf8_url_component_with_adjustments(
    text: &[u8],
    rules: UnescapeRule,
) -> (String, Vec<Adjustment>) {
    let (unescaped, unescape_adjustments) = unescape_with_adjustments(text, rules);
    let (result, mut adjustments, valid) = utf8_to_utf16_with_adjustments(&unescaped);
    if valid {
        // Compose the escaped-to-unescaped and unescaped-to-output adjustments.
        merge_sequential_adjustments(&unescape_adjustments, &mut adjustments);
        return (result, adjustments);
    }
    // Not valid UTF-8: return the escaped input, decoded.
    let (fallback, fallback_adjustments, _) = utf8_to_utf16_with_adjustments(text);
    (fallback, fallback_adjustments)
}

/// Unescapes `escaped_text` for use as binary data. Every `%XX` is decoded,
/// including NUL, control characters, invalid UTF-8, and path separators. This
/// must not be used for display. `rules` may only contain `NORMAL` and
/// `REPLACE_PLUS_WITH_SPACE`. Other flags are ignored in release builds, as in
/// the C++.
pub fn unescape_binary_url_component(escaped_text: &[u8], rules: UnescapeRule) -> Vec<u8> {
    debug_assert!(rules != UnescapeRule::NONE);
    debug_assert!(
        rules.bits()
            & !(UnescapeRule::NORMAL.bits() | UnescapeRule::REPLACE_PLUS_WITH_SPACE.bits())
            == 0
    );
    let replace_plus = rules.intersects(UnescapeRule::REPLACE_PLUS_WITH_SPACE);
    let mut out = Vec::with_capacity(escaped_text.len());
    let mut i = 0;
    while i < escaped_text.len() {
        if let Some(byte) = unescape_byte_at(escaped_text, i) {
            out.push(byte);
            i += 3;
        } else {
            if replace_plus && escaped_text[i] == b'+' {
                out.push(b' ');
            } else {
                out.push(escaped_text[i]);
            }
            i += 1;
        }
    }
    out
}

/// Returns `Some` with the binary-unescaped bytes if the input does not
/// escape a control character (`0x00..=0x1F`). With `fail_on_path_separators`,
/// escaped `/` and `\` also make it fail. An unescaped control character in the
/// input does not cause a failure. Returns `None` on failure.
pub fn unescape_binary_url_component_safe(
    escaped_text: &[u8],
    fail_on_path_separators: bool,
) -> Option<Vec<u8>> {
    let illegal = |b: u8| b <= 0x1F || (fail_on_path_separators && (b == b'/' || b == b'\\'));
    if any_encoded_byte(escaped_text, illegal) {
        return None;
    }
    Some(unescape_binary_url_component(escaped_text, UnescapeRule::NORMAL))
}

/// Returns true if `escaped_text` contains a `%XX` escape for any byte in
/// `bytes`. Matches the byte value, not the UTF-8 character.
pub fn contains_encoded_bytes(escaped_text: &[u8], bytes: &[u8]) -> bool {
    any_encoded_byte(escaped_text, |b| bytes.contains(&b))
}

fn any_encoded_byte(escaped_text: &[u8], pred: impl Fn(u8) -> bool) -> bool {
    let mut i = 0;
    while i < escaped_text.len() {
        if let Some(byte) = unescape_byte_at(escaped_text, i) {
            if pred(byte) {
                return true;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    false
}

/// Replaces the five HTML entities `&lt;`, `&gt;`, `&amp;`, `&quot;`, and
/// `&#39;` with the characters they stand for. Other `&` text is kept.
///
/// Each replacement is made once, left to right. For `&amp;lt;` the result is
/// `&lt;`, not `<`.
pub fn unescape_for_html(input: &str) -> String {
    const ENTITIES: [(&str, char); 5] =
        [("&lt;", '<'), ("&gt;", '>'), ("&amp;", '&'), ("&quot;", '"'), ("&#39;", '\'')];
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        match ENTITIES.iter().find(|(entity, _)| tail.starts_with(entity)) {
            Some((entity, ch)) => {
                out.push(*ch);
                rest = &tail[entity.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
