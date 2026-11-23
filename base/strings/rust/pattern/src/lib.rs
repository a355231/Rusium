// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::MatchPattern` (base/strings/pattern.{h,cc}).
//!
//! Glob-style matching: `*` matches any run of characters (including none),
//! `?` matches zero or one character, and `\` escapes the following `*`, `?`
//! or backslash. Matching is case-sensitive and anchored at both ends.
//!
//! The C++ overloads for `std::string_view` (UTF-8) and `std::u16string_view`
//! (UTF-16) are ported as [`match_pattern_utf8`] and [`match_pattern_utf16`].
//! Both take slices rather than `&str`, because the C++ behavior for invalid
//! UTF-8 is part of the contract and `&str` cannot represent it.
//!
//! The matching algorithm is the same greedy search as the C++ code (leftmost
//! literal placement, restarted after each mismatch). Results were checked
//! against the C++ implementation on random inputs, including invalid UTF-8
//! and lone surrogates.
//!
//! Differences from the C++ version:
//! - The C++ `int` count of `?` characters in a run is saturated instead of
//!   overflowing (signed overflow is undefined behavior in C++). This needs a
//!   pattern with more than `i32::MAX` consecutive `?` characters.
//! - The UTF-8 and UTF-16 decoders are written in safe Rust, but mirror the
//!   Chromium `CBU8_NEXT` / `CBU16_NEXT` macros from
//!   base/third_party/icu/icu_utf.h exactly, including how far an ill-formed
//!   UTF-8 sequence advances the index. Ill-formed UTF-8 decodes to a sentinel
//!   that never compares equal to anything, as in C++. Unpaired UTF-16
//!   surrogates decode to their own code unit, as in C++.
//! - Matching is greedy and does not backtrack past a `?`. For example
//!   `"*a?"` does not match `"baaa"`, and `"?a?a??"` does not match
//!   `"baaaab"`, although a backtracking matcher would accept both. C++
//!   behaves the same way. Patterns whose only wildcard is `*` always give
//!   the right answer.
//! - The C++ `escape` flag is not reset when a literal search restarts after a
//!   mismatch. So a pattern ending in an unpaired `\` can then compare that
//!   backslash as a literal, e.g. `"ba"` does not match `"b??\"`. This is
//!   kept for compatibility with C++.
//! - The `constexpr` / `UNSAFE_TODO` annotations are not ported.
//! - The libFuzzer harness in base/strings/pattern_fuzzer.cc is not ported.

#![forbid(unsafe_code)]

/// The code unit type a matcher works on (`u8` for UTF-8, `u16` for UTF-16).
trait Unit: Copy + Eq {
    const ASTERISK: Self;
    const QUESTION: Self;
    const BACKSLASH: Self;
}

impl Unit for u8 {
    const ASTERISK: Self = b'*';
    const QUESTION: Self = b'?';
    const BACKSLASH: Self = b'\\';
}

impl Unit for u16 {
    const ASTERISK: Self = b'*' as u16;
    const QUESTION: Self = b'?' as u16;
    const BACKSLASH: Self = b'\\' as u16;
}

/// Same value as the C++ `CBU_SENTINEL`: returned for ill-formed input.
const SENTINEL: i32 = -1;

/// Decodes the character at `*index` and advances `*index` past it.
type NextFn<U> = fn(&[U], &mut usize) -> i32;

fn is_wildcard<U: Unit>(unit: U) -> bool {
    unit == U::ASTERISK || unit == U::QUESTION
}

