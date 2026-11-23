// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Ports of the JoinString tests in base/strings/string_util_unittest.cc (the
// C++ tree has no join_string_unittest.cc; these are the tests for
// JoinString). Each C++ TEST below keeps the same inputs and expectations.

use crate::*;

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn join_string_strings() {
    let separator = ", ";
    let mut parts: Vec<String> = Vec::new();
    assert_eq!("", join_string(&parts, separator));

    parts.push(String::new());
    assert_eq!("", join_string(&parts, separator));
    parts.clear();

    parts.push("a".to_string());
    assert_eq!("a", join_string(&parts, separator));

    parts.push("b".to_string());
    parts.push("c".to_string());
    assert_eq!("a, b, c", join_string(&parts, separator));

    parts.push(String::new());
    assert_eq!("a, b, c, ", join_string(&parts, separator));
    parts.push(" ".to_string());
    assert_eq!("a|b|c|| ", join_string(&parts, "|"));
}

#[test]
fn join_string16_strings() {
    let separator = u(", ");
    let mut parts: Vec<Vec<u16>> = Vec::new();
    assert_eq!(Vec::<u16>::new(), join_string16(&parts, &separator));

    parts.push(Vec::new());
    assert_eq!(Vec::<u16>::new(), join_string16(&parts, &separator));
    parts.clear();

    parts.push(u("a"));
    assert_eq!(u("a"), join_string16(&parts, &separator));

    parts.push(u("b"));
    parts.push(u("c"));
    assert_eq!(u("a, b, c"), join_string16(&parts, &separator));

    parts.push(Vec::new());
    assert_eq!(u("a, b, c, "), join_string16(&parts, &separator));
    parts.push(u(" "));
    assert_eq!(u("a|b|c|| "), join_string16(&parts, &u("|")));
}

#[test]
fn join_string_piece() {
    let separator = ", ";
    let mut parts: Vec<&str> = Vec::new();
    assert_eq!("", join_string(&parts, separator));

    // Test empty first part (https://crbug.com/698073).
    parts.push("");
    assert_eq!("", join_string(&parts, separator));
    parts.clear();

    parts.push("a");
    assert_eq!("a", join_string(&parts, separator));

    parts.push("b");
    parts.push("c");
    assert_eq!("a, b, c", join_string(&parts, separator));

    parts.push("");
    assert_eq!("a, b, c, ", join_string(&parts, separator));
    parts.push(" ");
    assert_eq!("a|b|c|| ", join_string(&parts, "|"));
}

#[test]
fn join_string_piece16() {
    let separator = u(", ");
    let mut parts: Vec<&[u16]> = Vec::new();
    assert_eq!(Vec::<u16>::new(), join_string16(&parts, &separator));

    // Test empty first part (https://crbug.com/698073).
    parts.push(&[]);
    assert_eq!(Vec::<u16>::new(), join_string16(&parts, &separator));
    parts.clear();

    let k_a = u("a");
    parts.push(&k_a);
    assert_eq!(u("a"), join_string16(&parts, &separator));

    let k_b = u("b");
    parts.push(&k_b);
    let k_c = u("c");
    parts.push(&k_c);
    assert_eq!(u("a, b, c"), join_string16(&parts, &separator));

    parts.push(&[]);
    assert_eq!(u("a, b, c, "), join_string16(&parts, &separator));
    let k_space = u(" ");
    parts.push(&k_space);
    assert_eq!(u("a|b|c|| "), join_string16(&parts, &u("|")));
}

