// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::LevenshteinDistance`
//! (base/strings/levenshtein_distance.{h,cc}).
//!
//! Computes the Levenshtein distance (insertions, deletions and substitutions
//! each cost 1) between two sequences. An optional bound `k` lets the
//! computation stop early: if the true distance is larger than `k`, the result
//! is `k + 1`. The bounded version runs in `O(min(|a|, |b|) * k + k)` time and
//! `O(k)` memory.
//!
//! The C++ `std::string_view` and `std::u16string_view` overloads are the same
//! generic function here. Pass `s.as_bytes()` for C++ `std::string` semantics,
//! which compare bytes, not `char`s. Pass a `&[u16]` for UTF-16.
//!
//! Differences from the C++ version:
//! - The bound `k` is clamped to the length of the longer input. A distance
//!   can never exceed that length, so the result is the same. This avoids
//!   overflow of `2 * k + 1` when `k` is `usize::MAX`, which is undefined
//!   behavior in C++.
//! - The arguments are slices and the result is `usize`, matching C++
//!   `size_t`. There is no `std::optional`; `None` means no bound.

#![forbid(unsafe_code)]

/// Returns the Levenshtein distance of `a` and `b`.
///
/// If `max_distance` is `Some(k)`, the result is exact when the distance is at
/// most `k`. Otherwise it is `k + 1`.
///
/// # Examples
///
/// ```
/// use levenshtein_distance::levenshtein_distance;
/// assert_eq!(levenshtein_distance(b"chrome", b"chromium", None), 3);
/// assert_eq!(levenshtein_distance(b"street", b"str.", Some(1)), 2);
/// ```
pub fn levenshtein_distance<T: PartialEq>(a: &[T], b: &[T], max_distance: Option<usize>) -> usize {
    // Make `a` the shorter input. The distance is symmetric.
    let (a, b) = if a.len() > b.len() { (b, a) } else { (a, b) };

    // One of max(|a|, |b|) steps always suffices, so a larger bound is the same
    // as a bound of |b|. Clamping also keeps `2 * k + 1` from overflowing.
    let k = max_distance.unwrap_or(b.len()).min(b.len());
    // If the lengths differ by more than `k`, so does the distance.
    if a.len() + k < b.len() {
        return k + 1;
    }

    // Let dp[d] be the distance between a[..i] and b[..i + d - k] for the
    // current row `i`. Only |d - k| <= k can be within the bound, so the
    // table has 2 * k + 1 entries. Values that are certainly larger than `k`
    // are stored as `infinity`. Their exact value does not affect the result.
    let infinity = k + 1;
    let mut dp = vec![infinity; 2 * k + 1];
    // Row 0: the empty prefix of `a` against b[..j] costs j removals. Only
    // j >= 0 is reachable, i.e. d >= k.
    for (j, slot) in dp[k..].iter_mut().enumerate() {
        *slot = j;
    }
    for i in 0..a.len() {
        for d in 0..=2 * k {
            if i + d < k || i + d >= b.len() + k {
                // j = i + d - k is outside b. For j = -1 (the empty prefix of
                // b), the distance is i + 1.
                dp[d] = if i + d + 1 == k { i + 1 } else { infinity };
                continue;
            }
            let j = i + d - k;
            if a[i] != b[j] {
                // Substitution keeps d.
                let replace = dp[d];
                // Deletion moves to d + 1.
                let remove = if d != 2 * k { dp[d + 1] } else { infinity };
                // Insertion moves to d - 1. This uses the value already
                // updated in this row, as in the C++ code.
                let insert = if d != 0 { dp[d - 1] } else { infinity };
                dp[d] = 1 + replace.min(remove).min(insert);
            }
        }
    }
    dp[b.len() + k - a.len()].min(k + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    // Ports of WithoutMaxDistance in base/strings/levenshtein_distance_unittest.cc.
    #[test]
    fn without_max_distance() {
        assert_eq!(0, levenshtein_distance(b"banana", b"banana", None));

        assert_eq!(2, levenshtein_distance(b"ab", b"ba", None));
        assert_eq!(2, levenshtein_distance(b"ba", b"ab", None));

        assert_eq!(2, levenshtein_distance(b"ananas", b"banana", None));
        assert_eq!(2, levenshtein_distance(b"banana", b"ananas", None));

        assert_eq!(2, levenshtein_distance(b"unclear", b"nuclear", None));
        assert_eq!(2, levenshtein_distance(b"nuclear", b"unclear", None));

        assert_eq!(3, levenshtein_distance(b"chrome", b"chromium", None));
        assert_eq!(3, levenshtein_distance(b"chromium", b"chrome", None));

        assert_eq!(4, levenshtein_distance(b"", b"abcd", None));
        assert_eq!(4, levenshtein_distance(b"abcd", b"", None));

        // `std::u16string_view` version.
        assert_eq!(
            4,
            levenshtein_distance(&u16s("xxx"), &u16s("xxxxxxx"), None)
        );
        assert_eq!(
            4,
            levenshtein_distance(&u16s("xxxxxxx"), &u16s("xxx"), None)
        );

        assert_eq!(
            7,
            levenshtein_distance(&u16s("yyy"), &u16s("xxxxxxx"), None)
        );
        assert_eq!(
            7,
            levenshtein_distance(&u16s("xxxxxxx"), &u16s("yyy"), None)
        );
    }

    // Ports of WithMaxDistance in base/strings/levenshtein_distance_unittest.cc.
    #[test]
    fn with_max_distance() {
        assert_eq!(levenshtein_distance(b"aa", b"aa", Some(0)), 0);

        assert_eq!(levenshtein_distance(b"a", b"aa", Some(1)), 1);
        assert_eq!(levenshtein_distance(b"aa", b"a", Some(1)), 1);

        // If k is less than `LevenshteinDistance()`, the function should return k+1.
        assert_eq!(levenshtein_distance(b"", b"12", Some(1)), 2);
        assert_eq!(levenshtein_distance(b"12", b"", Some(1)), 2);

        assert_eq!(levenshtein_distance(b"street", b"str.", Some(1)), 2);
        assert_eq!(levenshtein_distance(b"str.", b"street", Some(1)), 2);

        assert_eq!(levenshtein_distance(b"asdf", b"fdsa", Some(2)), 3);
        assert_eq!(levenshtein_distance(b"fdsa", b"asdf", Some(2)), 3);

        assert_eq!(
            levenshtein_distance(&u16s(&"a".repeat(100)), &u16s(&"a".repeat(200)), Some(50)),
            51
        );
        assert_eq!(
            levenshtein_distance(&u16s(&"a".repeat(200)), &u16s(&"a".repeat(100)), Some(50)),
            51
        );
    }

    /// Straightforward O(|a| * |b|) DP, used as the reference.
    fn naive_distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for i in 1..=a.len() {
            let mut cur = vec![i; b.len() + 1];
            for j in 1..=b.len() {
                let subst = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
                cur[j] = subst.min(prev[j] + 1).min(cur[j - 1] + 1);
            }
            prev = cur;
        }
        prev[b.len()]
    }

    #[test]
    fn matches_naive_reference() {
        let mut state: u64 = 0x1234_5678_9abc_def1;
        let mut next_rand = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..5000 {
            let a: Vec<u8> = (0..next_rand() % 12)
                .map(|_| b"abc"[(next_rand() % 3) as usize])
                .collect();
            let b: Vec<u8> = (0..next_rand() % 12)
                .map(|_| b"abc"[(next_rand() % 3) as usize])
                .collect();
            let exact = naive_distance(&a, &b);
            assert_eq!(levenshtein_distance(&a, &b, None), exact);
            for k in 0..14usize {
                let expected = if exact <= k { exact } else { k + 1 };
                assert_eq!(
                    levenshtein_distance(&a, &b, Some(k)),
                    expected,
                    "a={a:?} b={b:?} k={k}"
                );
            }
        }
    }

    #[test]
    fn symmetric() {
        let a = b"the quick brown fox";
        let b = b"a quick brown dog!";
        for k in [None, Some(0), Some(3), Some(20)] {
            assert_eq!(
                levenshtein_distance(a, b, k),
                levenshtein_distance(b, a, k)
            );
        }
    }

    #[test]
    fn huge_bound_is_exact_and_does_not_overflow() {
        assert_eq!(levenshtein_distance(b"kitten", b"sitting", Some(usize::MAX)), 3);
        assert_eq!(levenshtein_distance(b"", b"", Some(usize::MAX)), 0);
    }

    #[test]
    fn empty_inputs() {
        assert_eq!(levenshtein_distance::<u8>(b"", b"", None), 0);
        assert_eq!(levenshtein_distance::<u8>(b"", b"", Some(0)), 0);
        assert_eq!(levenshtein_distance(b"", b"x", Some(0)), 1);
    }

    #[test]
    fn works_on_non_byte_units() {
        // Generic over any PartialEq element, e.g. chars for Unicode scalars.
        let a: Vec<char> = "h\u{e9}llo".chars().collect();
        let b: Vec<char> = "hello".chars().collect();
        assert_eq!(levenshtein_distance(&a, &b, None), 1);
    }
}