/// Searches for the next literal subpattern of `pattern` (starting at
/// `*pattern_index`) in `string` (starting at `*string_index`), placing it at
/// most `maximum_distance` characters past the current position. The
/// subpattern runs up to the first unescaped wildcard or the end of the
/// pattern. A negative `maximum_distance` means the distance is unlimited.
///
/// Returns true if the subpattern matched. On success the indices point just
/// past the matched text; on failure their values are unspecified.
fn search_for_chars<U: Unit>(
    pattern: &[U],
    pattern_index: &mut usize,
    string: &[U],
    string_index: &mut usize,
    mut maximum_distance: i32,
    next: NextFn<U>,
) -> bool {
    let pattern_start = *pattern_index;
    let mut string_start = *string_index;
    let mut escape = false;
    loop {
        if *pattern_index == pattern.len() {
            // At the end of the pattern only the end of the string matches;
            // anything else falls through to the mismatch case.
            if *string_index == string.len() {
                return true;
            }
        } else {
            // A wildcard ends the literal part.
            if !escape && is_wildcard(pattern[*pattern_index]) {
                return true;
            }
            // Skip an escape character and take the next character literally.
            if !escape && pattern[*pattern_index] == U::BACKSLASH {
                escape = true;
                next(pattern, pattern_index);
                continue;
            }
            escape = false;

            if *string_index == string.len() {
                return false;
            }

            let mut pattern_next = *pattern_index;
            let mut string_next = *string_index;
            let pattern_char = next(pattern, &mut pattern_next);
            if pattern_char == next(string, &mut string_next) && pattern_char != SENTINEL {
                *pattern_index = pattern_next;
                *string_index = string_next;
                continue;
            }
        }

        // Mismatch. Give up once the allowed distance is used up; otherwise
        // restart the literal at the next character of `string`.
        if maximum_distance == 0 {
            return false;
        }
        // A negative distance (unlimited) never reaches 0 here.
        maximum_distance -= 1;
        *pattern_index = pattern_start;
        next(string, &mut string_start);
        *string_index = string_start;
    }
}

/// Consumes consecutive `?` and `*` characters at `*pattern_index`. Returns the
/// maximum number of characters they can match: the number of `?` if there is
/// no `*`, or -1 (unlimited) if there is at least one `*`.
fn eat_wildcards<U: Unit>(pattern: &[U], pattern_index: &mut usize, next: NextFn<U>) -> i32 {
    let mut question_marks: i32 = 0;
    let mut has_asterisk = false;
    while *pattern_index != pattern.len() {
        let unit = pattern[*pattern_index];
        if unit == U::QUESTION {
            question_marks = question_marks.saturating_add(1);
        } else if unit == U::ASTERISK {
            has_asterisk = true;
        } else {
            break;
        }
        next(pattern, pattern_index);
    }
    if has_asterisk {
        -1
    } else {
        question_marks
    }
}

fn match_pattern_impl<U: Unit>(eval: &[U], pattern: &[U], next: NextFn<U>) -> bool {
    let mut pattern_index = 0;
    let mut eval_index = 0;
    // The C++ loop is a do-while, so the first iteration always runs, even for
    // an empty pattern.
    loop {
        let maximum_wildcard_length = eat_wildcards(pattern, &mut pattern_index, next);
        if !search_for_chars(
            pattern,
            &mut pattern_index,
            eval,
            &mut eval_index,
            maximum_wildcard_length,
            next,
        ) {
            return false;
        }
        if pattern_index == pattern.len() {
            return true;
        }
    }
}

/// Decodes one UTF-8 character at `*index`, mirroring `CBU8_NEXT`.
///
/// Returns the code point, or [`SENTINEL`] for an ill-formed sequence. The
/// index advances by the same amount as the C++ macro. In particular, a bad
/// lead byte consumes one byte and a truncated or bad sequence consumes the
/// bytes it inspected and accepted.
fn next_utf8(s: &[u8], index: &mut usize) -> i32 {
    // Indexed by `lead & 0xf` (for leads E0..EF): bit `t1 >> 5` says whether
    // the first trail byte `t1` is valid after that lead byte.
    const LEAD3_T1_BITS: [u8; 16] = [
        0x20, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x10, 0x30,
        0x30,
    ];
    // Indexed by `t1 >> 4`: bit `lead - 0xf0` says whether the lead byte
    // F0..F4 accepts the first trail byte `t1`.
    const LEAD4_T1_BITS: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0x0f, 0x0f, 0x0f, 0x00, 0x00, 0x00,
        0x00,
    ];

    let len = s.len();
    let lead = s[*index];
    *index += 1;
    if lead < 0x80 {
        return i32::from(lead);
    }
    if *index == len {
        return SENTINEL;
    }

    let mut c = u32::from(lead);
    if (0xe0..0xf0).contains(&lead) {
        // Three-byte sequence (U+0800..U+FFFF, excluding surrogates).
        c &= 0xf;
        let t1 = s[*index];
        if (LEAD3_T1_BITS[c as usize] & (1u8 << (t1 >> 5))) == 0 {
            return SENTINEL;
        }
        c = (c << 6) | u32::from(t1 & 0x3f);
        *index += 1;
        if *index == len {
            return SENTINEL;
        }
    } else if lead >= 0xf0 {
        // Four-byte sequence (U+10000..U+10FFFF).
        c -= 0xf0;
        let t1 = s[*index];
        if c > 4 || (LEAD4_T1_BITS[usize::from(t1 >> 4)] & (1u8 << c)) == 0 {
            return SENTINEL;
        }
        c = (c << 6) | u32::from(t1 & 0x3f);
        *index += 1;
        if *index == len {
            return SENTINEL;
        }
        let t2 = s[*index].wrapping_sub(0x80);
        if t2 > 0x3f {
            return SENTINEL;
        }
        c = (c << 6) | u32::from(t2);
        *index += 1;
        if *index == len {
            return SENTINEL;
        }
    } else if lead >= 0xc2 {
        // Two-byte sequence (U+0080..U+07FF).
        c &= 0x1f;
    } else {
        return SENTINEL;
    }

    // The final trail byte, which every multi-byte sequence ends with.
    let last = s[*index].wrapping_sub(0x80);
    if last > 0x3f {
        return SENTINEL;
    }
    c = (c << 6) | u32::from(last);
    *index += 1;
    c as i32
}

