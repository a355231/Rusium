// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/// A single byte range from a `Range` header, like `net::HttpByteRange`.
///
/// Only the parts needed by the header parsers in this crate are ported:
/// constructors, accessors and `is_valid()`. `ComputeBounds()` and
/// `GetHeaderValue()` are not ported.
///
/// Unlike the C++ class, which uses -1 as "not specified", each position is an
/// `Option`, so a parsed value of -1 is kept as -1 and is not mistaken for an
/// absent value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HttpByteRange {
    first_byte_position: Option<i64>,
    last_byte_position: Option<i64>,
    suffix_length: Option<i64>,
}

impl HttpByteRange {
    /// A range `first-last`.
    pub fn bounded(first_byte_position: i64, last_byte_position: i64) -> Self {
        HttpByteRange {
            first_byte_position: Some(first_byte_position),
            last_byte_position: Some(last_byte_position),
            suffix_length: None,
        }
    }

    /// A range `first-`, running to the end of the resource.
    pub fn right_unbounded(first_byte_position: i64) -> Self {
        HttpByteRange {
            first_byte_position: Some(first_byte_position),
            last_byte_position: None,
            suffix_length: None,
        }
    }

    /// A suffix range `-length`, selecting the last `suffix_length` bytes.
    pub fn suffix(suffix_length: i64) -> Self {
        HttpByteRange {
            first_byte_position: None,
            last_byte_position: None,
            suffix_length: Some(suffix_length),
        }
    }

    /// The first byte position, if one was given.
    pub fn first_byte_position(&self) -> Option<i64> {
        self.first_byte_position
    }

    /// The last byte position, if one was given.
    pub fn last_byte_position(&self) -> Option<i64> {
        self.last_byte_position
    }

    /// The suffix length, if this is a suffix range.
    pub fn suffix_length(&self) -> Option<i64> {
        self.suffix_length
    }

    /// True if this is a suffix range (`-N`).
    pub fn is_suffix_byte_range(&self) -> bool {
        self.suffix_length.is_some()
    }

    /// True if a first byte position was given.
    pub fn has_first_byte_position(&self) -> bool {
        self.first_byte_position.is_some()
    }

    /// True if a last byte position was given.
    pub fn has_last_byte_position(&self) -> bool {
        self.last_byte_position.is_some()
    }

    /// Mirrors `net::HttpByteRange::IsValid()`. A suffix range is valid when
    /// its length is positive. Otherwise the first position must be
    /// non-negative, and a last position, if present, must not be before it.
    pub fn is_valid(&self) -> bool {
        if matches!(self.suffix_length, Some(len) if len > 0) {
            return true;
        }
        match self.first_byte_position {
            Some(first) if first >= 0 => match self.last_byte_position {
                None => true,
                Some(last) => last >= first,
            },
            _ => false,
        }
    }
}
