// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! ReplaceStringPlaceholders from base/strings/string_util.h, with the strict
//! mode from string_util_impl_helpers.h.

use crate::code_unit::CodeUnit;

/// Core of `DoReplaceStringPlaceholders` for the `$` prefix.
///
/// - `$1`-`$9` are replaced by the matching substitution. Other digits are not
///   placeholders.
/// - `$$` produces one `$`. Runs of N dollar signs produce N-1 dollar signs.
/// - In non-strict mode, a `$` followed by anything else drops that character
///   and continues. A trailing `$` is dropped.
/// - In strict mode, those cases and a placeholder whose index is past the end
///   of `subst` make the function return `None`.
/// - When `want_offsets` is set, each placeholder's output position is recorded
///   with its index and returned in index order (the C++ `offsets` out-param).
///   A placeholder whose index is past the end of `subst` is recorded too, at
///   the position where it would have been inserted.
fn replace_core<T: CodeUnit, S: AsRef<[T]>>(
    format: &[T],
    subst: &[S],
    strict: bool,
    want_offsets: bool,
) -> Option<(Vec<T>, Vec<usize>)> {
    debug_assert!(subst.len() < 10, "only nine substitutions are supported");
    let dollar = T::from_u32_truncating(u32::from(b'$'));
    let mut formatted: Vec<T> = Vec::with_capacity(format.len());
    // (parameter index, output offset), kept sorted by parameter index.
    let mut r_offsets: Vec<(usize, usize)> = Vec::new();

    let mut i = 0;
    while i < format.len() {
        let c = format[i];
        if c != dollar {
            formatted.push(c);
            i += 1;
            continue;
        }
        if i + 1 == format.len() {
            if strict {
                return None;
            }
            // A trailing '$' is dropped.
            i += 1;
            continue;
        }
        i += 1;
        if format[i] == dollar {
            // Escape: each run of dollar signs gives one fewer '$'.
            loop {
                formatted.push(dollar);
                i += 1;
                if i >= format.len() || format[i] != dollar {
                    break;
                }
            }
            // `i` is now one past the run. The loop below advances once more, so
            // step back to the last '$' in the run.
            i -= 1;
        } else {
            let d = format[i].to_u32();
            if !((u32::from(b'1')..=u32::from(b'9')).contains(&d)) {
                if strict {
                    return None;
                }
                // Non-strict: the invalid character after '$' is skipped.
            } else {
                let index = (d - u32::from(b'1')) as usize;
                if want_offsets {
                    let at = r_offsets.partition_point(|e| e.0 <= index);
                    r_offsets.insert(at, (index, formatted.len()));
                }
                if let Some(s) = subst.get(index) {
                    formatted.extend_from_slice(s.as_ref());
                } else if strict {
                    return None;
                }
            }
        }
        i += 1;
    }

    let offsets = r_offsets.into_iter().map(|e| e.1).collect();
    Some((formatted, offsets))
}

/// C++ `ReplaceStringPlaceholders(format, subst, nullptr)`. Replaces `$1`-`$9`
/// with the matching entry of `subst`. Placeholders with no matching entry are
/// dropped. See [`replace_string_placeholders_with_offsets`] for the offsets.
pub fn replace_string_placeholders<T: CodeUnit, S: AsRef<[T]>>(format: &[T], subst: &[S]) -> Vec<T> {
    replace_core(format, subst, false, false)
        .map(|(s, _)| s)
        .unwrap_or_default()
}

/// C++ `ReplaceStringPlaceholders(format, subst, &offsets)`. Returns the
/// formatted string and the output offset of each placeholder, in parameter
/// index order. Offsets also cover placeholders with no matching entry, as the
/// C++ code does.
pub fn replace_string_placeholders_with_offsets<T: CodeUnit, S: AsRef<[T]>>(
    format: &[T],
    subst: &[S],
) -> (Vec<T>, Vec<usize>) {
    replace_core(format, subst, false, true).unwrap_or_default()
}

/// C++ `ReplaceStringPlaceholders(format, subst, size_t* offset)`: the one-
/// substitution shortcut. Returns the formatted string and the output offset of
/// the placeholder.
///
/// Intentional difference: the C++ code CHECK-fails unless there is exactly
/// one recorded placeholder. This port returns `None` for that case instead of
/// aborting.
pub fn replace_string_placeholders_with_offset<T: CodeUnit>(
    format: &[T],
    subst: &[T],
) -> (Vec<T>, Option<usize>) {
    let (text, offsets) = replace_string_placeholders_with_offsets(format, &[subst]);
    let offset = match offsets.as_slice() {
        [only] => Some(*only),
        _ => None,
    };
    (text, offset)
}

/// C++ `DoReplaceStringPlaceholders` with `is_strict_mode` set. Returns `None`
/// where the C++ code returns `std::nullopt`: a placeholder index past the end
/// of `subst`, a `$` followed by a character that is not `1`-`9` or `$`, or a
/// trailing `$`. The C++ version is internal, so this is an addition.
pub fn replace_string_placeholders_strict<T: CodeUnit, S: AsRef<[T]>>(
    format: &[T],
    subst: &[S],
) -> Option<Vec<T>> {
    replace_core(format, subst, true, false).map(|(s, _)| s)
}
