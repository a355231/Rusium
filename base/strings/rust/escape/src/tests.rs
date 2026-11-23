// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of every `TEST` in `base/strings/escape_unittest.cc`, plus extra
//! randomized and property tests.

use crate::*;

/// Small xorshift64 PRNG, so the tests need no external crates.
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        XorShift64(seed.max(1))
    }

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

    fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }

    fn bytes(&mut self, max_len: usize) -> Vec<u8> {
        let len = self.below(max_len + 1);
        (0..len).map(|_| self.byte()).collect()
    }
}

/// The printable mix used by several C++ tests: most of the ASCII range, plus
/// `\x80` and `\xff`.
const MIXED: &[u8] = b"\x02\n\x1d !\"#$%&'()*+,-./0123456789:;\
<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\x7f\x80\xff";

// ---- Ported from escape_unittest.cc ---------------------------------------

#[test]
fn ported_escape_text_for_form_submission() {
    let escape_cases = [("foo", "foo"), ("foo bar", "foo+bar"), ("foo++", "foo%2B%2B")];
    for (input, output) in escape_cases {
        assert_eq!(escape_query_param_value(input.as_bytes(), true), output);
    }

    let escape_cases_no_plus = [("foo", "foo"), ("foo bar", "foo%20bar"), ("foo++", "foo%2B%2B")];
    for (input, output) in escape_cases_no_plus {
        assert_eq!(escape_query_param_value(input.as_bytes(), false), output);
    }

    // Test all the values we're supposed to be escaping.
    let no_escape = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!'()*-._~";
    for i in 0..=u8::MAX {
        let input = [i];
        let out = escape_query_param_value(&input, true);
        if i == 0 {
            assert_eq!(out, "%00");
        } else if i == 32 {
            // Spaces are plus escaped like web forms.
            assert_eq!(out, "+");
        } else if !no_escape.contains(&i) {
            // Check %hex escaping.
            assert_eq!(out, format!("%{i:02X}"));
        } else {
            // No change for things in the no_escape list.
            assert_eq!(out.as_bytes(), &input);
        }
    }
}

#[test]
fn ported_escape_path() {
    assert_eq!(
        escape_path(MIXED),
        // Escaped
        "%02%0A%1D%20!%22%23$%25&'()*+,-./0123456789%3A;\
%3C=%3E%3F@ABCDEFGHIJKLMNOPQRSTUVWXYZ\
%5B%5C%5D%5E_%60abcdefghijklmnopqrstuvwxyz\
%7B%7C%7D~%7F%80%FF"
    );
}

#[test]
fn ported_escape_url_encoded_data() {
    assert_eq!(
        escape_url_encoded_data(MIXED, true),
        "%02%0A%1D+!%22%23%24%25%26%27()*%2B,-./0123456789:%3B\
%3C%3D%3E%3F%40ABCDEFGHIJKLMNOPQRSTUVWXYZ\
%5B%5C%5D%5E_%60abcdefghijklmnopqrstuvwxyz\
%7B%7C%7D~%7F%80%FF"
    );
}

#[test]
fn ported_escape_url_encoded_data_space() {
    assert_eq!(escape_url_encoded_data(b"a b", true), "a+b");
    assert_eq!(escape_url_encoded_data(b"a b", false), "a%20b");
}

#[test]
fn ported_escape_for_html() {
    let tests: [(&str, &str); 3] = [
        ("hello", "hello"),
        ("<hello>", "&lt;hello&gt;"),
        ("don't mess with me", "don&#39;t mess with me"),
    ];
    for (input, expected) in tests {
        let result = escape_for_html(input.as_bytes());
        assert_eq!(result, expected.as_bytes());
    }
}

