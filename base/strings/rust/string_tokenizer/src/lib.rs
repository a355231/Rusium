// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `base::StringTokenizerT`
//! (base/strings/string_tokenizer.h).
//!
//! Splits text on a set of delimiter characters. Optional behaviors: return the
//! delimiters as tokens, return empty tokens, treat a set of quote characters
//! as grouping (delimiters inside quotes are ignored), and skip ASCII
//! whitespace.
//!
//! Two flavors are provided, both generic over [`StringTokenizerT`]:
//! - [`StringTokenizer`] works on `&str`. Its unit is a `char`.
//! - [`StringTokenizer16`] works on `&[u16]`. Its unit is a UTF-16 code unit.
//!
//! Differences from the C++ version:
//! - Tokenizing is done on whole characters for `&str`. The C++ version works
//!   on bytes. For ASCII delimiters and quotes the results are identical. A
//!   non-ASCII delimiter or quote is matched as a whole `char`, so it cannot
//!   split a UTF-8 sequence, which the byte-level C++ code can do.
//! - The delimiter and quote sets are borrowed (`&'a str` / `&'a [u16]`). The
//!   C++ version copies them. The caller must keep them alive for the
//!   tokenizer's lifetime.
//! - The C++ iterator-pair constructor is not ported. Pass a sub-slice of the
//!   text instead.
//! - `token_begin()` and `token_end()` return byte offsets (for `&str`) or code
//!   unit offsets (for `&[u16]`) into the text, not iterators.
//! - `token()` returns a borrowed slice of the text. The C++ `token()` copies
//!   the token into a new string. `token_piece()` is not ported because it is
//!   the same as `token()`.
//! - `GetNextTokenView()` is `get_next_token_view()` and returns `Option<&S>`.
//!   The type also implements `Iterator`, which yields the same tokens.
//! - `Reset()` is `reset()`. Like the C++ code, it does not reset the
//!   `token_is_delim()` flag.
//! - The `Options` enum is replaced by the `RETURN_DELIMS` and
//!   `RETURN_EMPTY_TOKENS` constants (bit flags, as in C++).
//! - `EstimateMemoryUsage` and the deleted temporary-string constructor are not
//!   ported. Rust's borrow checker prevents the dangling-iterator case that
//!   constructor guards against.

mod private {
    pub trait Sealed {}
    impl Sealed for str {}
    impl Sealed for [u16] {}
}

/// The text a tokenizer scans. Implemented for `str` (unit = `char`) and
/// `[u16]` (unit = `u16` code unit). This trait is sealed.
pub trait Units: private::Sealed {
    /// One unit of text.
    type Unit: Copy + Eq;

    /// Returns the unit at byte/code-unit offset `pos`, and the offset just
    /// past it. Returns `None` at the end of the text.
    fn unit_at(&self, pos: usize) -> Option<(Self::Unit, usize)>;

    /// Returns true if `unit` is one of the characters in `self`.
    fn contains_unit(&self, unit: Self::Unit) -> bool;

    /// Converts an ASCII byte to a unit.
    fn ascii(byte: u8) -> Self::Unit;

    /// Returns true if `unit` is in the ASCII whitespace set
    /// `\t \n \v \f \r space`, the set from `kWhitespaceASCII`.
    fn is_ascii_whitespace(unit: Self::Unit) -> bool;

    /// Returns the sub-text between two offsets.
    fn sub(&self, start: usize, end: usize) -> &Self;
}

impl Units for str {
    type Unit = char;

    fn unit_at(&self, pos: usize) -> Option<(char, usize)> {
        let c = self.get(pos..)?.chars().next()?;
        Some((c, pos + c.len_utf8()))
    }

    fn contains_unit(&self, unit: char) -> bool {
        self.contains(unit)
    }

    fn ascii(byte: u8) -> char {
        char::from(byte)
    }

    fn is_ascii_whitespace(unit: char) -> bool {
        matches!(unit, '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ')
    }

    fn sub(&self, start: usize, end: usize) -> &str {
        &self[start..end]
    }
}

impl Units for [u16] {
    type Unit = u16;

