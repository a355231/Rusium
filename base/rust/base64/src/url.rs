// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Base64url (RFC 4648 section 5), port of `base::Base64UrlEncode` and
//! `base::Base64UrlDecode` from base/base64url.{h,cc}. See the crate docs for
//! the differences.

use crate::standard::{self, DecodePolicy as StdDecodePolicy};

/// Mirrors `base::Base64UrlEncodePolicy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodePolicy {
    /// Include trailing `=` padding when the input is not a multiple of 3.
    IncludePadding,
    /// Remove trailing `=` padding.
    OmitPadding,
}

/// Mirrors `base::Base64UrlDecodePolicy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePolicy {
    /// Input must include padding when its length is not a multiple of 4.
    RequirePadding,
    /// Input may be missing padding, or include it.
    IgnorePadding,
    /// Input must not contain any `=`.
    DisallowPadding,
}

/// Encodes `input` as base64url (`-` and `_` in place of `+` and `/`).
pub fn encode(input: &[u8], policy: EncodePolicy) -> String {
    let mut out: String = standard::encode(input)
        .chars()
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            other => other,
        })
        .collect();
    if policy == EncodePolicy::OmitPadding {
        let trimmed_len = out.trim_end_matches('=').len();
        out.truncate(trimmed_len);
    }
    out
}