#[test]
fn ported_unescape_for_html() {
    let tests: [(&str, &str); 11] = [
        ("", ""),
        ("&lt;hello&gt;", "<hello>"),
        ("don&#39;t mess with me", "don't mess with me"),
        ("&lt;&gt;&amp;&quot;&#39;", "<>&\"'"),
        ("& lt; &amp ; &; '", "& lt; &amp ; &; '"),
        ("&amp;", "&"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp; &", "& &"),
    ];
    for (input, expected) in tests {
        assert_eq!(unescape_for_html(input), expected);
    }
}

#[test]
fn ported_escape_external_handler_value() {
    assert_eq!(
        // Escaped
        "%02%0A%1D%20!%22#$%25&'()*+,-./0123456789:;\
%3C=%3E?@ABCDEFGHIJKLMNOPQRSTUVWXYZ\
[%5C]%5E_%60abcdefghijklmnopqrstuvwxyz\
%7B%7C%7D~%7F%80%FF",
        // Most of the character space we care about, un-escaped
        escape_external_handler_value(MIXED)
    );

    assert_eq!(
        "!#$&'()*+,-./0123456789:;=?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[]_\
abcdefghijklmnopqrstuvwxyz~",
        escape_external_handler_value(
            b"!#$&'()*+,-./0123456789:;=?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[]_\
abcdefghijklmnopqrstuvwxyz~"
        )
    );

    assert_eq!(escape_external_handler_value(b"%8k"), "%258k");
    assert_eq!(escape_external_handler_value(b"a%"), "a%25");
    assert_eq!(escape_external_handler_value(b"%a"), "%25a");
    assert_eq!(escape_external_handler_value(b"a%8"), "a%258");
    assert_eq!(escape_external_handler_value(b"%ab"), "%ab");
    assert_eq!(escape_external_handler_value(b"%AB"), "%AB");

    assert_eq!(
        escape_external_handler_value(b"http://example.com/path/sub?q=a|b|c&q=1|2|3#ref|"),
        "http://example.com/path/sub?q=a%7Cb%7Cc&q=1%7C2%7C3#ref%7C"
    );
    assert_eq!(
        escape_external_handler_value(
            b"http://example.com/path/sub?q=a%7Cb%7Cc&q=1%7C2%7C3#ref%7C"
        ),
        "http://example.com/path/sub?q=a%7Cb%7Cc&q=1%7C2%7C3#ref%7C"
    );
    assert_eq!(
        escape_external_handler_value(b"http://[2001:db8:0:1]:80"),
        "http://[2001:db8:0:1]:80"
    );
}

#[test]
fn ported_escape_non_ascii() {
    assert_eq!(escape_non_ascii_and_percent(b"abc\n%80\x80"), "abc\n%2580%80");
    assert_eq!(escape_non_ascii(b"abc\n%80\x80"), "abc\n%80%80");
}

#[test]
fn ported_data_url_with_accented_characters() {
    let url = "text/html;charset=utf-8,%3Chtml%3E%3Cbody%3ETonton,%20ton%20th%C3\
%A9%20t'a-t-il%20%C3%B4t%C3%A9%20ta%20toux%20";

    let (decoded, _adjustments) = unescape_and_decode_utf8_url_component_with_adjustments(
        url.as_bytes(),
        UnescapeRule::SPACES,
    );
    // The C++ test only checks that this does not crash. This also checks the
    // decoded text.
    assert!(decoded.starts_with("text/html;charset=utf-8,<html><body>Tonton, ton th\u{e9} "));
}

#[test]
fn ported_unescape_url_component() {
    type Case = (&'static [u8], UnescapeRule, &'static [u8]);
    let normal = UnescapeRule::NORMAL;
    let cases: &[Case] = &[
        (b"", normal, b""),
        (b"%2", normal, b"%2"),
        (b"%%%%%%", normal, b"%%%%%%"),
        (b"Don't escape anything", normal, b"Don't escape anything"),
        (b"Invalid %escape %2", normal, b"Invalid %escape %2"),
        (b"Some%20random text %25%2dOK", UnescapeRule::NONE, b"Some%20random text %25%2dOK"),
        (b"Some%20random text %25%2dOK", normal, b"Some%20random text %25-OK"),
        (b"Some%20random text %25%E1%A6", normal, b"Some%20random text %25\xE1\xA6"),
        (b"Some%20random text %25%E1%A6OK", normal, b"Some%20random text %25\xE1\xA6OK"),
        (b"Some%20random text %25%E1%A6%99OK", normal, b"Some%20random text %25\xE1\xA6\x99OK"),
        // BiDi control characters should not be unescaped.
        (b"Some%20random text %25%D8%9COK", normal, b"Some%20random text %25%D8%9COK"),
        (b"Some%20random text %25%E2%80%8EOK", normal, b"Some%20random text %25%E2%80%8EOK"),
        (b"Some%20random text %25%E2%80%8FOK", normal, b"Some%20random text %25%E2%80%8FOK"),
        (b"Some%20random text %25%E2%80%AAOK", normal, b"Some%20random text %25%E2%80%AAOK"),
        (b"Some%20random text %25%E2%80%ABOK", normal, b"Some%20random text %25%E2%80%ABOK"),
        (b"Some%20random text %25%E2%80%AEOK", normal, b"Some%20random text %25%E2%80%AEOK"),
        (b"Some%20random text %25%E2%81%A6OK", normal, b"Some%20random text %25%E2%81%A6OK"),
        (b"Some%20random text %25%E2%81%A9OK", normal, b"Some%20random text %25%E2%81%A9OK"),
        // Certain banned characters should not be unescaped.
        // U+1F50F LOCK WITH INK PEN
        (b"Some%20random text %25%F0%9F%94%8FOK", normal, b"Some%20random text %25%F0%9F%94%8FOK"),
        // U+1F510 CLOSED LOCK WITH KEY
        (b"Some%20random text %25%F0%9F%94%90OK", normal, b"Some%20random text %25%F0%9F%94%90OK"),
        // U+1F512 LOCK
        (b"Some%20random text %25%F0%9F%94%92OK", normal, b"Some%20random text %25%F0%9F%94%92OK"),
        // U+1F513 OPEN LOCK
        (b"Some%20random text %25%F0%9F%94%93OK", normal, b"Some%20random text %25%F0%9F%94%93OK"),
        // Spaces
        (
            b"(%C2%85)(%C2%A0)(%E1%9A%80)(%E2%80%80)",
            normal,
            b"(%C2%85)(%C2%A0)(%E1%9A%80)(%E2%80%80)",
        ),
        (
            b"(%E2%80%81)(%E2%80%82)(%E2%80%83)(%E2%80%84)",
            normal,
            b"(%E2%80%81)(%E2%80%82)(%E2%80%83)(%E2%80%84)",
        ),
        (
            b"(%E2%80%85)(%E2%80%86)(%E2%80%87)(%E2%80%88)",
            normal,
            b"(%E2%80%85)(%E2%80%86)(%E2%80%87)(%E2%80%88)",
        ),
        (
            b"(%E2%80%89)(%E2%80%8A)(%E2%80%A8)(%E2%80%A9)",
            normal,
            b"(%E2%80%89)(%E2%80%8A)(%E2%80%A8)(%E2%80%A9)",
        ),
        (b"(%E2%80%AF)(%E2%81%9F)(%E3%80%80)", normal, b"(%E2%80%AF)(%E2%81%9F)(%E3%80%80)"),
        (b"(%E2%A0%80)", normal, b"(%E2%A0%80)"),
        // Default Ignorable and Formatting characters should not be unescaped.
        (b"(%E2%81%A5)(%EF%BF%B0)(%EF%BF%B8)", normal, b"(%E2%81%A5)(%EF%BF%B0)(%EF%BF%B8)"),
        (
            b"(%F3%A0%82%80)(%F3%A0%83%BF)(%F3%A0%87%B0)",
            normal,
            b"(%F3%A0%82%80)(%F3%A0%83%BF)(%F3%A0%87%B0)",
        ),
        (b"(%F3%A0%BF%BF)(%C2%AD)(%CD%8F)", normal, b"(%F3%A0%BF%BF)(%C2%AD)(%CD%8F)"),
        (
            b"(%D8%80%20)(%D8%85)(%DB%9D)(%DC%8F)(%E0%A3%A2)",
            normal,
            b"(%D8%80%20)(%D8%85)(%DB%9D)(%DC%8F)(%E0%A3%A2)",
        ),
        (
            b"(%E1%85%9F)(%E1%85%A0)(%E1%9E%B4)(%E1%9E%B5)",
            normal,
            b"(%E1%85%9F)(%E1%85%A0)(%E1%9E%B4)(%E1%9E%B5)",
        ),
        (
            b"(%E1%A0%8B)(%E1%A0%8C)(%E1%A0%8D)(%E1%A0%8E)",
            normal,
            b"(%E1%A0%8B)(%E1%A0%8C)(%E1%A0%8D)(%E1%A0%8E)",
        ),
        (
            b"(%E2%80%8B)(%E2%80%8C)(%E2%80%8D)(%E2%81%A0)",
            normal,
            b"(%E2%80%8B)(%E2%80%8C)(%E2%80%8D)(%E2%81%A0)",
        ),
        (
            b"(%E2%81%A1)(%E2%81%A2)(%E2%81%A3)(%E2%81%A4)",
            normal,
            b"(%E2%81%A1)(%E2%81%A2)(%E2%81%A3)(%E2%81%A4)",
        ),
        (
            b"(%E3%85%A4)(%EF%BB%BF)(%EF%BE%A0)(%EF%BF%B9)",
            normal,
            b"(%E3%85%A4)(%EF%BB%BF)(%EF%BE%A0)(%EF%BF%B9)",
        ),
        (
            b"(%EF%BF%BB)(%F0%91%82%BD)(%F0%91%83%8D)",
            normal,
            b"(%EF%BF%BB)(%F0%91%82%BD)(%F0%91%83%8D)",
        ),
        (b"(%F0%93%90%B0)(%F0%93%90%B8)", normal, b"(%F0%93%90%B0)(%F0%93%90%B8)"),
        // General Punctuation - Deprecated (U+206A--206F)
        (b"(%E2%81%AA)(%E2%81%AD)(%E2%81%AF)", normal, b"(%E2%81%AA)(%E2%81%AD)(%E2%81%AF)"),
        // Variation selectors (U+FE00--FE0F)
        (b"(%EF%B8%80)(%EF%B8%8C)(%EF%B8%8D)", normal, b"(%EF%B8%80)(%EF%B8%8C)(%EF%B8%8D)"),
        // Shorthand format controls (U+1BCA0--1BCA3)
        (
            b"(%F0%9B%B2%A0)(%F0%9B%B2%A1)(%F0%9B%B2%A3)",
            normal,
            b"(%F0%9B%B2%A0)(%F0%9B%B2%A1)(%F0%9B%B2%A3)",
        ),
        // Musical symbols beams and slurs (U+1D173--1D17A)
        (
            b"(%F0%9D%85%B3)(%F0%9D%85%B9)(%F0%9D%85%BA)",
            normal,
            b"(%F0%9D%85%B3)(%F0%9D%85%B9)(%F0%9D%85%BA)",
        ),
        // Tags block (U+E0000--E007F), includes unassigned points
        (
            b"(%F3%A0%80%80)(%F3%A0%80%81)(%F3%A0%81%8F)",
            normal,
            b"(%F3%A0%80%80)(%F3%A0%80%81)(%F3%A0%81%8F)",
        ),
        // Ideographic-specific variation selectors (U+E0100--E01EF)
        (
            b"(%F3%A0%84%80)(%F3%A0%84%90)(%F3%A0%87%AF)",
            normal,
            b"(%F3%A0%84%80)(%F3%A0%84%90)(%F3%A0%87%AF)",
        ),
        // Two spoofing characters in a row should not be unescaped.
        (b"%D8%9C%D8%9C", normal, b"%D8%9C%D8%9C"),
        // Non-spoofing characters surrounded by spoofing characters should be
        // unescaped.
        (b"%D8%9C%C2%A1%D8%9C%C2%A1", normal, b"%D8%9C\xC2\xA1%D8%9C\xC2\xA1"),
        // Invalid UTF-8 characters surrounded by spoofing characters should be
        // unescaped.
        (b"%D8%9C%85%D8%9C%85", normal, b"%D8%9C\x85%D8%9C\x85"),
        // Test with enough trail bytes to overflow the CBU8_MAX_LENGTH-byte
        // buffer. The first two bytes are a spoofing character as well.
        (
            b"%D8%9C%9C%9C%9C%9C%9C%9C%9C%9C%9C",
            normal,
            b"%D8%9C\x9C\x9C\x9C\x9C\x9C\x9C\x9C\x9C\x9C",
        ),
        (b"Some%20random text %25%2dOK", UnescapeRule::SPACES, b"Some random text %25-OK"),
        (
            b"Some%20random text %25%2dOK",
            UnescapeRule::PATH_SEPARATORS,
            b"Some%20random text %25-OK",
        ),
        (
            b"Some%20random text %25%2dOK",
            UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
            b"Some%20random text %-OK",
        ),
        (
            b"Some%20random text %25%2dOK",
            UnescapeRule::SPACES | UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
            b"Some random text %-OK",
        ),
        (b"%A0%B1%C2%D3%E4%F5", normal, b"\xA0\xB1\xC2\xD3\xE4\xF5"),
        (b"%Aa%Bb%Cc%Dd%Ee%Ff", normal, b"\xAa\xBb\xCc\xDd\xEe\xFf"),
        // Certain URL-sensitive characters should not be unescaped unless asked.
        (
            b"Hello%20%13%10world %23# %3F? %3D= %26& %25% %2B+",
            UnescapeRule::SPACES,
            b"Hello %13%10world %23# %3F? %3D= %26& %25% %2B+",
        ),
        (
            b"Hello%20%13%10world %23# %3F? %3D= %26& %25% %2B+",
            UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
            b"Hello%20%13%10world ## ?? == && %% ++",
        ),
        // We can neither escape nor unescape '@' since some websites expect it to
        // be preserved as either '@' or "%40".
        // See http://b/996720 and http://crbug.com/23933 .
        (b"me@my%40example", normal, b"me@my%40example"),
        // Control characters.
        (
            b"%01%02%03%04%05%06%07%08%09 %25",
            UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
            b"%01%02%03%04%05%06%07%08%09 %",
        ),
        (b"Hello%20%13%10%02", UnescapeRule::SPACES, b"Hello %13%10%02"),
        // '/' and '\\' should only be unescaped by PATH_SEPARATORS.
        (b"%2F%5C", UnescapeRule::PATH_SEPARATORS, b"/\\"),
    ];

    for (input, rules, output) in cases {
        assert_eq!(
            unescape_url_component(input, *rules),
            *output,
            "input {:?}",
            String::from_utf8_lossy(input)
        );
    }

    // Test NULL character unescaping, which can't be tested above since those
    // are just char pointers in C++.
    let mut input = b"Null".to_vec();
    input.push(0); // Also have a NULL in the input.
    input.extend_from_slice(b"%00%39Test");

    let mut expected = b"Null".to_vec();
    expected.push(0);
    expected.extend_from_slice(b"%009Test");
    assert_eq!(unescape_url_component(&input, normal), expected);
}

#[test]
fn ported_unescape_and_decode_utf8_url_component_with_adjustments() {
    // (input, url_unescaped, query_unescaped, decoded).
    type Case = (&'static [u8], &'static [u8], &'static [u8], &'static str);
    let cases: &[Case] = &[
        (b"%", b"%", b"%", "%"),
        (b"+", b"+", b" ", "+"),
        (b"%2+", b"%2+", b"%2 ", "%2+"),
        (b"+%%%+%%%", b"+%%%+%%%", b" %%% %%%", "+%%%+%%%"),
        (
            b"Don't escape anything",
            b"Don't escape anything",
            b"Don't escape anything",
            "Don't escape anything",
        ),
        (
            b"+Invalid %escape %2+",
            b"+Invalid %escape %2+",
            b" Invalid %escape %2 ",
            "+Invalid %escape %2+",
        ),
        (
            b"Some random text %25%2dOK",
            b"Some random text %25-OK",
            b"Some random text %25-OK",
            "Some random text %25-OK",
        ),
        (
            b"%01%02%03%04%05%06%07%08%09",
            b"%01%02%03%04%05%06%07%08%09",
            b"%01%02%03%04%05%06%07%08%09",
            "%01%02%03%04%05%06%07%08%09",
        ),
        (
            b"%E4%BD%A0+%E5%A5%BD",
            "\u{4f60}+\u{597d}".as_bytes(),
            "\u{4f60} \u{597d}".as_bytes(),
            "\u{4f60}+\u{597d}",
        ),
        // Invalid UTF-8 -> kept unescaped in the decoded output.
        (b"%ED%ED", b"\xED\xED", b"\xED\xED", "%ED%ED"),
    ];

    for (input, url_unescaped, query_unescaped, decoded) in cases {
        assert_eq!(unescape_url_component(input, UnescapeRule::NORMAL), *url_unescaped);
        assert_eq!(
            unescape_url_component(input, UnescapeRule::REPLACE_PLUS_WITH_SPACE),
            *query_unescaped
        );
        // The adjustments are checked by the next test.
        let (got, _) =
            unescape_and_decode_utf8_url_component_with_adjustments(input, UnescapeRule::NORMAL);
        assert_eq!(got, *decoded);
    }
}

#[test]
fn ported_adjust_offset() {
    // (input, input_offset, output_offset). `None` is C++ `npos`.
    type Case = (&'static [u8], Option<usize>, Option<usize>);
    let cases: &[Case] = &[
        (b"", Some(0), Some(0)),
        (b"test", Some(0), Some(0)),
        (b"test", Some(2), Some(2)),
        (b"test", Some(4), Some(4)),
        (b"test", None, None),
        (b"%2dtest", Some(6), Some(4)),
        (b"%2dtest", Some(3), Some(1)),
        (b"%2dtest", Some(2), None),
        (b"%2dtest", Some(1), None),
        (b"%2dtest", Some(0), Some(0)),
        (b"test%2d", Some(2), Some(2)),
        (b"test%2e", Some(2), Some(2)),
        (b"%E4%BD%A0+%E5%A5%BD", Some(9), Some(1)),
        (b"%E4%BD%A0+%E5%A5%BD", Some(6), None),
        (b"%E4%BD%A0+%E5%A5%BD", Some(0), Some(0)),
        (b"%E4%BD%A0+%E5%A5%BD", Some(10), Some(2)),
        (b"%E4%BD%A0+%E5%A5%BD", Some(19), Some(3)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(18), Some(8)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(15), None),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(9), Some(7)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(19), Some(9)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(28), Some(10)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(0), Some(0)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(2), Some(2)),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(3), None),
        (b"hi%41test%E4%BD%A0+%E5%A5%BD", Some(5), Some(3)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(9), Some(1)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(6), None),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(0), Some(0)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(10), Some(2)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(19), Some(3)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(21), Some(5)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(22), None),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(24), Some(6)),
        (b"%E4%BD%A0+%E5%A5%BDhi%41test", Some(28), Some(10)),
        (b"%ED%B0%80+%E5%A5%BD", Some(6), Some(6)), // not convertible to UTF-8
    ];

    for (input, input_offset, output_offset) in cases {
        let (_, adjustments) =
            unescape_and_decode_utf8_url_component_with_adjustments(input, UnescapeRule::NORMAL);
        let got = offset_adjuster_adjust(&adjustments, *input_offset);
        assert_eq!(
            got,
            *output_offset,
            "input={} offset={input_offset:?}",
            String::from_utf8_lossy(input)
        );
    }
}

