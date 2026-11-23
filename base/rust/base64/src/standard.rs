// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Standard base64 (RFC 4648 section 4), port of `base::Base64Encode` and
//! `base::Base64Decode` from base/base64.{h,cc}. See the crate docs for the
//! differences.

use std::borrow::Cow;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PAD: u8 = b'=';

/// Largest input length accepted by [`encoded_len`]. Same value as the C++
/// `MODP_B64_MAX_INPUT_LEN`.
pub const MAX_INPUT_LEN: usize = (usize::MAX - 1) / 4 * 3;

/// Returns the length of the padded base64 text for `input_len` input bytes.
///
/// Panics if `input_len > MAX_INPUT_LEN`, mirroring the C++ `CHECK_LE`.
pub fn encoded_len(input_len: usize) -> usize {
    assert!(input_len <= MAX_INPUT_LEN, "base64 input is too large");
    input_len.div_ceil(3) * 4
}

/// Mirrors `base::Base64DecodePolicy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodePolicy {
    /// Input must match the output of [`encode`]: length divisible by 4, at
    /// most two trailing `=`, and no other characters outside the alphabet.
    Strict,
    /// Matches the WHATWG forgiving-base64 decode. All ASCII whitespace
    /// (TAB, LF, FF, CR, SPACE) is removed first. Padding may be omitted,
    /// as long as the length is not 1 mod 4. At most two `=` are accepted.
    Forgiving,
}

/// Encodes `input` as padded standard base64.
pub fn encode(input: &[u8]) -> String {
    let mut out = String::new();
    encode_append(input, &mut out);
    out
}

/// Encodes `input` as padded standard base64 and appends it to `output`.
///
/// Panics if the resulting length would overflow `usize`, like the C++ code.
pub fn encode_append(input: &[u8], output: &mut String) {
    let added = encoded_len(input.len());
    let _total = output.len().checked_add(added).expect("base64 output length overflows usize");
    output.reserve(added);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let mut quad = [PAD; 4];
        quad[0] = ALPHABET[(n >> 18) as usize];
        quad[1] = ALPHABET[((n >> 12) & 63) as usize];
        if chunk.len() > 1 {
            quad[2] = ALPHABET[((n >> 6) & 63) as usize];
        }
        if chunk.len() > 2 {
            quad[3] = ALPHABET[(n & 63) as usize];
        }
        output.extend(quad.iter().map(|&c| char::from(c)));
    }
}

/// Decodes `input` with the strict policy. Same as the C++
/// `base::Base64Decode(std::string_view)` overload.
pub fn decode_strict(input: &[u8]) -> Option<Vec<u8>> {
    decode(input, DecodePolicy::Strict)
}

/// Decodes base64 `input` under `policy`. Returns `None` if the input is
/// invalid.
pub fn decode(input: &[u8], policy: DecodePolicy) -> Option<Vec<u8>> {
    let data: Cow<'_, [u8]> = match policy {
        DecodePolicy::Strict => Cow::Borrowed(input),
        DecodePolicy::Forgiving if input.iter().any(|&b| is_infra_whitespace(b)) => {
            Cow::Owned(input.iter().copied().filter(|&b| !is_infra_whitespace(b)).collect())
        }
        DecodePolicy::Forgiving => Cow::Borrowed(input),
    };
    let body_len = if data.is_empty() { 0 } else { padded_body_len(&data, policy)? };
    decode_body(&data[..body_len])
}

/// ASCII whitespace as defined by the WHATWG infra standard. Vertical tab is
/// deliberately excluded.
fn is_infra_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

