// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::BucketRanges` (base/metrics/bucket_ranges.{h,cc}).
//!
//! `BucketRanges` holds the sorted list of boundaries that decide which bucket
//! a histogram sample goes into. It also holds a CRC-32 checksum of those
//! boundaries, which is used to detect corruption and to compare ranges
//! quickly.
//!
//! Differences from the C++ version:
//! - `Sample32` is `i32`, which is the C++ `HistogramBase::Sample32` type.
//! - `BucketRanges::from_ranges` is the C++ `BucketRanges(span)` constructor.
//!   Invalid input gives an empty `BucketRanges` (size 0, checksum 0), as in
//!   C++. It does not return an error.
//! - The C++ `DCHECK`s in `set_range` are `debug_assert!`s for the value check.
//!   An out-of-range index panics in every build. In C++, that check is a debug
//!   `DCHECK` and the release behavior is undefined.
//! - `bucket_count()` uses `wrapping_sub`, so an empty `BucketRanges` returns
//!   `usize::MAX`, as the C++ `size_t` arithmetic does.
//! - The checksum seeds CRC-32 with the number of ranges. It feeds each range as
//!   4 little-endian bytes. This matches the C++ native-endian layout on every
//!   little-endian target that Chromium builds for. The CRC-32 is reimplemented
//!   here, using the same zlib-compatible algorithm as `base::Crc32`.
//! - `BucketRanges` is not `Clone`, as in C++, where the copy constructor is
//!   deleted. It is `Debug`.
//! - The persistent reference is an `AtomicU32` with `Release` stores and
//!   `Acquire` loads, as in C++. Its setter takes `&self`.
//! - `Equals(const BucketRanges*)` takes `&BucketRanges`.
//!
//! Not ported: nothing from base/metrics/bucket_ranges.{h,cc} is omitted. The
//! `base::Crc32` dependency is reimplemented in this crate as `crc32`.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicU32, Ordering};

/// Same as C++ `HistogramBase::Sample32`.
pub type Sample32 = i32;

