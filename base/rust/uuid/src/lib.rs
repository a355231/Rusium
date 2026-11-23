// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::Uuid` (base/uuid.{h,cc}).
//!
//! `Uuid` is a 128-bit identifier in the canonical text form
//! `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`. It supports parsing, lowercase
//! canonicalization, validity checks, version-4 formatting, ordering, and
//! hashing.
//!
//! Differences from the C++ version:
//! - C++ `GenerateRandomV4()` reads its own randomness with `base::RandBytes`.
//!   This crate does not read randomness. The caller supplies the 16 bytes to
//!   `Uuid::from_random_bytes_v4`. The formatting is the same as C++
//!   `FormatRandomDataAsV4Impl`. Callers must use a cryptographically secure
//!   source for the bytes.
//! - The C++ `std::string` member is replaced by a `Option<[u8; 16]>` holding
//!   the 16 bytes in RFC order. An invalid `Uuid` is `None`. The text form is
//!   produced on demand by `as_lowercase_string()` and `Display`. Parsing
//!   accepts the same inputs and gives the same `is_valid()` results.
//! - Ordering follows the C++ `operator<=>`. Invalid `Uuid`s are the least
//!   value. Valid ones compare by their 16 bytes, which gives the same order as
//!   the lowercase string form.
//! - `UuidHash` is replaced by the derived `Hash`. As in C++, the hash values
//!   must not be stored.
//! - The `u16` overloads (`ParseCaseInsensitive(std::u16string_view)` and
//!   `ParseLowercase(std::u16string_view)`) are provided as
//!   `*_utf16(&[u16])`. Any non-ASCII code unit makes the input invalid, as it
//!   does in C++.
//! - `AsInteger()` returns `u128`, using the same value as C++ `absl::uint128`:
//!   the 16 bytes read as a big-endian number. Invalid `Uuid`s give 0.
//! - The C++ `operator<<` is `Display`. It prints the lowercase string, or
//!   nothing for an invalid `Uuid`.
//! - The Android/JNI conversions (`FromJniType` and `ToJniType`) are not
//!   ported. They need the Java bindings.

#![forbid(unsafe_code)]

use std::fmt;

/// A 128-bit identifier. The default value is invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid {
    /// The 16 bytes in RFC text order, or `None` for an invalid `Uuid`.
    bytes: Option<[u8; 16]>,
}

/// Length of the text form, in bytes.
const UUID_TEXT_LENGTH: usize = 36;

/// Byte indexes after which the text form has a hyphen: 8-4-4-4-12 hex digits.
const HYPHEN_BYTE_GROUPS: [usize; 4] = [4, 6, 8, 10];

/// Returns the value of a hex digit, or `None` if `c` is not one.
/// When `strict`, only lowercase `a` through `f` are accepted.
fn hex_value(c: u8, strict: bool) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' if !strict => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Parses `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`. Mirrors
/// `GetCanonicalUuidInternal` in the C++ code.
fn parse_canonical(input: &[u8], strict: bool) -> Option<[u8; 16]> {
    if input.len() != UUID_TEXT_LENGTH {
        return None;
    }
    let mut bytes = [0u8; 16];
    let mut nibble_index = 0usize;
    for (i, &c) in input.iter().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if c != b'-' {
                return None;
            }
        } else {
            let value = hex_value(c, strict)?;
            // High nibble first, then low nibble.
            let shift = if nibble_index % 2 == 0 { 4 } else { 0 };
            bytes[nibble_index / 2] |= value << shift;
            nibble_index += 1;
        }
    }
    Some(bytes)
}

/// Converts UTF-16 code units to bytes for `parse_canonical`. A non-ASCII
/// unit becomes 0, which is neither a hex digit nor a hyphen, so the input is
/// rejected in the same cases as in C++.
fn narrow_utf16(input: &[u16]) -> Vec<u8> {
    input
        .iter()
        .map(|&unit| if unit < 0x80 { unit as u8 } else { 0 })
        .collect()
}

impl Uuid {
    /// Length in bytes of the input required to format a version-4 `Uuid`.
    pub const GUID_V4_INPUT_LENGTH: usize = 16;

