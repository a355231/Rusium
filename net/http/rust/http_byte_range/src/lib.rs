// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::HttpByteRange` (net/http/http_byte_range.{h,cc}).
//!
//! Represents one byte range of an HTTP `Range` request, as defined by RFC 7233
//! section 2.1: `first-last`, `first-` or `-suffix`. It can also resolve those
//! ranges against the size of a resource.
//!
//! Differences from the C++ version:
//! - C++ stores "not specified" as the sentinel `-1`. Here the fields are
//!   `Option<i64>`. The setters keep the C++ convention: passing `-1` means
//!   "not specified", and `Bounded(first, -1)` leaves the last position unset.
//!   The getters return `Option<i64>` instead of returning `-1`.
//! - `get_header_value` returns `None` for an invalid range. The C++ version
//!   only has a `DCHECK(IsValid())` and its output is unspecified in release
//!   builds.
//! - `compute_bounds` uses saturating arithmetic. The C++ computes
//!   `size - min(size, suffix_length)`, which is signed overflow (undefined
//!   behaviour) when `suffix_length` is very negative. The result saturates at
//!   `i64::MAX` here. Such a range is only reachable with a negative suffix
//!   length that still passes `is_valid`.
//! - `HttpByteRange` derives `Copy`, `Eq` and `Default`. The C++ class is a
//!   POD with compiler-generated copy operations.
//! - Nothing is left unported.

#![forbid(unsafe_code)]

/// Sentinel the C++ setters use for "not specified".
const NOT_SPECIFIED: i64 = -1;

fn from_sentinel(value: i64) -> Option<i64> {
    (value != NOT_SPECIFIED).then_some(value)
}

/// A byte range from a `Range` request header.
///
/// A default-constructed range has no fields specified and is not valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HttpByteRange {
    first_byte_position: Option<i64>,
    last_byte_position: Option<i64>,
    suffix_length: Option<i64>,
    has_computed_bounds: bool,
}

impl HttpByteRange {
    /// Range `first..=last`.
    pub fn bounded(first_byte_position: i64, last_byte_position: i64) -> Self {
        let mut range = Self::default();
        range.set_first_byte_position(first_byte_position);
        range.set_last_byte_position(last_byte_position);
        range
    }

    /// Range `first-`, from `first` to the end of the resource.
    pub fn right_unbounded(first_byte_position: i64) -> Self {
        let mut range = Self::default();
        range.set_first_byte_position(first_byte_position);
        range
    }

    /// Suffix range `-suffix_length`, the last `suffix_length` bytes.
    pub fn suffix(suffix_length: i64) -> Self {
        let mut range = Self::default();
        range.set_suffix_length(suffix_length);
        range
    }

    /// The first byte position, or `None` if not specified.
    pub fn first_byte_position(&self) -> Option<i64> {
        self.first_byte_position
    }

    /// Sets the first byte position. `-1` means "not specified".
    pub fn set_first_byte_position(&mut self, value: i64) {
        self.first_byte_position = from_sentinel(value);
    }

    /// The last byte position, or `None` if not specified.
    pub fn last_byte_position(&self) -> Option<i64> {
        self.last_byte_position
    }

    /// Sets the last byte position. `-1` means "not specified".
    pub fn set_last_byte_position(&mut self, value: i64) {
        self.last_byte_position = from_sentinel(value);
    }

    /// The suffix length, or `None` if not specified.
    pub fn suffix_length(&self) -> Option<i64> {
        self.suffix_length
    }

    /// Sets the suffix length. `-1` means "not specified".
    pub fn set_suffix_length(&mut self, value: i64) {
        self.suffix_length = from_sentinel(value);
    }

    /// Returns true if this is a suffix byte range. Like the C++ code, this
    /// is true for any suffix length other than `-1`, including zero and
    /// negative values.
    pub fn is_suffix_byte_range(&self) -> bool {
        self.suffix_length.is_some()
    }

    /// Returns true if the first byte position is specified.
    pub fn has_first_byte_position(&self) -> bool {
        self.first_byte_position.is_some()
    }

    /// Returns true if the last byte position is specified.
    pub fn has_last_byte_position(&self) -> bool {
        self.last_byte_position.is_some()
    }

    /// Returns true if this range is valid.
    pub fn is_valid(&self) -> bool {
        if self.suffix_length.is_some_and(|suffix| suffix > 0) {
            return true;
        }
        match self.first_byte_position {
            Some(first) if first >= 0 => match self.last_byte_position {
                None => true,
                Some(last) => last >= first,
            },
            _ => false,
        }
    }

    /// Returns the header value, for example `bytes=0-100`, `bytes=100-` or
    /// `bytes=-100`. Returns `None` if the range is not valid.
    pub fn get_header_value(&self) -> Option<String> {
        if !self.is_valid() {
            return None;
        }
        if let Some(suffix) = self.suffix_length {
            return Some(format!("bytes=-{suffix}"));
        }
        let first = self.first_byte_position?;
        Some(match self.last_byte_position {
            None => format!("bytes={first}-"),
            Some(last) => format!("bytes={first}-{last}"),
        })
    }

