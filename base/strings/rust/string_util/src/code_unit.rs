// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! The code-unit abstraction shared by the 8-bit (`u8`, UTF-8 or raw bytes)
//! and 16-bit (`u16`, UTF-16) variants, plus the character classification
//! helpers from base/strings/string_util.h (IsAscii*, IsUnicode*, ...).

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
}

/// A code unit of a string: `u8` for byte strings (C++ `char`) or `u16` for
/// UTF-16 strings (C++ `char16_t`). This trait is sealed.
pub trait CodeUnit: Copy + Eq + Ord + Default + std::fmt::Debug + sealed::Sealed {
    /// Zero-extends the code unit to `u32`.
    fn to_u32(self) -> u32;
    /// Truncates `value` to the width of this code unit type.
    fn from_u32_truncating(value: u32) -> Self;
}

impl CodeUnit for u8 {
    #[inline]
    fn to_u32(self) -> u32 {
        u32::from(self)
    }
    #[inline]
    fn from_u32_truncating(value: u32) -> Self {
        value as u8
    }
}

impl CodeUnit for u16 {
    #[inline]
    fn to_u32(self) -> u32 {
        u32::from(self)
    }
    #[inline]
    fn from_u32_truncating(value: u32) -> Self {
        value as u16
    }
}

/// C++ `WHITESPACE_ASCII`: HTML5-style ASCII whitespace, including the
/// control characters VT (0x0B) and CR/LF.
pub const WHITESPACE_ASCII: &[u8] = b"\t\x0B\x0C \n\r";

/// C++ `WHITESPACE_ASCII_NO_CR_LF`.
pub const WHITESPACE_ASCII_NO_CR_LF: &[u8] = b"\t\x0B\x0C ";

/// C++ `WHITESPACE_UNICODE` for `char16_t`: `WHITESPACE_ASCII` followed by the
/// non-ASCII Unicode whitespace code units. Not NUL-terminated.
pub const WHITESPACE_UTF16: &[u16] = &[
    0x0009, 0x000B, 0x000C, 0x0020, 0x000A, 0x000D, // ASCII
    0x0085, // NEXT LINE (NEL)
    0x00A0, // NO-BREAK SPACE
    0x1680, // OGHAM SPACE MARK
    0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A,
    0x2028, // LINE SEPARATOR
    0x2029, // PARAGRAPH SEPARATOR
    0x202F, // NARROW NO-BREAK SPACE
    0x205F, // MEDIUM MATHEMATICAL SPACE
    0x3000, // IDEOGRAPHIC SPACE
];

/// C++ `WHITESPACE_UNICODE_NO_CR_LF` for `char16_t`.
pub const WHITESPACE_NO_CR_LF_UTF16: &[u16] = &[
    0x0009, 0x000B, 0x000C, 0x0020, // ASCII without CR and LF
    0x0085, 0x00A0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008,
    0x2009, 0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000,
];

/// C++ `kInfraAsciiWhitespace` (https://infra.spec.whatwg.org/#ascii-whitespace).
pub const INFRA_ASCII_WHITESPACE: &[u8] = &[0x09, 0x0A, 0x0C, 0x0D, 0x20];

/// C++ `kUtf8ByteOrderMark`.
pub const UTF8_BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

/// C++ `ToLowerASCII(CharT)` for one code unit. Only `A`-`Z` change.
#[inline]
pub fn to_lower_ascii_char<T: CodeUnit>(c: T) -> T {
    let v = c.to_u32();
    if (u32::from(b'A')..=u32::from(b'Z')).contains(&v) {
        T::from_u32_truncating(v + 32)
    } else {
        c
    }
}

/// C++ `ToUpperASCII(CharT)` for one code unit. Only `a`-`z` change.
#[inline]
pub fn to_upper_ascii_char<T: CodeUnit>(c: T) -> T {
    let v = c.to_u32();
    if (u32::from(b'a')..=u32::from(b'z')).contains(&v) {
        T::from_u32_truncating(v - 32)
    } else {
        c
    }
}

