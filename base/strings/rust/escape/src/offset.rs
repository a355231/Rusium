// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of the offset-adjustment parts of `OffsetAdjuster` from
//! `base/strings/utf_offset_string_conversions.cc`.
//!
//! An offset is a position between characters. `None` stands for the C++
//! `std::u16string::npos`, which means "no valid position". An offset that
//! falls inside a region that was collapsed or replaced maps to `None`.

/// One change made to a string: `original_length` bytes or units at
/// `original_offset` were replaced by `output_length` units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adjustment {
    pub original_offset: usize,
    pub original_length: usize,
    pub output_length: usize,
}

impl Adjustment {
    pub const fn new(original_offset: usize, original_length: usize, output_length: usize) -> Self {
        Adjustment { original_offset, original_length, output_length }
    }
}

/// Maps `offset` through `adjustments`, which must be sorted by
/// `original_offset`. Returns `None` if the offset is inside an adjusted range
/// or if the result exceeds `limit` (`None` means no limit). The result is
/// `None` whenever the input is `None`.
pub fn adjust_offset(
    adjustments: &[Adjustment],
    offset: Option<usize>,
    limit: Option<usize>,
) -> Option<usize> {
    let offset = offset?;
    let mut original_lengths = 0usize;
    let mut output_lengths = 0usize;
    for adj in adjustments {
        if offset <= adj.original_offset {
            break;
        }
        if offset < adj.original_offset + adj.original_length {
            return None;
        }
        original_lengths += adj.original_length;
        output_lengths += adj.output_length;
    }
    // The C++ computes this in unsigned arithmetic, which may wrap. The
    // wrapping operations here give the same bits.
    let adjusted = offset.wrapping_add(output_lengths).wrapping_sub(original_lengths);
    match limit {
        Some(limit) if adjusted > limit => None,
        _ => Some(adjusted),
    }
}

/// Composes `first` (applied to the original string) with
/// `adjustments_on_adjusted_string` (applied to the string that `first`
/// produced). The composed list is written back to
/// `adjustments_on_adjusted_string`, with offsets relative to the original
/// string.
///
/// Both lists must be sorted by offset. Only collapsing adjustments are
/// supported in `first`: its `output_length` must not exceed its
/// `original_length`.
pub fn merge_sequential_adjustments(
    first: &[Adjustment],
    adjustments_on_adjusted_string: &mut Vec<Adjustment>,
) {
    let adjusted = adjustments_on_adjusted_string;
    let mut builder = Vec::with_capacity(adjusted.len() + first.len());
    let mut ai = 0usize;
    let mut fi = 0usize;
    // `shift` is the number of units removed by `first` before the current
    // adjusted entry. `currently_collapsing` is the amount removed inside the
    // current entry, which is applied to `shift` after that entry is emitted.
    let mut shift = 0usize;
    let mut currently_collapsing = 0usize;
    while ai < adjusted.len() {
        if fi == first.len()
            || adjusted[ai].original_offset + shift + adjusted[ai].original_length
                <= first[fi].original_offset
        {
            // The whole adjusted entry comes before `first[fi]`.
            adjusted[ai].original_offset += shift;
            shift = shift.wrapping_add(currently_collapsing);
            currently_collapsing = 0;
            builder.push(adjusted[ai]);
            ai += 1;
        } else if adjusted[ai].original_offset + shift > first[fi].original_offset {
            // `first[fi]` comes before the adjusted entry.
            debug_assert!(
                first[fi].original_offset + first[fi].output_length
                    <= adjusted[ai].original_offset + shift
            );
            shift =
                shift.wrapping_add(first[fi].original_length.wrapping_sub(first[fi].output_length));
            builder.push(first[fi]);
            fi += 1;
        } else {
            // `first[fi]` lies inside the adjusted entry's range, so the entry
            // grows by the collapsed amount.
            debug_assert!(first[fi].original_length > first[fi].output_length);
            let collapse = first[fi].original_length - first[fi].output_length;
            adjusted[ai].original_length += collapse;
            currently_collapsing += collapse;
            fi += 1;
        }
    }
    debug_assert_eq!(currently_collapsing, 0);
    if fi < first.len() {
        // Only entries from `first` remain. Their offsets already refer to the
        // original string.
        debug_assert_eq!(ai, adjusted.len());
        builder.extend_from_slice(&first[fi..]);
    }
    *adjusted = builder;
}
