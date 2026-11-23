// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `components/base32` (components/base32/base32.{h,cc}).
//!
//! Base32 encoding and decoding as defined in RFC 4648 section 6, using the
//! standard alphabet `A-Z2-7`.
//!
//! The C++ component has two implementations (a legacy C++ one and a Rust one
//! behind the `kComponentsBase32InRust` feature). This crate ports the legacy
//! C++ algorithm, which is the reference behavior; the feature-flag dispatch
//! and the cxx bridge (`base32.rs`) are not ported.
//!
//! Differences from the C++ API:
//! - `Base32Encode(input, policy)` is `base32_encode(input, policy)`. The
//!   policy has a `Default` of `IncludePadding`, matching the C++ default
//!   argument.
//! - `Base32Decode` is `base32_decode`. It takes `&[u8]` rather than
//!   `std::string_view`, so arbitrary bytes are accepted, and returns a
//!   `Vec<u8>`. As in C++, an empty `Vec` signals an error or empty input.
//! - Where the C++ code uses `base::CheckedNumeric(...).ValueOrDie()` to
//!   CHECK-fail on size overflow, this crate panics with a message. That only
//!   happens for inputs of more than `usize::MAX / 8` bytes, which cannot be
//!   allocated in practice.
//! - The `internal::Base32EncodeCpp`, `Base32EncodeRust`, `Base32DecodeCpp` and
//!   `Base32DecodeRust` test hooks are not ported. There is only one
//!   implementation here.
//! - The `DCHECK`s on the output length are `debug_assert`s.
//!
//! Not ported: `base/feature_list` dispatch, the cxx bridge, and the fuzz tests
//! (`EncodeParity`/`DecodeParity` compared the two implementations). Their
//! role is covered by randomized differential tests against an independent
//! bit-level reference, and by golden vectors generated from the unmodified
//! C++ `base32.cc` in `tests/cpp_golden.rs`.

#![forbid(unsafe_code)]

/// Whether to include trailing `=` padding in the output of
/// [`base32_encode`]. Mirrors `base32::Base32EncodePolicy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Base32EncodePolicy {
    /// Include the trailing padding in the output, when necessary.
    #[default]
    IncludePadding,
    /// Omit trailing padding in the output. Such an output will not be
    /// decodable unless the decoder knows the input size. Its length is
    /// `ceil(input.len() * 8.0 / 5.0)`.
    OmitPadding,
}

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const PADDING_CHAR: u8 = b'=';

/// Encodes `input` in base32 as defined in RFC 4648. The `policy` controls
/// whether trailing `=` padding is included.
///
/// Empty input yields an empty string for either policy.
pub fn base32_encode(input: &[u8], policy: Base32EncodePolicy) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Per RFC 4648, the output is 8 characters per 40 bits of input (padded
    // form), and for the unpadded form one character per 5 bits, rounded up.
    // padded: ceil(len / 5) * 8. unpadded: ceil(len * 8 / 5).
    let padded_length = input.len().div_ceil(5) * 8;
    let unpadded_length = input
        .len()
        .checked_mul(8)
        .and_then(|bits| bits.checked_add(4))
        .expect("base32: input too large")
        / 5;

    let encoded_length = match policy {
        Base32EncodePolicy::IncludePadding => padded_length,
        Base32EncodePolicy::OmitPadding => unpadded_length,
    };

    let mut output = String::with_capacity(encoded_length);

    // A bit stream which is read from the left and appended to from the right
    // as it is emptied.
    let mut bit_stream: u16 = (input[0] as u16) << 8;
    let mut next_byte_index = 1usize;
    let mut free_bits: u32 = 8;
    while free_bits < 16 {
        // Extract the 5 leftmost bits of the stream.
        output.push(ALPHABET[(bit_stream >> 11) as usize] as char);
        bit_stream <<= 5;
        free_bits += 5;

        // If there is room for another byte, inject it if any are left.
        if free_bits >= 8 && next_byte_index < input.len() {
            free_bits -= 8;
            bit_stream = bit_stream.wrapping_add((input[next_byte_index] as u16) << free_bits);
            next_byte_index += 1;
        }
    }

    if policy == Base32EncodePolicy::IncludePadding {
        for _ in 0..(padded_length - unpadded_length) {
            output.push(PADDING_CHAR as char);
        }
    }

    debug_assert_eq!(output.len(), encoded_length);
    output
}

