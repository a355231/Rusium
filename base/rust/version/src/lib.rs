// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::Version` (base/version.{h,cc}).
//!
//! `Version` holds a dotted version number such as `1.2.3.4`, supports parsing,
//! comparison, wildcard comparison (`1.2.*`), and formatting.
//!
//! Differences from the C++ version:
//! - The C++ constructor sets the object to "invalid" when parsing fails. This
//!   crate keeps that model: `Version::new` returns an invalid `Version`
//!   instead of a `Result`. `is_valid()` reports the outcome.
//! - C++ DCHECKs that the operands are valid (`CompareTo`,
//!   `CompareToWildcardString`, and the comparison operators) and that a
//!   wildcard string is valid. Here `compare_to` and `compare_to_wildcard_string`
//!   return `None` in those cases. The `PartialEq` and `PartialOrd` impls return
//!   `false` and `None` respectively, so an invalid `Version` is never equal to
//!   or ordered against anything, including itself. The C++ release build
//!   returns an unspecified value in these cases, which is not reproduced.
//! - The C++ comparison operators are free functions. Here they are the
//!   `PartialEq` and `PartialOrd` trait impls.
//! - `operator<<` is `Display`, which prints `get_string()`.
//! - Only ASCII decimal digits are accepted in components. Components must fit
//!   in `u32`. Leading zeros are rejected in the first component, as in C++.
//! - The C++ `explicit` move constructors and move assignments are not ported.
//!   They are implicit in Rust.
//!
//! Not ported: nothing from base/version.{h,cc} is omitted.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fmt;

/// Parses a string of ASCII decimal digits into a `u32`.
///
/// Returns `None` for an empty string, any non-digit byte (including a leading
/// `+`, `-`, or whitespace), or a value that does not fit in `u32`.
fn parse_u32(s: &str) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(b - b'0'))?;
    }
    Some(value)
}

/// Parses a dotted list of components such as `1.2.3`.
///
/// Mirrors `ParseVersionNumbers` in the C++ code. The first component must be
/// written without leading zeros (`01.1` is rejected). Later components may
/// have leading zeros, which are ignored (`1.01` is `1.1`).
fn parse_version_numbers(s: &str) -> Option<Vec<u32>> {
    // C++ `SplitStringPiece("")` yields no pieces. Splitting "" here yields one
    // empty piece, which fails `parse_u32`. Both give `None`.
    let mut parsed = Vec::new();
    for (index, part) in s.split('.').enumerate() {
        let num = parse_u32(part)?;
        if index == 0 && num.to_string() != part {
            return None;
        }
        parsed.push(num);
    }
    Some(parsed)
}

