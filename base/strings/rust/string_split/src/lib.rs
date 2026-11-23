// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base/strings/string_split.{h,cc}` and the splitting templates
//! from `base/strings/string_split_internal.h`.
//!
//! Splits strings on a set of separator characters, on a substring delimiter,
//! at the first or last separator, and into key/value pairs. The UTF-8
//! functions take `&str`. The UTF-16 functions, whose names end in `_utf16`,
//! take `&[u16]`. The C++ `SplitString`/`SplitStringPiece` overloads map to the
//! `_utf16` functions for `std::u16string(_view)`.
//!
//! Differences from the C++ version:
//! - Out-parameters are replaced by return values. The `bool` returned by the
//!   key/value functions is the `complete` field of [`KeyValueSplit`]. The
//!   pairs are returned in the same struct, because the C++ fills them in even
//!   when it returns `false`.
//! - `std::string` results are `String`, and `std::string_view` results are
//!   `&str`. Owned UTF-16 results are `Vec<u16>` because the standard library
//!   has no owned UTF-16 string type.
//! - Separators and key/value delimiters are `char`s, not bytes. The C++ code
//!   matches individual bytes, so a multi-byte separator such as `"é"` would
//!   split a UTF-8 sequence. The `&str` functions here match whole characters.
//!   For ASCII separators the results are identical.
//! - `SplitStringOnce` and `RSplitStringOnce` skip the separator by its UTF-8
//!   length, not by one byte. C++ `npos` is replaced by `Option`.
//! - Whitespace trimming in the `&str` functions uses the ASCII set from
//!   `kWhitespaceASCII`: `\t \n \v \f \r` and space. This is not
//!   `u8::is_ascii_whitespace`, which omits `\v`. The `_utf16` functions use
//!   the full Unicode set from `kWhitespaceUTF16`, as the C++ code does for
//!   `char16_t`.
//! - Preserved quirk: with an empty delimiter, `SplitStringUsingSubstr` and
//!   `SplitStringIntoKeyValuePairsUsingSubstr` return the input as a single
//!   piece without trimming or `SPLIT_WANT_NONEMPTY` filtering. The C++ code
//!   does this, and so does this port.
//! - Preserved quirk: `SplitString` on an empty input returns no pieces, but
//!   `SplitStringUsingSubstr` on an empty input returns `[""]` under
//!   `SPLIT_WANT_ALL`.
//! - No logging. The `DVLOG` diagnostics in the key/value parser are not
//!   ported.
//!
//! Not ported:
//! - `constexpr` evaluation. The C++ functions return `std::vector`, which
//!   cannot be built in a const context on stable Rust. The
//!   `ConstexprSplitStringPieceTest` cases are ported as ordinary runtime tests
//!   with the same inputs and expected outputs.
//! - `base/strings/string_split_win.h`, which is Windows-only.
//! - The `StringPairs` and `StringViewPairs` aliases. Callers use `Vec` of
//!   tuples directly, through [`KeyValueSplit`].
//! - The `base::kWhitespaceASCIIAs16` and `kWhitespaceNoCrLfUTF16` constants.
//!   Only `kWhitespaceASCII` and `kWhitespaceUTF16` are used.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests;

/// Equivalent of `base::kWhitespaceASCII`: `\t`, `\v`, `\f`, space, `\n`, `\r`.
pub const WHITESPACE_ASCII: &str = "\t\u{0B}\u{0C} \n\r";

/// Equivalent of `base::kWhitespaceUTF16`: the ASCII set plus the non-ASCII
/// Unicode spaces listed in `whitespace_constants.h`.
pub const WHITESPACE_UTF16: &[u16] = &[
    0x09, 0x0B, 0x0C, 0x20, 0x0A, 0x0D, // ASCII set.
    0x0085, 0x00A0, 0x1680, //
    0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A, //
    0x2028, 0x2029, 0x202F, 0x205F, 0x3000,
];

/// Whether to trim leading and trailing whitespace from each piece.
/// Mirrors `base::WhitespaceHandling`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceHandling {
    KeepWhitespace,
    TrimWhitespace,
}

/// Whether empty pieces are kept. Mirrors `base::SplitResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitResult {
    /// Return every piece, including empty ones.
    SplitWantAll,
    /// Drop empty pieces. Pieces that are whitespace-only are dropped too when
    /// trimming.
    SplitWantNonEmpty,
}