/// Thin wrapper so the test reads like the C++ `OffsetAdjuster::AdjustOffset`.
fn offset_adjuster_adjust(adjustments: &[Adjustment], offset: Option<usize>) -> Option<usize> {
    adjust_offset(adjustments, offset, None)
}

#[test]
fn ported_unescape_binary_url_component() {
    type Case = (&'static [u8], UnescapeRule, &'static [u8]);
    let cases: &[Case] = &[
        // Check that ASCII characters with special handling in
        // UnescapeURLComponent() are still unescaped.
        (b"%09%20%25foo%2F", UnescapeRule::NORMAL, b"\x09 %foo/"),
        // UTF-8 Characters banned by UnescapeURLComponent() should also be
        // unescaped.
        (b"Some random text %D8%9COK", UnescapeRule::NORMAL, b"Some random text \xD8\x9COK"),
        (
            b"Some random text %F0%9F%94%8FOK",
            UnescapeRule::NORMAL,
            b"Some random text \xF0\x9F\x94\x8FOK",
        ),
        // As should invalid UTF-8 characters.
        (b"%A0%A0%E9%E9%A0%A0%A0%A0", UnescapeRule::NORMAL, b"\xA0\xA0\xE9\xE9\xA0\xA0\xA0\xA0"),
        // And valid UTF-8 characters that are not banned by
        // UnescapeURLComponent() should be unescaped, too!
        (b"%C2%A1%C2%A1", UnescapeRule::NORMAL, b"\xC2\xA1\xC2\xA1"),
        // '+' should be left alone by default.
        (b"++%2B++", UnescapeRule::NORMAL, b"+++++"),
        // But should magically be turned into a space if requested.
        (b"++%2B++", UnescapeRule::REPLACE_PLUS_WITH_SPACE, b"  +  "),
    ];

    for (input, rules, output) in cases {
        assert_eq!(unescape_binary_url_component(input, *rules), *output);
    }

    // Test NULL character unescaping, which can't be tested above since those
    // are just char pointers in C++.
    let mut input = b"Null".to_vec();
    input.push(0); // Also have a NULL in the input.
    input.extend_from_slice(b"%00%39Test");

    let mut expected = b"Null".to_vec();
    expected.push(0);
    expected.push(0);
    expected.extend_from_slice(b"9Test");
    assert_eq!(unescape_binary_url_component(&input, UnescapeRule::NORMAL), expected);
}

