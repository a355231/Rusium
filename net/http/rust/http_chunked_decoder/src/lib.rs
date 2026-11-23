// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `net::HttpChunkedDecoder`
//! (net/http/http_chunked_decoder.{h,cc}), an incremental decoder for the
//! HTTP chunked transfer coding (RFC 7230 section 4.1).
//!
//! The decoder is fed the body in pieces. `filter_buf` removes the chunk
//! framing from each piece in place and returns how many decoded body bytes
//! were moved to the front of the piece. Chunk-size lines may end in CRLF or
//! bare LF. Chunk extensions are ignored, and trailers are read and discarded.
//!
//! Differences from the C++ version:
//! - `filter_buf` returns `Result<usize, InvalidChunkedEncoding>`. The C++
//!   returns a negative net error code, and `FilterBuff` (sic) returned
//!   `ERR_INVALID_CHUNKED_ENCODING` (-321). `InvalidChunkedEncoding::net_error`
//!   returns that code.
//! - `bytes_after_eof` is `usize`, not `int`. The C++ `int` counter would
//!   `CHECK`-fail in the return conversion once more than `INT_MAX` bytes
//!   followed the final CRLF. There is no such limit here.
//! - The C++ class wraps either this C++ decoder or a Rust implementation,
//!   chosen by a feature flag. This crate is only the Rust implementation.
//!   The C++ `Impl`, `CreateForTesting`, and the `kHttpChunkedDecoderInRust`
//!   feature are not ported.
//! - `DVLOG` diagnostics are not ported.
//! - Error paths leave the decoder in the same state as C++, but the contents
//!   of `buf` after an error are unspecified, as they are in C++.
//! - The C++ `ParseChunkSize` relies on `base::HexStringToInt64`. That
//!   function rejects empty input and values above `i64::MAX`. The same rule
//!   is implemented directly here.
//! - The C++ fuzz test `DecodeParity` compares the C++ and Rust decoders. That
//!   parity check is run by the golden test in `tests/cpp_golden.rs`, which
//!   compares against the C++ source.

use std::fmt;

/// The net error code for invalid chunked encoding (`net/base/net_error_list.h`).
pub const ERR_INVALID_CHUNKED_ENCODING: i32 = -321;

/// The maximum length of the partial-line buffer between calls to
/// [`HttpChunkedDecoder::filter_buf`]. Exposed for tests, as in C++
/// (`HttpChunkedDecoder::kMaxLineBufLen`).
pub const MAX_LINE_BUF_LEN: usize = 16384;

/// The error returned for malformed chunked encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidChunkedEncoding;

impl InvalidChunkedEncoding {
    /// The equivalent net error code, `ERR_INVALID_CHUNKED_ENCODING`.
    pub const fn net_error(self) -> i32 {
        ERR_INVALID_CHUNKED_ENCODING
    }
}

impl fmt::Display for InvalidChunkedEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid chunked encoding")
    }
}

impl std::error::Error for InvalidChunkedEncoding {}

/// Incremental decoder for `Transfer-Encoding: chunked` bodies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpChunkedDecoder {
    // Bytes remaining in the current chunk's data.
    chunk_remaining: u64,
    // A partial line carried over from a previous call.
    line_buf: Vec<u8>,
    // True while waiting for the CRLF that ends a chunk's data.
    chunk_terminator_remaining: bool,
    // True after the zero-size last-chunk has been read.
    reached_last_chunk: bool,
    // True once the empty line that ends the trailer has been read.
    reached_eof: bool,
    // Bytes that followed the final CRLF.
    bytes_after_eof: usize,
}

impl HttpChunkedDecoder {
    /// Creates a decoder with no state.
    pub fn new() -> Self {
        Self::default()
    }

    /// True once `filter_buf` has seen the final CRLF of the body.
    pub fn reached_eof(&self) -> bool {
        self.reached_eof
    }

    /// The number of bytes that followed the final CRLF.
    pub fn bytes_after_eof(&self) -> usize {
        self.bytes_after_eof
    }