/// Decodes one UTF-16 code point at `*index`, mirroring `CBU16_NEXT`.
///
/// Unpaired surrogates are returned as their own code unit value.
fn next_utf16(s: &[u16], index: &mut usize) -> i32 {
    let unit = s[*index];
    *index += 1;
    if (0xd800..0xdc00).contains(&unit) && *index != s.len() {
        let trail = s[*index];
        if (0xdc00..0xe000).contains(&trail) {
            *index += 1;
            // Same as CBU16_GET_SUPPLEMENTARY: (lead << 10) + trail - offset.
            const SURROGATE_OFFSET: i32 = (0xd800 << 10) + 0xdc00 - 0x10000;
            return (i32::from(unit) << 10) + i32::from(trail) - SURROGATE_OFFSET;
        }
    }
    i32::from(unit)
}

/// Returns true if the UTF-8 `string` matches the glob `pattern`.
///
/// `?` matches zero or one character and `*` matches any run of characters.
/// `\` escapes `*`, `?` and `\`. Ill-formed UTF-8 is treated as one invalid
/// character, which matches `?` and `*` but never a literal.
///
/// Both arguments are byte slices rather than `&str` so that invalid UTF-8
/// can be matched, as the C++ `std::string_view` overload allows.
///
/// # Examples
///
/// ```
/// use pattern::match_pattern_utf8;
/// assert!(match_pattern_utf8(b"www.google.com", b"*.com"));
/// assert!(!match_pattern_utf8(b"www.msn.com", b"*.COM"));
/// ```
pub fn match_pattern_utf8(string: &[u8], pattern: &[u8]) -> bool {
    match_pattern_impl(string, pattern, next_utf8)
}