#[test]
fn ported_unescape_binary_url_component_safe() {
    // (input, expected output, has_path_separators). A `None` output means the
    // C++ expects failure when `fail_on_path_separators` is false.
    type BinaryCase = (&'static [u8], Option<&'static [u8]>, bool);
    let cases: &[BinaryCase] = &[
        // Spaces, percents, and invalid UTF-8 characters are all successfully
        // unescaped.
        (b"%20%25foo%81", Some(&b" %foo\x81"[..]), false),
        // Characters disallowed unconditionally.
        (b"foo%00", None, false),
        (b"foo%01", None, false),
        (b"foo%0A", None, false),
        (b"foo%0D", None, false),
        (b"foo%1F", None, false),
        // Path separators.
        (b"foo%2F", Some(&b"foo/"[..]), true),
        (b"foo%5C", Some(&b"foo\\"[..]), true),
        // Characters that are considered invalid to escape are ignored if
        // passed in unescaped.
        (b"foo\x01\r/\\", Some(&b"foo\x01\r/\\"[..]), false),
    ];

    for (input, expected_output, has_path_separators) in cases {
        match expected_output {
            None => {
                assert_eq!(unescape_binary_url_component_safe(input, false), None);
                assert_eq!(unescape_binary_url_component_safe(input, true), None);
            }
            Some(expected) => {
                assert_eq!(
                    unescape_binary_url_component_safe(input, false).as_deref(),
                    Some(*expected)
                );
                if *has_path_separators {
                    assert_eq!(unescape_binary_url_component_safe(input, true), None);
                } else {
                    assert_eq!(
                        unescape_binary_url_component_safe(input, true).as_deref(),
                        Some(*expected)
                    );
                }
            }
        }
    }
}