    fn unit_at(&self, pos: usize) -> Option<(u16, usize)> {
        let u = *self.get(pos)?;
        Some((u, pos + 1))
    }

    fn contains_unit(&self, unit: u16) -> bool {
        self.contains(&unit)
    }

    fn ascii(byte: u8) -> u16 {
        u16::from(byte)
    }

    fn is_ascii_whitespace(unit: u16) -> bool {
        matches!(unit, 0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20)
    }

    fn sub(&self, start: usize, end: usize) -> &[u16] {
        &self[start..end]
    }
}

/// Policy for ASCII whitespace characters. Mirrors
/// `StringTokenizerT::WhitespacePolicy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhitespacePolicy {
    /// Whitespace is an ordinary non-delimiter character.
    #[default]
    IncludeInTokens,
    /// Whitespace is skipped and never returned as part of a token. It also
    /// separates tokens. Inside quotes it is kept.
    SkipOver,
}

/// Option bit: return delimiters as tokens. Mirrors `RETURN_DELIMS`.
pub const RETURN_DELIMS: u32 = 1 << 0;
/// Option bit: return empty tokens. Mirrors `RETURN_EMPTY_TOKENS`.
pub const RETURN_EMPTY_TOKENS: u32 = 1 << 1;

/// Scratch state for the quote-aware scanner. Matches `AdvanceState`.
struct AdvanceState<U> {
    in_quote: bool,
    in_escape: bool,
    quote_char: U,
}

/// Tokenizer over text `S`. Use [`StringTokenizer`] or [`StringTokenizer16`].
pub struct StringTokenizerT<'a, S: Units + ?Sized> {
    text: &'a S,
    start_pos: usize,
    token_begin: usize,
    token_end: usize,
    delims: &'a S,
    quotes: Option<&'a S>,
    options: u32,
    token_is_delim: bool,
    whitespace_policy: WhitespacePolicy,
}

/// Tokenizer over `&str`.
pub type StringTokenizer<'a> = StringTokenizerT<'a, str>;
/// Tokenizer over `&[u16]`.
pub type StringTokenizer16<'a> = StringTokenizerT<'a, [u16]>;

impl<'a, S: Units + ?Sized> StringTokenizerT<'a, S> {
    /// Creates a tokenizer that keeps whitespace in tokens.
    pub fn new(text: &'a S, delims: &'a S) -> Self {
        Self::with_whitespace_policy(text, delims, WhitespacePolicy::default())
    }

    /// Creates a tokenizer with an explicit whitespace policy.
    pub fn with_whitespace_policy(
        text: &'a S,
        delims: &'a S,
        whitespace_policy: WhitespacePolicy,
    ) -> Self {
        Self {
            text,
            start_pos: 0,
            token_begin: 0,
            token_end: 0,
            delims,
            quotes: None,
            options: 0,
            token_is_delim: true,
            whitespace_policy,
        }
    }

    /// Sets the option bits: [`RETURN_DELIMS`] and [`RETURN_EMPTY_TOKENS`].
    pub fn set_options(&mut self, options: u32) {
        self.options = options;
    }

    /// Sets the quote characters. Inside quotes, delimiters are ignored, and a
    /// backslash makes the next character literal.
    pub fn set_quote_chars(&mut self, quotes: &'a S) {
        self.quotes = Some(quotes);
    }