/// Decodes `input` from base32. Returns an empty `Vec` on error, including
/// when `input` is empty.
///
/// Decoding stops at the first `=`; anything after it is ignored. Lowercase
/// letters and any byte outside the alphabet are errors. Only an integral
/// number of bytes is decoded, so trailing bits that do not fill a byte are
/// discarded.
pub fn base32_decode(input: &[u8]) -> Vec<u8> {
    let input = match input.iter().position(|&c| c == PADDING_CHAR) {
        Some(pos) => &input[..pos],
        None => input,
    };
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(input.len().saturating_mul(5) / 8);

    // A bit stream which is filled from the right and flushed from the left a
    // byte at a time.
    let mut bit_stream: u16 = 0;
    let mut free_bits: u32 = 16;
    for &input_char in input {
        let Some(decoded_5bits) = reverse_mapping(input_char) else {
            return Vec::new();
        };

        // Place the next 5 decoded bits into the stream.
        bit_stream |= (decoded_5bits as u16) << (free_bits - 5);
        free_bits -= 5;

        // Once the stream holds a full byte, flush it to the output.
        if free_bits <= 8 {
            output.push((bit_stream >> 8) as u8);
            bit_stream <<= 8;
            free_bits += 8;
        }
    }

    output
}

/// Returns the 5-bit value of an alphabet character, or `None` if the byte is
/// not in the alphabet.
fn reverse_mapping(input_char: u8) -> Option<u8> {
    match input_char {
        b'A'..=b'Z' => Some(input_char - b'A'),
        b'2'..=b'7' => Some(input_char - b'2' + 26),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small xorshift64* PRNG for the randomized tests.
    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }

        fn bytes(&mut self, len: usize) -> Vec<u8> {
            (0..len).map(|_| self.next_u64() as u8).collect()
        }
    }

    /// Bit-level reference encoder, written independently of the production
    /// code: it expands the input to a bit string and reads 5-bit groups.
    fn reference_encode(input: &[u8], padded: bool) -> String {
        let mut bits: Vec<bool> = Vec::new();
        for &b in input {
            for i in (0..8).rev() {
                bits.push((b >> i) & 1 == 1);
            }
        }
        while !bits.len().is_multiple_of(5) {
            bits.push(false);
        }
        let mut out = String::new();
        for group in bits.chunks(5) {
            let v = group.iter().fold(0usize, |acc, &b| acc * 2 + usize::from(b));
            out.push(ALPHABET[v] as char);
        }
        if padded {
            while !out.len().is_multiple_of(8) {
                out.push('=');
            }
        }
        out
    }

    /// Bit-level reference decoder: maps each character to 5 bits, keeps only
    /// the complete bytes, and stops at the first '='.
    fn reference_decode(input: &[u8]) -> Vec<u8> {
        let end = input.iter().position(|&c| c == b'=').unwrap_or(input.len());
        let mut bits: Vec<bool> = Vec::new();
        for &c in &input[..end] {
            let v = match c {
                b'A'..=b'Z' => c - b'A',
                b'2'..=b'7' => c - b'2' + 26,
                _ => return Vec::new(),
            };
            for i in (0..5).rev() {
                bits.push((v >> i) & 1 == 1);
            }
        }
        bits.chunks_exact(8)
            .map(|byte| byte.iter().fold(0u8, |acc, &b| acc * 2 + u8::from(b)))
            .collect()
    }

    // Ported from EncodesRfcTestVectorsCorrectlyWithoutPadding.
    #[test]
    fn encodes_rfc_test_vectors_correctly_without_padding() {
        let test_data = b"foobar";
        let expected = ["", "MY", "MZXQ", "MZXW6", "MZXW6YQ", "MZXW6YTB", "MZXW6YTBOI"];

        // Run the tests, with one more letter in the input every pass.
        for (i, expected) in expected.iter().enumerate() {
            let encoded = base32_encode(&test_data[..i], Base32EncodePolicy::OmitPadding);
            assert_eq!(*expected, encoded, "i = {i}");
            let decoded = base32_decode(encoded.as_bytes());
            assert_eq!(&test_data[..i], decoded.as_slice(), "i = {i}");
        }
    }

    // Ported from EncodesRfcTestVectorsCorrectlyWithPadding.
    #[test]
    fn encodes_rfc_test_vectors_correctly_with_padding() {
        let test_data = b"foobar";
        let expected =
            ["", "MY======", "MZXQ====", "MZXW6===", "MZXW6YQ=", "MZXW6YTB", "MZXW6YTBOI======"];

        // Run the tests, with one more letter in the input every pass.
        for (i, expected) in expected.iter().enumerate() {
            let encoded = base32_encode(&test_data[..i], Base32EncodePolicy::default());
            assert_eq!(*expected, encoded, "i = {i}");
            let decoded = base32_decode(encoded.as_bytes());
            assert_eq!(&test_data[..i], decoded.as_slice(), "i = {i}");
        }
    }

    // Ported from EncodesSha256HashCorrectly.
    #[test]
    fn encodes_sha256_hash_correctly() {
        // Useful to test with longer input than the RFC test vectors, and
        // encoding SHA-256 hashes is one of the use cases for this component.
        let hash: [u8; 32] = [
            0x1f, 0x25, 0xe1, 0xca, 0xba, 0x4f, 0xf9, 0xb8, 0x27, 0x24, 0x83, 0x0f, 0xca, 0x60,
            0xe4, 0xc2, 0xbe, 0xa8, 0xc3, 0xa9, 0x44, 0x1c, 0x27, 0xb0, 0xb4, 0x3e, 0x6a, 0x96,
            0x94, 0xc7, 0xb8, 0x04,
        ];
        let test_span = &hash[..32];
        let encoded = base32_encode(test_span, Base32EncodePolicy::OmitPadding);
        assert_eq!("D4S6DSV2J743QJZEQMH4UYHEYK7KRQ5JIQOCPMFUHZVJNFGHXACA", encoded);
        let decoded = base32_decode(encoded.as_bytes());
        assert_eq!(test_span, decoded.as_slice());
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(base32_encode(&[], Base32EncodePolicy::IncludePadding), "");
        assert_eq!(base32_encode(&[], Base32EncodePolicy::OmitPadding), "");
        assert!(base32_decode(b"").is_empty());
        // Input that is only padding is also empty after truncation.
        assert!(base32_decode(b"======").is_empty());
    }

    #[test]
    fn decode_rejects_invalid_characters() {
        // Lowercase letters, digits 0, 1, 8, 9, and symbols are not in the
        // alphabet, so the whole decode fails.
        for bad in [&b"mzxw6"[..], b"MZXW1", b"MZXW8", b"MZ-W6", b"MZXW6 ", b"\xff"] {
            assert!(base32_decode(bad).is_empty(), "{bad:?}");
        }
        // An invalid character after the first '=' is ignored.
        assert_eq!(base32_decode(b"MY=!!"), b"f");
    }

    #[test]
    fn decode_discards_partial_trailing_bits() {
        // One character is 5 bits, which is not a whole byte.
        assert!(base32_decode(b"M").is_empty());
        // Two characters are 10 bits: one byte, with 2 bits discarded.
        assert_eq!(base32_decode(b"MY"), b"f");
        // Seven characters are 35 bits: four bytes, with 3 bits discarded.
        assert_eq!(base32_decode(b"MZXW6YQ").len(), 4);
    }

    #[test]
    fn omit_padding_length_is_ceil_of_bits_over_five() {
        for len in 0..40usize {
            let input = vec![0xabu8; len];
            let encoded = base32_encode(&input, Base32EncodePolicy::OmitPadding);
            assert_eq!(encoded.len(), (len * 8).div_ceil(5), "len = {len}");
        }
    }

    #[test]
    fn padded_length_is_multiple_of_eight() {
        for len in 0..40usize {
            let input = vec![0x5au8; len];
            let encoded = base32_encode(&input, Base32EncodePolicy::IncludePadding);
            assert_eq!(encoded.len() % 8, 0, "len = {len}");
            assert_eq!(encoded.len(), len.div_ceil(5) * 8, "len = {len}");
        }
    }

    // Randomized differential test against the bit-level reference, covering
    // the role of the C++ EncodeParity fuzz test.
    #[test]
    fn encode_matches_reference_on_random_inputs() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..2000 {
            let len = rng.below(70);
            let input = rng.bytes(len);
            assert_eq!(
                base32_encode(&input, Base32EncodePolicy::IncludePadding),
                reference_encode(&input, true),
                "input = {input:02x?}"
            );
            assert_eq!(
                base32_encode(&input, Base32EncodePolicy::OmitPadding),
                reference_encode(&input, false),
                "input = {input:02x?}"
            );
        }
    }

    // Randomized differential test against the bit-level decoder, covering the
    // role of the C++ DecodeParity fuzz test. The inputs mix alphabet
    // characters, padding, and occasional invalid bytes.
    #[test]
    fn decode_matches_reference_on_random_inputs() {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567=a1!";
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for _ in 0..4000 {
            let len = rng.below(40);
            let input: Vec<u8> = (0..len).map(|_| CHARS[rng.below(CHARS.len())]).collect();
            assert_eq!(
                base32_decode(&input),
                reference_decode(&input),
                "input = {:?}",
                String::from_utf8_lossy(&input)
            );
        }
    }

    #[test]
    fn round_trip_random_bytes() {
        let mut rng = Rng(42);
        for _ in 0..1000 {
            let len = rng.below(100);
            let input = rng.bytes(len);
            let omit = base32_encode(&input, Base32EncodePolicy::OmitPadding);
            assert_eq!(base32_decode(omit.as_bytes()), input);
            let pad = base32_encode(&input, Base32EncodePolicy::IncludePadding);
            assert_eq!(base32_decode(pad.as_bytes()), input);
        }
    }
}