#[test]
fn ported_contains_encoded_bytes() {
    assert!(!contains_encoded_bytes(b"abc/def", b"/\\"));
    assert!(!contains_encoded_bytes(b"abc%2Fdef", b"%"));
    assert!(contains_encoded_bytes(b"abc%252Fdef", b"%"));
    assert!(contains_encoded_bytes(b"abc%2Fdef", b"/\\"));
    assert!(contains_encoded_bytes(b"abc%5Cdef", b"/\\"));
    assert!(contains_encoded_bytes(b"abc%2fdef", b"/\\"));

    // Should be looking for byte values, not UTF-8 character values.
    assert!(contains_encoded_bytes(b"caf%C3%A9", &[0xc3]));
    assert!(!contains_encoded_bytes(b"caf%C3%A9", &[0xe9]));
}

// ---- Extra tests -----------------------------------------------------------

/// Every single-byte `%XX` escape, under each rule combination, against a
/// reference set written out from the C++ rules.
#[test]
fn exhaustive_single_escape_rules() {
    let rule_sets = [
        UnescapeRule::NORMAL,
        UnescapeRule::SPACES | UnescapeRule::NORMAL,
        UnescapeRule::PATH_SEPARATORS,
        UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
        UnescapeRule::SPACES | UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
        UnescapeRule::PATH_SEPARATORS | UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS,
        UnescapeRule::NONE,
    ];
    for rules in rule_sets {
        for c in 0..=u8::MAX {
            let escaped = format!("%{c:02X}");
            let got = unescape_url_component(escaped.as_bytes(), rules);
            let expect_unescaped = if rules == UnescapeRule::NONE {
                // NONE returns the input unchanged.
                false
            } else if c >= 0x80 {
                // A lone high byte is never a complete character. It is
                // unescaped to a raw byte under every other rule, as in C++.
                true
            } else {
                let ascii_default = c.is_ascii_alphanumeric() || b"\"-<>_~".contains(&c);
                let spaces = c == b' ' && rules.intersects(UnescapeRule::SPACES);
                let seps =
                    (c == b'/' || c == b'\\') && rules.intersects(UnescapeRule::PATH_SEPARATORS);
                let special = c > b' '
                    && c != b'/'
                    && c != b'\\'
                    && rules.intersects(UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS);
                ascii_default || spaces || seps || special
            };
            let expected = if expect_unescaped { vec![c] } else { escaped.as_bytes().to_vec() };
            assert_eq!(got, expected, "rules {:#x} byte {c:#04x}", rules.bits());
        }
    }
}

