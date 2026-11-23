// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of the UTF-8 decoding macro `CBU8_NEXT` (with `sub` = -1) from
//! base/third_party/icu/icu_utf.h. Crate-internal.
//!
//! The control flow mirrors the macro step by step, so that the number of bytes
//! consumed for an ill-formed sequence is the same as in C++: the lead byte plus
//! every following byte that is valid so far, stopping at the first byte that is
//! invalid or at the end of input. This is the "maximal subpart" rule.

/// `CBU8_LEAD3_T1_BITS`. Indexed by `lead & 0xf` for leads E0..EF. Bit `t1 >> 5`
/// is set if the first trail byte `t1` starts a valid 3-byte sequence.
const LEAD3_T1_BITS: [u8; 16] = [
    0x20, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, //
    0x30, 0x30, 0x30, 0x30, 0x30, 0x10, 0x30, 0x30,
];

/// `CBU8_LEAD4_T1_BITS`. Indexed by `t1 >> 4`. Bit `lead - 0xF0` is set if the
/// first trail byte `t1` starts a valid 4-byte sequence for lead F0..F4.
const LEAD4_T1_BITS: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x1E, 0x0F, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x00,
];

fn is_trail(byte: u8) -> bool {
    (0x80..=0xbf).contains(&byte)
}

/// Decodes the sequence that starts at `start`, which must be less than
/// `src.len()`. Returns the decoded character, or `None` if the sequence is
/// ill-formed, and the index just past the bytes that were consumed.
pub(crate) fn decode_utf8(src: &[u8], start: usize) -> (Option<char>, usize) {
    let length = src.len();
    let lead = src[start];
    let mut i = start + 1;
    if lead < 0x80 {
        return (Some(char::from(lead)), i);
    }
    if i == length {
        return (None, i);
    }
    let code_point: u32 = match lead {
        // U+0080..U+07FF. Leads C0 and C1 are rejected as overlong.
        0xc2..=0xdf => {
            let t1 = src[i];
            if !is_trail(t1) {
                return (None, i);
            }
            i += 1;
            (u32::from(lead & 0x1f) << 6) | u32::from(t1 & 0x3f)
        }
        // U+0800..U+FFFF, excluding surrogates.
        0xe0..=0xef => {
            let t1 = src[i];
            if LEAD3_T1_BITS[usize::from(lead & 0xf)] & (1 << (t1 >> 5)) == 0 {
                return (None, i);
            }
            let c = (u32::from(lead & 0xf) << 6) | u32::from(t1 & 0x3f);
            i += 1;
            if i == length {
                return (None, i);
            }
            let t2 = src[i];
            if !is_trail(t2) {
                return (None, i);
            }
            i += 1;
            (c << 6) | u32::from(t2 & 0x3f)
        }
        // U+10000..U+10FFFF.
        0xf0..=0xf4 => {
            let t1 = src[i];
            if LEAD4_T1_BITS[usize::from(t1 >> 4)] & (1 << (lead - 0xf0)) == 0 {
                return (None, i);
            }
            let c = (u32::from(lead & 7) << 6) | u32::from(t1 & 0x3f);
            i += 1;
            if i == length {
                return (None, i);
            }
            let t2 = src[i];
            if !is_trail(t2) {
                return (None, i);
            }
            let c = (c << 6) | u32::from(t2 & 0x3f);
            i += 1;
            if i == length {
                return (None, i);
            }
            let t3 = src[i];
            if !is_trail(t3) {
                return (None, i);
            }
            i += 1;
            (c << 6) | u32::from(t3 & 0x3f)
        }
        // Continuation bytes, C0, C1, and F5..FF are never valid leads.
        _ => return (None, i),
    };
    (char::from_u32(code_point), i)
}
