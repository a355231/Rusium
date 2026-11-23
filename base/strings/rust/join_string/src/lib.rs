// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `base::JoinString` (the `strings_internal::JoinStringT`
//! template in base/strings/join_string_internal.h, and the `JoinString`
//! overloads in base/strings/string_util.h).
//!
//! `join_string` joins UTF-8 pieces with a separator. `join_string16` joins
//! UTF-16 pieces. `join_string_by` and `join_string16_by` also apply a function
//! to each element before joining. Those two are not in the C++ tree.
//!
//! Differences from the C++ version:
//! - The C++ overloads for `std::string`, `std::string_view`, `std::u16string`
//!   and `std::u16string_view` become one generic function each, over
//!   `S: AsRef<str>` or `S: AsRef<[u16]>`. A call must use one element type,
//!   for example `&[&str]` or `&[String]`.
//! - The separator is a borrowed `&str` or `&[u16]`. The C++ version takes a
//!   `std::string_view`, which is the same idea.
//! - The C++ code pre-sizes the result with a fold over the parts. This port
//!   does the same with `Iterator::sum`.
//! - The per-element transform (`join_string_by`, `join_string16_by`) is not in
//!   the C++ tree. It is added here for callers that need to format or convert
//!   each element without building an intermediate `Vec`. Each element is
//!   transformed once, in order. The transform's result must be `AsRef<str>`
//!   (or `AsRef<[u16]>`).
//! - The `BASE_EXPORT`, `LazyInstantiation` and `constexpr` details are not
//!   ported. Rust functions are not `constexpr` here.
//! - The Windows `std::wstring` overloads (string_util_win.h) are not ported.

/// Joins UTF-8 `parts` with `sep` between each pair of adjacent parts. Empty
/// `parts` gives `""`. A single part is returned without `sep`.
pub fn join_string<S: AsRef<str>>(parts: &[S], sep: &str) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let total: usize =
        parts.iter().map(|p| p.as_ref().len()).sum::<usize>() + (parts.len() - 1) * sep.len();
    let mut result = String::with_capacity(total);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            result.push_str(sep);
        }
        result.push_str(part.as_ref());
    }
    result
}

/// Joins UTF-16 `parts` with `sep` between each pair of adjacent parts. Empty
/// `parts` gives an empty result.
pub fn join_string16<S: AsRef<[u16]>>(parts: &[S], sep: &[u16]) -> Vec<u16> {
    if parts.is_empty() {
        return Vec::new();
    }
    let total: usize =
        parts.iter().map(|p| p.as_ref().len()).sum::<usize>() + (parts.len() - 1) * sep.len();
    let mut result = Vec::with_capacity(total);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            result.extend_from_slice(sep);
        }
        result.extend_from_slice(part.as_ref());
    }
    result
}

/// Applies `f` to each element of `parts` in order, then joins the results
/// with `sep`. Not in the C++ tree.
pub fn join_string_by<T, R, F>(parts: &[T], sep: &str, mut f: F) -> String
where
    R: AsRef<str>,
    F: FnMut(&T) -> R,
{
    let pieces: Vec<R> = parts.iter().map(&mut f).collect();
    join_string(&pieces, sep)
}

/// Applies `f` to each element of `parts` in order, then joins the UTF-16
/// results with `sep`. Not in the C++ tree.
pub fn join_string16_by<T, R, F>(parts: &[T], sep: &[u16], mut f: F) -> Vec<u16>
where
    R: AsRef<[u16]>,
    F: FnMut(&T) -> R,
{
    let pieces: Vec<R> = parts.iter().map(&mut f).collect();
    join_string16(&pieces, sep)
}

#[cfg(test)]
mod tests;