/// Random byte strings are escaped by each function and compared with a
/// per-byte reference predicate written separately from the bitmaps.
#[test]
fn escape_functions_match_reference_predicates() {
    let mut rng = XorShift64::new(0x9E37_79B9_7F4A_7C15);
    for _ in 0..2000 {
        let input = rng.bytes(48);

        let mut unreserved = String::new();
        let mut query = String::new();
        let mut query_no_plus = String::new();
        let mut path = String::new();
        let mut url_encoded = String::new();
        let mut external = String::new();
        let mut non_ascii = String::new();
        let mut non_ascii_pct = String::new();

        for (i, &c) in input.iter().enumerate() {
            let hex = |s: &mut String| s.push_str(&format!("%{c:02X}"));

            // EscapeAllExceptUnreserved.
            if c.is_ascii_alphanumeric() || b"-._~".contains(&c) {
                unreserved.push(char::from(c));
            } else {
                hex(&mut unreserved);
            }

            // EscapeQueryParamValue, with and without plus.
            if c == b' ' {
                query.push('+');
                query_no_plus.push_str("%20");
            } else if c.is_ascii_alphanumeric() || b"!'()*-._~".contains(&c) {
                query.push(char::from(c));
                query_no_plus.push(char::from(c));
            } else {
                hex(&mut query);
                hex(&mut query_no_plus);
            }

            // EscapePath.
            if c <= 0x20 || c >= 0x7F || b"\"#%:<>?[\\]^`{|}".contains(&c) {
                hex(&mut path);
            } else {
                path.push(char::from(c));
            }

            // EscapeUrlEncodedData (plus).
            if c == b' ' {
                url_encoded.push('+');
            } else if c <= 0x20 || c >= 0x7F || b"\"#$%&'+;<=>?@[\\]^`{|}".contains(&c) {
                hex(&mut url_encoded);
            } else {
                url_encoded.push(char::from(c));
            }

            // EscapeExternalHandlerValue. '%' is escaped only when it does not
            // start a valid %XX sequence.
            let valid_percent = c == b'%'
                && i + 2 < input.len()
                && input[i + 1].is_ascii_hexdigit()
                && input[i + 2].is_ascii_hexdigit();
            if valid_percent {
                external.push('%');
            } else if c <= 0x20 || c >= 0x7F || b"\"%<>\\^`{|}".contains(&c) {
                hex(&mut external);
            } else {
                external.push(char::from(c));
            }

            // EscapeNonASCII and EscapeNonASCIIAndPercent.
            if c >= 0x80 {
                hex(&mut non_ascii);
                hex(&mut non_ascii_pct);
            } else if c == b'%' {
                non_ascii.push('%');
                hex(&mut non_ascii_pct);
            } else {
                non_ascii.push(char::from(c));
                non_ascii_pct.push(char::from(c));
            }
        }

        assert_eq!(escape_all_except_unreserved(&input), unreserved);
        assert_eq!(escape_query_param_value(&input, true), query);
        assert_eq!(escape_query_param_value(&input, false), query_no_plus);
        assert_eq!(escape_path(&input), path);
        assert_eq!(escape_url_encoded_data(&input, true), url_encoded);
        assert_eq!(escape_external_handler_value(&input), external);
        assert_eq!(escape_non_ascii(&input), non_ascii);
        assert_eq!(escape_non_ascii_and_percent(&input), non_ascii_pct);
    }
}