    /// Formats 16 random bytes as a version-4 `Uuid`, as RFC 4122 section 4.4
    /// describes. The format is `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`, where
    /// `y` is one of `8`, `9`, `a`, `b`.
    ///
    /// The bytes must come from a cryptographically secure random source. This
    /// function does not read randomness.
    ///
    /// Matches C++ `FormatRandomDataAsV4Impl`. Each 8-byte half is read as a
    /// little-endian `u64`, the version and variant bits are set, and the value
    /// is printed in hex.
    pub fn from_random_bytes_v4(input: [u8; 16]) -> Uuid {
        let mut first = u64::from_le_bytes(input[0..8].try_into().expect("8 bytes"));
        let mut second = u64::from_le_bytes(input[8..16].try_into().expect("8 bytes"));

        // Version 4: clear the version nibble, then set it to 4.
        first &= 0xffff_ffff_ffff_0fff;
        first |= 0x0000_0000_0000_4000;

        // Variant 0b10 (RFC 9562 section 5.4): clear bit 65, set bit 64.
        second &= 0x3fff_ffff_ffff_ffff;
        second |= 0x8000_0000_0000_0000;

        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&first.to_be_bytes());
        bytes[8..16].copy_from_slice(&second.to_be_bytes());
        Uuid { bytes: Some(bytes) }
    }

    /// Parses `input` if it has the `Uuid` form. Letters `a` through `f` may be
    /// upper or lower case. Returns an invalid `Uuid` otherwise.
    pub fn parse_case_insensitive(input: &str) -> Uuid {
        Uuid {
            bytes: parse_canonical(input.as_bytes(), false),
        }
    }

    /// Like `parse_case_insensitive`, for UTF-16 input.
    pub fn parse_case_insensitive_utf16(input: &[u16]) -> Uuid {
        Uuid {
            bytes: parse_canonical(&narrow_utf16(input), false),
        }
    }

    /// Like `parse_case_insensitive`, but hex letters must be lowercase.
    pub fn parse_lowercase(input: &str) -> Uuid {
        Uuid {
            bytes: parse_canonical(input.as_bytes(), true),
        }
    }

    /// Like `parse_lowercase`, for UTF-16 input.
    pub fn parse_lowercase_utf16(input: &[u16]) -> Uuid {
        Uuid {
            bytes: parse_canonical(&narrow_utf16(input), true),
        }
    }

    /// Returns true if this `Uuid` is valid.
    pub fn is_valid(&self) -> bool {
        self.bytes.is_some()
    }

    /// Returns the lowercase text form, which `parse_lowercase` accepts. Returns
    /// an empty string for an invalid `Uuid`.
    pub fn as_lowercase_string(&self) -> String {
        self.to_string()
    }

    /// Returns the `Uuid` as a 128-bit integer, or 0 if it is invalid.
    ///
    /// The value is the 16 bytes read as a big-endian number. That is the hex
    /// digits of the text form read as one number.
    pub fn as_integer(&self) -> u128 {
        match self.bytes {
            Some(bytes) => u128::from_be_bytes(bytes),
            None => 0,
        }
    }
}

