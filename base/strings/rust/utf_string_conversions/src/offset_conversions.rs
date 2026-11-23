// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of base/strings/utf_offset_string_conversions.{h,cc}.
//!
//! Offsets are insertion points between characters: in "abcd", 0 is before
//! 'a' and 4 is the end. An offset that cannot be mapped (for example one in
//! the middle of a multi-unit sequence that was collapsed) becomes `None`, which
//! is the C++ `std::u16string::npos`. An input of `None` stays `None`.
//!
//! Unsigned arithmetic wraps, as in the C++ code, using `wrapping_*`.
//! The C++ DCHECKs in `merge_sequential_adjustments` are `debug_assert!`s here.

use crate::conversions::{transcode, Converted};
use crate::conversion_utils::{
    read_unicode_character_utf16, read_unicode_character_utf8, write_unicode_character_utf16,
    write_unicode_character_utf8,
};

/// Equivalent of `std::u16string::npos`. Offsets equal to this are never
/// produced; they are represented as `None`.
const NPOS: usize = usize::MAX;

/// One change to a string: `original_length` units at `original_offset` were
/// replaced by `output_length` units. The adjustments are sorted by offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adjustment {
    pub original_offset: usize,
    pub original_length: usize,
    pub output_length: usize,
}

impl Adjustment {
    pub fn new(original_offset: usize, original_length: usize, output_length: usize) -> Self {
        Self {
            original_offset,
            original_length,
            output_length,
        }
    }
}

/// Maps `offset` through `adjustments`. Returns `None` if the offset falls inside
/// a replaced range, or if the result is greater than `limit` (`None` means no
/// limit). `None` input is returned unchanged.
pub fn adjust_offset(
    adjustments: &[Adjustment],
    offset: Option<usize>,
    limit: Option<usize>,
) -> Option<usize> {
    let mut offset = offset.filter(|&o| o != NPOS)?;
    let mut original_lengths = 0usize;
    let mut output_lengths = 0usize;
    for adj in adjustments {
        if offset <= adj.original_offset {
            break;
        }
        if offset < adj.original_offset.wrapping_add(adj.original_length) {
            return None;
        }
        original_lengths = original_lengths.wrapping_add(adj.original_length);
        output_lengths = output_lengths.wrapping_add(adj.output_length);
    }
    offset = offset.wrapping_add(output_lengths.wrapping_sub(original_lengths));
    if limit.is_some_and(|limit| offset > limit) {
        return None;
    }
    Some(offset).filter(|&o| o != NPOS)
}

/// Applies [`adjust_offset`] to each offset.
pub fn adjust_offsets(
    adjustments: &[Adjustment],
    offsets: &[Option<usize>],
    limit: Option<usize>,
) -> Vec<Option<usize>> {
    offsets
        .iter()
        .map(|&offset| adjust_offset(adjustments, offset, limit))
        .collect()
}

/// Maps an offset in the adjusted string back to the original string. Returns
/// `None` if it falls inside a range that was produced by an adjustment.
pub fn unadjust_offset(adjustments: &[Adjustment], offset: Option<usize>) -> Option<usize> {
    let mut offset = offset.filter(|&o| o != NPOS)?;
    let mut original_lengths = 0usize;
    let mut output_lengths = 0usize;
    for adj in adjustments {
        let position = offset
            .wrapping_add(original_lengths)
            .wrapping_sub(output_lengths);
        if position <= adj.original_offset {
            break;
        }
        original_lengths = original_lengths.wrapping_add(adj.original_length);
        output_lengths = output_lengths.wrapping_add(adj.output_length);
        let position = offset
            .wrapping_add(original_lengths)
            .wrapping_sub(output_lengths);
        if position < adj.original_offset.wrapping_add(adj.original_length) {
            return None;
        }
    }
    offset = offset.wrapping_add(original_lengths.wrapping_sub(output_lengths));
    Some(offset).filter(|&o| o != NPOS)
}

/// Applies [`unadjust_offset`] to each offset.
pub fn unadjust_offsets(
    adjustments: &[Adjustment],
    offsets: &[Option<usize>],
) -> Vec<Option<usize>> {
    offsets
        .iter()
        .map(|&offset| unadjust_offset(adjustments, offset))
        .collect()
}