/// Escaping and then unescaping in binary mode returns the input.
#[test]
fn escape_then_binary_unescape_round_trips() {
    let mut rng = XorShift64::new(42);
    for _ in 0..2000 {
        let input = rng.bytes(64);
        let all = escape_all_except_unreserved(&input);
        assert_eq!(unescape_binary_url_component(all.as_bytes(), UnescapeRule::NORMAL), input);
        let query = escape_query_param_value(&input, true);
        assert_eq!(
            unescape_binary_url_component(
                query.as_bytes(),
                UnescapeRule::NORMAL | UnescapeRule::REPLACE_PLUS_WITH_SPACE
            ),
            input
        );
    }
}

/// `UnescapeURLComponent` with NONE returns its input. With NORMAL, the output
/// is never longer than the input.
#[test]
fn unescape_url_component_properties() {
    let mut rng = XorShift64::new(7);
    let alphabet: &[&[u8]] = &[
        b"%",
        b"2",
        b"0",
        b"a",
        b"F",
        b"+",
        b"%41",
        b"%E4%BD%A0",
        b"%C2%A1",
        b"%ED",
        b"%80",
        b"%F0%9F%98%80",
        b"x",
        b" ",
        b"%2F",
        b"%00",
        b"%D8%9C",
    ];
    for _ in 0..2000 {
        let mut input = Vec::new();
        for _ in 0..rng.below(12) {
            input.extend_from_slice(alphabet[rng.below(alphabet.len())]);
        }
        assert_eq!(unescape_url_component(&input, UnescapeRule::NONE), input);
        let normal = unescape_url_component(&input, UnescapeRule::NORMAL);
        assert!(normal.len() <= input.len());
        let all = unescape_url_component(
            &input,
            UnescapeRule::SPACES
                | UnescapeRule::PATH_SEPARATORS
                | UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS
                | UnescapeRule::REPLACE_PLUS_WITH_SPACE,
        );
        assert!(all.len() <= input.len());
    }
}

