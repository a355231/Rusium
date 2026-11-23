// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `net::HttpVersion` (net/http/http_version.h), an HTTP
//! `(major, minor)` version pair.
//!
//! Differences from the C++ version:
//! - The C++ class packs both numbers into one `uint32_t` and derives its
//!   comparison operators from that value. This port stores two `u16` fields
//!   and derives `Ord` from them in field order. That gives the same ordering
//!   as the packed value, and a test checks this.
//! - The C++ constructor computes `major << 16` as an `int`, which is undefined
//!   behavior for majors >= 0x8000. The packed value is computed in `u32` here,
//!   so that case is well-defined.
//! - `Default` gives 0.0, as the C++ default constructor does.
//! - The C++ header has no unit tests. The tests in this crate are new, and
//!   `tests/cpp_golden.rs` checks ordering against the real C++ header.

/// An HTTP `(major, minor)` version. Ordering is lexicographic: major first,
/// then minor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpVersion {
    major: u16,
    minor: u16,
}

impl HttpVersion {
    /// Builds a version from an unsigned major and minor pair.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// The major version number.
    pub const fn major_value(self) -> u16 {
        self.major
    }

    /// The minor version number.
    pub const fn minor_value(self) -> u16 {
        self.minor
    }
}

#[cfg(test)]
mod tests {
    use super::HttpVersion;

    /// The C++ packed representation, `<major>:<minor>` in one `u32`.
    fn packed(v: HttpVersion) -> u32 {
        (u32::from(v.major_value()) << 16) | u32::from(v.minor_value())
    }

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
    }

    #[test]
    fn default_is_zero_zero() {
        let v = HttpVersion::default();
        assert_eq!(v, HttpVersion::new(0, 0));
        assert_eq!(v.major_value(), 0);
        assert_eq!(v.minor_value(), 0);
    }

    #[test]
    fn accessors() {
        let v = HttpVersion::new(1, 1);
        assert_eq!(v.major_value(), 1);
        assert_eq!(v.minor_value(), 1);
        let v = HttpVersion::new(u16::MAX, 0x1234);
        assert_eq!(v.major_value(), u16::MAX);
        assert_eq!(v.minor_value(), 0x1234);
    }

    #[test]
    fn equality() {
        assert_eq!(HttpVersion::new(2, 0), HttpVersion::new(2, 0));
        assert_ne!(HttpVersion::new(2, 0), HttpVersion::new(2, 1));
        assert_ne!(HttpVersion::new(1, 2), HttpVersion::new(2, 1));
    }

    #[test]
    fn ordering_examples() {
        let ordered = [
            HttpVersion::new(0, 0),
            HttpVersion::new(0, 9),
            HttpVersion::new(1, 0),
            HttpVersion::new(1, 1),
            HttpVersion::new(2, 0),
        ];
        for pair in ordered.windows(2) {
            assert!(pair[0] < pair[1]);
            assert!(pair[1] > pair[0]);
            assert!(pair[0] != pair[1]);
        }
    }

    #[test]
    fn major_dominates_minor() {
        assert!(HttpVersion::new(1, u16::MAX) < HttpVersion::new(2, 0));
    }

    #[test]
    fn high_bit_major_orders_as_unsigned() {
        // In C++ this is the case where `major << 16` overflows `int`.
        assert!(HttpVersion::new(0x8000, 0) > HttpVersion::new(0x7fff, u16::MAX));
        assert_eq!(packed(HttpVersion::new(0x8000, 0)), 0x8000_0000);
        assert_eq!(
            packed(HttpVersion::new(u16::MAX, u16::MAX)),
            u32::MAX
        );
    }

    #[test]
    fn copy_preserves_value() {
        let a = HttpVersion::new(3, 7);
        let b = a;
        assert_eq!(a, b);
        assert_eq!(a.major_value(), b.major_value());
        assert_eq!(a.minor_value(), b.minor_value());
    }

    #[test]
    fn ordering_matches_packed_value_randomized() {
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..20_000 {
            // Small values often share a major, so both fields get exercised.
            let mut pick = |r: u64| -> u16 {
                if r & 1 == 0 {
                    (r >> 1) as u16 % 4
                } else {
                    (r >> 1) as u16
                }
            };
            let a = HttpVersion::new(pick(rng.next()), pick(rng.next()));
            let b = HttpVersion::new(pick(rng.next()), pick(rng.next()));
            assert_eq!(a.cmp(&b), packed(a).cmp(&packed(b)), "{a:?} vs {b:?}");
            assert_eq!(a == b, packed(a) == packed(b));
        }
    }

    #[test]
    fn sorting_matches_packed_sort() {
        let mut rng = XorShift64(42);
        let mut versions: Vec<HttpVersion> = (0..500)
            .map(|_| {
                let r = rng.next();
                HttpVersion::new((r >> 48) as u16 % 8, (r >> 16) as u16 % 8)
            })
            .collect();
        let mut by_packed = versions.clone();
        versions.sort();
        by_packed.sort_by_key(|v| packed(*v));
        assert_eq!(versions, by_packed);
    }
}
