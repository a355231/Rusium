// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Trimming, whitespace collapsing, and ContainsOnlyChars from
//! base/strings/string_util.h.

use crate::code_unit::{
    is_ascii_whitespace, is_unicode_whitespace, CodeUnit, WHITESPACE_ASCII, WHITESPACE_UTF16,
};

/// C++ `TrimPositions`: which ends of a string to trim, and, as a result, which
/// ends were actually trimmed.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TrimPositions {
    /// Trim nothing (C++ `TRIM_NONE`).
    #[default]
    None,
    /// Trim the start only (C++ `TRIM_LEADING`).
    Leading,
    /// Trim the end only (C++ `TRIM_TRAILING`).
    Trailing,
    /// Trim both ends (C++ `TRIM_ALL`).
    All,
}

impl TrimPositions {
    /// Builds a value from the two C++ bit flags.
    pub fn from_flags(leading: bool, trailing: bool) -> Self {
        match (leading, trailing) {
            (false, false) => TrimPositions::None,
            (true, false) => TrimPositions::Leading,
            (false, true) => TrimPositions::Trailing,
            (true, true) => TrimPositions::All,
        }
    }

    /// True when the start is (or was) trimmed (C++ `positions & TRIM_LEADING`).
    pub fn trims_leading(self) -> bool {
        matches!(self, TrimPositions::Leading | TrimPositions::All)
    }

    /// True when the end is (or was) trimmed (C++ `positions & TRIM_TRAILING`).
    pub fn trims_trailing(self) -> bool {
        matches!(self, TrimPositions::Trailing | TrimPositions::All)
    }
}

/// Returns the `(start, end, trimmed)` bounds of `input` after trimming
/// `trim_chars` from the requested ends. When the whole input is trimmed, the
/// range is empty and `trimmed` is the requested `positions`, as in C++
/// `TrimStringT`. Empty input reports `TrimPositions::None`.
fn trim_bounds<T: CodeUnit>(
    input: &[T],
    trim_chars: &[T],
    positions: TrimPositions,
) -> (usize, usize, TrimPositions) {
    if input.is_empty() {
        return (0, 0, TrimPositions::None);
    }
    let is_trim = |c: &T| trim_chars.contains(c);
    let first = if positions.trims_leading() {
        input.iter().position(|c| !is_trim(c))
    } else {
        Some(0)
    };
    let last = if positions.trims_trailing() {
        input.iter().rposition(|c| !is_trim(c))
    } else {
        Some(input.len() - 1)
    };
    match (first, last) {
        (Some(f), Some(l)) => {
            let trimmed = TrimPositions::from_flags(f != 0, l != input.len() - 1);
            (f, l + 1, trimmed)
        }
        _ => (0, 0, positions),
    }
}

/// C++ `TrimString(std::string_view, trim_chars, TrimPositions)` and the
/// `u16string_view` overload. Returns a sub-slice of `input` with `trim_chars`
/// removed from the requested ends.
pub fn trim_string<'a, T: CodeUnit>(
    input: &'a [T],
    trim_chars: &[T],
    positions: TrimPositions,
) -> &'a [T] {
    let (start, end, _) = trim_bounds(input, trim_chars, positions);
    &input[start..end]
}

/// C++ `TrimString(input, trim_chars, output)` and `TrimWhitespace(input,
/// positions, output)`. Returns the trimmed copy and which ends were trimmed.
/// Callers that only need a yes/no answer (the C++ `bool` overloads) can test
/// `!= TrimPositions::None`.
pub fn trim_string_to_vec<T: CodeUnit>(
    input: &[T],
    trim_chars: &[T],
    positions: TrimPositions,
) -> (TrimPositions, Vec<T>) {
    let (start, end, trimmed) = trim_bounds(input, trim_chars, positions);
    (trimmed, input[start..end].to_vec())
}

/// C++ `TrimWhitespace(std::u16string_view, TrimPositions)`: trims Unicode
/// whitespace (`WHITESPACE_UTF16`).
pub fn trim_whitespace(input: &[u16], positions: TrimPositions) -> &[u16] {
    trim_string(input, WHITESPACE_UTF16, positions)
}

/// C++ `TrimWhitespace(u16string_view, TrimPositions, u16string*)`.
pub fn trim_whitespace_to_vec(input: &[u16], positions: TrimPositions) -> (TrimPositions, Vec<u16>) {
    trim_string_to_vec(input, WHITESPACE_UTF16, positions)
}

