// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Ports of base/strings/string_tokenizer_unittest.cc. Each C++ TEST becomes a
// #[test] in both flavors (&str and &[u16]), using the same inputs and
// expectations.

use crate::*;

fn enc8(s: &str) -> String {
    s.to_string()
}

fn enc16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

macro_rules! tokenizer_unittests {
    ($m:ident, $tok:ident, $enc:ident) => {
        mod $m {
            use super::*;

            #[test]
            fn simple() {
                let input = $enc("this is a test");
                let delims = $enc(" ");
                let mut t = $tok::new(&input, &delims);
                // The start of the string, before any tokens, is a delimiter.
                assert!(t.token_is_delim());

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("this")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("is")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("a")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("test")[..]);

                assert!(!t.get_next());
                // The end of the string, after the last token, is a delimiter.
                assert!(t.token_is_delim());
            }

            #[test]
            fn simple_using_token_view() {
                let input = $enc("this is a test");
                let delims = $enc(" ");
                let mut t = $tok::new(&input, &delims);
                assert!(t.token_is_delim());

                assert_eq!(t.get_next_token_view(), Some(&$enc("this")[..]));
                assert!(!t.token_is_delim());

                assert_eq!(t.get_next_token_view(), Some(&$enc("is")[..]));
                assert!(!t.token_is_delim());

                assert_eq!(t.get_next_token_view(), Some(&$enc("a")[..]));
                assert!(!t.token_is_delim());

                assert_eq!(t.get_next_token_view(), Some(&$enc("test")[..]));
                assert!(!t.token_is_delim());

                assert_eq!(t.get_next_token_view(), None);
                assert!(t.token_is_delim());
            }

            #[test]
            fn reset() {
                let input = $enc("this is a test");
                let delims = $enc(" ");
                let mut t = $tok::new(&input, &delims);

                for _ in 0..2 {
                    assert!(t.token_is_delim());

                    assert!(t.get_next());
                    assert!(!t.token_is_delim());
                    assert_eq!(t.token(), &$enc("this")[..]);

                    assert!(t.get_next());
                    assert!(!t.token_is_delim());
                    assert_eq!(t.token(), &$enc("is")[..]);

                    assert!(t.get_next());
                    assert!(!t.token_is_delim());
                    assert_eq!(t.token(), &$enc("a")[..]);

                    assert!(t.get_next());
                    assert!(!t.token_is_delim());
                    assert_eq!(t.token(), &$enc("test")[..]);

                    assert!(!t.get_next());
                    assert!(t.token_is_delim());

                    t.reset();
                }
            }

            #[test]
            fn ret_delims() {
                let input = $enc("this is a test");
                let delims = $enc(" ");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_DELIMS);
                assert!(t.token_is_delim());

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("this")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("is")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("a")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("test")[..]);

                assert!(!t.get_next());
                assert!(t.token_is_delim());
            }

            #[test]
            fn ret_empty_tokens() {
                let input = $enc("foo='a, b',,bar,,baz,quux");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_EMPTY_TOKENS);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo='a, b'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("baz")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("quux")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn ret_empty_tokens_at_start() {
                let input = $enc(",bar");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_EMPTY_TOKENS);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn ret_empty_tokens_at_end() {
                let input = $enc("bar,");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_EMPTY_TOKENS);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn ret_empty_tokens_both() {
                let input = $enc(",");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_EMPTY_TOKENS);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn ret_empty_tokens_empty() {
                let input = $enc("");
                let delims = $enc(",");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_EMPTY_TOKENS);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn ret_delims_and_empty_tokens() {
                let input = $enc("foo='a, b',,bar,,baz,quux");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_DELIMS | RETURN_EMPTY_TOKENS);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo='a, b'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc(",")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc(",")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc(",")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc(",")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("baz")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc(",")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("quux")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn many_delims() {
                let input = $enc("this: is, a-test");
                let delims = $enc(": ,-");
                let mut t = $tok::new(&input, &delims);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("this")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("is")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("a")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("test")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_header() {
                let input = $enc("Content-Type: text/html ; charset=UTF-8");
                let delims = $enc(": ;=");
                let mut t = $tok::new(&input, &delims);
                t.set_options(RETURN_DELIMS);
                assert!(t.token_is_delim());

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("Content-Type")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(":")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("text/html")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(";")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc(" ")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("charset")[..]);

                assert!(t.get_next());
                assert!(t.token_is_delim());
                assert_eq!(t.token(), &$enc("=")[..]);

                assert!(t.get_next());
                assert!(!t.token_is_delim());
                assert_eq!(t.token(), &$enc("UTF-8")[..]);

                assert!(!t.get_next());
                assert!(t.token_is_delim());
            }

            #[test]
            fn parse_quoted_string() {
                let input = $enc("foo bar 'hello world' baz");
                let delims = $enc(" ");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("'hello world'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("baz")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_quoted_string_malformed() {
                let input = $enc("bar 'hello wo");
                let delims = $enc(" ");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("'hello wo")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_quoted_string_multiple() {
                let input = $enc("bar 'hel\"lo\" wo' baz\"");
                let delims = $enc(" ");
                let quotes = $enc("'\"");
                let mut t = $tok::new(&input, &delims);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("'hel\"lo\" wo'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("baz\"")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_quoted_string_escaped_quotes() {
                let input = $enc("foo 'don\\'t do that'");
                let delims = $enc(" ");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("'don\\'t do that'")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_quoted_string_escaped_quotes2() {
                let input = $enc("foo='a, b', bar");
                let delims = $enc(", ");
                let quotes = $enc("'");
                let mut t = $tok::new(&input, &delims);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo='a, b'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_with_whitespace_no_quotes() {
                let input = $enc("\t\t\t     foo=a,\r\n b,\r\n\t\t\t      bar\t ");
                let delims = $enc(",");
                let mut t =
                    $tok::with_whitespace_policy(&input, &delims, WhitespacePolicy::SkipOver);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo=a")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("b")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(!t.get_next());
            }

            #[test]
            fn parse_with_whitespace_quotes() {
                let input = $enc("\t\t\t     foo='a, b',\t\t\t      bar\t ");
                let delims = $enc(",");
                let quotes = $enc("'");
                let mut t =
                    $tok::with_whitespace_policy(&input, &delims, WhitespacePolicy::SkipOver);
                t.set_quote_chars(&quotes);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("foo='a, b'")[..]);

                assert!(t.get_next());
                assert_eq!(t.token(), &$enc("bar")[..]);

                assert!(!t.get_next());
            }
        }
    };
}

tokenizer_unittests!(str_flavor, StringTokenizer, enc8);
tokenizer_unittests!(u16_flavor, StringTokenizer16, enc16);

// ---------------------------------------------------------------------------
// Extra tests: reference comparisons and randomized checks.
// ---------------------------------------------------------------------------

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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn string_from(&mut self, alphabet: &[char], max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| alphabet[self.below(alphabet.len())]).collect()
    }
}