#[test]
fn join_string_initializer_list() {
    let separator = ", ";
    assert_eq!("", join_string::<&str>(&[], separator));

    // Test empty first part (https://crbug.com/698073).
    assert_eq!("", join_string(&[""], separator));

    // With const char*s.
    assert_eq!("a", join_string(&["a"], separator));
    assert_eq!("a, b, c", join_string(&["a", "b", "c"], separator));
    assert_eq!("a, b, c, ", join_string(&["a", "b", "c", ""], separator));
    assert_eq!("a|b|c|| ", join_string(&["a", "b", "c", "", " "], "|"));

    // With std::strings.
    let k_a = String::from("a");
    let k_b = String::from("b");
    assert_eq!("a, b", join_string(&[k_a.clone(), k_b.clone()], separator));

    // With string slices.
    let piece_a: &str = &k_a;
    let piece_b: &str = &k_b;
    assert_eq!("a, b", join_string(&[piece_a, piece_b], separator));
}

#[test]
fn join_string_initializer_list16() {
    let separator = u(", ");
    let empty16: &[u16] = &[];
    assert_eq!(Vec::<u16>::new(), join_string16::<&[u16]>(&[], &separator));

    // Test empty first part (https://crbug.com/698073).
    assert_eq!(Vec::<u16>::new(), join_string16(&[empty16], &separator));

    // With string16s.
    let k_a = u("a");
    assert_eq!(u("a"), join_string16(&[k_a.as_slice()], &separator));

    let k_b = u("b");
    let k_c = u("c");
    assert_eq!(
        u("a, b, c"),
        join_string16(&[k_a.as_slice(), k_b.as_slice(), k_c.as_slice()], &separator)
    );

    assert_eq!(
        u("a, b, c, "),
        join_string16(&[k_a.as_slice(), k_b.as_slice(), k_c.as_slice(), empty16], &separator)
    );
    let k_space = u(" ");
    assert_eq!(
        u("a|b|c|| "),
        join_string16(
            &[k_a.as_slice(), k_b.as_slice(), k_c.as_slice(), empty16, k_space.as_slice()],
            &u("|")
        )
    );

    // With string-view slices (the same as the owned ones here).
    let piece_a: &[u16] = &k_a;
    let piece_b: &[u16] = &k_b;
    assert_eq!(u("a, b"), join_string16(&[piece_a, piece_b], &separator));
}

// Extra tests.

#[test]
fn join_string_by_applies_transform_in_order() {
    let nums = [1, 22, 333];
    let joined = join_string_by(&nums, "|", |n| n.to_string());
    assert_eq!("1|22|333", joined);

    let mut calls = 0;
    let joined = join_string_by(&["a", "b", "c"], ", ", |s| {
        calls += 1;
        s.to_uppercase()
    });
    assert_eq!("A, B, C", joined);
    assert_eq!(3, calls);

    let empty: [&str; 0] = [];
    assert_eq!("", join_string_by(&empty, ", ", |s| *s));
}

#[test]
fn join_string16_by_applies_transform_in_order() {
    let words = ["a", "bc"];
    let joined = join_string16_by(&words, &u("|"), |w| u(&w.to_uppercase()));
    assert_eq!(u("A|BC"), joined);

    let empty: [&str; 0] = [];
    assert_eq!(Vec::<u16>::new(), join_string16_by(&empty, &u("|"), |w| u(w)));
}

struct XorShift(u64);

impl XorShift {
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
fn random_matches_std_join() {
    let pool = ["", "a", "bc", "é", "日本", " ", "\u{1F600}", ","];
    let seps = [", ", "|", "", " ", "é"];
    let mut rng = XorShift(0xabcd_ef01_2345_6789);
    for _ in 0..2000 {
        let n = (rng.next() % 6) as usize;
        let parts: Vec<&str> =
            (0..n).map(|_| pool[(rng.next() % pool.len() as u64) as usize]).collect();
        let sep = seps[(rng.next() % seps.len() as u64) as usize];
        let expected = parts.join(sep);
        assert_eq!(join_string(&parts, sep), expected);

        let parts16: Vec<Vec<u16>> = parts.iter().map(|p| u(p)).collect();
        assert_eq!(join_string16(&parts16, &u(sep)), u(&expected));
    }
}