/// Table-free bitwise CRC-32 (reflected polynomial `0xEDB88320`) that matches
/// zlib's `crc32_z` for the same input. `crc` is the running value, as in
/// zlib's `crc32(crc, buf, len)`. The initial value is 0.
fn crc32_zlib(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &byte in data {
        c ^= u32::from(byte);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// Port of `base::Crc32(uint32_t sum, span<const uint8_t> data)`.
///
/// `sum` is the seed or the value to continue from. Empty `data` returns `sum`
/// unchanged. The result is the same as the C++ `zlib` call with the same
/// inversions:
/// `~crc32_z(~sum, data)`.
pub fn crc32(sum: u32, data: &[u8]) -> u32 {
    if data.is_empty() {
        return sum;
    }
    !crc32_zlib(!sum, data)
}

/// Stores the boundaries of histogram buckets and their checksum.
///
/// A bucket is defined by a consecutive pair of boundaries, so there is one
/// fewer bucket than there are ranges. For example, the ranges `[0, 1, 3, 7,
/// i32::MAX]` define the buckets `[0, 1)`, `[1, 3)`, `[3, 7)`, and `[7,
/// i32::MAX)`.
#[derive(Debug)]
pub struct BucketRanges {
    /// Non-decreasing list of boundaries. Each entry is the smallest sample
    /// that belongs to the bucket that starts there.
    ranges: Vec<Sample32>,
    /// Checksum of `ranges`, used to detect corruption and to compare ranges.
    checksum: u32,
    /// Index of the persistent-memory record that holds these ranges. Zero when
    /// unset.
    persistent_reference: AtomicU32,
}

impl BucketRanges {
    /// Creates `num_ranges` ranges, all set to 0. The checksum is 0.
    pub fn new(num_ranges: usize) -> BucketRanges {
        BucketRanges {
            ranges: vec![0; num_ranges],
            checksum: 0,
            persistent_reference: AtomicU32::new(0),
        }
    }

    /// Creates ranges from `data`. The checksum is set.
    ///
    /// The data must be non-empty, the first value must be non-negative, and
    /// the values must be strictly increasing. If any of these fails, the result
    /// is an empty `BucketRanges` with a checksum of 0.
    pub fn from_ranges(data: &[Sample32]) -> BucketRanges {
        let mut ranges = BucketRanges {
            ranges: data.to_vec(),
            checksum: 0,
            persistent_reference: AtomicU32::new(0),
        };
        // Because the values must be sorted, it is enough to check that the
        // first one is non-negative.
        if !ranges.ranges.is_empty()
            && ranges.ranges[0] >= 0
            && ranges.ranges.windows(2).all(|pair| pair[0] < pair[1])
        {
            ranges.reset_checksum();
        } else {
            ranges.ranges.clear();
        }
        ranges
    }

    /// Number of ranges, including the final boundary.
    pub fn size(&self) -> usize {
        self.ranges.len()
    }

    /// Returns the boundary at index `i`. Panics if `i >= size()`.
    pub fn range(&self, i: usize) -> Sample32 {
        self.ranges[i]
    }

    /// The whole list of boundaries.
    pub fn ranges(&self) -> &[Sample32] {
        &self.ranges
    }

    /// Sets the boundary at index `i`. Panics if `i >= size()`. In debug builds,
    /// panics if `value` is negative. The checksum is not updated.
    pub fn set_range(&mut self, i: usize, value: Sample32) {
        debug_assert!(value >= 0, "range values must be non-negative");
        self.ranges[i] = value;
    }

    /// The stored checksum. It is not recomputed on `set_range`.
    pub fn checksum(&self) -> u32 {
        self.checksum
    }

    /// Sets the stored checksum without checking it.
    pub fn set_checksum(&mut self, checksum: u32) {
        self.checksum = checksum;
    }

    /// Number of buckets. This is `size() - 1`, wrapping on an empty list.
    pub fn bucket_count(&self) -> usize {
        self.ranges.len().wrapping_sub(1)
    }

    /// Computes the checksum of the current ranges. An empty list gives 0.
    ///
    /// The seed is the number of ranges, and each range is fed as 4
    /// little-endian bytes.
    pub fn calculate_checksum(&self) -> u32 {
        if self.ranges.is_empty() {
            return 0;
        }
        let seed = self.ranges.len() as u32;
        self.ranges.iter().fold(seed, |crc, range| {
            // Each chunk is non-empty, so `crc32` never returns early. Feeding
            // chunks in sequence gives the same result as one call over all the
            // bytes, because `crc32` continues a running value.
            crc32(crc, &range.to_le_bytes())
        })
    }

    /// Returns true if the stored checksum matches the current ranges.
    pub fn has_valid_checksum(&self) -> bool {
        self.calculate_checksum() == self.checksum
    }

    /// Stores the checksum of the current ranges.
    pub fn reset_checksum(&mut self) {
        self.checksum = self.calculate_checksum();
    }

    /// Returns true if `other` has the same checksum and the same ranges.
    pub fn equals(&self, other: &BucketRanges) -> bool {
        self.checksum == other.checksum && self.ranges == other.ranges
    }

    /// Stores the persistent-memory reference for these ranges.
    pub fn set_persistent_reference(&self, reference: u32) {
        self.persistent_reference.store(reference, Ordering::Release);
    }

    /// Returns the persistent-memory reference, or 0 if unset.
    pub fn persistent_reference(&self) -> u32 {
        self.persistent_reference.load(Ordering::Acquire)
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
    }

    #[test]
    fn normal_setup() {
        let mut ranges = BucketRanges::new(5);
        assert_eq!(5, ranges.size());
        assert_eq!(4, ranges.bucket_count());

        for i in 0..5 {
            assert_eq!(0, ranges.range(i));
        }
        assert_eq!(0, ranges.checksum());

        ranges.set_range(3, 100);
        assert_eq!(100, ranges.range(3));
    }

    #[test]
    fn equals() {
        // Compare empty ranges.
        let mut ranges1 = BucketRanges::new(3);
        let mut ranges2 = BucketRanges::new(3);
        let ranges3 = BucketRanges::new(5);

        assert!(ranges1.equals(&ranges2));
        assert!(!ranges1.equals(&ranges3));
        assert!(!ranges2.equals(&ranges3));

        // Compare full filled ranges.
        ranges1.set_range(0, 0);
        ranges1.set_range(1, 1);
        ranges1.set_range(2, 2);
        ranges1.set_checksum(100);
        ranges2.set_range(0, 0);
        ranges2.set_range(1, 1);
        ranges2.set_range(2, 2);
        ranges2.set_checksum(100);

        assert!(ranges1.equals(&ranges2));

        // Checksum does not match.
        ranges1.set_checksum(99);
        assert!(!ranges1.equals(&ranges2));
        ranges1.set_checksum(100);

        // Range does not match.
        ranges1.set_range(1, 3);
        assert!(!ranges1.equals(&ranges2));
    }

    #[test]
    fn checksum() {
        let mut ranges = BucketRanges::new(3);
        ranges.set_range(0, 0);
        ranges.set_range(1, 1);
        ranges.set_range(2, 2);

        ranges.reset_checksum();
        assert_eq!(289217253u32, ranges.checksum());

        ranges.set_range(2, 3);
        assert!(!ranges.has_valid_checksum());

        ranges.reset_checksum();
        assert_eq!(2843835776u32, ranges.checksum());
        assert!(ranges.has_valid_checksum());
    }

    #[test]
    fn from_data() {
        // Known good value #1
        {
            let ranges = BucketRanges::from_ranges(&[0, 1, 2]);
            assert_eq!(ranges.size(), 3);
            assert_eq!(ranges.range(0), 0);
            assert_eq!(ranges.range(1), 1);
            assert_eq!(ranges.range(2), 2);
            assert_eq!(ranges.checksum(), 289217253u32);
        }
        // Known good value #2
        {
            let ranges = BucketRanges::from_ranges(&[0, 1, 3]);
            assert_eq!(ranges.size(), 3);
            assert_eq!(ranges.range(0), 0);
            assert_eq!(ranges.range(1), 1);
            assert_eq!(ranges.range(2), 3);
            assert_eq!(ranges.checksum(), 2843835776u32);
        }

        // Must be non-empty.
        {
            let ranges = BucketRanges::from_ranges(&[]);
            assert_eq!(ranges.size(), 0);
            assert_eq!(ranges.checksum(), 0);
        }

        // Must be non-negative.
        {
            let ranges = BucketRanges::from_ranges(&[-1, 1, 2]);
            assert_eq!(ranges.size(), 0);
            assert_eq!(ranges.checksum(), 0);
        }

        // Must be in sorted order.
        {
            let ranges = BucketRanges::from_ranges(&[0, 2, 1]);
            assert_eq!(ranges.size(), 0);
            assert_eq!(ranges.checksum(), 0);
        }

        // Must not contain duplicate values.
        {
            let ranges = BucketRanges::from_ranges(&[0, 0, 1]);
            assert_eq!(ranges.size(), 0);
            assert_eq!(ranges.checksum(), 0);
        }
    }

    /// Reference for `base::Crc32`, written from the bitwise definition.
    ///
    /// Expanding `~crc32_z(~sum, data)` with zlib's `crc32_z(c, d) = ~R(~c, d)`
    /// gives `R(sum, data)`, where `R` is the raw register update with no
    /// inversion. Empty data returns `sum`.
    fn reference_crc32(seed: u32, data: &[u8]) -> u32 {
        if data.is_empty() {
            return seed;
        }
        let mut r = seed;
        for &b in data {
            r ^= u32::from(b);
            for _ in 0..8 {
                r = if r & 1 != 0 { (r >> 1) ^ 0xedb8_8320 } else { r >> 1 };
            }
        }
        r
    }

    #[test]
    fn crc32_known_answer() {
        // Standard CRC-32 check value for "123456789", as zlib's crc32(0, ...).
        assert_eq!(crc32_zlib(0, b"123456789"), 0xcbf4_3926);
        // Single zero byte, seed 0 (standard CRC-32 value).
        assert_eq!(crc32_zlib(0, &[0]), 0xd202_ef8d);
        // The wrapper with seed 0 is the raw register from zero, with no
        // inversion in or out. It is not the standard value.
        assert_eq!(crc32(0, b"123456789"), reference_crc32(0, b"123456789"));
        // An empty span returns the seed unchanged.
        assert_eq!(crc32(0xdead_beef, &[]), 0xdead_beef);
    }

    #[test]
    fn crc32_continuation_and_reference() {
        let mut rng = XorShift64(0x5eed_1234_abcd_0001);
        for _ in 0..500 {
            let len = (rng.next_u64() % 64) as usize;
            let data: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            let seed = rng.next_u64() as u32;
            assert_eq!(crc32(seed, &data), reference_crc32(seed, &data));

            // Splitting the input and continuing gives the same result.
            if !data.is_empty() {
                let split = (rng.next_u64() as usize) % data.len();
                let first = crc32(seed, &data[..split]);
                assert_eq!(crc32(first, &data[split..]), crc32(seed, &data));
            }
        }
    }

    #[test]
    fn from_ranges_matches_checksum_reference() {
        // The checksum equals the CRC over the seed and the little-endian bytes.
        let mut rng = XorShift64(0x0bad_cafe_f00d_1111);
        for _ in 0..200 {
            let len = 1 + (rng.next_u64() % 8) as usize;
            let mut value: i32 = (rng.next_u64() % 10) as i32;
            let mut data = Vec::with_capacity(len);
            for _ in 0..len {
                data.push(value);
                value += 1 + (rng.next_u64() % 5) as i32;
            }
            let ranges = BucketRanges::from_ranges(&data);
            assert_eq!(ranges.ranges(), data.as_slice());
            let mut bytes = Vec::new();
            for v in &data {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            assert_eq!(ranges.checksum(), reference_crc32(len as u32, &bytes));
            assert!(ranges.has_valid_checksum());
        }
    }

    #[test]
    fn invalid_ranges_are_empty_and_checksum_detects_changes() {
        let mut ranges = BucketRanges::from_ranges(&[0, 1, 2]);
        assert!(ranges.has_valid_checksum());
        ranges.set_range(0, 5);
        assert!(!ranges.has_valid_checksum());
        assert_eq!(ranges.bucket_count(), 2);
    }

    #[test]
    fn empty_bucket_count_wraps_like_cpp() {
        let ranges = BucketRanges::new(0);
        assert_eq!(ranges.size(), 0);
        assert_eq!(ranges.bucket_count(), usize::MAX);
        assert_eq!(ranges.calculate_checksum(), 0);
    }

    #[test]
    fn persistent_reference_round_trip() {
        let ranges = BucketRanges::new(2);
        assert_eq!(ranges.persistent_reference(), 0);
        ranges.set_persistent_reference(42);
        assert_eq!(ranges.persistent_reference(), 42);
    }
}