impl fmt::Display for Uuid {
    /// Writes the lowercase text form, or nothing for an invalid `Uuid`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(bytes) = self.bytes else {
            return Ok(());
        };
        for (i, byte) in bytes.iter().enumerate() {
            if HYPHEN_BYTE_GROUPS.contains(&i) {
                f.write_str("-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple xorshift64 PRNG for randomized tests. Not used by the library.
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

        fn next_bytes16(&mut self) -> [u8; 16] {
            let mut out = [0u8; 16];
            for chunk in out.chunks_mut(8) {
                chunk.copy_from_slice(&self.next_u64().to_le_bytes());
            }
            out
        }
    }

    /// The format of version 4 must be xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx,
    /// where y is one of [8, 9, a, b].
    fn is_valid_v4(guid: &Uuid) -> bool {
        if !guid.is_valid() {
            return false;
        }
        let s = guid.as_lowercase_string();
        let s = s.as_bytes();
        s[14] == b'4' && matches!(s[19], b'8' | b'9' | b'a' | b'b')
    }

    #[test]
    fn uuid_basic_uniqueness() {
        // The C++ test uses RandBytes. Here fixed-seed PRNG bytes are used.
        let mut rng = XorShift64(0x1234_5678_9abc_def1);
        for _ in 0..10 {
            let guid1 = Uuid::from_random_bytes_v4(rng.next_bytes16());
            let guid2 = Uuid::from_random_bytes_v4(rng.next_bytes16());
            assert_ne!(guid1, guid2);
            assert!(guid1.is_valid());
            assert!(is_valid_v4(&guid1));
            assert!(guid2.is_valid());
            assert!(is_valid_v4(&guid2));
        }
    }

    fn test_uuid_validity(input: &str, case_insensitive: bool, strict: bool) {
        let guid = Uuid::parse_case_insensitive(input);
        assert_eq!(case_insensitive, guid.is_valid(), "{input}");
        let guid = Uuid::parse_lowercase(input);
        assert_eq!(strict, guid.is_valid(), "{input}");
    }

    #[test]
    fn validity() {
        // An empty Uuid is invalid.
        assert!(!Uuid::default().is_valid());

        #[derive(Clone, Copy, PartialEq)]
        enum Parsability {
            DoesntParse,
            ParsesCaseInsensitiveOnly,
            AlwaysParses,
        }
        use Parsability::*;

        let uuid_validity: &[(&str, Parsability)] = &[
            ("invalid", DoesntParse),
            ("0123456789ab-cdef-fedc-ba98-76543210", DoesntParse),
            ("0123456789abcdeffedcba9876543210", DoesntParse),
            ("01234567-89Zz-ZzZz-ZzZz-Zz9876543210", DoesntParse),
            ("DEADBEEFDEADBEEFDEADBEEFDEADBEEF", DoesntParse),
            ("deadbeefWdeadXbeefYdeadZbeefdeadbeef", DoesntParse),
            ("XXXdeadbeefWdeadXbeefYdeadZbeefdeadbeefXXX", DoesntParse),
            ("01234567-89aB-cDeF-fEdC-bA9876543210", ParsesCaseInsensitiveOnly),
            ("DEADBEEF-DEAD-BEEF-DEAD-BEEFDEADBEEF", ParsesCaseInsensitiveOnly),
            ("00000000-0000-0000-0000-000000000000", AlwaysParses),
            ("deadbeef-dead-beef-dead-beefdeadbeef", AlwaysParses),
        ];

        for &(input, parsability) in uuid_validity {
            let case_insensitive = parsability != DoesntParse;
            let strict = parsability == AlwaysParses;
            test_uuid_validity(input, case_insensitive, strict);
        }
    }

    #[test]
    fn equality_and_round_trip() {
        const CANONICAL_STR: &str = "deadbeef-dead-4eef-bead-beefdeadbeef";

        let from_lower = Uuid::parse_case_insensitive(&CANONICAL_STR.to_ascii_lowercase());
        assert_eq!(CANONICAL_STR, from_lower.as_lowercase_string());

        let from_upper = Uuid::parse_case_insensitive(&CANONICAL_STR.to_ascii_uppercase());
        assert_eq!(CANONICAL_STR, from_upper.as_lowercase_string());

        assert_eq!(from_lower, from_upper);

        // Invalid Uuids are equal.
        assert_eq!(Uuid::default(), Uuid::default());
    }

    #[test]
    fn unordered_set() {
        use std::collections::HashSet;

        let mut guid_set: HashSet<Uuid> = HashSet::new();

        const UUID1: &str = "01234567-89ab-cdef-fedc-ba9876543210";
        guid_set.insert(Uuid::parse_case_insensitive(&UUID1.to_ascii_lowercase()));
        assert_eq!(1, guid_set.len());
        guid_set.insert(Uuid::parse_case_insensitive(&UUID1.to_ascii_uppercase()));
        assert_eq!(1, guid_set.len());

        const UUID2: &str = "deadbeef-dead-beef-dead-beefdeadbeef";
        guid_set.insert(Uuid::parse_case_insensitive(&UUID2.to_ascii_lowercase()));
        assert_eq!(2, guid_set.len());
        guid_set.insert(Uuid::parse_case_insensitive(&UUID2.to_ascii_uppercase()));
        assert_eq!(2, guid_set.len());
    }

    #[test]
    fn set() {
        use std::collections::BTreeSet;

        let mut guid_set = BTreeSet::new();

        const UUID1: &str = "01234567-89ab-cdef-0123-456789abcdef";
        let guid1 = Uuid::parse_lowercase(UUID1);
        assert!(guid1.is_valid());
        guid_set.insert(guid1);

        const UUID2: &str = "deadbeef-dead-beef-dead-beefdeadbeef";
        let guid2 = Uuid::parse_lowercase(UUID2);
        assert!(guid2.is_valid());
        guid_set.insert(guid2);

        // The order of the Uuids is preserved.
        let mut it = guid_set.iter();
        assert_eq!(Some(&guid1), it.next());
        assert_eq!(Some(&guid2), it.next());
        assert_eq!(None, it.next());
    }

    #[test]
    fn compare() {
        const UUID: &str = "21abd97f-73e8-4b88-9389-a9fee6abda5e";
        const UUID_LESS: &str = "1e0dcaca-9e7c-4f4b-bcc6-e4c02b0c99df";
        const UUID_GREATER: &str = "6eeb1bc8-186b-433c-9d6a-a827bc96b2d4";

        let guid = Uuid::parse_lowercase(UUID);
        let guid_eq = Uuid::parse_lowercase(UUID);
        let guid_lt = Uuid::parse_lowercase(UUID_LESS);
        let guid_gt = Uuid::parse_lowercase(UUID_GREATER);
        let guid_invalid = Uuid::default();

        assert!(guid_eq == guid);
        assert!(!(guid_eq != guid));
        assert!(!(guid_eq < guid));
        assert!(guid_eq <= guid);
        assert!(!(guid_eq > guid));
        assert!(guid_eq >= guid);

        assert!(!(guid_lt == guid));
        assert!(guid_lt != guid);
        assert!(guid_lt < guid);
        assert!(guid_lt <= guid);
        assert!(!(guid_lt > guid));
        assert!(!(guid_lt >= guid));

        assert!(!(guid_gt == guid));
        assert!(guid_gt != guid);
        assert!(!(guid_gt < guid));
        assert!(!(guid_gt <= guid));
        assert!(guid_gt > guid);
        assert!(guid_gt >= guid);

        // Invalid Uuids are the least.
        assert!(!(guid_invalid == guid));
        assert!(guid_invalid != guid);
        assert!(guid_invalid < guid);
        assert!(guid_invalid <= guid);
        assert!(!(guid_invalid > guid));
        assert!(!(guid_invalid >= guid));
    }

    #[test]
    fn as_integer_valid() {
        let uuid = Uuid::parse_case_insensitive("01234567-89ab-cdef-fedc-ba9876543210");
        assert!(uuid.is_valid());
        assert_eq!(
            uuid.as_integer(),
            (0x0123_4567_89ab_cdef_u128 << 64) | 0xfedc_ba98_7654_3210_u128
        );
    }

    #[test]
    fn as_integer_invalid() {
        let uuid = Uuid::parse_case_insensitive("invalid");
        assert!(!uuid.is_valid());
        assert_eq!(uuid.as_integer(), 0);
    }

    #[test]
    fn known_answer_v4_from_fixed_bytes() {
        // Bytes 0x00..=0x0f. Each half is read little-endian, so the text is
        // the reversed bytes with the version and variant bits set.
        let input: [u8; 16] = core::array::from_fn(|i| i as u8);
        let uuid = Uuid::from_random_bytes_v4(input);
        assert_eq!(uuid.as_lowercase_string(), "07060504-0302-4100-8f0e-0d0c0b0a0908");
        assert!(is_valid_v4(&uuid));

        // All 0xff: version 4 and variant 'b'.
        let uuid = Uuid::from_random_bytes_v4([0xff; 16]);
        assert_eq!(uuid.as_lowercase_string(), "ffffffff-ffff-4fff-bfff-ffffffffffff");
        assert!(is_valid_v4(&uuid));

        // All zero bytes: version 4 and variant '8'.
        let uuid = Uuid::from_random_bytes_v4([0; 16]);
        assert_eq!(uuid.as_lowercase_string(), "00000000-0000-4000-8000-000000000000");
        assert!(is_valid_v4(&uuid));
        // Lowercase text parses back to the same value.
        assert_eq!(Uuid::parse_lowercase(&uuid.as_lowercase_string()), uuid);
    }

    #[test]
    fn v4_formatting_matches_reference_bit_manipulation() {
        // Independent reference: build the text from the bytes directly.
        let mut rng = XorShift64(0xdead_beef_cafe_f00d);
        for _ in 0..2000 {
            let input = rng.next_bytes16();
            let mut expected = [0u8; 16];
            for i in 0..8 {
                expected[i] = input[7 - i];
                expected[8 + i] = input[15 - i];
            }
            expected[6] = (expected[6] & 0x0f) | 0x40;
            expected[8] = (expected[8] & 0x3f) | 0x80;
            let hex: String = expected.iter().map(|b| format!("{b:02x}")).collect();
            let text = format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            );
            let uuid = Uuid::from_random_bytes_v4(input);
            assert_eq!(uuid.as_lowercase_string(), text);
            assert!(is_valid_v4(&uuid));
        }
    }

    #[test]
    fn order_matches_string_order() {
        // The derived ordering must match the order of the lowercase strings.
        let mut rng = XorShift64(0x0123_4567_89ab_cdef);
        let uuids: Vec<Uuid> = (0..500)
            .map(|_| Uuid::from_random_bytes_v4(rng.next_bytes16()))
            .collect();
        for a in &uuids {
            for b in &uuids {
                assert_eq!(
                    a.cmp(b),
                    a.as_lowercase_string().cmp(&b.as_lowercase_string())
                );
            }
        }
    }

    #[test]
    fn randomized_round_trip_and_case() {
        let mut rng = XorShift64(0xfeed_face_0bad_c0de);
        for _ in 0..2000 {
            let uuid = Uuid::from_random_bytes_v4(rng.next_bytes16());
            let text = uuid.as_lowercase_string();
            assert_eq!(text.len(), 36);
            assert_eq!(Uuid::parse_lowercase(&text), uuid);
            assert_eq!(Uuid::parse_case_insensitive(&text), uuid);
            assert_eq!(Uuid::parse_case_insensitive(&text.to_ascii_uppercase()), uuid);
            // The strict parser rejects uppercase hex letters.
            if text.bytes().any(|b| b.is_ascii_alphabetic()) {
                assert!(!Uuid::parse_lowercase(&text.to_ascii_uppercase()).is_valid());
            }
            // The UTF-16 overloads agree with the byte ones.
            let utf16: Vec<u16> = text.encode_utf16().collect();
            assert_eq!(Uuid::parse_lowercase_utf16(&utf16), uuid);
            assert_eq!(Uuid::parse_case_insensitive_utf16(&utf16), uuid);
            // Independent check of as_integer: the hex digits of the text read
            // as one number, split into the high and low 64 bits.
            let hi = format!("{}{}{}", &text[0..8], &text[9..13], &text[14..18]);
            let lo = format!("{}{}", &text[19..23], &text[24..36]);
            let expected_int = (u128::from_str_radix(&hi, 16).unwrap() << 64)
                | u128::from_str_radix(&lo, 16).unwrap();
            assert_eq!(uuid.as_integer(), expected_int);
        }
    }

    #[test]
    fn utf16_non_ascii_is_invalid() {
        // A non-ASCII unit is never valid, even when it stands for a hex digit.
        let mut text: Vec<u16> = "deadbeef-dead-beef-dead-beefdeadbeef".encode_utf16().collect();
        text[0] = 0xff41; // Fullwidth 'a' lookalike, not ASCII.
        assert!(!Uuid::parse_case_insensitive_utf16(&text).is_valid());
        assert!(!Uuid::parse_lowercase_utf16(&text).is_valid());
    }

    #[test]
    fn display_matches_lowercase_string() {
        let uuid = Uuid::parse_case_insensitive("DEADBEEF-DEAD-BEEF-DEAD-BEEFDEADBEEF");
        assert_eq!(uuid.to_string(), "deadbeef-dead-beef-dead-beefdeadbeef");
        assert_eq!(Uuid::default().to_string(), "");
    }

    #[test]
    fn wrong_length_and_hyphens_are_rejected() {
        assert!(!Uuid::parse_case_insensitive("").is_valid());
        // 37 characters.
        assert!(!Uuid::parse_case_insensitive("00000000-0000-0000-0000-0000000000000").is_valid());
        // Hyphen in the wrong place.
        assert!(!Uuid::parse_case_insensitive("00000000-00000-000-0000-000000000000").is_valid());
    }
}