    /// Resolves the range against a resource of `size` bytes, replacing the
    /// first and last byte positions with the concrete bounds.
    ///
    /// Returns `false`, with no change to the range, if `size` is negative, if
    /// the range is not valid for `size`, or if bounds were already computed.
    /// The bounds are computed at most once, so a second call always returns
    /// `false`.
    pub fn compute_bounds(&mut self, size: i64) -> bool {
        if size < 0 {
            return false;
        }
        if self.has_computed_bounds {
            return false;
        }
        self.has_computed_bounds = true;

        // Empty values: the whole resource.
        if !self.has_first_byte_position()
            && !self.has_last_byte_position()
            && !self.is_suffix_byte_range()
        {
            self.first_byte_position = Some(0);
            self.last_byte_position = Some(size - 1);
            return true;
        }
        if !self.is_valid() {
            return false;
        }
        if let Some(suffix) = self.suffix_length {
            // Saturates where the C++ version overflows; see the crate docs.
            self.first_byte_position = Some(size.saturating_sub(size.min(suffix)));
            self.last_byte_position = Some(size - 1);
            return true;
        }
        let Some(first) = self.first_byte_position else {
            return false;
        };
        if first < size {
            self.last_byte_position = Some(match self.last_byte_position {
                Some(last) => last.min(size - 1),
                None => size - 1,
            });
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small deterministic PRNG (xorshift64), so the tests need no crates.
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

        /// Mix of small values, boundary values and full-range values.
        fn next_value(&mut self) -> i64 {
            const SPECIAL: [i64; 12] =
                [-1, -2, 0, 1, 2, 99, 100, 101, i64::MAX, i64::MAX - 1, i64::MIN, i64::MIN + 1];
            match self.next_u64() % 4 {
                0 => SPECIAL[(self.next_u64() % SPECIAL.len() as u64) as usize],
                1 | 2 => (self.next_u64() % 300) as i64 - 5,
                _ => self.next_u64() as i64,
            }
        }
    }

    /// Reference model of `compute_bounds` using `i128`, so it cannot
    /// overflow. Returns the resolved `(first, last)` or `None`.
    fn reference_compute_bounds(
        first: i64,
        last: i64,
        suffix: i64,
        size: i64,
    ) -> Option<(i64, i64)> {
        let (f, l, s, n) = (first as i128, last as i128, suffix as i128, size as i128);
        if n < 0 {
            return None;
        }
        let has_first = first != -1;
        let has_last = last != -1;
        let has_suffix = suffix != -1;
        if !has_first && !has_last && !has_suffix {
            return Some((0, size - 1));
        }
        let valid = s > 0 || (f >= 0 && (!has_last || l >= f));
        if !valid {
            return None;
        }
        if has_suffix {
            let first_out = n - s.min(n);
            let clamped = first_out.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
            return Some((clamped, size - 1));
        }
        if f < n {
            let last_out = if has_last { l.min(n - 1) } else { n - 1 };
            return Some((first, last_out as i64));
        }
        None
    }

    #[test]
    fn valid_ranges() {
        let tests: &[(i64, i64, i64, bool)] = &[
            (-1, -1, 0, false),
            (0, 0, 0, true),
            (-10, 0, 0, false),
            (10, 0, 0, false),
            (10, -1, 0, true),
            (-1, -1, -1, false),
            (-1, 50, 0, false),
            (10, 10000, 0, true),
            (-1, -1, 100000, true),
        ];

        for &(first, last, suffix, valid) in tests {
            let mut range = HttpByteRange::default();
            range.set_first_byte_position(first);
            range.set_last_byte_position(last);
            range.set_suffix_length(suffix);
            assert_eq!(valid, range.is_valid(), "{first} {last} {suffix}");
        }
    }

    #[test]
    fn set_instance_size() {
        // (first, last, suffix, instance_size, expected_return_value,
        //  expected_lower_bound, expected_upper_bound)
        let tests: &[(i64, i64, i64, i64, bool, i64, i64)] = &[
            (-10, 0, -1, 0, false, -1, -1),
            (10, 0, -1, 0, false, -1, -1),
            // Zero instance size is valid, this is the case that user has to handle.
            (-1, -1, -1, 0, true, 0, -1),
            (-1, -1, 500, 0, true, 0, -1),
            (-1, 50, -1, 0, false, -1, -1),
            (-1, -1, 500, 300, true, 0, 299),
            (-1, -1, -1, 100, true, 0, 99),
            (10, -1, -1, 100, true, 10, 99),
            (-1, -1, 500, 1000, true, 500, 999),
            (10, 10000, -1, 1000000, true, 10, 10000),
        ];

        for &(first, last, suffix, size, expected_return, lower, upper) in tests {
            let mut range = HttpByteRange::default();
            range.set_first_byte_position(first);
            range.set_last_byte_position(last);
            range.set_suffix_length(suffix);

            let return_value = range.compute_bounds(size);
            assert_eq!(expected_return, return_value);
            if return_value {
                assert_eq!(Some(lower), range.first_byte_position());
                assert_eq!(Some(upper), range.last_byte_position());

                // A second call returns false and has no side effect.
                assert!(!range.compute_bounds(size));
                assert_eq!(Some(lower), range.first_byte_position());
                assert_eq!(Some(upper), range.last_byte_position());
                // -1 is the "not specified" sentinel, as in the C++ test.
                assert_eq!((suffix != -1).then_some(suffix), range.suffix_length());
            }
        }
    }

    #[test]
    fn get_header_value() {
        let tests: Vec<(HttpByteRange, &str)> = vec![
            (HttpByteRange::bounded(0, 0), "bytes=0-0"),
            (HttpByteRange::bounded(0, 100), "bytes=0-100"),
            (HttpByteRange::bounded(0, -1), "bytes=0-"),
            (HttpByteRange::right_unbounded(100), "bytes=100-"),
            (HttpByteRange::suffix(100), "bytes=-100"),
        ];
        for (range, expected) in tests {
            assert_eq!(Some(expected.to_string()), range.get_header_value());
        }
    }

    #[test]
    fn get_header_value_is_none_when_invalid() {
        assert_eq!(None, HttpByteRange::default().get_header_value());
        assert_eq!(None, HttpByteRange::bounded(10, 5).get_header_value());
        assert_eq!(None, HttpByteRange::bounded(-5, 10).get_header_value());
    }

    #[test]
    fn default_is_unspecified_and_invalid() {
        let range = HttpByteRange::default();
        assert!(!range.has_first_byte_position());
        assert!(!range.has_last_byte_position());
        assert!(!range.is_suffix_byte_range());
        assert!(!range.is_valid());
        assert_eq!(None, range.first_byte_position());
    }

    #[test]
    fn empty_range_is_whole_resource() {
        let mut range = HttpByteRange::default();
        assert!(range.compute_bounds(1000));
        assert_eq!(Some(0), range.first_byte_position());
        assert_eq!(Some(999), range.last_byte_position());
    }

    #[test]
    fn compute_bounds_rejects_negative_size_without_consuming_bounds() {
        let mut range = HttpByteRange::right_unbounded(0);
        assert!(!range.compute_bounds(-1));
        // The negative-size call must not count as the one allowed computation.
        assert!(range.compute_bounds(10));
        assert_eq!(Some(9), range.last_byte_position());
    }

    #[test]
    fn suffix_longer_than_resource_is_clamped_to_start() {
        let mut range = HttpByteRange::suffix(500);
        assert!(range.compute_bounds(300));
        assert_eq!(Some(0), range.first_byte_position());
        assert_eq!(Some(299), range.last_byte_position());
    }

    #[test]
    fn negative_suffix_saturates_instead_of_overflowing() {
        // A negative suffix passes is_valid() when a first position is set.
        let mut range = HttpByteRange::default();
        range.set_first_byte_position(10);
        range.set_suffix_length(i64::MIN);
        assert!(range.is_valid());
        assert!(range.compute_bounds(5));
        assert_eq!(Some(i64::MAX), range.first_byte_position());
        assert_eq!(Some(4), range.last_byte_position());
    }

    #[test]
    fn differential_compute_bounds_against_reference() {
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..20_000 {
            let first = rng.next_value();
            let last = rng.next_value();
            let suffix = rng.next_value();
            // Mostly valid sizes, sometimes negative ones.
            let size = if rng.next_u64().is_multiple_of(8) {
                -2
            } else {
                rng.next_value().rem_euclid(1000)
            };

            let mut range = HttpByteRange::default();
            range.set_first_byte_position(first);
            range.set_last_byte_position(last);
            range.set_suffix_length(suffix);
            let expected = reference_compute_bounds(first, last, suffix, size);
            let got = range.compute_bounds(size);
            match expected {
                Some((f, l)) => {
                    assert!(got, "first={first} last={last} suffix={suffix} size={size}");
                    assert_eq!(Some(f), range.first_byte_position());
                    assert_eq!(Some(l), range.last_byte_position());
                }
                None => assert!(!got, "first={first} last={last} suffix={suffix} size={size}"),
            }
        }
    }

    #[test]
    fn differential_valid_against_reference() {
        let mut rng = XorShift64(0x0123_4567_89ab_cdef);
        for _ in 0..20_000 {
            let first = rng.next_value();
            let last = rng.next_value();
            let suffix = rng.next_value();
            let mut range = HttpByteRange::default();
            range.set_first_byte_position(first);
            range.set_last_byte_position(last);
            range.set_suffix_length(suffix);
            let expected = suffix > 0 || (first >= 0 && (last == -1 || last >= first));
            assert_eq!(expected, range.is_valid(), "{first} {last} {suffix}");
        }
    }
}