    /// Removes chunk framing from `buf` in place and returns the number of
    /// decoded body bytes, which are moved to `buf[..n]`.
    ///
    /// Call [`reached_eof`](Self::reached_eof) afterwards to check for the end
    /// of the body. Bytes after the end are counted by
    /// [`bytes_after_eof`](Self::bytes_after_eof) and are not written to `buf`.
    pub fn filter_buf(&mut self, buf: &mut [u8]) -> Result<usize, InvalidChunkedEncoding> {
        let len = buf.len();
        // `read` is the position in `buf` being parsed. `write` is where the
        // next decoded body byte goes. `write` never passes `read`.
        let mut read = 0;
        let mut write = 0;
        while read < len {
            if self.chunk_remaining > 0 {
                let available = len - read;
                let num = usize::try_from(self.chunk_remaining)
                    .unwrap_or(usize::MAX)
                    .min(available);
                self.chunk_remaining -= num as u64;
                if write != read {
                    buf.copy_within(read..read + num, write);
                }
                write += num;
                read += num;
                // After each chunk's data there must be a CRLF.
                if self.chunk_remaining == 0 {
                    self.chunk_terminator_remaining = true;
                }
                continue;
            } else if self.reached_eof {
                self.bytes_after_eof += len - read;
                break;
            }

            let consumed = self.scan_for_chunk_remaining(&buf[read..])?;
            read += consumed;
        }
        Ok(write)
    }

    /// Consumes one line from the start of `buf`, or buffers a partial line.
    /// Returns the number of bytes consumed. This is never more than
    /// `buf.len()`, and `buf` is not empty when this is called.
    fn scan_for_chunk_remaining(&mut self, buf: &[u8]) -> Result<usize, InvalidChunkedEncoding> {
        match buf.iter().position(|&c| c == b'\n') {
            Some(index_of_lf) => {
                let mut line = &buf[..index_of_lf];
                // Eliminate a preceding CR.
                if line.last() == Some(&b'\r') {
                    line = &line[..line.len() - 1];
                }
                let consumed = index_of_lf + 1;

                if self.line_buf.is_empty() {
                    self.process_line(line)?;
                } else {
                    if self.line_buf.len() + line.len() > MAX_LINE_BUF_LEN {
                        return Err(InvalidChunkedEncoding);
                    }
                    // Join the saved partial line with the rest of this one.
                    // Put the buffer back even on error, as C++ does.
                    let mut full = std::mem::take(&mut self.line_buf);
                    full.extend_from_slice(line);
                    let result = self.process_line(&full);
                    self.line_buf = full;
                    result?;
                }
                self.line_buf.clear();
                Ok(consumed)
            }
            None => {
                // Save the partial line and wait for more data.
                let mut line = buf;
                // Ignore a trailing CR, which may be followed by its LF later.
                if line.last() == Some(&b'\r') {
                    line = &line[..line.len() - 1];
                }
                if self.line_buf.len() + line.len() > MAX_LINE_BUF_LEN {
                    return Err(InvalidChunkedEncoding);
                }
                self.line_buf.extend_from_slice(line);
                Ok(buf.len())
            }
        }
    }

    /// Interprets one complete line (CRLF removed) according to the decoder
    /// state.
    fn process_line(&mut self, line: &[u8]) -> Result<(), InvalidChunkedEncoding> {
        if self.reached_last_chunk {
            if line.is_empty() {
                self.reached_eof = true;
            }
            // Otherwise this is a trailer line, which is ignored.
        } else if self.chunk_terminator_remaining {
            if !line.is_empty() {
                return Err(InvalidChunkedEncoding);
            }
            self.chunk_terminator_remaining = false;
        } else if !line.is_empty() {
            // Ignore any chunk-extensions.
            let size = match line.iter().position(|&c| c == b';') {
                Some(semicolon) => &line[..semicolon],
                None => line,
            };
            let chunk_size = parse_chunk_size(size).ok_or(InvalidChunkedEncoding)?;
            self.chunk_remaining = chunk_size;
            if chunk_size == 0 {
                self.reached_last_chunk = true;
            }
        } else {
            // An empty line where a chunk-size was expected.
            return Err(InvalidChunkedEncoding);
        }
        Ok(())
    }
}