/// C++ `TrimWhitespaceASCII(std::string_view, TrimPositions)`: trims only the
/// ASCII whitespace set (`WHITESPACE_ASCII`).
pub fn trim_whitespace_ascii(input: &[u8], positions: TrimPositions) -> &[u8] {
    trim_string(input, WHITESPACE_ASCII, positions)
}

/// C++ `TrimWhitespaceASCII(std::string_view, TrimPositions, std::string*)`.
pub fn trim_whitespace_ascii_to_vec(input: &[u8], positions: TrimPositions) -> (TrimPositions, Vec<u8>) {
    trim_string_to_vec(input, WHITESPACE_ASCII, positions)
}

/// `&str` form of [`trim_whitespace_ascii`]. Trimmed bytes are ASCII, so the
/// boundaries always fall on character boundaries.
pub fn trim_whitespace_ascii_str(input: &str, positions: TrimPositions) -> &str {
    let (start, end, _) = trim_bounds(input.as_bytes(), WHITESPACE_ASCII, positions);
    input.get(start..end).unwrap_or("")
}

/// Shared implementation of CollapseWhitespace, generic over the item type.
/// `is_ws` detects whitespace, `is_line_break` detects CR or LF, and `space` is
/// the replacement item.
fn collapse_core<T: Copy>(
    text: impl IntoIterator<Item = T>,
    trim_sequences_with_line_breaks: bool,
    is_ws: impl Fn(T) -> bool,
    is_line_break: impl Fn(T) -> bool,
    space: T,
) -> Vec<T> {
    let mut result: Vec<T> = Vec::new();
    // Start as if already in a trimmed whitespace run so leading whitespace is
    // dropped.
    let mut in_whitespace = true;
    let mut already_trimmed = true;

    for c in text {
        if is_ws(c) {
            if !in_whitespace {
                // Reduce each whitespace run to a single space.
                in_whitespace = true;
                result.push(space);
            }
            if trim_sequences_with_line_breaks && !already_trimmed && is_line_break(c) {
                // A run containing CR or LF is removed entirely, including the
                // space pushed above.
                already_trimmed = true;
                result.pop();
            }
        } else {
            in_whitespace = false;
            already_trimmed = false;
            result.push(c);
        }
    }

    if in_whitespace && !already_trimmed {
        // Drop trailing whitespace.
        result.pop();
    }
    result
}

/// C++ `CollapseWhitespace(std::u16string_view, bool)`. Uses the Unicode
/// whitespace set, and the space written in place of a run is U+0020.
pub fn collapse_whitespace(text: &[u16], trim_sequences_with_line_breaks: bool) -> Vec<u16> {
    collapse_core(
        text.iter().copied(),
        trim_sequences_with_line_breaks,
        |c| is_unicode_whitespace(u32::from(c)),
        |c| c == u16::from(b'\n') || c == u16::from(b'\r'),
        u16::from(b' '),
    )
}

/// C++ `CollapseWhitespaceASCII(std::string_view, bool)`. Only the ASCII
/// whitespace set counts, so bytes such as U+00A0 in UTF-8 are left alone.
pub fn collapse_whitespace_ascii(text: &[u8], trim_sequences_with_line_breaks: bool) -> Vec<u8> {
    collapse_core(
        text.iter().copied(),
        trim_sequences_with_line_breaks,
        |c| is_ascii_whitespace(c),
        |c| c == b'\n' || c == b'\r',
        b' ',
    )
}

/// `&str` form of [`collapse_whitespace_ascii`]. Only ASCII bytes are removed
/// or replaced, so the result is valid UTF-8.
pub fn collapse_whitespace_ascii_str(text: &str, trim_sequences_with_line_breaks: bool) -> String {
    collapse_core(
        text.chars(),
        trim_sequences_with_line_breaks,
        |c| c.is_ascii() && is_ascii_whitespace(c as u8),
        |c| c == '\n' || c == '\r',
        ' ',
    )
    .into_iter()
    .collect()
}

/// C++ `ContainsOnlyChars`: true when `input` is empty or every unit of `input`
/// appears in `characters`.
pub fn contains_only_chars<T: CodeUnit>(input: &[T], characters: &[T]) -> bool {
    input.iter().all(|c| characters.contains(c))
}
