// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ported from `base/strings/string_split_unittest.cc`, plus extra tests.
//!
//! Each `TYPED_TEST` from the C++ file runs twice here: once with owned output
//! (the `_owned` test) and once with borrowed output (the `_view` test).

use super::*;
use SplitResult::{SplitWantAll as All, SplitWantNonEmpty as NonEmpty};
use WhitespaceHandling::{KeepWhitespace as Keep, TrimWhitespace as Trim};

type Pairs = Vec<(String, String)>;
type Kv = (bool, Pairs);
type CharKvFn = fn(&str, char, char) -> Kv;
type SubstrKvFn = fn(&str, char, &str) -> Kv;

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn u(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn utf16_list(list: &[&str]) -> Vec<Vec<u16>> {
    list.iter().map(|s| u(s)).collect()
}

fn pairs(list: &[(&str, &str)]) -> Pairs {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn owned_char(input: &str, kv: char, pair: char) -> Kv {
    let r = split_string_into_key_value_pairs(input, kv, pair);
    (r.complete, r.pairs)
}

fn view_char(input: &str, kv: char, pair: char) -> Kv {
    let r = split_string_into_key_value_view_pairs(input, kv, pair);
    (r.complete, r.pairs.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

fn owned_substr(input: &str, kv: char, pair: &str) -> Kv {
    let r = split_string_into_key_value_pairs_using_substr(input, kv, pair);
    (r.complete, r.pairs)
}

fn view_substr(input: &str, kv: char, pair: &str) -> Kv {
    let r = split_string_into_key_value_view_pairs_using_substr(input, kv, pair);
    (r.complete, r.pairs.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

/// Generates one `#[test]` per output flavor, both calling `$body` with a
/// different parser.
macro_rules! both_variants {
    ($body:ident, $owned:ident, $view:ident, $owned_fn:expr, $view_fn:expr) => {
        #[test]
        fn $owned() {
            $body($owned_fn);
        }
        #[test]
        fn $view() {
            $body($view_fn);
        }
    };
}

// ---------------------------------------------------------------------------
// SplitStringIntoKeyValuePairsTest (char pair delimiter). TYPED_TEST x2.
// ---------------------------------------------------------------------------

fn check_char_empty_string(f: CharKvFn) {
    assert_eq!(f("", ':', ','), (true, pairs(&[])));
}

fn check_char_missing_key_value_delimiter(f: CharKvFn) {
    assert_eq!(f("key1,key2:value2", ':', ','), (false, pairs(&[("", ""), ("key2", "value2")])));
}

fn check_char_empty_key_with_key_value_delimiter(f: CharKvFn) {
    assert_eq!(
        f(":value1,key2:value2", ':', ','),
        (true, pairs(&[("", "value1"), ("key2", "value2")]))
    );
}

fn check_char_trailing_and_leading_pair_delimiter(f: CharKvFn) {
    assert_eq!(
        f(",key1:value1,key2:value2,", ':', ','),
        (true, pairs(&[("key1", "value1"), ("key2", "value2")]))
    );
}

fn check_char_empty_pair(f: CharKvFn) {
    assert_eq!(
        f("key1:value1,,key3:value3", ':', ','),
        (true, pairs(&[("key1", "value1"), ("key3", "value3")]))
    );
}

fn check_char_empty_value(f: CharKvFn) {
    assert_eq!(
        f("key1:,key2:value2", ':', ','),
        (false, pairs(&[("key1", ""), ("key2", "value2")]))
    );
}

fn check_char_untrimmed_whitespace(f: CharKvFn) {
    assert_eq!(f("key1 : value1", ':', ','), (true, pairs(&[("key1 ", " value1")])));
}

fn check_char_trimmed_whitespace(f: CharKvFn) {
    assert_eq!(
        f("key1:value1 , key2:value2", ':', ','),
        (true, pairs(&[("key1", "value1"), ("key2", "value2")]))
    );
}

fn check_char_multiple_key_value_delimiters(f: CharKvFn) {
    assert_eq!(
        f("key1:::value1,key2:value2", ':', ','),
        (true, pairs(&[("key1", "value1"), ("key2", "value2")]))
    );
}

fn check_char_only_split_at_given_separator(f: CharKvFn) {
    let a = "a ?!@#$%^&*()_+:/{}\\\t\nb";
    let b = format!("{a}X{a}Y{a}X{a}");
    assert_eq!(f(&b, 'X', 'Y'), (true, pairs(&[(a, a), (a, a)])));
}

fn check_char_delimiter_in_value(f: CharKvFn) {
    assert_eq!(
        f("key1:va:ue1,key2:value2", ':', ','),
        (true, pairs(&[("key1", "va:ue1"), ("key2", "value2")]))
    );
}

both_variants!(
    check_char_empty_string,
    char_empty_string_owned,
    char_empty_string_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_missing_key_value_delimiter,
    char_missing_key_value_delimiter_owned,
    char_missing_key_value_delimiter_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_empty_key_with_key_value_delimiter,
    char_empty_key_with_key_value_delimiter_owned,
    char_empty_key_with_key_value_delimiter_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_trailing_and_leading_pair_delimiter,
    char_trailing_and_leading_pair_delimiter_owned,
    char_trailing_and_leading_pair_delimiter_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_empty_pair,
    char_empty_pair_owned,
    char_empty_pair_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_empty_value,
    char_empty_value_owned,
    char_empty_value_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_untrimmed_whitespace,
    char_untrimmed_whitespace_owned,
    char_untrimmed_whitespace_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_trimmed_whitespace,
    char_trimmed_whitespace_owned,
    char_trimmed_whitespace_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_multiple_key_value_delimiters,
    char_multiple_key_value_delimiters_owned,
    char_multiple_key_value_delimiters_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_only_split_at_given_separator,
    char_only_split_at_given_separator_owned,
    char_only_split_at_given_separator_view,
    owned_char,
    view_char
);
both_variants!(
    check_char_delimiter_in_value,
    char_delimiter_in_value_owned,
    char_delimiter_in_value_view,
    owned_char,
    view_char
);

// ---------------------------------------------------------------------------
// SplitStringIntoKeyValuePairsUsingSubstrTest (substring pair delimiter).
// TYPED_TEST x2.
// ---------------------------------------------------------------------------

fn check_substr_empty_string(f: SubstrKvFn) {
    assert_eq!(f("", ':', ","), (true, pairs(&[])));
}

fn check_substr_missing_key_value_delimiter(f: SubstrKvFn) {
    assert_eq!(f("key1,,key2:value2", ':', ",,"), (false, pairs(&[("", ""), ("key2", "value2")])));
}

fn check_substr_missing_key_value_pair_delimiter(f: SubstrKvFn) {
    assert_eq!(
        f("key1:value1,,key3:value3", ':', ",,,"),
        (true, pairs(&[("key1", "value1,,key3:value3")]))
    );
}

fn check_substr_untrimmed_whitespace(f: SubstrKvFn) {
    assert_eq!(f("key1 : value1", ':', ","), (true, pairs(&[("key1 ", " value1")])));
}

fn check_substr_only_split_at_given_separator(f: SubstrKvFn) {
    let a = "a ?!@#$%^&*()_+:/{}\\\t\nb";
    let b = format!("{a}X{a}XY{a}YX{a}");
    let a_y = format!("{a}Y");
    assert_eq!(f(&b, 'X', "XY"), (true, pairs(&[(a, a), (a_y.as_str(), a)])));
}

both_variants!(
    check_substr_empty_string,
    substr_empty_string_owned,
    substr_empty_string_view,
    owned_substr,
    view_substr
);
both_variants!(
    check_substr_missing_key_value_delimiter,
    substr_missing_key_value_delimiter_owned,
    substr_missing_key_value_delimiter_view,
    owned_substr,
    view_substr
);
both_variants!(
    check_substr_missing_key_value_pair_delimiter,
    substr_missing_key_value_pair_delimiter_owned,
    substr_missing_key_value_pair_delimiter_view,
    owned_substr,
    view_substr
);
both_variants!(
    check_substr_untrimmed_whitespace,
    substr_untrimmed_whitespace_owned,
    substr_untrimmed_whitespace_view,
    owned_substr,
    view_substr
);
both_variants!(
    check_substr_only_split_at_given_separator,
    substr_only_split_at_given_separator_owned,
    substr_only_split_at_given_separator_view,
    owned_substr,
    view_substr
);

// ---------------------------------------------------------------------------
// SplitStringUsingSubstrTest (owned) and SplitStringPieceUsingSubstrTest.
// ---------------------------------------------------------------------------

#[test]
fn split_string_using_substr_empty_string() {
    let results = split_string_using_substr("", "DELIMITER", Trim, All);
    assert_eq!(results, strs(&[""]));
}

#[test]
fn split_string_using_substr_empty_delimiter() {
    let results = split_string_using_substr("TEST", "", Trim, All);
    assert_eq!(results, strs(&["TEST"]));
}

#[test]
fn split_string_using_substr_string_with_no_delimiter() {
    let results = split_string_using_substr("alongwordwithnodelimiter", "DELIMITER", Trim, All);
    assert_eq!(results, strs(&["alongwordwithnodelimiter"]));
}

#[test]
fn split_string_using_substr_leading_delimiters_skipped() {
    let results = split_string_using_substr(
        "DELIMITERDELIMITERDELIMITERoneDELIMITERtwoDELIMITERthree",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, strs(&["", "", "", "one", "two", "three"]));
}

#[test]
fn split_string_using_substr_consecutive_delimiters_skipped() {
    let results = split_string_using_substr(
        "unoDELIMITERDELIMITERDELIMITERdosDELIMITERtresDELIMITERDELIMITERcuatro",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, strs(&["uno", "", "", "dos", "tres", "", "cuatro"]));
}

#[test]
fn split_string_using_substr_trailing_delimiters_skipped() {
    let results = split_string_using_substr(
        "unDELIMITERdeuxDELIMITERtroisDELIMITERquatreDELIMITERDELIMITERDELIMITER",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, strs(&["un", "deux", "trois", "quatre", "", "", ""]));
}

#[test]
fn split_string_piece_using_substr_string_with_no_delimiter() {
    let results =
        split_string_piece_using_substr("alongwordwithnodelimiter", "DELIMITER", Trim, All);
    assert_eq!(results, vec!["alongwordwithnodelimiter"]);
}

#[test]
fn split_string_piece_using_substr_leading_delimiters_skipped() {
    let results = split_string_piece_using_substr(
        "DELIMITERDELIMITERDELIMITERoneDELIMITERtwoDELIMITERthree",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, vec!["", "", "", "one", "two", "three"]);
}

#[test]
fn split_string_piece_using_substr_consecutive_delimiters_skipped() {
    let results = split_string_piece_using_substr(
        "unoDELIMITERDELIMITERDELIMITERdosDELIMITERtresDELIMITERDELIMITERcuatro",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, vec!["uno", "", "", "dos", "tres", "", "cuatro"]);
}

#[test]
fn split_string_piece_using_substr_trailing_delimiters_skipped() {
    let results = split_string_piece_using_substr(
        "unDELIMITERdeuxDELIMITERtroisDELIMITERquatreDELIMITERDELIMITERDELIMITER",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, vec!["un", "deux", "trois", "quatre", "", "", ""]);
}

#[test]
fn split_string_piece_using_substr_keep_whitespace() {
    let results = split_string_piece_using_substr(
        "un DELIMITERdeux\tDELIMITERtrois\nDELIMITERquatre",
        "DELIMITER",
        Keep,
        All,
    );
    assert_eq!(results, vec!["un ", "deux\t", "trois\n", "quatre"]);
}

#[test]
fn split_string_piece_using_substr_trim_whitespace() {
    let results = split_string_piece_using_substr(
        "un DELIMITERdeux\tDELIMITERtrois\nDELIMITERquatre",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, vec!["un", "deux", "trois", "quatre"]);
}

#[test]
fn split_string_piece_using_substr_split_want_all() {
    let results = split_string_piece_using_substr(
        "unDELIMITERdeuxDELIMITERtroisDELIMITERDELIMITER",
        "DELIMITER",
        Trim,
        All,
    );
    assert_eq!(results, vec!["un", "deux", "trois", "", ""]);
}

#[test]
fn split_string_piece_using_substr_split_want_non_empty() {
    let results = split_string_piece_using_substr(
        "unDELIMITERdeuxDELIMITERtroisDELIMITERDELIMITER",
        "DELIMITER",
        Trim,
        NonEmpty,
    );
    assert_eq!(results, vec!["un", "deux", "trois"]);
}

// ---------------------------------------------------------------------------
// StringUtilTest, StringSplitTest, and the rest of the plain TESTs.
// ---------------------------------------------------------------------------

#[test]
fn string_util_test_split_string_basics() {
    let r = split_string("", ",:;", Keep, All);
    assert!(r.is_empty());

    // Empty separator list.
    let r = split_string("hello, world", "", Keep, All);
    assert_eq!(r, strs(&["hello, world"]));

    // Should split on any of the separators.
    let r = split_string("::,,;;", ",:;", Keep, All);
    assert_eq!(r.len(), 7);
    for s in &r {
        assert!(s.is_empty());
    }

    let r = split_string("red, green; blue:", ",:;", Trim, NonEmpty);
    assert_eq!(r, strs(&["red", "green", "blue"]));

    // Want to split a string along whitespace sequences.
    let r = split_string("  red green   \tblue\n", " \t\n", Trim, NonEmpty);
    assert_eq!(r, strs(&["red", "green", "blue"]));

    // Weird case of splitting on spaces but not trimming.
    let r = split_string(" red ", " ", Trim, All);
    assert_eq!(r, strs(&["", "red", ""]));
}

#[test]
fn string_util_test_split_string_whitespace_and_result_type() {
    // Empty input handling.
    let r = split_string("", ",", Keep, All);
    assert!(r.is_empty());
    let r = split_string("", ",", Keep, NonEmpty);
    assert!(r.is_empty());

    // Input string is space and we're trimming.
    let r = split_string(" ", ",", Trim, All);
    assert_eq!(r, strs(&[""]));
    let r = split_string(" ", ",", Trim, NonEmpty);
    assert!(r.is_empty());

    // Test all 4 combinations of flags on ", ,".
    let r = split_string(", ,", ",", Keep, All);
    assert_eq!(r, strs(&["", " ", ""]));
    let r = split_string(", ,", ",", Keep, NonEmpty);
    assert_eq!(r, strs(&[" "]));
    let r = split_string(", ,", ",", Trim, All);
    assert_eq!(r, strs(&["", "", ""]));
    let r = split_string(", ,", ",", Trim, NonEmpty);
    assert!(r.is_empty());
}

#[test]
fn string_split_test_split_string_once() {
    // None of the separators are in the input, so should always be None.
    assert_eq!(split_string_once_any("", ""), None);
    assert_eq!(split_string_once_any("a", ""), None);
    assert_eq!(split_string_once_any("ab", ""), None);

    assert_eq!(split_string_once("a:b:c", ':'), Some(("a", "b:c")));
    assert_eq!(split_string_once("a:", ':'), Some(("a", "")));
    assert_eq!(split_string_once(":b", ':'), Some(("", "b")));

    // Now the same using the multiple separators overload, but with only one
    // separator specified.
    assert_eq!(split_string_once_any("a:b:c", ":"), Some(("a", "b:c")));
    assert_eq!(split_string_once_any("a:", ":"), Some(("a", "")));
    assert_eq!(split_string_once_any(":b", ":"), Some(("", "b")));

    // Multiple separators overload, but only the first one present.
    assert_eq!(split_string_once_any("a:b:c", ":="), Some(("a", "b:c")));
    assert_eq!(split_string_once_any("a:", ":="), Some(("a", "")));
    assert_eq!(split_string_once_any(":b", ":="), Some(("", "b")));

    // Multiple separators overload, but only the second one present.
    assert_eq!(split_string_once_any("a:b:c", "=:"), Some(("a", "b:c")));
    assert_eq!(split_string_once_any("a:", "=:"), Some(("a", "")));
    assert_eq!(split_string_once_any(":b", "=:"), Some(("", "b")));

    // Multiple separators overload, both present. The separator that comes
    // first in the input string (not separators string) should win.
    assert_eq!(split_string_once_any("a:b=c", ":="), Some(("a", "b=c")));
    assert_eq!(split_string_once_any("a:b=c", "=:"), Some(("a", "b=c")));
    assert_eq!(split_string_once_any("a=b:c", ":="), Some(("a", "b:c")));
    assert_eq!(split_string_once_any("a=b:c", "=:"), Some(("a", "b:c")));
}

#[test]
fn string_split_test_rsplit_string_once() {
    // None of the separators are in the input, so should always be None.
    assert_eq!(rsplit_string_once_any("", ""), None);
    assert_eq!(rsplit_string_once_any("a", ""), None);
    assert_eq!(rsplit_string_once_any("ab", ""), None);

    assert_eq!(rsplit_string_once(":", ':'), Some(("", "")));
    assert_eq!(rsplit_string_once("a:b:c", ':'), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once("a:", ':'), Some(("a", "")));
    assert_eq!(rsplit_string_once(":b", ':'), Some(("", "b")));

    // Now the same using the multiple separators overload, but with only one
    // separator specified.
    assert_eq!(rsplit_string_once_any("a:b:c", ":"), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once_any("a:", ":"), Some(("a", "")));
    assert_eq!(rsplit_string_once_any(":b", ":"), Some(("", "b")));

    // Multiple separators overload, but only the first one present.
    assert_eq!(rsplit_string_once_any("a:b:c", ":="), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once_any("a:", ":="), Some(("a", "")));
    assert_eq!(rsplit_string_once_any(":b", ":="), Some(("", "b")));

    // Multiple separators overload, but only the second one present.
    assert_eq!(rsplit_string_once_any("a:b:c", "=:"), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once_any("a:", "=:"), Some(("a", "")));
    assert_eq!(rsplit_string_once_any(":b", "=:"), Some(("", "b")));

    // Multiple separators overload, both present. The separator that comes
    // first in the input string (not separators string) should win.
    assert_eq!(rsplit_string_once_any("a:b=c", ":="), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once_any("a:b=c", "=:"), Some(("a:b", "c")));
    assert_eq!(rsplit_string_once_any("a=b:c", ":="), Some(("a=b", "c")));
    assert_eq!(rsplit_string_once_any("a=b:c", "=:"), Some(("a=b", "c")));
}

#[test]
fn string_split_test_string_split_keep_whitespace() {
    let r = split_string("   ", "*", Keep, All);
    assert_eq!(r, strs(&["   "]));

    let r = split_string("\t  \ta\t ", "\t", Keep, All);
    assert_eq!(r, strs(&["", "  ", "a", " "]));

    let r = split_string("\ta\t\nb\tcc", "\n", Keep, All);
    assert_eq!(r, strs(&["\ta\t", "b\tcc"]));
}

#[test]
fn string_split_test_split_string_along_whitespace() {
    // (input, expected_result_count, output1, output2)
    let data: [(&str, usize, &str, &str); 13] = [
        ("a", 1, "a", ""),
        (" ", 0, "", ""),
        (" a", 1, "a", ""),
        (" ab ", 1, "ab", ""),
        (" ab c", 2, "ab", "c"),
        (" ab c ", 2, "ab", "c"),
        (" ab cd", 2, "ab", "cd"),
        (" ab cd ", 2, "ab", "cd"),
        (" \ta\t", 1, "a", ""),
        (" b\ta\t", 2, "b", "a"),
        (" b\tat", 2, "b", "at"),
        ("b\tat", 2, "b", "at"),
        ("b\t at", 2, "b", "at"),
    ];
    for (input, count, out1, out2) in data {
        let results = split_string(input, WHITESPACE_ASCII, Keep, NonEmpty);
        assert_eq!(results.len(), count, "input {input:?}");
        if count > 0 {
            assert_eq!(results[0], out1, "input {input:?}");
        }
        if count > 1 {
            assert_eq!(results[1], out2, "input {input:?}");
        }
    }
}

#[test]
fn string_split_test_null_separator_want_non_empty() {
    let data: [(&str, Vec<&str>); 4] = [
        ("a", vec!["a"]),
        ("a\0", vec!["a"]),
        ("a\0a", vec!["a", "a"]),
        ("\0a\0\0a\0", vec!["a", "a"]),
    ];
    for (input, expected) in data {
        let results = split_string_piece(input, "\0", Keep, NonEmpty);
        assert_eq!(results, expected);
    }
}

#[test]
fn string_split_test_null_separator_want_all() {
    let data: [(&str, Vec<&str>); 4] = [
        ("a", vec!["a"]),
        ("a\0", vec!["a", ""]),
        ("a\0a", vec!["a", "a"]),
        ("\0a\0\0a\0", vec!["", "a", "", "a", ""]),
    ];
    for (input, expected) in data {
        let results = split_string_piece(input, "\0", Keep, All);
        assert_eq!(results, expected);
    }
}

// The C++ versions of these are `constexpr`. Vec-returning functions cannot be
// evaluated in a const context on stable Rust, so these run at runtime with the
// same inputs and expected pieces.

#[test]
fn constexpr_split_string_piece_split_string_pieces() {
    let s = "abc- def--ghi";
    let r = split_string_piece(s, "-", Trim, All);
    assert_eq!(r[0], "abc");
    assert_eq!(r[1], "def");
    assert_eq!(r[2], "");
    assert_eq!(r[3], "ghi");
}

#[test]
fn constexpr_split_string_piece_split_string_pieces_skip_empty() {
    let s = "abc- def--ghi";
    let r = split_string_piece(s, "-", Trim, NonEmpty);
    assert_eq!(r[0], "abc");
    assert_eq!(r[1], "def");
    assert_eq!(r[2], "ghi");
}

#[test]
fn constexpr_split_string_piece_split_string_using_substr() {
    let r = split_string_piece_using_substr("abcDELIMITERdef", "DELIMITER", Keep, All);
    assert_eq!(r[0], "abc");
    assert_eq!(r[1], "def");
}

// ---------------------------------------------------------------------------
// Extra tests: UTF-16 variants.
// ---------------------------------------------------------------------------

#[test]
fn utf16_split_string_basics() {
    assert!(split_string_utf16(&u(""), &u(",:;"), Keep, All).is_empty());
    assert_eq!(
        split_string_utf16(&u("hello, world"), &u(""), Keep, All),
        utf16_list(&["hello, world"])
    );
    let r = split_string_utf16(&u("::,,;;"), &u(",:;"), Keep, All);
    assert_eq!(r.len(), 7);
    assert!(r.iter().all(Vec::is_empty));
    assert_eq!(
        split_string_utf16(&u("red, green; blue:"), &u(",:;"), Trim, NonEmpty),
        utf16_list(&["red", "green", "blue"])
    );
    assert_eq!(split_string_utf16(&u(" red "), &u(" "), Trim, All), utf16_list(&["", "red", ""]));
}

#[test]
fn utf16_trims_non_ascii_whitespace() {
    // U+00A0, U+3000 and U+2028 are whitespace in kWhitespaceUTF16.
    let input = u("\u{a0}a\u{3000}\u{2028}");
    assert_eq!(split_string_utf16(&input, &[], Trim, All), utf16_list(&["a"]));
    assert_eq!(split_string_utf16(&input, &[], Keep, All), vec![input.clone()]);
}

#[test]
fn utf16_split_string_piece_points_into_input() {
    let input = u("a,b ,c");
    let pieces = split_string_piece_utf16(&input, &u(","), Trim, All);
    assert_eq!(pieces, vec![&u("a")[..], &u("b")[..], &u("c")[..]]);
    // Pieces are element-aligned sub-slices of the input, so their offsets in
    // elements (not bytes) must fit inside the input.
    let base = input.as_ptr() as usize;
    for piece in &pieces {
        let start = (piece.as_ptr() as usize - base) / std::mem::size_of::<u16>();
        assert!(start + piece.len() <= input.len());
    }
}

#[test]
fn utf16_substr_split_matches_utf8_on_examples() {
    let cases: [(&str, &str); 6] = [
        ("", "DELIMITER"),
        ("alongwordwithnodelimiter", "DELIMITER"),
        ("DELIMITERDELIMITERDELIMITERoneDELIMITERtwoDELIMITERthree", "DELIMITER"),
        ("un DELIMITERdeux\tDELIMITERtrois\nDELIMITERquatre", "DELIMITER"),
        ("unDELIMITERdeuxDELIMITERtroisDELIMITERDELIMITER", "DELIMITER"),
        ("aaaa", "aa"),
    ];
    for (input, delim) in cases {
        for ws in [Keep, Trim] {
            for rt in [All, NonEmpty] {
                let expected: Vec<Vec<u16>> =
                    split_string_using_substr(input, delim, ws, rt).iter().map(|s| u(s)).collect();
                assert_eq!(
                    split_string_using_substr_utf16(&u(input), &u(delim), ws, rt),
                    expected,
                    "input {input:?} delim {delim:?} ws {ws:?} rt {rt:?}"
                );
            }
        }
    }
}

#[test]
fn utf16_substr_delimiter_longer_than_input() {
    assert_eq!(
        split_string_using_substr_utf16(&u("ab"), &u("abc"), Trim, All),
        utf16_list(&["ab"])
    );
    assert!(split_string_using_substr_utf16(&u("ab"), &u("abc"), Trim, NonEmpty)
        .iter()
        .all(|s| !s.is_empty()));
}

#[test]
fn utf16_substr_empty_delimiter_returns_input_untrimmed() {
    assert_eq!(
        split_string_using_substr_utf16(&u("  a  "), &[], Trim, NonEmpty),
        utf16_list(&["  a  "])
    );
    assert_eq!(split_string_using_substr_utf16(&[], &[], Trim, NonEmpty), utf16_list(&[""]));
}

// ---------------------------------------------------------------------------
// Extra tests: UTF-8 specifics and documented differences.
// ---------------------------------------------------------------------------

#[test]
fn utf8_trims_only_ascii_whitespace() {
    // U+00A0 is not trimmed by the UTF-8 functions (kWhitespaceASCII).
    assert_eq!(split_string_piece("\u{a0}a\u{a0},b", ",", Trim, All), vec!["\u{a0}a\u{a0}", "b"]);
}

#[test]
fn utf8_trims_vertical_tab() {
    // Rust's u8::is_ascii_whitespace omits \v. The C++ set includes it.
    assert_eq!(split_string_piece("\u{0B}a\u{0B}", ",", Trim, NonEmpty), vec!["a"]);
}

#[test]
fn utf8_multibyte_separator_matches_whole_character() {
    assert_eq!(split_string_piece("a→b→c", "→", Keep, All), vec!["a", "b", "c"]);
    assert_eq!(split_string_piece("aéb", "é", Keep, All), vec!["a", "b"]);
}

#[test]
fn utf8_once_multibyte_separator() {
    assert_eq!(split_string_once("abc", '→'), None);
    assert_eq!(split_string_once("a→b→c", '→'), Some(("a", "b→c")));
    assert_eq!(rsplit_string_once("a→b→c", '→'), Some(("a→b", "c")));
    assert_eq!(split_string_once_any("x→y=z", "=→"), Some(("x", "y=z")));
    assert_eq!(rsplit_string_once_any("x→y=z", "=→"), Some(("x→y", "z")));
}

#[test]
fn utf8_substr_empty_delimiter_quirk_is_preserved() {
    assert_eq!(split_string_using_substr("  a  ", "", Trim, NonEmpty), strs(&["  a  "]));
    assert_eq!(split_string_using_substr("", "", Trim, NonEmpty), strs(&[""]));
    assert!(split_string_using_substr("", "X", Keep, NonEmpty).is_empty());
    assert_eq!(split_string_using_substr("", "X", Keep, All), strs(&[""]));
}

#[test]
fn utf8_substr_non_overlapping_matches() {
    assert_eq!(split_string_piece_using_substr("aaaa", "aa", Keep, All), vec!["", "", ""]);
}

#[test]
fn utf8_piece_points_into_input() {
    let input = String::from("a,b ,c");
    let pieces = split_string_piece(&input, ",", Trim, All);
    assert_eq!(pieces, vec!["a", "b", "c"]);
    let base = input.as_ptr() as usize;
    for piece in pieces {
        let start = piece.as_ptr() as usize - base;
        assert!(start + piece.len() <= input.len());
    }
}

#[test]
fn key_value_empty_pair_delimiter_quirk_is_preserved() {
    // An empty pair delimiter yields the whole input as one pair, untrimmed
    // and unfiltered. An empty input therefore gives one ("", "") pair.
    let r = split_string_into_key_value_pairs_using_substr("", ':', "");
    assert_eq!(r, KeyValueSplit { pairs: pairs(&[("", "")]), complete: false });

    let r = split_string_into_key_value_pairs_using_substr("  key1:value1 ", ':', "");
    assert_eq!(r, KeyValueSplit { pairs: pairs(&[("  key1", "value1 ")]), complete: true });
}

#[test]
fn key_value_non_ascii_delimiters() {
    let r = split_string_into_key_value_pairs("k→v;x→y", '→', ';');
    assert_eq!(r, KeyValueSplit { pairs: pairs(&[("k", "v"), ("x", "y")]), complete: true });
}

#[test]
fn key_value_view_pairs_point_into_input() {
    let input = "key1:value1,key2:value2";
    let r = split_string_into_key_value_view_pairs(input, ':', ',');
    let base = input.as_ptr() as usize;
    for (k, v) in &r.pairs {
        let ks = k.as_ptr() as usize - base;
        let vs = v.as_ptr() as usize - base;
        assert!(ks + k.len() <= input.len());
        assert!(vs + v.len() <= input.len());
    }
}

// ---------------------------------------------------------------------------
// Extra tests: randomized differential tests against simple references.
// ---------------------------------------------------------------------------

/// xorshift64 PRNG. Seeds must be non-zero.
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
}

/// All BMP characters, so that UTF-8 and UTF-16 inputs have the same
/// character sequence.
const ALPHABET: &[char] = &[
    'a', 'b', ',', ':', ' ', '\t', '\u{0B}', '\u{0C}', '\n', '\r', '\u{A0}', 'é', '→', '\u{3000}',
    '=', '\0',
];

const SEPARATOR_POOL: &[char] = &[',', ':', ' ', '\t', 'é', '\0', '\u{0B}', '\u{A0}', 'b'];

fn random_text(rng: &mut XorShift64, max_len: usize) -> String {
    let n = rng.below(max_len + 1);
    (0..n).map(|_| ALPHABET[rng.below(ALPHABET.len())]).collect()
}

fn random_separators(rng: &mut XorShift64) -> Vec<char> {
    let n = rng.below(4);
    (0..n).map(|_| SEPARATOR_POOL[rng.below(SEPARATOR_POOL.len())]).collect()
}

fn ref_ws_ascii(c: char) -> bool {
    matches!(c as u32, 0x09..=0x0D | 0x20)
}

fn ref_ws_utf16(c: char) -> bool {
    matches!(
        c as u32,
        0x09..=0x0D
            | 0x20
            | 0x85
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
    )
}

fn finish_reference(
    pieces: Vec<Vec<char>>,
    ws: fn(char) -> bool,
    trim: bool,
    keep_all: bool,
) -> Vec<String> {
    pieces
        .into_iter()
        .map(|piece| {
            if !trim {
                return piece;
            }
            let start = piece.iter().position(|&c| !ws(c)).unwrap_or(piece.len());
            let end = piece.iter().rposition(|&c| !ws(c)).map_or(start, |last| last + 1);
            piece[start..end].to_vec()
        })
        .filter(|piece| keep_all || !piece.is_empty())
        .map(|piece| piece.into_iter().collect())
        .collect()
}

fn ref_split(
    input: &str,
    seps: &[char],
    ws: fn(char) -> bool,
    trim: bool,
    keep_all: bool,
) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    let mut pieces: Vec<Vec<char>> = vec![Vec::new()];
    for c in input.chars() {
        if seps.contains(&c) {
            pieces.push(Vec::new());
        } else {
            pieces.last_mut().unwrap().push(c);
        }
    }
    finish_reference(pieces, ws, trim, keep_all)
}

fn ref_substr(
    input: &str,
    delim: &str,
    ws: fn(char) -> bool,
    trim: bool,
    keep_all: bool,
) -> Vec<String> {
    if delim.is_empty() {
        return vec![input.to_string()];
    }
    let text: Vec<char> = input.chars().collect();
    let d: Vec<char> = delim.chars().collect();
    let mut pieces = Vec::new();
    let mut cur = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(&d) {
            pieces.push(std::mem::take(&mut cur));
            i += d.len();
        } else {
            cur.push(text[i]);
            i += 1;
        }
    }
    pieces.push(cur);
    finish_reference(pieces, ws, trim, keep_all)
}

fn ref_once(input: &str, seps: &[char], last: bool) -> Option<(String, String)> {
    let chars: Vec<char> = input.chars().collect();
    let idx = if last {
        chars.iter().rposition(|c| seps.contains(c))
    } else {
        chars.iter().position(|c| seps.contains(c))
    }?;
    Some((chars[..idx].iter().collect(), chars[idx + 1..].iter().collect()))
}

fn to_utf16_strings(list: &[String]) -> Vec<Vec<u16>> {
    list.iter().map(|s| u(s)).collect()
}

#[test]
fn differential_split_string_matches_reference() {
    let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
    for _ in 0..5000 {
        let input = random_text(&mut rng, 24);
        let seps = random_separators(&mut rng);
        let sep_str: String = seps.iter().collect();
        for ws in [Keep, Trim] {
            for rt in [All, NonEmpty] {
                let trim = ws == Trim;
                let keep_all = rt == All;
                let expected = ref_split(&input, &seps, ref_ws_ascii, trim, keep_all);
                assert_eq!(
                    split_string(&input, &sep_str, ws, rt),
                    expected,
                    "input {input:?} seps {seps:?} ws {ws:?} rt {rt:?}"
                );
                assert_eq!(
                    split_string_piece(&input, &sep_str, ws, rt),
                    expected.iter().map(String::as_str).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn differential_split_string_utf16_matches_reference() {
    let mut rng = XorShift64(0xD1B5_4A32_D192_ED03);
    for _ in 0..5000 {
        let input = random_text(&mut rng, 24);
        let seps = random_separators(&mut rng);
        let sep_str: String = seps.iter().collect();
        for ws in [Keep, Trim] {
            for rt in [All, NonEmpty] {
                let expected = ref_split(&input, &seps, ref_ws_utf16, ws == Trim, rt == All);
                assert_eq!(
                    split_string_utf16(&u(&input), &u(&sep_str), ws, rt),
                    to_utf16_strings(&expected),
                    "input {input:?} seps {seps:?} ws {ws:?} rt {rt:?}"
                );
            }
        }
    }
}

#[test]
fn differential_split_string_using_substr_matches_reference() {
    let mut rng = XorShift64(0x2545_F491_4F6C_DD1D);
    let delims = ["", "a", "ab", "aa", ",:", "é→", "\0", " \t"];
    for _ in 0..5000 {
        let input = random_text(&mut rng, 24);
        let delim = delims[rng.below(delims.len())];
        for ws in [Keep, Trim] {
            for rt in [All, NonEmpty] {
                let expected = ref_substr(&input, delim, ref_ws_ascii, ws == Trim, rt == All);
                assert_eq!(
                    split_string_using_substr(&input, delim, ws, rt),
                    expected,
                    "input {input:?} delim {delim:?} ws {ws:?} rt {rt:?}"
                );
                assert_eq!(
                    split_string_using_substr_utf16(&u(&input), &u(delim), ws, rt),
                    to_utf16_strings(&ref_substr(
                        &input,
                        delim,
                        ref_ws_utf16,
                        ws == Trim,
                        rt == All
                    )),
                    "utf16 input {input:?} delim {delim:?} ws {ws:?} rt {rt:?}"
                );
            }
        }
    }
}

#[test]
fn differential_once_matches_reference() {
    let mut rng = XorShift64(0x1234_5678_9ABC_DEF1);
    for _ in 0..5000 {
        let input = random_text(&mut rng, 16);
        let seps = random_separators(&mut rng);
        let sep_str: String = seps.iter().collect();
        for last in [false, true] {
            let expected = ref_once(&input, &seps, last);
            let got = if last {
                rsplit_string_once_any(&input, &sep_str)
            } else {
                split_string_once_any(&input, &sep_str)
            };
            let got = got.map(|(a, b)| (a.to_string(), b.to_string()));
            assert_eq!(got, expected, "input {input:?} seps {seps:?} last {last}");
        }
    }
}

#[test]
fn differential_key_value_owned_and_view_agree_and_hold_invariants() {
    let mut rng = XorShift64(0xA5A5_5A5A_1357_9BDF);
    let kv_chars = [':', '=', '→'];
    let pair_chars = [',', ';', ' ', '\t'];
    for _ in 0..3000 {
        let input = random_text(&mut rng, 30);
        let kv = kv_chars[rng.below(kv_chars.len())];
        let pair = pair_chars[rng.below(pair_chars.len())];
        let owned = split_string_into_key_value_pairs(&input, kv, pair);
        let view = split_string_into_key_value_view_pairs(&input, kv, pair);
        assert_eq!(owned.complete, view.complete, "input {input:?}");
        assert_eq!(owned.pairs.len(), view.pairs.len());
        for ((ok, ov), (vk, vv)) in owned.pairs.iter().zip(&view.pairs) {
            assert_eq!(ok, vk);
            assert_eq!(ov, vv);
            assert!(!ok.contains(kv), "key contains kv delimiter: {input:?}");
            assert!(!ok.contains(pair), "key contains pair delimiter: {input:?}");
            assert!(!ov.contains(pair), "value contains pair delimiter: {input:?}");
        }
        // `complete` requires a non-empty value. An empty key is allowed, as
        // in the EmptyKeyWithKeyValueDelimiter test.
        if owned.complete {
            for (_, v) in &owned.pairs {
                assert!(!v.is_empty(), "input {input:?}");
            }
        }
    }
}