    /// Advances to the next token. Returns `None` when the text is exhausted.
    pub fn get_next_token_view(&mut self) -> Option<&'a S> {
        if self.get_next() {
            Some(self.token())
        } else {
            None
        }
    }

    /// Advances to the next token or delimiter. Returns false when exhausted.
    pub fn get_next(&mut self) -> bool {
        if self.quotes_empty() && self.options == 0 {
            self.quick_get_next()
        } else {
            self.full_get_next()
        }
    }

    /// Restarts iteration from the beginning of the text.
    pub fn reset(&mut self) {
        self.token_end = self.start_pos;
    }

    /// Returns true if the current token is a delimiter. Before the first
    /// `get_next()` and after it returns false, this is true.
    pub fn token_is_delim(&self) -> bool {
        self.token_is_delim
    }

    /// Byte (or code-unit) offset where the current token starts.
    pub fn token_begin(&self) -> usize {
        self.token_begin
    }

    /// Byte (or code-unit) offset just past the current token.
    pub fn token_end(&self) -> usize {
        self.token_end
    }

    /// The current token, borrowed from the text.
    pub fn token(&self) -> &'a S {
        let text: &'a S = self.text;
        text.sub(self.token_begin, self.token_end)
    }

    fn quotes_empty(&self) -> bool {
        self.quotes.is_none_or(|q| q.unit_at(0).is_none())
    }

    fn is_quote(&self, unit: S::Unit) -> bool {
        self.quotes.is_some_and(|q| q.contains_unit(unit))
    }

    fn should_skip(&self, unit: S::Unit) -> bool {
        self.whitespace_policy == WhitespacePolicy::SkipOver && S::is_ascii_whitespace(unit)
    }

    fn skip_whitespace(&mut self) {
        let text = self.text;
        while let Some((c, next)) = text.unit_at(self.token_end) {
            if !self.should_skip(c) {
                break;
            }
            self.token_end = next;
        }
    }

    // Scanner used when there are no quote characters and no options. Mirrors
    // QuickGetNext().
    fn quick_get_next(&mut self) -> bool {
        let text = self.text;
        self.token_is_delim = false;
        loop {
            self.token_begin = self.token_end;
            let Some((c, next)) = text.unit_at(self.token_end) else {
                self.token_is_delim = true;
                return false;
            };
            self.token_end = next;
            if !self.delims.contains_unit(c) && !self.should_skip(c) {
                break;
            }
            // Otherwise skip over the delimiter or skippable character.
        }
        while let Some((c, next)) = text.unit_at(self.token_end) {
            if self.delims.contains_unit(c) || self.should_skip(c) {
                break;
            }
            self.token_end = next;
        }
        true
    }

    // Scanner used when quotes or options are set. Mirrors FullGetNext().
    fn full_get_next(&mut self) -> bool {
        let text = self.text;
        let mut state = AdvanceState { in_quote: false, in_escape: false, quote_char: S::ascii(0) };

        self.skip_whitespace();
        loop {
            if self.token_is_delim {
                // The previous token was a delimiter (also true at the start).
                // The next token is a non-delimiter, which may be empty.
                self.token_is_delim = false;
                self.token_begin = self.token_end;

                // Slurp all non-delimiter characters into the token.
                while let Some((c, next)) = text.unit_at(self.token_end) {
                    if !self.advance_one(&mut state, c) {
                        break;
                    }
                    self.token_end = next;
                }

                // Return the token if it is non-empty, or if empty tokens were
                // requested.
                if self.token_begin != self.token_end || (self.options & RETURN_EMPTY_TOKENS) != 0 {
                    return true;
                }
            }

            // The previous token was a regular token. The next token is a
            // delimiter. The end of the text also acts as one.
            self.token_is_delim = true;
            self.token_begin = self.token_end;

            let Some((_, next)) = text.unit_at(self.token_end) else {
                return false;
            };

            // Consume the delimiter.
            self.token_end = next;
            if (self.options & RETURN_DELIMS) != 0 {
                return true;
            }
        }
    }

    // Returns true if the character is not a delimiter, or, depending on the
    // whitespace policy, is not whitespace. Mirrors AdvanceOne().
    fn advance_one(&self, state: &mut AdvanceState<S::Unit>, c: S::Unit) -> bool {
        if state.in_quote {
            if state.in_escape {
                state.in_escape = false;
            } else if c == S::ascii(b'\\') {
                state.in_escape = true;
            } else if c == state.quote_char {
                state.in_quote = false;
            }
        } else {
            if self.delims.contains_unit(c) || self.should_skip(c) {
                return false;
            }
            state.quote_char = c;
            state.in_quote = self.is_quote(c);
        }
        true
    }
}

impl<'a, S: Units + ?Sized> Iterator for StringTokenizerT<'a, S> {
    type Item = &'a S;

    fn next(&mut self) -> Option<&'a S> {
        self.get_next_token_view()
    }
}

#[cfg(test)]
mod tests;