/// C++ `IsAsciiWhitespace`. Matches the `WHITESPACE_ASCII` set.
#[inline]
pub fn is_ascii_whitespace<T: CodeUnit>(c: T) -> bool {
    let v = c.to_u32();
    WHITESPACE_ASCII.iter().any(|&w| u32::from(w) == v)
}

/// C++ `IsAsciiAlpha`.
#[inline]
pub fn is_ascii_alpha<T: CodeUnit>(c: T) -> bool {
    is_ascii_upper(c) || is_ascii_lower(c)
}

/// C++ `IsAsciiUpper`.
#[inline]
pub fn is_ascii_upper<T: CodeUnit>(c: T) -> bool {
    (u32::from(b'A')..=u32::from(b'Z')).contains(&c.to_u32())
}

/// C++ `IsAsciiLower`.
#[inline]
pub fn is_ascii_lower<T: CodeUnit>(c: T) -> bool {
    (u32::from(b'a')..=u32::from(b'z')).contains(&c.to_u32())
}

/// C++ `IsAsciiDigit`.
#[inline]
pub fn is_ascii_digit<T: CodeUnit>(c: T) -> bool {
    (u32::from(b'0')..=u32::from(b'9')).contains(&c.to_u32())
}

/// C++ `IsAsciiAlphaNumeric`.
#[inline]
pub fn is_ascii_alphanumeric<T: CodeUnit>(c: T) -> bool {
    is_ascii_alpha(c) || is_ascii_digit(c)
}

/// C++ `IsAsciiPrintable`: space through tilde.
#[inline]
pub fn is_ascii_printable<T: CodeUnit>(c: T) -> bool {
    (0x20..=0x7E).contains(&c.to_u32())
}

/// C++ `IsAsciiControl`: 0x00-0x1F and 0x7F.
#[inline]
pub fn is_ascii_control<T: CodeUnit>(c: T) -> bool {
    let v = c.to_u32();
    v <= 0x1F || v == 0x7F
}

/// C++ `IsUnicodeControl`: ASCII control characters plus the C1 range
/// U+0080-U+009F.
#[inline]
pub fn is_unicode_control<T: CodeUnit>(c: T) -> bool {
    is_ascii_control(c) || (0x80..=0x9F).contains(&c.to_u32())
}

/// C++ `IsAsciiPunctuation`: printable, non-space, non-alphanumeric ASCII.
#[inline]
pub fn is_ascii_punctuation<T: CodeUnit>(c: T) -> bool {
    let v = c.to_u32();
    v > 0x20 && v < 0x7F && !is_ascii_alphanumeric(c)
}

/// C++ `IsHexDigit`.
#[inline]
pub fn is_hex_digit<T: CodeUnit>(c: T) -> bool {
    let v = c.to_u32();
    (u32::from(b'0')..=u32::from(b'9')).contains(&v)
        || (u32::from(b'A')..=u32::from(b'F')).contains(&v)
        || (u32::from(b'a')..=u32::from(b'f')).contains(&v)
}

/// C++ `IsUnicodeWhitespace` for a 16-bit or 32-bit code point value. Takes a
/// `u32` so callers can pass UTF-16 units (widened) or Unicode scalar values
/// (`char as u32`). Not meaningful for 8-bit code units; use
/// [`is_ascii_whitespace`] for those.
#[inline]
pub fn is_unicode_whitespace(c: u32) -> bool {
    WHITESPACE_UTF16.iter().any(|&w| u32::from(w) == c)
}

/// C++ `IsWhitespace` (documented as dangerous in C++): selects the Unicode
/// set for code units wider than one byte and the ASCII set otherwise.
#[inline]
pub fn is_whitespace<T: CodeUnit>(c: T) -> bool {
    if std::mem::size_of::<T>() > 1 {
        is_unicode_whitespace(c.to_u32())
    } else {
        is_ascii_whitespace(c)
    }
}
