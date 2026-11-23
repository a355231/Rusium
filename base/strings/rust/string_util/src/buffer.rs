// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! strlcpy and WriteInto from base/strings/string_util.h.

use crate::code_unit::CodeUnit;

/// C++ `strlcpy` / `u16cstrlcpy` (both the span and pointer forms). Copies
/// `src` into `dst`, truncating so that `dst` is NUL-terminated when it is not
/// empty. Returns the length of `src` in code units. If the return value is at
/// least `dst.len()`, the output was truncated.
///
/// The C++ `wcslcpy` overloads are not ported. See the crate docs.
pub fn strlcpy<T: CodeUnit>(dst: &mut [T], src: &[T]) -> usize {
    let dst_size = dst.len();
    let mut i = 0;
    while i + 1 < dst_size {
        if i == src.len() {
            break;
        }
        dst[i] = src[i];
        i += 1;
    }
    if let Some(slot) = dst.get_mut(i) {
        *slot = T::default();
    }
    src.len()
}

/// C++ `WriteInto(std::string*, size_t length_with_null)`. Returns a zero-filled
/// buffer of `length_with_null - 1` code units for the caller to fill. The C++
/// returns a pointer into a string and sets its size. A Rust caller uses the
/// returned Vec directly. `length_with_null` must be at least 1. The C++ code
/// DCHECKs this. Here 0 gives an empty buffer.
pub fn write_into<T: CodeUnit>(length_with_null: usize) -> Vec<T> {
    debug_assert!(length_with_null >= 1, "length_with_null must include the NUL");
    vec![T::default(); length_with_null.saturating_sub(1)]
}