/// Compares two component lists. Missing trailing components count as zero.
///
/// Mirrors `CompareVersionComponents` in the C++ code.
fn compare_components(a: &[u32], b: &[u32]) -> Ordering {
    let count = a.len().min(b.len());
    for (x, y) in a[..count].iter().zip(&b[..count]) {
        match x.cmp(y) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    match a.len().cmp(&b.len()) {
        Ordering::Greater if a[count..].iter().any(|&c| c > 0) => Ordering::Greater,
        Ordering::Less if b[count..].iter().any(|&c| c > 0) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

/// Removes a trailing `.*` wildcard, if present.
fn strip_wildcard(wildcard_string: &str) -> &str {
    wildcard_string.strip_suffix(".*").unwrap_or(wildcard_string)
}

/// A dotted version number, such as `1.2.3.4`.
///
/// An invalid `Version` (the default, or the result of a failed parse) has no
/// components. Its `get_string()` is `"invalid"`.
#[derive(Clone, Debug, Default)]
pub struct Version {
    components: Vec<u32>,
}

impl Version {
    /// Parses a decimal dotted version number such as `"0.1.1"`.
    ///
    /// Returns an invalid `Version` if the input does not parse. Use
    /// `is_valid()` to check the outcome.
    pub fn new(version_str: &str) -> Version {
        Version { components: parse_version_numbers(version_str).unwrap_or_default() }
    }

    /// Builds a `Version` from its components, such as `[1, 2, 3, 4]`.
    ///
    /// An empty vector gives an invalid `Version`, as in C++.
    pub fn from_components(components: Vec<u32>) -> Version {
        Version { components }
    }

    /// Returns true if this object holds a valid version number.
    pub fn is_valid(&self) -> bool {
        !self.components.is_empty()
    }

    /// Returns true if `wildcard_string` is a valid version or a valid version
    /// ending in `.*`, such as `1.2.*` or `1.*`.
    ///
    /// Any other use of `*` is invalid, such as `1.*.3` or `1.2.3*`. A string
    /// with no wildcard is checked as a plain version.
    pub fn is_valid_wildcard_string(wildcard_string: &str) -> bool {
        Version::new(strip_wildcard(wildcard_string)).is_valid()
    }

    /// Compares `self` with `other`. Returns `None` if either is invalid.
    pub fn compare_to(&self, other: &Version) -> Option<Ordering> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }
        Some(compare_components(&self.components, &other.components))
    }

    /// Compares `self` with `wildcard_string`, such as `1.2.*`.
    ///
    /// Returns `None` if `self` is invalid or `wildcard_string` is not a valid
    /// wildcard string. Otherwise the result matches `compare_to`, with the
    /// wildcard treated as matching any value in its place:
    /// - `1.2.3` vs `1.2.3.*` is `Equal`.
    /// - `3.2.3` vs `1.*` is `Greater`.
    /// - `1.2.0.0.0.0` vs `1.2.*` is `Equal`.
    pub fn compare_to_wildcard_string(&self, wildcard_string: &str) -> Option<Ordering> {
        if !self.is_valid() || !Version::is_valid_wildcard_string(wildcard_string) {
            return None;
        }

        // No trailing wildcard: compare as a plain version.
        let Some(prefix) = wildcard_string.strip_suffix(".*") else {
            return self.compare_to(&Version::new(wildcard_string));
        };

        let parsed = parse_version_numbers(prefix)?;
        let comparison = compare_components(&self.components, &parsed);
        // If `self` is smaller than or equal to the wildcard's prefix, the
        // wildcard has no effect (see the C++ comment in `CompareToWildcardString`).
        if comparison != Ordering::Greater {
            return Some(comparison);
        }

        // `self` is greater on the compared digits. It is equal if its digits
        // match `parsed` wherever both have a component.
        let min_len = self.components.len().min(parsed.len());
        let prefix_matches =
            self.components[..min_len].iter().zip(&parsed[..min_len]).all(|(a, b)| a == b);
        Some(if prefix_matches { Ordering::Equal } else { Ordering::Greater })
    }

    /// Returns the string form, such as `"1.2.3"`, or `"invalid"`.
    pub fn get_string(&self) -> String {
        if !self.is_valid() {
            return "invalid".to_string();
        }
        self.components.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
    }

    /// The components of this version, such as `[1, 2, 3, 4]`.
    pub fn components(&self) -> &[u32] {
        &self.components
    }
}

impl PartialEq for Version {
    /// Equal when `compare_to` is `Equal`. Two invalid versions are not equal.
    fn eq(&self, other: &Version) -> bool {
        self.compare_to(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        self.compare_to(other)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.get_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple xorshift64 PRNG for randomized tests. Not used by the library.
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

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Maps an `Option<Ordering>` to the -1/0/1 convention of the C++ API.
    fn as_int(o: Option<Ordering>) -> i32 {
        match o {
            Some(Ordering::Less) => -1,
            Some(Ordering::Equal) => 0,
            Some(Ordering::Greater) => 1,
            None => panic!("expected a comparison result, got None"),
        }
    }

    /// Reference comparison: zero-pad the shorter list and compare lexicographically.
    fn reference_compare(a: &[u32], b: &[u32]) -> Ordering {
        let n = a.len().max(b.len());
        for i in 0..n {
            let x = a.get(i).copied().unwrap_or(0);
            let y = b.get(i).copied().unwrap_or(0);
            match x.cmp(&y) {
                Ordering::Equal => {}
                other => return other,
            }
        }
        Ordering::Equal
    }

    /// Reference wildcard comparison, written independently of the code under
    /// test: only positions below `wild.len()` are compared.
    fn reference_wildcard(a: &[u32], wild: &[u32]) -> Ordering {
        for (i, &w) in wild.iter().enumerate() {
            let x = a.get(i).copied().unwrap_or(0);
            match x.cmp(&w) {
                Ordering::Equal => {}
                other => return other,
            }
        }
        Ordering::Equal
    }

    #[test]
    fn default_constructor() {
        let v = Version::default();
        assert!(!v.is_valid());
    }

    #[test]
    fn value_semantics() {
        let v1 = Version::new("1.2.3.4");
        assert!(v1.is_valid());
        let mut v3 = Version::default();
        assert!(!v3.is_valid());
        {
            let v2 = v1.clone();
            v3 = v2.clone();
            assert!(v2.is_valid());
            assert_eq!(v1, v2);
        }
        assert_eq!(v3, v1);
    }

    #[test]
    fn move_semantics() {
        let components = vec![1, 2, 3, 4];
        let v1 = Version::from_components(components);
        assert!(v1.is_valid());
        let v2 = Version::new("1.2.3.4");
        assert_eq!(v1, v2);
    }

    #[test]
    fn get_version_from_string() {
        // (input, number of parts, first part, success)
        let cases: &[(&str, usize, u32, bool)] = &[
            ("", 0, 0, false),
            (" ", 0, 0, false),
            ("\t", 0, 0, false),
            ("\n", 0, 0, false),
            ("  ", 0, 0, false),
            (".", 0, 0, false),
            (" . ", 0, 0, false),
            ("0", 1, 0, true),
            ("0.", 0, 0, false),
            ("0.0", 2, 0, true),
            ("4294967295.0", 2, 4294967295, true),
            ("4294967296.0", 0, 0, false),
            ("-1.0", 0, 0, false),
            ("1.-1.0", 0, 0, false),
            ("1,--1.0", 0, 0, false),
            ("+1.0", 0, 0, false),
            ("1.+1.0", 0, 0, false),
            ("1+1.0", 0, 0, false),
            ("++1.0", 0, 0, false),
            ("1.0a", 0, 0, false),
            ("1.2.3.4.5.6.7.8.9.0", 10, 1, true),
            ("02.1", 0, 0, false),
            ("0.01", 2, 0, true),
            ("f.1", 0, 0, false),
            ("15.007.20011", 3, 15, true),
            ("15.5.28.130162", 4, 15, true),
        ];

        for &(input, parts, firstpart, success) in cases {
            let version = Version::new(input);
            assert_eq!(success, version.is_valid(), "input {input:?}");
            if success {
                assert_eq!(parts, version.components().len(), "input {input:?}");
                assert_eq!(firstpart, version.components()[0], "input {input:?}");
            }
        }
    }

    #[test]
    fn compare() {
        // (lhs, rhs, expected)
        let cases: &[(&str, &str, i32)] = &[
            ("1.0", "1.0", 0),
            ("1.0", "0.0", 1),
            ("1.0", "2.0", -1),
            ("1.0", "1.1", -1),
            ("1.1", "1.0", 1),
            ("1.0", "1.0.1", -1),
            ("1.1", "1.0.1", 1),
            ("1.1", "1.0.1", 1),
            ("1.0.0", "1.0", 0),
            ("1.0.3", "1.0.20", -1),
            ("11.0.10", "15.007.20011", -1),
            ("11.0.10", "15.5.28.130162", -1),
            ("15.5.28.130162", "15.5.28.130162", 0),
        ];
        for &(lhs_str, rhs_str, expected) in cases {
            let lhs = Version::new(lhs_str);
            let rhs = Version::new(rhs_str);
            assert_eq!(as_int(lhs.compare_to(&rhs)), expected, "{lhs_str} ? {rhs_str}");
            // compare_to_wildcard_string() behaves like compare_to() when no
            // wildcard is present.
            assert_eq!(
                as_int(lhs.compare_to_wildcard_string(rhs_str)),
                expected,
                "{lhs_str} ? {rhs_str}"
            );
            assert_eq!(
                as_int(rhs.compare_to_wildcard_string(lhs_str)),
                -expected,
                "{lhs_str} ? {rhs_str}"
            );

            // Comparison operators, as the C++ EXPECT_* checks in this order:
            // ==, !=, <, <=, >, >=.
            let got = [lhs == rhs, lhs != rhs, lhs < rhs, lhs <= rhs, lhs > rhs, lhs >= rhs];
            let want = match expected {
                -1 => [false, true, true, true, false, false],
                0 => [true, false, false, true, false, true],
                1 => [false, true, false, false, true, true],
                _ => unreachable!(),
            };
            assert_eq!(got, want, "{lhs_str} ? {rhs_str}");
        }
    }

    #[test]
    fn compare_to_wildcard_string_cases() {
        // (lhs, wildcard, expected)
        let cases: &[(&str, &str, i32)] = &[
            ("1.0", "1.*", 0),
            ("1.0", "0.*", 1),
            ("1.0", "2.*", -1),
            ("1.2.3", "1.2.3.*", 0),
            ("10.0", "1.0.*", 1),
            ("1.0", "3.0.*", -1),
            ("1.4", "1.3.0.*", 1),
            ("1.3.9", "1.3.*", 0),
            ("1.4.1", "1.3.*", 1),
            ("1.3", "1.4.5.*", -1),
            ("1.5", "1.4.5.*", 1),
            ("1.3.9", "1.3.*", 0),
            ("1.2.0.0.0.0", "1.2.*", 0),
        ];
        for &(lhs, rhs, expected) in cases {
            let version = Version::new(lhs);
            let result = version.compare_to_wildcard_string(rhs);
            assert_eq!(as_int(result), expected, "{lhs}?{rhs}");
        }
    }

    #[test]
    fn is_valid_wildcard_string_cases() {
        // (version, expected)
        let cases: &[(&str, bool)] = &[
            ("1.0", true),
            ("", false),
            ("1.2.3.4.5.6", true),
            ("1.2.3.*", true),
            ("1.2.3.5*", false),
            ("1.2.3.56*", false),
            ("1.*.3", false),
            ("20.*", true),
            ("+2.*", false),
            ("*", false),
            ("*.2", false),
        ];
        for &(version, expected) in cases {
            assert_eq!(
                Version::is_valid_wildcard_string(version),
                expected,
                "{version}?{expected}"
            );
        }
    }

    #[test]
    fn leading_zeros() {
        {
            // Leading zeros in the first component are not allowed.
            let v = Version::new("01.1");
            assert!(!v.is_valid());
        }

        {
            // Leading zeros in later components are allowed and ignored, because
            // the value is parsed as an integer: 1.01 == 1.1.
            let v1 = Version::new("1.01");
            assert!(v1.is_valid());
            assert_eq!("1.1", v1.get_string());
            let v2 = Version::new("1.1");
            assert_eq!(v1, v2);
        }

        // Since leading zeros are ignored, 1.02 > 1.1 (1.02 is read as 1.2).
        assert!(Version::new("1.02") > Version::new("1.1"));
    }

    #[test]
    fn get_string_cases() {
        // (version, valid, string)
        let cases: &[(&str, bool, &str)] = &[
            ("", false, "invalid"),
            ("1", true, "1"),
            ("1.0", true, "1.0"),
            ("0.0.1.0", true, "0.0.1.0"),
            ("1.2.3.4.5.6", true, "1.2.3.4.5.6"),
            ("1.*.3", false, "invalid"),
        ];

        for &(version, valid, string) in cases {
            let v = Version::new(version);
            assert_eq!(v.is_valid(), valid);
            assert_eq!(v.get_string(), string);
        }
    }

    #[test]
    fn known_answer_extra_boundaries() {
        // Largest component, and overflow by one.
        assert_eq!(Version::new("4294967295").components(), &[u32::MAX]);
        assert!(!Version::new("4294967296").is_valid());
        // Leading zeros in later components are not limited in length.
        assert_eq!(Version::new("1.0000000000000000001").components(), &[1, 1]);
        // Display matches get_string, and invalid prints "invalid".
        assert_eq!(Version::new("1.2.3").to_string(), "1.2.3");
        assert_eq!(Version::default().to_string(), "invalid");
        // Operands that are invalid are not comparable.
        let invalid = Version::default();
        let other_invalid = Version::default();
        let valid = Version::new("1.0");
        assert_eq!(invalid.compare_to(&valid), None);
        assert_eq!(valid.compare_to_wildcard_string("1.*.2"), None);
        assert_eq!(valid.compare_to_wildcard_string("*"), None);
        assert!(invalid != other_invalid);
        assert_eq!(invalid.partial_cmp(&other_invalid), None);
        assert_eq!(invalid.compare_to_wildcard_string("1.*"), None);
    }

    #[test]
    fn wildcard_string_trailing_only() {
        // Only a single trailing ".*" is a wildcard.
        assert!(!Version::is_valid_wildcard_string("1.*.*"));
        assert!(!Version::is_valid_wildcard_string(".*"));
        assert!(Version::is_valid_wildcard_string("1.2.3"));
    }

    #[test]
    fn randomized_compare_matches_reference() {
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..5000 {
            let len_a = 1 + rng.below(5) as usize;
            let len_b = 1 + rng.below(5) as usize;
            // Small values make ties and zero padding common.
            let a: Vec<u32> = (0..len_a).map(|_| rng.below(3) as u32).collect();
            let b: Vec<u32> = (0..len_b).map(|_| rng.below(3) as u32).collect();
            let va = Version::from_components(a.clone());
            let vb = Version::from_components(b.clone());
            assert_eq!(va.compare_to(&vb), Some(reference_compare(&a, &b)));
            // Parsing the string form gives back the same components.
            let reparsed = Version::new(&va.get_string());
            assert_eq!(reparsed.components(), a.as_slice());
        }
    }

    #[test]
    fn randomized_wildcard_matches_reference() {
        let mut rng = XorShift64(0x2545_f491_4f6c_dd1d);
        for _ in 0..5000 {
            let len_a = 1 + rng.below(5) as usize;
            let len_w = 1 + rng.below(4) as usize;
            let a: Vec<u32> = (0..len_a).map(|_| rng.below(3) as u32).collect();
            let w: Vec<u32> = (0..len_w).map(|_| rng.below(3) as u32).collect();
            let wildcard =
                format!("{}.*", w.iter().map(u32::to_string).collect::<Vec<_>>().join("."));
            let va = Version::from_components(a.clone());
            assert!(Version::is_valid_wildcard_string(&wildcard));
            assert_eq!(
                va.compare_to_wildcard_string(&wildcard),
                Some(reference_wildcard(&a, &w)),
                "{a:?} vs {wildcard}"
            );
        }
    }
}