/// Result of a key/value split. Mirrors the `bool` return value and the
/// out-parameter of the C++ key/value functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueSplit<T> {
    /// Parsed pairs in input order. A pair that could not be parsed is still
    /// included, as `("", "")` when the key/value delimiter is missing, or as
    /// `(key, "")` when there is no value.
    pub pairs: Vec<(T, T)>,
    /// `true` only if every pair has a key/value delimiter and a non-empty
    /// value. An empty key is allowed. The C++ header comment says keys must be
    /// non-empty too, but the code and `EmptyKeyWithKeyValueDelimiter` return
    /// `true` for `:value`, so this port follows the code. Corresponds to the
    /// C++ `bool` return value.
    pub complete: bool,
}

// ---------------------------------------------------------------------------
// Whitespace helpers (replaces base::kWhitespaceASCII / kWhitespaceUTF16 use).
// ---------------------------------------------------------------------------

fn is_ascii_whitespace_char(c: char) -> bool {
    matches!(c, '\t' | '\u{0B}' | '\u{0C}' | ' ' | '\n' | '\r')
}

fn is_utf16_whitespace_unit(unit: u16) -> bool {
    WHITESPACE_UTF16.contains(&unit)
}

fn trim_utf8(piece: &str, whitespace: WhitespaceHandling) -> &str {
    match whitespace {
        WhitespaceHandling::KeepWhitespace => piece,
        WhitespaceHandling::TrimWhitespace => piece.trim_matches(is_ascii_whitespace_char),
    }
}

fn trim_utf16(piece: &[u16], whitespace: WhitespaceHandling) -> &[u16] {
    if whitespace == WhitespaceHandling::KeepWhitespace {
        return piece;
    }
    let start = piece.iter().position(|&u| !is_utf16_whitespace_unit(u)).unwrap_or(piece.len());
    let end =
        piece.iter().rposition(|&u| !is_utf16_whitespace_unit(u)).map_or(start, |last| last + 1);
    &piece[start..end]
}

fn keep_piece(piece_is_empty: bool, result_type: SplitResult) -> bool {
    result_type == SplitResult::SplitWantAll || !piece_is_empty
}

// ---------------------------------------------------------------------------
// Split on a set of separators.
// ---------------------------------------------------------------------------