/// Combines two sequential sets of adjustments. `first_adjustments` changed the
/// original string into an intermediate string. `adjustments_on_adjusted_string`
/// are offsets in that intermediate string. The result is offsets in the
/// original string. Both inputs must be sorted by offset.
///
/// Only collapsing `first_adjustments` (output no longer than input) are
/// supported, as in the C++ code.
pub fn merge_sequential_adjustments(
    first_adjustments: &[Adjustment],
    adjustments_on_adjusted_string: &[Adjustment],
) -> Vec<Adjustment> {
    let mut adjusted = adjustments_on_adjusted_string.to_vec();
    let mut adjusted_index = 0usize;
    let mut first_index = 0usize;
    // Characters collapsed by `first_adjustments` before the current entry.
    let mut shift = 0usize;
    // Characters collapsed into the current entry's length, which change `shift`
    // only after that entry is emitted.
    let mut currently_collapsing = 0usize;
    let mut builder = Vec::with_capacity(adjusted.len() + first_adjustments.len());

    while adjusted_index < adjusted.len() {
        let adj = adjusted[adjusted_index];
        let first = first_adjustments.get(first_index).copied();
        let adj_start = adj.original_offset.wrapping_add(shift);
        let adj_end = adj_start.wrapping_add(adj.original_length);
        match first {
            // The whole entry (with its shift and length) comes before `first`.
            Some(f) if adj_end > f.original_offset => {
                if adj_start > f.original_offset {
                    // `first` comes before this entry: emit it and advance.
                    debug_assert!(
                        f.original_offset.wrapping_add(f.output_length) <= adj_start
                    );
                    shift = shift.wrapping_add(f.original_length.wrapping_sub(f.output_length));
                    builder.push(f);
                    first_index += 1;
                } else {
                    // `first` lies inside this entry: widen the entry by the
                    // characters that `first` collapsed.
                    debug_assert!(f.original_length > f.output_length);
                    let collapse = f.original_length.wrapping_sub(f.output_length);
                    adjusted[adjusted_index].original_length =
                        adj.original_length.wrapping_add(collapse);
                    currently_collapsing = currently_collapsing.wrapping_add(collapse);
                    first_index += 1;
                }
            }
            _ => {
                adjusted[adjusted_index].original_offset = adj_start;
                shift = shift.wrapping_add(currently_collapsing);
                currently_collapsing = 0;
                builder.push(adjusted[adjusted_index]);
                adjusted_index += 1;
            }
        }
    }
    debug_assert_eq!(currently_collapsing, 0);
    if first_index < first_adjustments.len() {
        // Only `first_adjustments` remain. Their offsets are already in the
        // original string.
        debug_assert_eq!(adjusted_index, adjusted.len());
        builder.extend_from_slice(&first_adjustments[first_index..]);
    }
    builder
}

/// Shared driver for the three conversion-with-adjustments functions below. It
/// records an adjustment each time a character's length changes.
fn convert_with_adjustments<S, D: Default>(
    src: &[S],
    read: impl FnMut(&[S], &mut usize) -> Option<char>,
    write: impl FnMut(char, &mut D) -> usize,
) -> (Converted<D>, Vec<Adjustment>) {
    let mut adjustments = Vec::new();
    let converted = transcode(src, read, write, |original, consumed, written| {
        adjustments.push(Adjustment::new(original, consumed, written));
    });
    (converted, adjustments)
}

/// Converts UTF-8 to UTF-16 and returns the adjustments. A U+FFFD replacement
/// for a multi-byte ill-formed sequence has a different length than the bytes
/// it covers, so it produces an adjustment.
pub fn utf8_to_utf16_with_adjustments(src: &[u8]) -> (Converted<Vec<u16>>, Vec<Adjustment>) {
    convert_with_adjustments(
        src,
        read_unicode_character_utf8,
        write_unicode_character_utf16,
    )
}

/// Converts UTF-8 to UTF-16 and maps each offset (into UTF-8 bytes) to the
/// UTF-16 position. Offsets past the end of `src` become `None`.
pub fn utf8_to_utf16_and_adjust_offsets(
    src: &[u8],
    offsets: &[Option<usize>],
) -> (Vec<u16>, Vec<Option<usize>>) {
    let checked: Vec<Option<usize>> = offsets
        .iter()
        .map(|offset| offset.filter(|&o| o <= src.len()))
        .collect();
    let (converted, adjustments) = utf8_to_utf16_with_adjustments(src);
    let adjusted = adjust_offsets(&adjustments, &checked, None);
    (converted.text, adjusted)
}