/// Parses a chunk-size as a hex number.
///
/// This is stricter than RFC 7230 in one way and more lenient in another. Any
/// number of trailing spaces (0x20) is accepted, as some servers pad the
/// size. Nothing else is accepted: no leading space, sign, `0x` prefix, or
/// tab. The value must fit in `i64`, as in the C++ `HexStringToInt64` call.
fn parse_chunk_size(buf: &[u8]) -> Option<u64> {
    let mut digits = buf;
    while let [rest @ .., b' '] = digits {
        digits = rest;
    }
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut value: u64 = 0;
    for &c in digits {
        let digit = u64::from((c as char).to_digit(16)?);
        value = value.checked_mul(16)?.checked_add(digit)?;
    }
    if value > i64::MAX as u64 {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::{HttpChunkedDecoder, InvalidChunkedEncoding, MAX_LINE_BUF_LEN};

    /// Feeds each input to a fresh decoder and checks the decoded output, the
    /// EOF flag, and the count of bytes after EOF.
    fn run_test(
        inputs: &[&[u8]],
        expected_output: &str,
        expected_eof: bool,
        bytes_after_eof: usize,
    ) {
        let mut decoder = HttpChunkedDecoder::new();
        assert!(!decoder.reached_eof());

        let mut result = Vec::new();
        for input in inputs {
            // filter_buf modifies its input, so feed a copy.
            let mut copy = input.to_vec();
            let n = decoder.filter_buf(&mut copy).expect("decode failed");
            result.extend_from_slice(&copy[..n]);
        }

        assert_eq!(expected_output.as_bytes(), result.as_slice());
        assert_eq!(expected_eof, decoder.reached_eof());
        assert_eq!(bytes_after_eof, decoder.bytes_after_eof());
    }

    /// Feeds the inputs until one fails, which must be the input at
    /// `fail_index`.
    fn run_test_until_failure(inputs: &[&[u8]], fail_index: usize) {
        let mut decoder = HttpChunkedDecoder::new();
        assert!(!decoder.reached_eof());

        for (i, input) in inputs.iter().enumerate() {
            let mut copy = input.to_vec();
            if let Err(err) = decoder.filter_buf(&mut copy) {
                assert_eq!(err, InvalidChunkedEncoding);
                assert_eq!(err.net_error(), super::ERR_INVALID_CHUNKED_ENCODING);
                assert_eq!(fail_index, i);
                return;
            }
        }
        panic!("should have failed on input {fail_index}");
    }

    #[test]
    fn basic() {
        run_test(&[b"B\r\nhello hello\r\n0\r\n\r\n"], "hello hello", true, 0);
    }

    #[test]
    fn one_chunk() {
        run_test(&[b"5\r\nhello\r\n"], "hello", false, 0);
    }

    #[test]
    fn typical() {
        run_test(
            &[b"5\r\nhello\r\n", b"1\r\n \r\n", b"5\r\nworld\r\n", b"0\r\n\r\n"],
            "hello world",
            true,
            0,
        );
    }

    #[test]
    fn incremental() {
        run_test(
            &[
                b"5", b"\r", b"\n", b"hello", b"\r", b"\n", b"0", b"\r", b"\n", b"\r", b"\n",
            ],
            "hello",
            true,
            0,
        );
    }

    // Same as above, but group carriage returns with previous input.
    #[test]
    fn incremental2() {
        run_test(
            &[b"5\r", b"\n", b"hello\r", b"\n", b"0\r", b"\n\r", b"\n"],
            "hello",
            true,
            0,
        );
    }

    #[test]
    fn lf_instead_of_crlf() {
        run_test(
            &[b"5\nhello\n", b"1\n \n", b"5\nworld\n", b"0\n\n"],
            "hello world",
            true,
            0,
        );
    }

    #[test]
    fn extensions() {
        run_test(&[b"5;x=0\r\nhello\r\n", b"0;y=\"2 \"\r\n\r\n"], "hello", true, 0);
    }

    #[test]
    fn trailers() {
        run_test(
            &[b"5\r\nhello\r\n", b"0\r\n", b"Foo: 1\r\n", b"Bar: 2\r\n", b"\r\n"],
            "hello",
            true,
            0,
        );
    }

    #[test]
    fn trailers_unfinished() {
        run_test(&[b"5\r\nhello\r\n", b"0\r\n", b"Foo: 1\r\n"], "hello", false, 0);
    }

    #[test]
    fn invalid_chunk_size_too_big() {
        // The chunk size is larger than an i64 can hold.
        run_test_until_failure(&[b"48469410265455838241\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_0x() {
        run_test_until_failure(&[b"0x5\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    // Note that this test covers behavior contrary to RFC 7230.
    #[test]
    fn chunk_size_trailing_space() {
        run_test(&[b"5      \r\nhello\r\n", b"0\r\n\r\n"], "hello", true, 0);
    }

    #[test]
    fn invalid_chunk_size_trailing_tab() {
        run_test_until_failure(&[b"5\t\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_trailing_form_feed() {
        run_test_until_failure(&[b"5\x0c\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_trailing_vertical_tab() {
        run_test_until_failure(&[b"5\x0b\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_trailing_non_hex_digit() {
        run_test_until_failure(&[b"5H\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_leading_space() {
        run_test_until_failure(&[b" 5\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_leading_separator() {
        run_test_until_failure(&[b"\r\n5\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_no_separator() {
        run_test_until_failure(&[b"5\r\nhello", b"1\r\n \r\n", b"0\r\n\r\n"], 1);
    }

    #[test]
    fn invalid_chunk_size_negative() {
        run_test_until_failure(&[b"8\r\n12345678\r\n-5\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_chunk_size_plus() {
        run_test_until_failure(&[b"+5\r\nhello\r\n", b"0\r\n\r\n"], 0);
    }

    #[test]
    fn invalid_consecutive_crlfs() {
        run_test_until_failure(&[b"5\r\nhello\r\n", b"\r\n\r\n\r\n\r\n", b"0\r\n\r\n"], 1);
    }

    #[test]
    fn really_big_chunks() {
        // Number of bytes fed through the decoder per loop iteration.
        const WRITTEN_BYTES_PER_ITERATION: usize = 0x10000;
        // Lengths of the chunk to test. Each is a multiple of the iteration
        // size. One overflows a signed 32-bit int, the other an unsigned one.
        const CHUNK_LENGTHS: [u64; 2] = [0x0c000_0000, 0x1_0000_0000];

        for chunk_length in CHUNK_LENGTHS {
            let mut decoder = HttpChunkedDecoder::new();
            assert!(!decoder.reached_eof());

            // Feed just the header.
            let mut header = format!("{chunk_length:x}\r\n").into_bytes();
            assert_eq!(decoder.filter_buf(&mut header), Ok(0));
            assert!(!decoder.reached_eof());

            // Repeatedly feed data with no chunk framing. The decoder must not
            // move it, so it keeps its pattern.
            let mut data: Vec<u8> = (0..WRITTEN_BYTES_PER_ITERATION)
                .map(|i| i as u8)
                .collect();
            let mut total_written: u64 = 0;
            while total_written < chunk_length {
                assert_eq!(
                    decoder.filter_buf(&mut data),
                    Ok(WRITTEN_BYTES_PER_ITERATION)
                );
                assert!(!decoder.reached_eof());
                total_written += WRITTEN_BYTES_PER_ITERATION as u64;
            }

            // Chunk terminator and the final chunk.
            let mut final_chunk = *b"\r\n0\r\n\r\n";
            assert_eq!(decoder.filter_buf(&mut final_chunk), Ok(0));
            assert!(decoder.reached_eof());

            for (i, &byte) in data.iter().enumerate() {
                assert_eq!(byte, i as u8);
            }
        }
    }

    #[test]
    fn excessive_chunk_len() {
        // Smallest number that can't be represented as an i64.
        run_test_until_failure(&[b"8000000000000000\r\nhello\r\n"], 0);
    }

    #[test]
    fn excessive_chunk_len2() {
        // Smallest number that can't be represented as a u64.
        run_test_until_failure(&[b"10000000000000000\r\nhello\r\n"], 0);
    }

    #[test]
    fn largest_i64_chunk_len_is_accepted_as_a_header() {
        // 0x7fffffffffffffff is the largest accepted size. The decoder waits
        // for that much data, so only the header is fed here.
        let mut decoder = HttpChunkedDecoder::new();
        let mut header = b"7fffffffffffffff\r\n".to_vec();
        assert_eq!(decoder.filter_buf(&mut header), Ok(0));
        assert!(!decoder.reached_eof());
    }

    #[test]
    fn basic_extra_data() {
        run_test(&[b"5\r\nhello\r\n0\r\n\r\nextra bytes"], "hello", true, 11);
    }

    #[test]
    fn incremental_extra_data() {
        run_test(
            &[
                b"5", b"\r", b"\n", b"hello", b"\r", b"\n", b"0", b"\r", b"\n", b"\r",
                b"\nextra bytes",
            ],
            "hello",
            true,
            11,
        );
    }

    #[test]
    fn multiple_extra_data_blocks() {
        run_test(&[b"5\r\nhello\r\n0\r\n\r\nextra", b" bytes"], "hello", true, 11);
    }

    // Test when the line with the chunk length is too long.
    #[test]
    fn long_chunk_length_line() {
        let big_chunk = vec![b'0'; MAX_LINE_BUF_LEN];
        run_test_until_failure(&[&big_chunk, b"5"], 1);
    }

    // Test when the extension portion of the line with the chunk length is too
    // long.
    #[test]
    fn long_length_length_line() {
        let mut big_chunk = b"5;".to_vec();
        big_chunk.extend(std::iter::repeat_n(b'0', MAX_LINE_BUF_LEN));
        let inputs: [&[u8]; 1] = [&big_chunk];
        // The first input is the "5;" part, then the long part.
        let first = b"5;";
        let second = &big_chunk[2..];
        let _ = inputs;
        run_test_until_failure(&[first, second], 1);
    }

    // Test when a chunk is too big but it spans multiple reads.
    #[test]
    fn long_chunk_length_line_bypass() {
        let part1 = vec![b'A'; MAX_LINE_BUF_LEN - 1];
        run_test_until_failure(&[&part1, b"BC\n"], 1);
    }

    #[test]
    fn long_extension_in_single_buffer() {
        let mut input = b"5;".to_vec();
        input.extend(std::iter::repeat_n(b'a', MAX_LINE_BUF_LEN + 1));
        input.extend_from_slice(b"\r\nhello\r\n0\r\n\r\n");
        run_test(&[&input], "hello", true, 0);
    }

    // Randomized checks. These are a Rust-only stand-in for the C++ fuzz test
    // and do not need the C++ build.

    struct XorShift64(u64);

    impl XorShift64 {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Builds a random chunked body. Returns the wire bytes and the decoded
    /// body.
    fn random_body(rng: &mut XorShift64) -> (Vec<u8>, Vec<u8>) {
        let mut wire = Vec::new();
        let mut body = Vec::new();
        for _ in 0..rng.below(6) {
            let size = rng.below(40);
            let eol: &[u8] = if rng.below(3) == 0 { b"\n" } else { b"\r\n" };
            wire.extend_from_slice(format!("{size:x}").as_bytes());
            if rng.below(4) == 0 {
                wire.extend_from_slice(b";ext=\"v\"");
            }
            if rng.below(4) == 0 {
                wire.push(b' ');
            }
            wire.extend_from_slice(eol);
            for _ in 0..size {
                let c = b'a' + rng.below(26) as u8;
                wire.push(c);
                body.push(c);
            }
            wire.extend_from_slice(eol);
        }
        wire.extend_from_slice(b"0\r\n");
        if rng.below(2) == 0 {
            wire.extend_from_slice(b"X-Trailer: 1\r\n");
        }
        wire.extend_from_slice(b"\r\n");
        (wire, body)
    }

    // Splitting the input at any boundary must not change the result. This
    // checks the partial-line buffering.
    #[test]
    fn split_invariance_randomized() {
        let mut rng = XorShift64(0x2545_f491_4f6c_dd1d);
        for _ in 0..2_000 {
            let (wire, body) = random_body(&mut rng);

            // Reference: decode the whole input in one call.
            let mut whole = HttpChunkedDecoder::new();
            let mut one = wire.clone();
            let n = whole.filter_buf(&mut one).expect("whole input decodes");
            assert_eq!(&one[..n], body.as_slice());
            assert!(whole.reached_eof());
            assert_eq!(whole.bytes_after_eof(), 0);

            // Same input in random pieces.
            let mut split = HttpChunkedDecoder::new();
            let mut out = Vec::new();
            let mut pos = 0;
            while pos < wire.len() {
                let take = 1 + rng.below(wire.len() - pos);
                let mut piece = wire[pos..pos + take].to_vec();
                let n = split.filter_buf(&mut piece).expect("split input decodes");
                out.extend_from_slice(&piece[..n]);
                pos += take;
            }
            assert_eq!(out, body);
            assert!(split.reached_eof());
            assert_eq!(split.bytes_after_eof(), 0);
        }
    }

    // Random junk must never panic. It may decode or fail, and the outcome
    // must not depend on how the input is split.
    #[test]
    fn random_junk_does_not_panic_and_is_split_invariant() {
        let alphabet = b"0123456789abcdefABCDEFxX;= \t\r\n+-";
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..3_000 {
            let len = rng.below(60);
            let input: Vec<u8> = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect();

            let mut whole = HttpChunkedDecoder::new();
            let mut one = input.clone();
            let whole_result = whole.filter_buf(&mut one);

            let mut split = HttpChunkedDecoder::new();
            let mut out = Vec::new();
            let mut split_result = Ok(());
            let mut pos = 0;
            while pos < input.len() {
                let take = 1 + rng.below(input.len() - pos);
                let mut piece = input[pos..pos + take].to_vec();
                match split.filter_buf(&mut piece) {
                    Ok(n) => out.extend_from_slice(&piece[..n]),
                    Err(e) => {
                        split_result = Err(e);
                        break;
                    }
                }
                pos += take;
            }

            match whole_result {
                Ok(n) => {
                    assert!(split_result.is_ok(), "input {input:?}");
                    assert_eq!(out.as_slice(), &one[..n], "input {input:?}");
                    assert_eq!(whole.reached_eof(), split.reached_eof());
                }
                Err(_) => assert!(split_result.is_err(), "input {input:?}"),
            }
        }
    }
}
