// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `url/url_parse_file.cc`: parsing of `file:` URLs, non-Windows rules.
//!
//! The C++ `#ifdef WIN32` branches (drive letters such as `c:` and UNC paths
//! with backslashes) are not ported. On non-Windows builds the C++ code takes
//! the branches implemented here.
//!
//! Examples (C++ comment, non-Windows):
//! - `file:/foo/bar` is path `/foo/bar` with no host.
//! - `file://foo/bar` has host `foo` and path `/bar`.
//! - `file:///foo` has no host and path `/foo`.
//! - `file:////foo` has no host and path `/foo`. Only exactly two slashes
//!   give a host (UNC-style).

use crate::{
    count_consecutive_slashes_or_backslashes, do_extract_scheme, is_slash_or_backslash,
    make_range, parse_path_internal, trim_url, Parsed, PathComponents, Unit,
};

/// Returns the index of the next slash or backslash at or after `begin_index`,
/// or `url.len()` if there is none.
fn find_next_slash<U: Unit>(url: &[U], begin_index: usize) -> usize {
    let mut idx = begin_index;
    while idx < url.len() && !is_slash_or_backslash(url[idx].code()) {
        idx += 1;
    }
    idx
}

fn set_path(parsed: &mut Parsed, parts: PathComponents) {
    parsed.path = parts.path;
    parsed.query = parts.query;
    parsed.reference = parts.reference;
}

/// Port of `DoParseUnc`. `after_slashes` is the offset of the first character
/// after the slashes that follow `file:`. The text up to the next slash is the
/// host. The rest is the path. Leaves the other components untouched.
fn do_parse_unc<U: Unit>(url: &[U], after_slashes: usize, parsed: &mut Parsed) {
    let url_len = url.len();
    let next_slash = find_next_slash(url, after_slashes);

    // Everything up to the first slash (or end) is the host. For example,
    // "file://foo/bar.txt" has host "foo" and path "/bar.txt".
    if after_slashes < next_slash {
        parsed.host = make_range(after_slashes, next_slash);
    } else {
        parsed.host.reset();
    }

    if next_slash < url_len {
        set_path(parsed, parse_path_internal(url, make_range(next_slash, url_len)));
    } else {
        parsed.path.reset();
    }
}

/// Port of `DoParseLocalFile`. The input is a local file path starting at
/// `path_begin`. There is no host.
fn do_parse_local_file<U: Unit>(url: &[U], path_begin: usize, parsed: &mut Parsed) {
    parsed.host.reset();
    set_path(
        parsed,
        parse_path_internal(url, make_range(path_begin, url.len())),
    );
}

/// Port of `DoParseFileUrl` (non-Windows). Accepts input with a scheme, or
/// starting at the first character after `file:`, such as `"file:c:\foo"` or
/// `"/foo.c:5"`.
pub(crate) fn do_parse_file_url<U: Unit>(url_full: &[U]) -> Parsed {
    // Strip leading and trailing spaces and control characters.
    let (begin, end) = trim_url(url_full, true);
    let url = &url_full[..end];

    let mut parsed = Parsed::default();

    // With leading slashes there is no scheme. This is how "/foo.c:5" stays a
    // path rather than a "/foo.c" scheme.
    let num_slashes = count_consecutive_slashes_or_backslashes(url, begin);
    let scheme = if num_slashes == 0 {
        do_extract_scheme(&url[begin..]).map(|mut s| {
            s.offset_by(begin);
            s
        })
    } else {
        None
    };
    let after_scheme = match scheme {
        Some(s) => {
            parsed.scheme = s;
            s.checked_end() + 1
        }
        None => {
            parsed.scheme.reset();
            begin
        }
    };

    // Empty input, whitespace only, or just the scheme ("file:").
    if after_scheme == url.len() {
        return parsed;
    }

    let num_slashes = count_consecutive_slashes_or_backslashes(url, after_scheme);
    let after_slashes = after_scheme + num_slashes;

    // Exactly two slashes means a host follows (UNC-style: "file://server/x").
    if num_slashes == 2 {
        do_parse_unc(url, after_slashes, &mut parsed);
        return parsed;
    }

    // Common case: the path follows the slashes. Keep the last slash as part of
    // the path ("file:///foo" has path "/foo").
    let path_begin = if num_slashes > 0 {
        after_scheme + num_slashes - 1
    } else {
        after_scheme
    };
    do_parse_local_file(url, path_begin, &mut parsed);
    parsed
}

/// Parses `url` as a `file:` URL (non-Windows rules). The scheme is
/// recognized only when there are no leading slashes.
pub fn parse_file_url<U: Unit>(url: &[U]) -> Parsed {
    do_parse_file_url(url)
}