/// Splits `input` at every character in `separators` and returns owned pieces.
/// Equivalent of `base::SplitString(std::string_view, ...)`.
///
/// An empty `input` yields no pieces. An empty `separators` yields the whole
/// input as one piece.
pub fn split_string(
    input: &str,
    separators: &str,
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<String> {
    split_string_piece(input, separators, whitespace, result_type)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Borrowing version of [`split_string`]. The returned pieces point into
/// `input`. Equivalent of `base::SplitStringPiece(std::string_view, ...)`.
pub fn split_string_piece<'a>(
    input: &'a str,
    separators: &str,
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<&'a str> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(|c: char| separators.contains(c))
        .map(|piece| trim_utf8(piece, whitespace))
        .filter(|piece| keep_piece(piece.is_empty(), result_type))
        .collect()
}

/// UTF-16 version of [`split_string`]. Equivalent of
/// `base::SplitString(std::u16string_view, ...)`.
pub fn split_string_utf16(
    input: &[u16],
    separators: &[u16],
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<Vec<u16>> {
    split_string_piece_utf16(input, separators, whitespace, result_type)
        .into_iter()
        .map(<[u16]>::to_vec)
        .collect()
}

/// Borrowing UTF-16 version of [`split_string`]. Equivalent of
/// `base::SplitStringPiece(std::u16string_view, ...)`.
pub fn split_string_piece_utf16<'a>(
    input: &'a [u16],
    separators: &[u16],
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<&'a [u16]> {
    if input.is_empty() {
        return Vec::new();
    }
    input
        .split(|unit: &u16| separators.contains(unit))
        .map(|piece| trim_utf16(piece, whitespace))
        .filter(|piece| keep_piece(piece.is_empty(), result_type))
        .collect()
}

// ---------------------------------------------------------------------------
// Split on a substring delimiter.
// ---------------------------------------------------------------------------

/// Splits `input` on each occurrence of the substring `delimiter`. Equivalent
/// of `base::SplitStringUsingSubstr(std::string_view, ...)`.
///
/// An empty `delimiter` returns `[input]` without trimming or filtering. An
/// empty `input` with a non-empty delimiter returns `[""]` under
/// `SplitWantAll`.
pub fn split_string_using_substr(
    input: &str,
    delimiter: &str,
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<String> {
    split_string_piece_using_substr(input, delimiter, whitespace, result_type)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Borrowing version of [`split_string_using_substr`]. Equivalent of
/// `base::SplitStringPieceUsingSubstr(std::string_view, ...)`.
pub fn split_string_piece_using_substr<'a>(
    input: &'a str,
    delimiter: &str,
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<&'a str> {
    if delimiter.is_empty() {
        return vec![input];
    }
    input
        .split(delimiter)
        .map(|piece| trim_utf8(piece, whitespace))
        .filter(|piece| keep_piece(piece.is_empty(), result_type))
        .collect()
}

/// UTF-16 version of [`split_string_using_substr`]. Equivalent of
/// `base::SplitStringUsingSubstr(std::u16string_view, ...)`.
pub fn split_string_using_substr_utf16(
    input: &[u16],
    delimiter: &[u16],
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<Vec<u16>> {
    split_string_piece_using_substr_utf16(input, delimiter, whitespace, result_type)
        .into_iter()
        .map(<[u16]>::to_vec)
        .collect()
}

/// Borrowing UTF-16 version of [`split_string_using_substr`]. Equivalent of
/// `base::SplitStringPieceUsingSubstr(std::u16string_view, ...)`.
pub fn split_string_piece_using_substr_utf16<'a>(
    input: &'a [u16],
    delimiter: &[u16],
    whitespace: WhitespaceHandling,
    result_type: SplitResult,
) -> Vec<&'a [u16]> {
    if delimiter.is_empty() {
        return vec![input];
    }
    let mut result = Vec::new();
    let mut begin = 0;
    loop {
        let found = find_subslice(input, delimiter, begin);
        let term_end = found.unwrap_or(input.len());
        let term = trim_utf16(&input[begin..term_end], whitespace);
        if keep_piece(term.is_empty(), result_type) {
            result.push(term);
        }
        match found {
            Some(at) => begin = at + delimiter.len(),
            None => break,
        }
    }
    result
}

/// Index of the first occurrence of `needle` in `haystack` at or after `from`.
/// `needle` must not be empty.
fn find_subslice(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    let last_start = haystack.len().checked_sub(needle.len())?;
    (from..=last_start).find(|&i| haystack[i..i + needle.len()] == *needle)
}

// ---------------------------------------------------------------------------
// Split once at the first or last separator.
// ---------------------------------------------------------------------------

fn split_at_separator(input: &str, found: Option<(usize, char)>) -> Option<(&str, &str)> {
    let (pos, separator) = found?;
    Some((&input[..pos], &input[pos + separator.len_utf8()..]))
}

/// Splits at the first `separator`. Returns `None` if it is not present.
/// Equivalent of `base::SplitStringOnce(std::string_view, char)`.
pub fn split_string_once(input: &str, separator: char) -> Option<(&str, &str)> {
    split_at_separator(input, input.find(separator).map(|pos| (pos, separator)))
}

/// Splits at the first character in `separators`. Returns `None` if none is
/// present. Equivalent of `base::SplitStringOnce(std::string_view, string_view)`.
pub fn split_string_once_any<'a>(input: &'a str, separators: &str) -> Option<(&'a str, &'a str)> {
    split_at_separator(input, input.char_indices().find(|&(_, c)| separators.contains(c)))
}

/// Splits at the last `separator`. Returns `None` if it is not present.
/// Equivalent of `base::RSplitStringOnce(std::string_view, char)`.
pub fn rsplit_string_once(input: &str, separator: char) -> Option<(&str, &str)> {
    split_at_separator(input, input.rfind(separator).map(|pos| (pos, separator)))
}

