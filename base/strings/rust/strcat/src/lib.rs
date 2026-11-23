// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `base::StrCat` and `base::StrAppend`
//! (base/strings/strcat.{h,cc} and base/strings/strcat_internal.h).
//!
//! `str_cat` concatenates a list of strings into one new string. `str_append`
//! appends a list of strings to an existing one. The `str_cat16` and
//! `str_append16` functions do the same for UTF-16 `&[u16]` data.
//!
//! Differences from the C++ version:
//! - The C++ overloads for `std::string_view`, `std::string` and string
//!   literals become one generic function over `S: AsRef<str>` (or
//!   `AsRef<[u16]>`). A call must pass one element type, for example `&[&str]`
//!   or `&[String]`. Mixed lists need `.as_str()` on each element.
//! - The C++ `initializer_list` overloads are the same functions, because a
//!   Rust array slice takes the place of a braced list.
//! - The out-parameter of `StrAppend` is `&mut` instead of a pointer.
//! - The result size is computed first, and the buffer is reserved once. This
//!   matches `StrAppendT`'s single-allocation goal. The C++ code uses
//!   `resize_and_overwrite` to avoid writing the terminating NUL. Rust has no
//!   such NUL, so the port uses `push_str`. The output is the same.
//! - An allocation that cannot be satisfied panics, as with other Rust
//!   collections. The C++ code relies on `checked_cast` and `std::string`,
//!   which abort in the same case.
//! - The Windows `StrCat` macro guard (strcat_win.h) is not ported.

/// Concatenates `pieces` into a new `String`. Mirrors `base::StrCat` for
/// `std::string_view` and `std::string` pieces.
pub fn str_cat<S: AsRef<str>>(pieces: &[S]) -> String {
    let total: usize = pieces.iter().map(|p| p.as_ref().len()).sum();
    let mut result = String::with_capacity(total);
    for piece in pieces {
        result.push_str(piece.as_ref());
    }
    result
}

/// Concatenates UTF-16 `pieces` into a new `Vec<u16>`. Mirrors `base::StrCat`
/// for `std::u16string_view` and `std::u16string` pieces.
pub fn str_cat16<S: AsRef<[u16]>>(pieces: &[S]) -> Vec<u16> {
    let total: usize = pieces.iter().map(|p| p.as_ref().len()).sum();
    let mut result = Vec::with_capacity(total);
    for piece in pieces {
        result.extend_from_slice(piece.as_ref());
    }
    result
}

/// Appends `pieces` to `dest`. Mirrors `base::StrAppend` for UTF-8 pieces.
///
/// The buffer is reserved for the total size first. If `dest` already has
/// enough spare capacity, it is not reallocated.
pub fn str_append<S: AsRef<str>>(dest: &mut String, pieces: &[S]) {
    let total: usize = pieces.iter().map(|p| p.as_ref().len()).sum();
    dest.reserve(total);
    for piece in pieces {
        dest.push_str(piece.as_ref());
    }
}

/// Appends UTF-16 `pieces` to `dest`. Mirrors `base::StrAppend` for UTF-16
/// pieces.
pub fn str_append16<S: AsRef<[u16]>>(dest: &mut Vec<u16>, pieces: &[S]) {
    let total: usize = pieces.iter().map(|p| p.as_ref().len()).sum();
    dest.reserve(total);
    for piece in pieces {
        dest.extend_from_slice(piece.as_ref());
    }
}

#[cfg(test)]
mod tests;