/// Decodes base64url `input` under `policy`. Returns `None` if the input is
/// invalid.
///
/// Standard base64 characters `+` and `/` are always rejected. Missing
/// padding is restored before decoding, and the result is decoded with the
/// strict standard decoder.
pub fn decode(input: &[u8], policy: DecodePolicy) -> Option<Vec<u8>> {
    if input.iter().any(|&b| b == b'+' || b == b'/') {
        return None;
    }
    let remainder = input.len() % 4;
    match policy {
        DecodePolicy::RequirePadding => {
            if remainder != 0 {
                return None;
            }
        }
        DecodePolicy::IgnorePadding => {}
        DecodePolicy::DisallowPadding => {
            if input.contains(&b'=') {
                return None;
            }
        }
    }

    let needs_translation = input.iter().any(|&b| b == b'-' || b == b'_');
    if remainder == 0 && !needs_translation {
        return standard::decode(input, StdDecodePolicy::Strict);
    }

    let mut std_input: Vec<u8> = input
        .iter()
        .map(|&b| match b {
            b'-' => b'+',
            b'_' => b'/',
            other => other,
        })
        .collect();
    if remainder != 0 {
        std_input.resize(input.len() + 4 - remainder, b'=');
    }
    standard::decode(&std_input, StdDecodePolicy::Strict)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple xorshift64 PRNG. The seed must be non-zero.
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

        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }

        fn bytes(&mut self, len: usize) -> Vec<u8> {
            (0..len).map(|_| self.next_u64() as u8).collect()
        }
    }

    const ENC_POLICIES: [EncodePolicy; 2] =
        [EncodePolicy::IncludePadding, EncodePolicy::OmitPadding];

    // Ported from TEST(Base64UrlTest, BinaryIncludePaddingPolicy).
    #[test]
    fn binary_include_padding_policy() {
        let data: [u8; 4] = [0x00, 0x01, 0xFE, 0xFF];

        let binary_encoded_with_padding = encode(&data, EncodePolicy::IncludePadding);
        assert_eq!("AAH-_w==", binary_encoded_with_padding);

        // Decoding the result gives the same bytes.
        assert_eq!(
            decode(binary_encoded_with_padding.as_bytes(), DecodePolicy::RequirePadding),
            Some(data.to_vec())
        );
        assert_eq!(
            decode(binary_encoded_with_padding.as_bytes(), DecodePolicy::IgnorePadding),
            Some(data.to_vec())
        );
        assert_eq!(
            decode(binary_encoded_with_padding.as_bytes(), DecodePolicy::DisallowPadding),
            None
        );
    }

    // Ported from TEST(Base64UrlTest, BinaryOmitPaddingPolicy).
    #[test]
    fn binary_omit_padding_policy() {
        let data: [u8; 4] = [0x00, 0x01, 0xFE, 0xFF];

        let binary_encoded_without_padding = encode(&data, EncodePolicy::OmitPadding);
        assert_eq!("AAH-_w", binary_encoded_without_padding);

        assert_eq!(
            decode(binary_encoded_without_padding.as_bytes(), DecodePolicy::DisallowPadding),
            Some(data.to_vec())
        );
        assert_eq!(
            decode(binary_encoded_without_padding.as_bytes(), DecodePolicy::IgnorePadding),
            Some(data.to_vec())
        );
        assert_eq!(
            decode(binary_encoded_without_padding.as_bytes(), DecodePolicy::RequirePadding),
            None
        );
    }

    // Ported from TEST(Base64UrlTest, EncodeIncludePaddingPolicy).
    #[test]
    fn encode_include_padding_policy() {
        let p = EncodePolicy::IncludePadding;
        assert_eq!("aGVsbG8_d29ybGQ=", encode(b"hello?world", p));
        // Very short and empty strings.
        assert_eq!("Pz8=", encode(b"??", p));
        assert_eq!("", encode(b"", p));
    }

    // Ported from TEST(Base64UrlTest, EncodeOmitPaddingPolicy).
    #[test]
    fn encode_omit_padding_policy() {
        let p = EncodePolicy::OmitPadding;
        assert_eq!("aGVsbG8_d29ybGQ", encode(b"hello?world", p));
        assert_eq!("Pz8", encode(b"??", p));
        assert_eq!("", encode(b"", p));
    }

    // Ported from TEST(Base64UrlTest, EncodeInPlaceOmitPaddingPolicy). Rust
    // cannot alias input and output, so the returned value is checked.
    #[test]
    fn encode_in_place_omit_padding_policy() {
        let input = "hello?world";
        let output = encode(input.as_bytes(), EncodePolicy::OmitPadding);
        assert_eq!("aGVsbG8_d29ybGQ", output);
    }

    // Ported from TEST(Base64UrlTest, EncodeInPlaceIncludePaddingPolicy).
    #[test]
    fn encode_in_place_include_padding_policy() {
        let input = "hello?world";
        let output = encode(input.as_bytes(), EncodePolicy::IncludePadding);
        assert_eq!("aGVsbG8_d29ybGQ=", output);
    }

    // Ported from TEST(Base64UrlTest, DecodeRequirePaddingPolicy).
    #[test]
    fn decode_require_padding_policy() {
        let p = DecodePolicy::RequirePadding;
        assert_eq!(decode(b"aGVsbG8_d29ybGQ=", p), Some(b"hello?world".to_vec()));
        assert_eq!(decode(b"aGVsbG8_d29ybGQ", p), None);
        // Very short and empty strings.
        assert_eq!(decode(b"Pz8=", p), Some(b"??".to_vec()));
        assert_eq!(decode(b"", p), Some(Vec::new()));
    }

    // Ported from TEST(Base64UrlTest, DecodeIgnorePaddingPolicy).
    #[test]
    fn decode_ignore_padding_policy() {
        let p = DecodePolicy::IgnorePadding;
        assert_eq!(decode(b"aGVsbG8_d29ybGQ", p), Some(b"hello?world".to_vec()));
        // Including the padding is accepted as well.
        assert_eq!(decode(b"aGVsbG8_d29ybGQ=", p), Some(b"hello?world".to_vec()));
    }

    // Ported from TEST(Base64UrlTest, DecodeIntoVector).
    #[test]
    fn decode_into_vector() {
        assert_eq!(decode(b"invalid=", DecodePolicy::DisallowPadding), None);

        let expected: &[u8] = b"1234";
        let result = decode(b"MTIzNA", DecodePolicy::DisallowPadding);
        assert_eq!(result.as_deref(), Some(expected));
    }

    // Ported from TEST(Base64UrlTest, DecodeDisallowPaddingPolicy).
    #[test]
    fn decode_disallow_padding_policy() {
        let p = DecodePolicy::DisallowPadding;
        assert_eq!(decode(b"aGVsbG8_d29ybGQ=", p), None);
        // Input without padding is allowed.
        assert_eq!(decode(b"aGVsbG8_d29ybGQ", p), Some(b"hello?world".to_vec()));
    }

    // Ported from TEST(Base64UrlTest, DecodeDisallowsBase64Alphabet).
    #[test]
    fn decode_disallows_base64_alphabet() {
        // "/" is in the standard alphabet only. Base64url uses "_".
        assert_eq!(decode(b"aGVsbG8/d29ybGQ=", DecodePolicy::RequirePadding), None);
        assert_eq!(decode(b"aGVsbG8+d29ybGQ=", DecodePolicy::RequirePadding), None);
    }

    // Ported from TEST(Base64UrlTest, DecodeDisallowsPaddingOnly).
    #[test]
    fn decode_disallows_padding_only() {
        for input in [&b"="[..], b"==", b"===", b"===="] {
            assert_eq!(decode(input, DecodePolicy::IgnorePadding), None);
        }
    }

    // Ported from the FUZZ_TEST(Base64UrlTest, FuzzBase64UrlEncode) body. The
    // fuzztest domain is replaced by a seeded PRNG. Also checks the
    // invariants that `FuzzBase64UrlEncode` checks.
    #[test]
    fn fuzz_base64url_encode() {
        let mut rng = XorShift64(0x5151_2323_4545_6767);
        for _ in 0..2000 {
            let len = rng.below(64);
            let input = rng.bytes(len);
            for policy in ENC_POLICIES {
                let encoded = encode(&input, policy);
                assert!(encoded
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'='));
                if policy == EncodePolicy::OmitPadding {
                    assert!(!encoded.contains('='));
                }
                let decode_policy = match policy {
                    EncodePolicy::IncludePadding => DecodePolicy::RequirePadding,
                    EncodePolicy::OmitPadding => DecodePolicy::DisallowPadding,
                };
                assert_eq!(decode(encoded.as_bytes(), decode_policy), Some(input.clone()));
                assert_eq!(
                    decode(encoded.as_bytes(), DecodePolicy::IgnorePadding),
                    Some(input.clone())
                );
            }
        }
    }

    // Ported from the FUZZ_TEST(Base64UrlTest, FuzzBase64UrlDecode) body. The
    // original only checks that decoding does not crash. Also checks that the
    // policies agree where their rules overlap.
    #[test]
    fn fuzz_base64url_decode() {
        let alphabet = b"ABCxyz019-_=+/ \x00\xff";
        let mut rng = XorShift64(0x7777_1111_9999_3333);
        for _ in 0..20000 {
            let len = rng.below(30);
            let input: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
            let require = decode(&input, DecodePolicy::RequirePadding);
            let ignore = decode(&input, DecodePolicy::IgnorePadding);
            let disallow = decode(&input, DecodePolicy::DisallowPadding);
            if require.is_some() {
                assert_eq!(require, ignore, "{input:?}");
            }
            if disallow.is_some() {
                assert_eq!(disallow, ignore, "{input:?}");
            }
        }
    }

    // Extra test: RFC 4648 section 10 style vector with URL-only characters.
    #[test]
    fn url_alphabet_vector() {
        // Standard base64 of [0xfb, 0xff] is "+/8=".
        assert_eq!(encode(&[0xfb, 0xff], EncodePolicy::IncludePadding), "-_8=");
        assert_eq!(encode(&[0xfb, 0xff], EncodePolicy::OmitPadding), "-_8");
        assert_eq!(decode(b"-_8", DecodePolicy::IgnorePadding), Some(vec![0xfb, 0xff]));
    }
}