/// Random mixes of tokens whose unescaped forms are known. The adjustments must
/// map every escaped boundary to the matching UTF-16 offset, and map offsets
/// inside an atomic escape to `None`.
#[test]
fn adjustments_match_token_model() {
    struct Token {
        escaped: &'static [u8],
        decoded: &'static str,
        // Escapes that change the string are atomic: offsets inside them have
        // no mapping. Literal text maps offset by offset.
        atomic: bool,
    }
    const TOKENS: &[Token] = &[
        Token { escaped: b"a", decoded: "a", atomic: false },
        Token { escaped: b"%41", decoded: "A", atomic: true },
        Token { escaped: b"%E4%BD%A0", decoded: "\u{4f60}", atomic: true },
        Token { escaped: b"%F0%9F%98%80", decoded: "\u{1F600}", atomic: true },
        Token { escaped: b"%20", decoded: "%20", atomic: false },
        Token { escaped: b"+", decoded: "+", atomic: false },
        Token { escaped: b"%2B", decoded: "%2B", atomic: false },
    ];

    let mut rng = XorShift64::new(0xDEAD_BEEF);
    for _ in 0..1000 {
        let count = 1 + rng.below(8);
        let picked: Vec<&Token> = (0..count).map(|_| &TOKENS[rng.below(TOKENS.len())]).collect();

        let mut escaped = Vec::new();
        let mut decoded = String::new();
        // (escaped start, UTF-16 start, token index into `picked`).
        let mut spans = Vec::new();
        let mut utf16_pos = 0usize;
        for t in &picked {
            spans.push((escaped.len(), utf16_pos));
            escaped.extend_from_slice(t.escaped);
            decoded.push_str(t.decoded);
            utf16_pos += t.decoded.encode_utf16().count();
        }

        let (got_text, adjustments) =
            unescape_and_decode_utf8_url_component_with_adjustments(&escaped, UnescapeRule::NORMAL);
        assert_eq!(got_text, decoded, "escaped {:?}", String::from_utf8_lossy(&escaped));

        for offset in 0..=escaped.len() {
            // Find the token containing `offset`, or the end of the string.
            let expected = if offset == escaped.len() {
                Some(utf16_pos)
            } else {
                let idx = spans.iter().rposition(|&(start, _)| start <= offset).unwrap_or(0);
                let (start, utf16_start) = spans[idx];
                let token = picked[idx];
                if offset == start {
                    Some(utf16_start)
                } else if token.atomic {
                    None
                } else {
                    Some(utf16_start + (offset - start))
                }
            };
            assert_eq!(
                adjust_offset(&adjustments, Some(offset), None),
                expected,
                "escaped {:?} offset {offset}",
                String::from_utf8_lossy(&escaped)
            );
        }
    }
}

/// `utf8_to_utf16` of arbitrary bytes gives the same text as Rust's lossy
/// decoder, which replaces each maximal invalid subsequence with one U+FFFD.
/// This is the same rule as the ICU `CBU8_NEXT` loop.
#[test]
fn invalid_utf8_matches_lossy_decoder() {
    let mut rng = XorShift64::new(0x1234_5678_9ABC_DEF1);
    for _ in 0..5000 {
        let input = rng.bytes(24);
        let (got, _, valid) = crate::utf8::utf8_to_utf16_with_adjustments(&input);
        let lossy = String::from_utf8_lossy(&input);
        assert_eq!(got, lossy, "input {input:02X?}");
        assert_eq!(valid, std::str::from_utf8(&input).is_ok(), "input {input:02X?}");
    }
}

/// Random strings with the HTML specials round-trip through escape and unescape.
#[test]
fn html_escape_round_trips() {
    let mut rng = XorShift64::new(99);
    let alphabet = b"ab&<>\"'; lt;amp;#39;";
    for _ in 0..2000 {
        let len = rng.below(20);
        let text: String =
            (0..len).map(|_| char::from(alphabet[rng.below(alphabet.len())])).collect();
        let escaped = escape_for_html(text.as_bytes());
        let escaped = String::from_utf8(escaped).expect("ASCII output");
        assert_eq!(unescape_for_html(&escaped), text, "text {text:?}");
    }
}

#[test]
fn append_escaped_char_for_html_handles_each_special() {
    let mut out = Vec::new();
    for c in [b'<', b'>', b'&', b'"', b'\'', b'a', 0x80] {
        append_escaped_char_for_html(c, &mut out);
    }
    assert_eq!(out, b"&lt;&gt;&amp;&quot;&#39;a\x80");
}

#[test]
fn escape_ns_url_precursor_matches_nsurl_set() {
    assert_eq!(
        escape_ns_url_precursor(b"a b#%<>[\\]^`{|}\"?:"),
        "a%20b%23%25%3C%3E%5B%5C%5D%5E%60%7B%7C%7D%22?:"
    );
}

#[test]
fn offset_adjustment_helpers() {
    // Collapsing an escape in the middle of a string.
    let adj = [Adjustment::new(2, 3, 1)];
    assert_eq!(adjust_offset(&adj, Some(1), None), Some(1));
    assert_eq!(adjust_offset(&adj, Some(3), None), None);
    assert_eq!(adjust_offset(&adj, Some(5), None), Some(3));
    // The limit applies to the adjusted result.
    assert_eq!(adjust_offset(&adj, Some(5), Some(2)), None);

    // Merge: the first step turns "%41b" into "Ab" (adjustment 0..3 -> 1), and
    // the second step drops the 'b'. Composing the two should map back to the
    // original string.
    let first = [Adjustment::new(0, 3, 1)];
    let mut second = vec![Adjustment::new(1, 1, 0)];
    merge_sequential_adjustments(&first, &mut second);
    assert_eq!(adjust_offset(&second, Some(0), None), Some(0));
    assert_eq!(adjust_offset(&second, Some(3), None), Some(1));
}