/// Removes trailing padding and checks the length rules, like modp_b64's
/// `do_decode_padding`. Returns the length of the part to decode. `data` must
/// not be empty.
fn padded_body_len(data: &[u8], policy: DecodePolicy) -> Option<usize> {
    let mut len = data.len();
    let remainder = len % 4;
    if policy == DecodePolicy::Strict && (remainder != 0 || len < 4) {
        return None;
    }
    if remainder == 0 && len >= 1 && data[len - 1] == PAD {
        len -= 1;
        if len >= 1 && data[len - 1] == PAD {
            len -= 1;
        }
    }
    if len % 4 == 1 {
        return None;
    }
    Some(len)
}

fn decode_char(c: u8) -> Option<u32> {
    let v = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(v))
}

/// Decodes a body whose length is 0, 2, or 3 mod 4 (or a multiple of 4). Any
/// unused low bits in the final character are ignored.
fn decode_body(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(body.len() * 3 / 4 + 1);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &c in body {
        acc = (acc << 6) | decode_char(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
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

    /// Reference encoder that uses a bit accumulator and no chunking.
    fn ref_encode(input: &[u8]) -> String {
        let mut out = String::new();
        let mut acc: u32 = 0;
        let mut bits: u32 = 0;
        for &b in input {
            acc = (acc << 8) | u32::from(b);
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                out.push(char::from(ALPHABET[((acc >> bits) & 63) as usize]));
            }
            acc &= (1 << bits) - 1;
        }
        if bits > 0 {
            out.push(char::from(ALPHABET[((acc << (6 - bits)) & 63) as usize]));
        }
        while !out.len().is_multiple_of(4) {
            out.push('=');
        }
        out
    }

    /// Reference decoder written separately from `decode`. It follows the
    /// same rules: whitespace removal for forgiving mode, then trailing
    /// padding removal only when the length is a multiple of 4, then
    /// per-character validation.
    fn ref_decode(input: &[u8], forgiving: bool) -> Option<Vec<u8>> {
        let s: Vec<u8> = if forgiving {
            input
                .iter()
                .copied()
                .filter(|&b| !matches!(b, b'\t' | b'\n' | 0x0C | b'\r' | b' '))
                .collect()
        } else {
            input.to_vec()
        };
        let mut end = s.len();
        if s.len().is_multiple_of(4) {
            let mut stripped = 0;
            while stripped < 2 && end > 0 && s[end - 1] == b'=' {
                end -= 1;
                stripped += 1;
            }
        } else if !forgiving {
            return None;
        }
        let body = &s[..end];
        if body.len() % 4 == 1 {
            return None;
        }
        let mut out = Vec::new();
        let mut acc: u64 = 0;
        let mut nbits = 0;
        for &c in body {
            let v = ALPHABET.iter().position(|&a| a == c)? as u64;
            acc = (acc << 6) | v;
            nbits += 6;
            while nbits >= 8 {
                nbits -= 8;
                out.push((acc >> nbits) as u8);
            }
            acc &= (1 << nbits) - 1;
        }
        Some(out)
    }

    fn decode_tests_strict(input: &str) -> Option<Vec<u8>> {
        decode(input.as_bytes(), DecodePolicy::Strict)
    }

    // Ported from TEST_P(Base64Test, Basic). The C++ test is parameterized on
    // base64-simd vs modp, and both produce the same output here.
    #[test]
    fn basic() {
        let text = "hello world";
        let base64_text = "aGVsbG8gd29ybGQ=";

        let encoded = encode(text.as_bytes());
        assert_eq!(base64_text, encoded);

        let decoded = decode_strict(encoded.as_bytes());
        assert_eq!(Some(text.as_bytes().to_vec()), decoded);
    }

    // Ported from TEST_P(Base64Test, ForgivingAndStrictDecode).
    #[test]
    fn forgiving_and_strict_decode() {
        // nullptr in C++ (decode failure) is None here.
        type Case = (&'static str, Option<&'static [u8]>, Option<&'static [u8]>);
        let cases: &[Case] = &[
            // Failures in all modes.
            ("abc&", None, None),
            ("ab-d", None, None),
            ("abcde", None, None),
            ("a", None, None),
            // Invalid padding fails in both modes.
            ("abcd=", None, None),
            ("abcd==", None, None),
            ("abcd===", None, None),
            ("abcd====", None, None),
            ("abcd==============", None, None),
            ("abcde===", None, None),
            ("=", None, None),
            ("====", None, None),
            // Multiples of 4 always succeed.
            ("abcd", Some(b"i\xB7\x1D"), Some(b"i\xB7\x1D")),
            ("abc=", Some(b"i\xB7"), Some(b"i\xB7")),
            ("abcdefgh", Some(b"i\xB7\x1Dy\xF8!"), Some(b"i\xB7\x1Dy\xF8!")),
            // Forgiving mode allows omitted padding when len % 4 != 1.
            ("abcdef", Some(b"i\xB7\x1Dy"), None),
            ("abc", Some(b"i\xB7"), None),
            ("ab", Some(b"i"), None),
            // Whitespace is stripped in forgiving mode.
            (" a bcd", Some(b"i\xB7\x1D"), None),
            ("ab\t\tc=", Some(b"i\xB7"), None),
            ("ab c\ndefgh", Some(b"i\xB7\x1Dy\xF8!"), None),
            ("a\tb\nc\x0C d\r", Some(b"i\xB7\x1D"), None),
            ("this should fail", Some(b"\xB6\x18\xAC\xB2\x1A.\x95\xD7\xDA\x8A"), None),
            // VT is not whitespace.
            ("ab\x0Bcd", None, None),
            // Empty input gives empty output.
            ("", Some(b""), Some(b"")),
        ];
        for &(input, forgiving, _) in cases {
            let got = decode(input.as_bytes(), DecodePolicy::Forgiving);
            assert_eq!(forgiving.map(<[u8]>::to_vec), got, "Forgiving: {input:?}");
        }
        for &(input, _, strict) in cases {
            let got = decode(input.as_bytes(), DecodePolicy::Strict);
            assert_eq!(strict.map(<[u8]>::to_vec), got, "Strict: {input:?}");
        }
    }

    // Ported from TEST_P(Base64Test, Binary).
    #[test]
    fn binary() {
        let data: [u8; 4] = [0x00, 0x01, 0xFE, 0xFF];

        // The C++ std::string_view overload is the same function here, since
        // `encode` takes `&[u8]` for both binary and text input.
        let binary_encoded = encode(&data);
        assert_eq!(binary_encoded, "AAH+/w==");

        assert_eq!(decode_strict(binary_encoded.as_bytes()), Some(data.to_vec()));
        assert_eq!(decode_strict(b"invalid base64!"), None);

        let mut encoded_with_prefix = String::from("PREFIX");
        encode_append(&data, &mut encoded_with_prefix);
        assert_eq!(encoded_with_prefix, format!("PREFIX{binary_encoded}"));
    }

    // Ported from TEST_P(Base64Test, InPlace). Rust does not allow the
    // input and output to alias, so this checks the round trip instead.
    #[test]
    fn in_place() {
        let text = "hello world";
        let base64_text = "aGVsbG8gd29ybGQ=";

        let encoded = encode(text.as_bytes());
        assert_eq!(base64_text, encoded);

        let decoded = decode_strict(encoded.as_bytes()).expect("valid input");
        assert_eq!(decoded, text.as_bytes());
    }

    // Ported from TEST_P(Base64Test, Overflow). The C++ test uses a fake span
    // of MODP_B64_MAX_INPUT_LEN + 1 bytes and EXPECT_CHECK_DEATH. Rust slices
    // cannot be that large, so the length check is tested directly.
    #[test]
    #[should_panic(expected = "base64 input is too large")]
    fn overflow() {
        let _ = encoded_len(MAX_INPUT_LEN + 1);
    }

    #[test]
    fn overflow_max_input_len_is_representable() {
        // Same reasoning as the comment in modp_b64.h: the encoded length of
        // MAX_INPUT_LEN fits in usize.
        assert_eq!(encoded_len(MAX_INPUT_LEN), (usize::MAX - 1) / 4 * 4);
        assert_eq!(encoded_len(0), 0);
        assert_eq!(encoded_len(1), 4);
        assert_eq!(encoded_len(3), 4);
        assert_eq!(encoded_len(4), 8);
    }

    #[test]
    fn rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, enc) in cases {
            assert_eq!(encode(plain.as_bytes()), enc);
            assert_eq!(decode_strict(enc.as_bytes()), Some(plain.as_bytes().to_vec()));
        }
    }

    #[test]
    fn strict_padding_quirks_match_modp() {
        // "ab==" has length 4, so one padding run is stripped. The body "ab"
        // decodes to one byte, the same as in modp_b64.
        assert_eq!(decode_tests_strict("ab=="), Some(b"i".to_vec()));
        // Unused low bits in the final character are ignored.
        assert_eq!(decode_tests_strict("aGk="), Some(b"hi".to_vec()));
        assert_eq!(decode_tests_strict("aGl="), Some(b"hi".to_vec()));
        assert_eq!(decode_tests_strict("a==="), None);
    }

    #[test]
    fn encode_append_keeps_prefix() {
        let mut out = String::from("x");
        encode_append(b"", &mut out);
        assert_eq!(out, "x");
        encode_append(b"f", &mut out);
        assert_eq!(out, "xZg==");
    }

    #[test]
    fn encode_str_and_bytes_agree() {
        assert_eq!(encode("hello?world".as_bytes()), "aGVsbG8/d29ybGQ=");
    }

    #[test]
    fn encode_matches_reference_random() {
        let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
        for _ in 0..2000 {
            let len = rng.below(70);
            let data = rng.bytes(len);
            assert_eq!(encode(&data), ref_encode(&data), "len {len}");
        }
    }

    #[test]
    fn round_trip_random() {
        let mut rng = XorShift64(0x0123_4567_89AB_CDEF);
        for _ in 0..2000 {
            let len = rng.below(70);
            let data = rng.bytes(len);
            let enc = encode(&data);
            assert_eq!(decode_strict(enc.as_bytes()).as_deref(), Some(&data[..]));
            assert_eq!(decode(enc.as_bytes(), DecodePolicy::Forgiving).as_deref(), Some(&data[..]));
        }
    }

    #[test]
    fn forgiving_ignores_inserted_whitespace() {
        let mut rng = XorShift64(0xDEAD_BEEF_F00D_1234);
        let ws = [b' ', b'\t', b'\n', 0x0C, b'\r'];
        for _ in 0..1000 {
            let len = rng.below(40);
            let data = rng.bytes(len);
            let enc = encode(&data).into_bytes();
            let mut spaced = Vec::new();
            for &c in &enc {
                if rng.below(3) == 0 {
                    spaced.push(ws[rng.below(ws.len())]);
                }
                spaced.push(c);
            }
            assert_eq!(decode(&spaced, DecodePolicy::Forgiving).as_deref(), Some(&data[..]));
            if !spaced.iter().all(|&b| enc.contains(&b)) {
                // Strict mode never accepts whitespace.
                assert_eq!(decode(&spaced, DecodePolicy::Strict), None);
            }
        }
    }

    #[test]
    fn decode_matches_reference_random() {
        let alphabet = b"ABCxyz019+/=-_ \t\n\x0C\r\x0B!";
        let mut rng = XorShift64(0xA5A5_5A5A_1357_9BDF);
        for _ in 0..20000 {
            let len = rng.below(25);
            let input: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
            assert_eq!(
                decode(&input, DecodePolicy::Strict),
                ref_decode(&input, false),
                "strict {input:?}"
            );
            assert_eq!(
                decode(&input, DecodePolicy::Forgiving),
                ref_decode(&input, true),
                "forgiving {input:?}"
            );
        }
    }
}