// Collects all tokens (and delimiter tokens if RETURN_DELIMS is set) as
// (is_delim, text).
fn collect_str(
    input: &str,
    delims: &str,
    quotes: Option<&str>,
    options: u32,
    policy: WhitespacePolicy,
) -> Vec<(bool, String)> {
    let mut t = StringTokenizer::with_whitespace_policy(input, delims, policy);
    t.set_options(options);
    if let Some(q) = quotes {
        t.set_quote_chars(q);
    }
    let mut out = Vec::new();
    while t.get_next() {
        out.push((t.token_is_delim(), t.token().to_string()));
    }
    out
}

#[test]
fn no_quotes_no_options_matches_split_ignoring_empty() {
    // Quick path: no quotes, no options. Expect the std split with empty
    // pieces dropped.
    let alphabet = ['a', 'b', ',', ' ', ';', '\t', 'é'];
    let delim_sets = [",", " ;", "a", ", \t", "é"];
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 20);
        let delims = delim_sets[rng.below(delim_sets.len())];
        let expected: Vec<String> = input
            .split(|c: char| delims.contains(c))
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        let got: Vec<String> =
            collect_str(&input, delims, None, 0, WhitespacePolicy::IncludeInTokens)
                .into_iter()
                .map(|(_, s)| s)
                .collect();
        assert_eq!(got, expected, "input={input:?} delims={delims:?}");
    }
}

#[test]
fn skip_over_no_quotes_matches_split_on_delims_and_whitespace() {
    let alphabet = ['a', 'b', ',', ' ', ';', '\t', '\n', '\r', '\u{0B}', '\u{0C}'];
    let mut rng = XorShift(0x1234_5678_9abc_def1);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 20);
        let delims = ",;";
        let expected: Vec<String> = input
            .split(|c: char| delims.contains(c) || StrUnits::is_ws(c))
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        let got: Vec<String> = collect_str(&input, delims, None, 0, WhitespacePolicy::SkipOver)
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        assert_eq!(got, expected, "input={input:?}");
    }
}

// Helper for the whitespace check in the reference above.
struct StrUnits;
impl StrUnits {
    fn is_ws(c: char) -> bool {
        matches!(c, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ')
    }
}

#[test]
fn empty_tokens_match_std_split() {
    // Full path with RETURN_EMPTY_TOKENS and no quotes: every delimiter
    // separates two (possibly empty) tokens, as with str::split.
    let alphabet = ['a', ',', ';'];
    let mut rng = XorShift(0xdead_beef_cafe_f00d);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 12);
        let expected: Vec<String> = input.split([',', ';']).map(str::to_string).collect();
        let got: Vec<String> =
            collect_str(&input, ",;", None, RETURN_EMPTY_TOKENS, WhitespacePolicy::IncludeInTokens)
                .into_iter()
                .map(|(_, s)| s)
                .collect();
        assert_eq!(got, expected, "input={input:?}");
    }
}