/// Splits at the last character in `separators`. Returns `None` if none is
/// present. Equivalent of `base::RSplitStringOnce(std::string_view, string_view)`.
pub fn rsplit_string_once_any<'a>(input: &'a str, separators: &str) -> Option<(&'a str, &'a str)> {
    split_at_separator(input, input.char_indices().rev().find(|&(_, c)| separators.contains(c)))
}

// ---------------------------------------------------------------------------
// Key/value pairs.
// ---------------------------------------------------------------------------

/// Parses one already-trimmed pair. Pushes `(key, value)` and returns whether
/// the pair had both a key/value delimiter and a non-empty value.
fn append_key_value<'a>(
    pair: &'a str,
    key_value_delimiter: char,
    pairs: &mut Vec<(&'a str, &'a str)>,
) -> bool {
    let Some(end_key) = pair.find(key_value_delimiter) else {
        // No delimiter. The C++ code still appends an empty pair.
        pairs.push(("", ""));
        return false;
    };
    let key = &pair[..end_key];
    // Skip every consecutive delimiter. The value is what remains, which may
    // itself contain delimiters.
    let value = pair[end_key..].trim_start_matches(key_value_delimiter);
    pairs.push((key, value));
    !value.is_empty()
}

fn split_key_value_views<'a>(
    input: &'a str,
    key_value_delimiter: char,
    key_value_pair_delimiter: &str,
) -> KeyValueSplit<&'a str> {
    let mut pairs = Vec::new();
    let mut complete = true;
    for pair in split_string_piece_using_substr(
        input,
        key_value_pair_delimiter,
        WhitespaceHandling::TrimWhitespace,
        SplitResult::SplitWantNonEmpty,
    ) {
        if !append_key_value(pair, key_value_delimiter, &mut pairs) {
            // Keep going, so that later pairs are still parsed.
            complete = false;
        }
    }
    KeyValueSplit { pairs, complete }
}

fn to_owned_key_values(split: KeyValueSplit<&str>) -> KeyValueSplit<String> {
    KeyValueSplit {
        pairs: split
            .pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        complete: split.complete,
    }
}

/// Splits `input` into key/value pairs, with owned results. Equivalent of
/// `base::SplitStringIntoKeyValuePairs(std::string_view, char, char, ...)`.
///
/// Each pair is split on `key_value_pair_delimiter`, then trimmed. Its key is
/// everything before the first `key_value_delimiter`, and its value is what
/// follows the run of `key_value_delimiter`s. Whitespace inside a key or
/// value is kept. See [`KeyValueSplit`] for the failure cases.
pub fn split_string_into_key_value_pairs(
    input: &str,
    key_value_delimiter: char,
    key_value_pair_delimiter: char,
) -> KeyValueSplit<String> {
    to_owned_key_values(split_string_into_key_value_view_pairs(
        input,
        key_value_delimiter,
        key_value_pair_delimiter,
    ))
}

/// Borrowing version of [`split_string_into_key_value_pairs`]. Equivalent of
/// `base::SplitStringIntoKeyValueViewPairs`.
pub fn split_string_into_key_value_view_pairs(
    input: &str,
    key_value_delimiter: char,
    key_value_pair_delimiter: char,
) -> KeyValueSplit<&str> {
    let mut buf = [0u8; 4];
    split_key_value_views(
        input,
        key_value_delimiter,
        key_value_pair_delimiter.encode_utf8(&mut buf),
    )
}

/// Like [`split_string_into_key_value_pairs`], but the pair delimiter is a
/// substring. Equivalent of
/// `base::SplitStringIntoKeyValuePairsUsingSubstr`.
pub fn split_string_into_key_value_pairs_using_substr(
    input: &str,
    key_value_delimiter: char,
    key_value_pair_delimiter: &str,
) -> KeyValueSplit<String> {
    to_owned_key_values(split_key_value_views(input, key_value_delimiter, key_value_pair_delimiter))
}

/// Borrowing version of
/// [`split_string_into_key_value_pairs_using_substr`]. Equivalent of
/// `base::SplitStringIntoKeyValueViewPairsUsingSubstr`.
pub fn split_string_into_key_value_view_pairs_using_substr<'a>(
    input: &'a str,
    key_value_delimiter: char,
    key_value_pair_delimiter: &str,
) -> KeyValueSplit<&'a str> {
    split_key_value_views(input, key_value_delimiter, key_value_pair_delimiter)
}
