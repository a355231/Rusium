// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Ports of base/strings/strcat_unittest.cc, plus randomized checks.

use crate::*;

#[test]
fn str_cat_8bit() {
    assert_eq!("", str_cat(&[""]));
    assert_eq!("1", str_cat(&["1"]));
    assert_eq!("122", str_cat(&["1", "22"]));
    assert_eq!("122333", str_cat(&["1", "22", "333"]));
    assert_eq!("1223334444", str_cat(&["1", "22", "333", "4444"]));
    assert_eq!("122333444455555", str_cat(&["1", "22", "333", "4444", "55555"]));
}

#[test]
fn str_cat_16bit() {
    let arg1: Vec<u16> = "1".encode_utf16().collect();
    let arg2: Vec<u16> = "22".encode_utf16().collect();
    let arg3: Vec<u16> = "333".encode_utf16().collect();
    let u = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };

    assert_eq!(u(""), str_cat16(&[Vec::<u16>::new()]));
    assert_eq!(u("1"), str_cat16(std::slice::from_ref(&arg1)));
    assert_eq!(u("122"), str_cat16(&[arg1.clone(), arg2.clone()]));
    assert_eq!(u("122333"), str_cat16(&[arg1, arg2, arg3]));
}

#[test]
fn str_append_8bit() {
    let mut result: String;

    result = "foo".to_string();
    str_append(&mut result, &[String::new()]);
    assert_eq!("foo", result);

    result = "foo".to_string();
    str_append(&mut result, &["1"]);
    assert_eq!("foo1", result);

    result = "foo".to_string();
    str_append(&mut result, &["1", "22", "333"]);
    assert_eq!("foo122333", result);
}

#[test]
fn str_append_16bit() {
    let arg1: Vec<u16> = "1".encode_utf16().collect();
    let arg2: Vec<u16> = "22".encode_utf16().collect();
    let arg3: Vec<u16> = "333".encode_utf16().collect();
    let u = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };

    let mut result: Vec<u16>;

    result = u("foo");
    str_append16(&mut result, &[Vec::<u16>::new()]);
    assert_eq!(u("foo"), result);

    result = u("foo");
    str_append16(&mut result, std::slice::from_ref(&arg1));
    assert_eq!(u("foo1"), result);

    result = u("foo");
    str_append16(&mut result, &[arg1, arg2, arg3]);
    assert_eq!(u("foo122333"), result);
}

// Mirrors StrAppendT.ReserveAdditionalIfNeeded: appending exactly the
// remaining capacity must not reallocate, and one more character must grow
// the buffer by at least 2x.
#[test]
fn reserve_additional_if_needed() {
    let mut s = String::with_capacity(16);
    s.push_str("foo");
    let prev_data = s.as_ptr();
    let prev_capacity = s.capacity();
    // Fully exhaust the current capacity.
    let fill = "o".repeat(s.capacity() - s.len());
    str_append(&mut s, &[fill]);
    // We hit capacity, but did not reallocate.
    assert_eq!(s.capacity(), s.len());
    assert_eq!(prev_data, s.as_ptr());
    assert_eq!(prev_capacity, s.capacity());

    // Force a reallocation by appending one more character.
    str_append(&mut s, &["o"]);
    assert!(2 * prev_capacity <= s.capacity());
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

#[test]
fn random_matches_naive_concatenation() {
    let alphabet = ["a", "é", "日", "", " ", "\u{1F600}"];
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    for _ in 0..2000 {
        let n = (xorshift(&mut rng) % 6) as usize;
        let pieces: Vec<&str> = (0..n)
            .map(|_| alphabet[(xorshift(&mut rng) % alphabet.len() as u64) as usize])
            .collect();
        let expected: String = pieces.concat();

        assert_eq!(str_cat(&pieces), expected);
        let expected16: Vec<u16> = expected.encode_utf16().collect();
        assert_eq!(
            str_cat16(
                &pieces.iter().map(|p| p.encode_utf16().collect::<Vec<u16>>()).collect::<Vec<_>>()
            ),
            expected16
        );

        let prefix = "prefix:";
        let mut dest = prefix.to_string();
        str_append(&mut dest, &pieces);
        assert_eq!(dest, format!("{prefix}{expected}"));
    }
}