#[test]
fn ret_delims_with_empty_tokens_interleaves_std_split() {
    let alphabet = ['a', ',', ';'];
    let mut rng = XorShift(0x0123_4567_89ab_cdef);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 12);
        let pieces: Vec<&str> = input.split([',', ';']).collect();
        // Rebuild the expected sequence: piece, delim, piece, delim, ... piece.
        // Each delimiter is the char at the boundary.
        let mut expected: Vec<(bool, String)> = Vec::new();
        let delim_chars: Vec<char> = input.chars().filter(|&c| matches!(c, ',' | ';')).collect();
        for (i, p) in pieces.iter().enumerate() {
            expected.push((false, p.to_string()));
            if i < delim_chars.len() {
                expected.push((true, delim_chars[i].to_string()));
            }
        }
        let got = collect_str(
            &input,
            ",;",
            None,
            RETURN_DELIMS | RETURN_EMPTY_TOKENS,
            WhitespacePolicy::IncludeInTokens,
        );
        assert_eq!(got, expected, "input={input:?}");
    }
}

#[test]
fn ret_delims_with_quotes_covers_whole_input() {
    // With RETURN_DELIMS and IncludeInTokens, every character is in a token
    // or a delimiter, so concatenating the results rebuilds the input.
    let alphabet = ['a', ' ', ',', '\'', '"', '\\', 'b', '\t'];
    let mut rng = XorShift(0x5555_aaaa_3333_cccc);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 24);
        let got = collect_str(
            &input,
            ", ",
            Some("'\""),
            RETURN_DELIMS,
            WhitespacePolicy::IncludeInTokens,
        );
        let rebuilt: String = got.into_iter().map(|(_, s)| s).collect();
        assert_eq!(rebuilt, input);
    }
}

#[test]
fn str_and_u16_flavors_agree_on_ascii() {
    let alphabet = ['a', 'b', ',', ' ', '\'', '\\', '\t', '\n', ';'];
    let mut rng = XorShift(0x7777_1111_2222_3333);
    for _ in 0..3000 {
        let input = rng.string_from(&alphabet, 24);
        let input16: Vec<u16> = input.encode_utf16().collect();
        let delims = ", ;";
        let delims16: Vec<u16> = delims.encode_utf16().collect();
        let quotes = "'\\";
        let quotes16: Vec<u16> = quotes.encode_utf16().collect();
        for options in [0, RETURN_DELIMS, RETURN_EMPTY_TOKENS, RETURN_DELIMS | RETURN_EMPTY_TOKENS]
        {
            for with_quotes in [false, true] {
                for policy in [WhitespacePolicy::IncludeInTokens, WhitespacePolicy::SkipOver] {
                    let mut a = StringTokenizer::with_whitespace_policy(&input, delims, policy);
                    let mut b =
                        StringTokenizer16::with_whitespace_policy(&input16, &delims16, policy);
                    a.set_options(options);
                    b.set_options(options);
                    if with_quotes {
                        a.set_quote_chars(quotes);
                        b.set_quote_chars(&quotes16);
                    }
                    loop {
                        let ra = a.get_next();
                        let rb = b.get_next();
                        assert_eq!(ra, rb, "input={input:?} options={options}");
                        if !ra {
                            break;
                        }
                        assert_eq!(a.token_is_delim(), b.token_is_delim());
                        assert_eq!(a.token_begin(), b.token_begin());
                        assert_eq!(a.token_end(), b.token_end());
                        assert_eq!(
                            a.token().encode_utf16().collect::<Vec<u16>>(),
                            b.token().to_vec()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn non_ascii_text_with_ascii_delims() {
    let input = "héllo, wörld,日本";
    let got: Vec<&str> = StringTokenizer::new(input, ",").collect();
    assert_eq!(got, vec!["héllo", " wörld", "日本"]);
    let input16: Vec<u16> = input.encode_utf16().collect();
    let delims16: Vec<u16> = vec![u16::from(b',')];
    let got16: Vec<Vec<u16>> =
        StringTokenizer16::new(&input16, &delims16).map(|t| t.to_vec()).collect();
    let expect16: Vec<Vec<u16>> =
        ["héllo", " wörld", "日本"].iter().map(|s| s.encode_utf16().collect()).collect();
    assert_eq!(got16, expect16);
}

#[test]
fn token_offsets_are_reported() {
    let mut t = StringTokenizer::new("ab  cd", " ");
    assert!(t.get_next());
    assert_eq!((t.token_begin(), t.token_end()), (0, 2));
    assert!(t.get_next());
    assert_eq!((t.token_begin(), t.token_end()), (4, 6));
    assert!(!t.get_next());
}

#[test]
fn iterator_yields_same_tokens_as_get_next() {
    let tokens: Vec<&str> = StringTokenizer::new("a b  c", " ").collect();
    assert_eq!(tokens, vec!["a", "b", "c"]);
}