/// Converts UTF-16 to UTF-8 and maps each offset (into UTF-16 units) to the
/// UTF-8 byte position. Offsets past the end of `src` become `None`. A split
/// surrogate pair maps to `None`.
pub fn utf16_to_utf8_and_adjust_offsets(
    src: &[u16],
    offsets: &[Option<usize>],
) -> (String, Vec<Option<usize>>) {
    let checked: Vec<Option<usize>> = offsets
        .iter()
        .map(|offset| offset.filter(|&o| o <= src.len()))
        .collect();
    let (converted, adjustments) =
        convert_with_adjustments(src, read_unicode_character_utf16, write_unicode_character_utf8);
    let adjusted = adjust_offsets(&adjustments, &checked, None);
    (converted.text, adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversions::utf8_to_utf16;
    use crate::test_util::XorShift64;

    fn some(values: &[usize]) -> Vec<Option<usize>> {
        values.iter().map(|&v| Some(v)).collect()
    }

    // Port of UTFOffsetStringConversionsTest.AdjustOffset.
    #[test]
    fn adjust_offset() {
        // (utf8, input_offset, output_offset)
        let utf8_to_utf16_cases: &[(&[u8], Option<usize>, Option<usize>)] = &[
            (b"", Some(0), Some(0)),
            (b"", None, None),
            (b"\xe4\xbd\xa0\xe5\xa5\xbd", Some(1), None),
            (b"\xe4\xbd\xa0\xe5\xa5\xbd", Some(3), Some(1)),
            (b"\xed\xb0\x80z", Some(3), Some(3)),
            (b"A\xF0\x90\x8C\x80z", Some(1), Some(1)),
            (b"A\xF0\x90\x8C\x80z", Some(2), None),
            (b"A\xF0\x90\x8C\x80z", Some(5), Some(3)),
            (b"A\xF0\x90\x8C\x80z", Some(6), Some(4)),
            (b"A\xF0\x90\x8C\x80z", None, None),
        ];
        for &(utf8, input, expected) in utf8_to_utf16_cases {
            let (_, offsets) = utf8_to_utf16_and_adjust_offsets(utf8, &[input]);
            assert_eq!(offsets[0], expected);
        }

        // (utf16, input_offset, output_offset). The C++ strings are
        // NUL-terminated, so the zero-padded array is just its prefix here.
        let utf16_to_utf8_cases: &[(&[u16], Option<usize>, Option<usize>)] = &[
            (&[], Some(0), Some(0)),
            // Converted to 3-byte utf-8 sequences.
            (&[0x5909, 0x63DB], Some(3), None),
            (&[0x5909, 0x63DB], Some(2), Some(6)),
            (&[0x5909, 0x63DB], Some(1), Some(3)),
            (&[0x5909, 0x63DB], Some(0), Some(0)),
            // Converted to 2-byte utf-8 sequences.
            (&[b'A' as u16, 0x00bc, 0x00be, b'z' as u16], Some(1), Some(1)),
            (&[b'A' as u16, 0x00bc, 0x00be, b'z' as u16], Some(2), Some(3)),
            (&[b'A' as u16, 0x00bc, 0x00be, b'z' as u16], Some(3), Some(5)),
            (&[b'A' as u16, 0x00bc, 0x00be, b'z' as u16], Some(4), Some(6)),
            // Surrogate pair.
            (&[b'A' as u16, 0xd800, 0xdf00, b'z' as u16], Some(1), Some(1)),
            (&[b'A' as u16, 0xd800, 0xdf00, b'z' as u16], Some(2), None),
            (&[b'A' as u16, 0xd800, 0xdf00, b'z' as u16], Some(3), Some(5)),
            (&[b'A' as u16, 0xd800, 0xdf00, b'z' as u16], Some(4), Some(6)),
        ];
        for (i, &(utf16, input, expected)) in utf16_to_utf8_cases.iter().enumerate() {
            let (_, offsets) = utf16_to_utf8_and_adjust_offsets(utf16, &[input]);
            assert_eq!(offsets[0], expected, "case {i}");
        }
    }

    // Port of UTFOffsetStringConversionsTest.LimitOffsets.
    #[test]
    fn limit_offsets() {
        let limit = 10;
        let items = 20;
        let forward: Vec<Option<usize>> = (0..items)
            .map(|t| adjust_offset(&[], Some(t), Some(limit)))
            .collect();
        assert_eq!(forward.iter().flatten().count(), 11);

        // Reverse order and try again.
        let backward: Vec<Option<usize>> = (0..items)
            .rev()
            .map(|t| adjust_offset(&[], Some(t), Some(limit)))
            .collect();
        assert_eq!(backward.iter().flatten().count(), 11);
    }

    // Port of UTFOffsetStringConversionsTest.AdjustOffsets.
    #[test]
    fn adjust_offsets_cases() {
        // 1: abcXXXdef ==> abcXdef
        let offsets: Vec<usize> = (0..=9).collect();
        let adjustments = [Adjustment::new(3, 3, 1)];
        let expected = [
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            None,
            None,
            Some(4),
            Some(5),
            Some(6),
            Some(7),
        ];
        assert_eq!(adjust_offsets(&adjustments, &some(&offsets), None), expected);

        // 2: XXXaXXXXbcXXXXXXXdefXXX ==> XaXXbcXXXXdefX
        let offsets: Vec<usize> = (0..=23).collect();
        let adjustments = [
            Adjustment::new(0, 3, 1),
            Adjustment::new(4, 4, 2),
            Adjustment::new(10, 7, 4),
            Adjustment::new(20, 3, 1),
        ];
        let expected = [
            Some(0),
            None,
            None,
            Some(1),
            Some(2),
            None,
            None,
            None,
            Some(4),
            Some(5),
            Some(6),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(10),
            Some(11),
            Some(12),
            Some(13),
            None,
            None,
            Some(14),
        ];
        assert_eq!(adjust_offsets(&adjustments, &some(&offsets), None), expected);

        // 3: XXXaXXXXbcdXXXeXX ==> aXXXXbcdXXXe
        let offsets: Vec<usize> = (0..=17).collect();
        let adjustments = [
            Adjustment::new(0, 3, 0),
            Adjustment::new(4, 4, 4),
            Adjustment::new(11, 3, 3),
            Adjustment::new(15, 2, 0),
        ];
        let expected = [
            Some(0),
            None,
            None,
            Some(0),
            Some(1),
            None,
            None,
            None,
            Some(5),
            Some(6),
            Some(7),
            Some(8),
            None,
            None,
            Some(11),
            Some(12),
            None,
            Some(12),
        ];
        assert_eq!(adjust_offsets(&adjustments, &some(&offsets), None), expected);
    }

    // Port of UTFOffsetStringConversionsTest.UnadjustOffsets.
    #[test]
    fn unadjust_offsets_cases() {
        // 1: abcXXXdef ==> abcXdef
        let offsets: Vec<usize> = (0..=7).collect();
        let adjustments = [Adjustment::new(3, 3, 1)];
        assert_eq!(
            unadjust_offsets(&adjustments, &some(&offsets)),
            [Some(0), Some(1), Some(2), Some(3), Some(6), Some(7), Some(8), Some(9)]
        );

        // 2: XXXaXXXXbcXXXXXXXdefXXX ==> XaXXbcXXXXdefX
        let offsets: Vec<usize> = (0..=14).collect();
        let adjustments = [
            Adjustment::new(0, 3, 1),
            Adjustment::new(4, 4, 2),
            Adjustment::new(10, 7, 4),
            Adjustment::new(20, 3, 1),
        ];
        let expected = [
            Some(0),
            Some(3),
            Some(4),
            None,
            Some(8),
            Some(9),
            Some(10),
            None,
            None,
            None,
            Some(17),
            Some(18),
            Some(19),
            Some(20),
            Some(23),
        ];
        assert_eq!(unadjust_offsets(&adjustments, &some(&offsets)), expected);

        // 3: XXXaXXXXbcdXXXeXX ==> aXXXXbcdXXXe
        let offsets: Vec<usize> = (0..=12).collect();
        let adjustments = [
            Adjustment::new(0, 3, 0),
            Adjustment::new(4, 4, 4),
            Adjustment::new(11, 3, 3),
            Adjustment::new(15, 2, 0),
        ];
        let expected = [
            Some(0), // this could just as easily be 3
            Some(4),
            None,
            None,
            None,
            Some(8),
            Some(9),
            Some(10),
            Some(11),
            None,
            None,
            Some(14),
            Some(15), // this could just as easily be 17
        ];
        assert_eq!(unadjust_offsets(&adjustments, &some(&offsets)), expected);
    }

    // Port of UTFOffsetStringConversionsTest.MergeSequentialAdjustments.
    #[test]
    fn merge_sequential_adjustments_case() {
        // Pretend the input string is "abcdefghijklmnopqrstuvwxyz".
        //
        // first_adjustments: remove "a", combine "bc" into ".", remove "f", remove
        // "tuv". The result is ".deghijklmnopqrswxyz".
        let first_adjustments = [
            Adjustment::new(0, 1, 0),
            Adjustment::new(1, 2, 1),
            Adjustment::new(5, 1, 0),
            Adjustment::new(19, 3, 0),
        ];
        // adjustments_on_adjusted_string: combine "." with "d" into "?", remove
        // "egh", expand "i" into "12", combine "jkl" into "@", expand "z" into "34".
        // The result is "?12@mnopqrswxy34".
        let adjustments_on_adjusted_string = [
            Adjustment::new(0, 2, 1),
            Adjustment::new(2, 3, 0),
            Adjustment::new(5, 1, 2),
            Adjustment::new(6, 3, 1),
            Adjustment::new(19, 1, 2),
        ];
        let merged =
            merge_sequential_adjustments(&first_adjustments, &adjustments_on_adjusted_string);
        assert_eq!(
            merged,
            [
                Adjustment::new(0, 4, 1),
                Adjustment::new(4, 4, 0),
                Adjustment::new(8, 1, 2),
                Adjustment::new(9, 3, 1),
                Adjustment::new(19, 3, 0),
                Adjustment::new(25, 1, 2),
            ]
        );
    }

    #[test]
    fn utf8_with_adjustments_records_length_changes() {
        // E4 A0 is replaced by one U+FFFD (3 bytes in UTF-8, 1 unit in UTF-16).
        let (converted, adjustments) = utf8_to_utf16_with_adjustments(b"a\xe4\xa0b");
        assert!(!converted.success);
        assert_eq!(converted.text, [0x61, 0xfffd, 0x62]);
        // The replacement is 1 unit for 2 bytes, so it is recorded.
        assert_eq!(adjustments, [Adjustment::new(1, 2, 1)]);

        let (converted, adjustments) = utf8_to_utf16_with_adjustments(b"\xe4\xbd\xa0");
        assert!(converted.success);
        assert_eq!(converted.text, [0x4f60]);
        // 3 bytes for 1 unit is a length change, so an adjustment is recorded.
        assert_eq!(adjustments, [Adjustment::new(0, 3, 1)]);
    }

    #[test]
    fn utf16_to_utf8_offsets_out_of_range_are_none() {
        let (text, offsets) = utf16_to_utf8_and_adjust_offsets(&[0x41], &[Some(2), None]);
        assert_eq!(text, "A");
        assert_eq!(offsets, [None, None]);
    }

    #[test]
    fn adjust_offset_treats_npos_as_none() {
        let adjustments = [Adjustment::new(0, 2, 1)];
        assert_eq!(adjust_offset(&adjustments, Some(NPOS), None), None);
        assert_eq!(unadjust_offset(&adjustments, Some(NPOS)), None);
        assert_eq!(adjust_offset(&[], None, None), None);
    }

    // Differential test: for well-formed UTF-8, every character boundary must map
    // to the UTF-16 length of the prefix, and every interior byte to None.
    #[test]
    fn utf8_offsets_match_prefix_length_for_valid_text() {
        let mut rng = XorShift64::new(0x5eed_0101);
        for _ in 0..500 {
            let mut text = String::new();
            for _ in 0..rng.below(12) {
                let code_point = match rng.below(4) {
                    0 => rng.below(0x80) as u32,
                    1 => 0x80 + rng.below(0x780) as u32,
                    2 => 0x800 + rng.below(0xF800) as u32,
                    _ => 0x10000 + rng.below(0x100000) as u32,
                };
                if let Some(c) = char::from_u32(code_point) {
                    text.push(c);
                }
            }
            let bytes = text.as_bytes();
            let offsets: Vec<Option<usize>> = (0..=bytes.len() + 2).map(Some).collect();
            let (units, adjusted) = utf8_to_utf16_and_adjust_offsets(bytes, &offsets);
            assert_eq!(units, text.encode_utf16().collect::<Vec<_>>());
            for (offset, mapped) in adjusted.iter().enumerate() {
                let expected = if offset > bytes.len() {
                    None
                } else if text.is_char_boundary(offset) {
                    Some(text[..offset].encode_utf16().count())
                } else {
                    None
                };
                assert_eq!(*mapped, expected, "text {text:?} offset {offset}");
            }
        }
    }

    #[test]
    fn utf8_offsets_with_ill_formed_input_stay_in_range() {
        // Checks that every mapped offset is at most the output length. Ill-formed
        // input is also covered by the conversion tests.
        let input = b"\xf0\x9f\x98\x80\xff\xe4\xa0\xe5\xa5\xbd";
        let (units, adjusted) =
            utf8_to_utf16_and_adjust_offsets(input, &(0..=input.len()).map(Some).collect::<Vec<_>>());
        assert_eq!(units, utf8_to_utf16(input).text);
        for mapped in adjusted.into_iter().flatten() {
            assert!(mapped <= units.len());
        }
    }
}