/// Returns true if the UTF-16 `string` matches the glob `pattern`. The
/// wildcard and escape rules are the same as for [`match_pattern_utf8`].
///
/// # Examples
///
/// ```
/// use pattern::match_pattern_utf16;
/// let s: Vec<u16> = "Hello*1234".encode_utf16().collect();
/// let p: Vec<u16> = "He??o\\*1*".encode_utf16().collect();
/// assert!(match_pattern_utf16(&s, &p));
/// ```
pub fn match_pattern_utf16(string: &[u16], pattern: &[u16]) -> bool {
    match_pattern_impl(string, pattern, next_utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    // Ports of MatchPatternTest in base/strings/pattern_unittest.cc.
    #[test]
    fn match_pattern_test() {
        assert!(match_pattern_utf8(b"www.google.com", b"*.com"));
        assert!(match_pattern_utf8(b"www.google.com", b"*"));
        assert!(!match_pattern_utf8(b"www.google.com", b"www*.g*.org"));
        assert!(match_pattern_utf8(b"Hello", b"H?l?o"));
        assert!(!match_pattern_utf8(b"www.google.com", b"http://*)"));
        assert!(!match_pattern_utf8(b"www.msn.com", b"*.COM"));
        assert!(match_pattern_utf8(b"Hello*1234", b"He??o\\*1*"));
        assert!(!match_pattern_utf8(b"", b"*.*"));
        assert!(match_pattern_utf8(b"", b"*"));
        assert!(match_pattern_utf8(b"", b"?"));
        assert!(match_pattern_utf8(b"", b""));
        assert!(!match_pattern_utf8(b"Hello", b""));
        assert!(match_pattern_utf8(b"Hello*", b"Hello*"));
        assert!(match_pattern_utf8(b"abcd", b"*???"));
        assert!(!match_pattern_utf8(b"abcd", b"???"));
        assert!(match_pattern_utf8(b"abcb", b"a*b"));
        assert!(!match_pattern_utf8(b"abcb", b"a?b"));

        // Test UTF8 matching.
        assert!(match_pattern_utf8(b"heart: \xe2\x99\xa0", b"*\xe2\x99\xa0"));
        assert!(match_pattern_utf8(b"heart: \xe2\x99\xa0.", b"heart: ?."));
        assert!(match_pattern_utf8(b"hearts: \xe2\x99\xa0\xe2\x99\xa0", b"*"));
        // Invalid sequences should be handled as a single invalid character.
        assert!(match_pattern_utf8(b"invalid: \xef\xbf\xbe", b"invalid: ?"));
        // If the pattern has invalid characters, it shouldn't match anything.
        assert!(!match_pattern_utf8(b"\xf4\x90\x80\x80", b"\xf4\x90\x80\x80"));

        // Test UTF16 character matching.
        assert!(match_pattern_utf16(&u16s("www.google.com"), &u16s("*.com")));
        assert!(match_pattern_utf16(&u16s("Hello*1234"), &u16s("He??o\\*1*")));

        // Some test cases that might cause naive implementations to exhibit
        // exponential run time or fail.
        assert!(match_pattern_utf8(b"Hello", b"He********************************o"));
        assert!(match_pattern_utf8(b"123456789012345678", b"?????????????????*"));
        assert!(match_pattern_utf8(b"aaaaaaaaaaab", b"a*a*a*a*a*a*a*a*a*a*a*b"));
    }

    // Expected values were produced by running the C++ MatchPattern from
    // base/strings/pattern.cc on each input (see the notes in the crate docs).
    #[test]
    fn matches_cpp_on_edge_cases() {
        let cases: &[(&[u8], &[u8], bool)] = &[
            (b"\xe2\x80", b"?", true),
            (b"\xff", b"?", true),
            (b"\xff", b"\xff", false),
            (b"\xe2\x80*", b"??*", true),
            (b"a\xf0\x9f\x98\x80z", b"a?z", true),
            (b"a\xf0\x9f\x98\x80z", b"a??z", true),
            (b"a\xf0\x9f\x98\x80z", b"a?*z", true),
            (b"abc\\", b"abc", false),
            (b"abc", b"abc\\", true),
            (b"a*", b"a\\*", true),
            (b"a", b"\\", false),
            (b"\\", b"\\\\", true),
            (b"ab", b"a?", true),
            (b"a", b"a?", true),
            (b"a", b"a??", true),
            (b"xab", b"?ab", true),
            (b"xxab", b"?ab", false),
            (b"\xed\xa0\x80", b"?", false),
            (b"\xc0\x80", b"?", false),
            (b"\xf0\x9f\x98", b"??", true),
            (b"xy", b"x*?y", true),
        ];
        for &(string, pattern, expected) in cases {
            assert_eq!(
                match_pattern_utf8(string, pattern),
                expected,
                "string={string:x?} pattern={pattern:x?}"
            );
        }
    }

    /// Reference matcher: `?` is "zero or one character", `*` is "any run",
    /// and `\` escapes the next byte. A trailing `\` matches nothing. Works
    /// on ASCII input only. Computes the set of reachable positions per token.
    fn reference_match(string: &[u8], pattern: &[u8]) -> bool {
        #[derive(Clone, Copy)]
        enum Tok {
            Lit(u8),
            OneOrNone,
            AnyRun,
        }
        let mut toks = Vec::new();
        let mut i = 0;
        while i < pattern.len() {
            match pattern[i] {
                b'?' => toks.push(Tok::OneOrNone),
                b'*' => toks.push(Tok::AnyRun),
                b'\\' => {
                    if i + 1 < pattern.len() {
                        toks.push(Tok::Lit(pattern[i + 1]));
                        i += 1;
                    }
                }
                c => toks.push(Tok::Lit(c)),
            }
            i += 1;
        }
        let n = string.len();
        let mut reach = vec![false; n + 1];
        reach[0] = true;
        for tok in toks {
            let mut next = vec![false; n + 1];
            for pos in 0..=n {
                if !reach[pos] {
                    continue;
                }
                match tok {
                    Tok::Lit(c) => {
                        if pos < n && string[pos] == c {
                            next[pos + 1] = true;
                        }
                    }
                    Tok::OneOrNone => {
                        next[pos] = true;
                        if pos < n {
                            next[pos + 1] = true;
                        }
                    }
                    Tok::AnyRun => {
                        for slot in next.iter_mut().skip(pos) {
                            *slot = true;
                        }
                    }
                }
            }
            reach = next;
        }
        reach[n]
    }

    #[test]
    fn randomized_ascii_matches_reference() {
        // xorshift64; no external crates.
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next_rand = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        // The greedy search is exact for patterns whose only wildcard is `*`
        // (see the crate docs); `?` can need backtracking, so it is not
        // covered by this reference.
        const ALPHABET: &[u8] = b"ab*\\";
        let mut positives = 0;
        for _ in 0..20000 {
            let string_len = (next_rand() % 8) as usize;
            let pattern_len = (next_rand() % 7) as usize;
            let string: Vec<u8> =
                (0..string_len).map(|_| b"ab"[(next_rand() % 2) as usize]).collect();
            let mut pattern: Vec<u8> = (0..pattern_len)
                .map(|_| ALPHABET[(next_rand() % ALPHABET.len() as u64) as usize])
                .collect();
            // A trailing unpaired backslash has the quirk described in the
            // crate docs; it is covered by its own test.
            while pattern.last() == Some(&b'\\') {
                pattern.pop();
            }
            let expected = reference_match(&string, &pattern);
            assert_eq!(
                match_pattern_utf8(&string, &pattern),
                expected,
                "string={:?} pattern={:?}",
                String::from_utf8_lossy(&string),
                String::from_utf8_lossy(&pattern)
            );
            positives += usize::from(expected);
        }
        // Make sure the test is not trivially all-false.
        assert!(positives > 1000);
    }

    #[test]
    fn utf16_matches_utf8_on_bmp_text() {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next_rand = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        // Characters in the BMP, but not surrogates or `*`, `?`, `\`.
        const CHARS: &[char] = &['a', 'b', '\u{e9}', '\u{2660}', '\u{ffff}'];
        for _ in 0..2000 {
            let string: String = (0..(next_rand() % 6))
                .map(|_| CHARS[(next_rand() % CHARS.len() as u64) as usize])
                .collect();
            let pattern: String = (0..(next_rand() % 5))
                .map(|_| {
                    let r = next_rand() % 8;
                    match r {
                        0 => '*',
                        1 => '?',
                        _ => CHARS[(r as usize) % CHARS.len()],
                    }
                })
                .collect();
            let wide_s: Vec<u16> = string.encode_utf16().collect();
            let wide_p: Vec<u16> = pattern.encode_utf16().collect();
            assert_eq!(
                match_pattern_utf8(string.as_bytes(), pattern.as_bytes()),
                match_pattern_utf16(&wide_s, &wide_p),
                "string={string:?} pattern={pattern:?}"
            );
        }
    }

    #[test]
    fn greedy_search_does_not_backtrack() {
        // These are the same results as C++ (verified against
        // base/strings/pattern.cc), even though a backtracking matcher would
        // say true.
        assert!(match_pattern_utf8(b"baaa", b"*a"));
        assert!(!match_pattern_utf8(b"baaa", b"*a?"));
        assert!(!match_pattern_utf8(b"baaaab", b"?a?a??"));
    }

    #[test]
    fn trailing_backslash_restart_quirk() {
        // Same as C++: the escape state survives the restart, so the final
        // backslash is compared literally against the string.
        assert!(!match_pattern_utf8(b"ba", b"b??\\"));
        assert!(match_pattern_utf8(b"b\\", b"b\\\\"));
    }

    #[test]
    fn lone_surrogates_match_literally() {
        // Unpaired surrogates are their own code units, so they match only
        // themselves and are matched by `?`.
        assert!(match_pattern_utf16(&[0xd800], &[0xd800]));
        assert!(!match_pattern_utf16(&[0xd800], &[0xdc00]));
        assert!(match_pattern_utf16(&[0xdc00, 0xd800], &[0x3f, 0xd800]));
        // A well-formed pair is one code point, so one `?` covers it.
        assert!(match_pattern_utf16(&[0xd83d, 0xde00], &[0x3f]));
        assert!(!match_pattern_utf16(&[0xd83d, 0xde00], &[0xd83d]));
    }
}
