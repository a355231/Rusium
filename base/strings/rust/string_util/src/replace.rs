// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! ReplaceChars, RemoveChars, ReplaceSubstringsAfterOffset,
//! ReplaceFirstSubstringAfterOffset, and JoinString from
//! base/strings/string_util.h.

use crate::code_unit::CodeUnit;

/// C++ `ReplaceChars`: replaces every unit of `input` that appears in
/// `replace_chars` with all of `replace_with`. Returns the result and whether
/// any unit was replaced. Replacement text is not rescanned.
pub fn replace_chars<T: CodeUnit>(
    input: &[T],
    replace_chars: &[T],
    replace_with: &[T],
) -> (Vec<T>, bool) {
    let mut found = false;
    let mut out = Vec::with_capacity(input.len());
    for &c in input {
        if replace_chars.contains(&c) {
            found = true;
            out.extend_from_slice(replace_with);
        } else {
            out.push(c);
        }
    }
    if found {
        (out, true)
    } else {
        (input.to_vec(), false)
    }
}

/// C++ `RemoveChars`: removes every unit of `input` that appears in
/// `remove_chars`. Returns the result and whether anything was removed.
pub fn remove_chars<T: CodeUnit>(input: &[T], remove_chars: &[T]) -> (Vec<T>, bool) {
    replace_chars(input, remove_chars, &[])
}

/// Finds the first occurrence of `needle` in `hay` at or after `from`. An empty
/// needle never matches, as in the C++ code. A `from` past the end finds
/// nothing.
fn find_from<T: Eq>(hay: &[T], from: usize, needle: &[T]) -> Option<usize> {
    if needle.is_empty() || from > hay.len() || needle.len() > hay.len() - from {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// Shared core of [`replace_substrings_after_offset`] and
/// [`replace_first_substring_after_offset`]. Replaces non-overlapping matches
/// of `find_this` that start at or after `start_offset`, scanning left to right.
fn replace_matches_after_offset<T: CodeUnit>(
    input: &[T],
    start_offset: usize,
    find_this: &[T],
    replace_with: &[T],
    all: bool,
) -> Vec<T> {
    let Some(first) = find_from(input, start_offset, find_this) else {
        return input.to_vec();
    };
    let mut out = Vec::with_capacity(input.len());
    let mut copied = 0;
    let mut next_match = Some(first);
    while let Some(m) = next_match {
        out.extend_from_slice(&input[copied..m]);
        out.extend_from_slice(replace_with);
        copied = m + find_this.len();
        if !all {
            break;
        }
        next_match = find_from(input, copied, find_this);
    }
    out.extend_from_slice(&input[copied..]);
    out
}

/// C++ `ReplaceSubstringsAfterOffset`: replaces every occurrence of `find_this`
/// at or after `start_offset`. An empty `find_this` changes nothing.
pub fn replace_substrings_after_offset<T: CodeUnit>(
    input: &[T],
    start_offset: usize,
    find_this: &[T],
    replace_with: &[T],
) -> Vec<T> {
    replace_matches_after_offset(input, start_offset, find_this, replace_with, true)
}

/// C++ `ReplaceFirstSubstringAfterOffset`: replaces only the first occurrence
/// of `find_this` at or after `start_offset`.
pub fn replace_first_substring_after_offset<T: CodeUnit>(
    input: &[T],
    start_offset: usize,
    find_this: &[T],
    replace_with: &[T],
) -> Vec<T> {
    replace_matches_after_offset(input, start_offset, find_this, replace_with, false)
}

/// C++ `JoinString`: concatenates `parts`, placing `separator` between each
/// pair. Accepts any parts that view as code units (`String`, `&str`, `Vec`,
/// slices). For `&str` parts, `[S]::join` from std gives the same result.
pub fn join_string<T: CodeUnit, P: AsRef<[T]>>(parts: &[P], separator: &[T]) -> Vec<T> {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(separator);
        }
        out.extend_from_slice(part.as_ref());
    }
    out
}
