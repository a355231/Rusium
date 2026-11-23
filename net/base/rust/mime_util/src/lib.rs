// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `net::` MIME utilities (net/base/mime_util.{h,cc}), together
//! with the built-in MIME tables from the same file.
//!
//! The MIME tables (`kPrimaryMappings`, `kSecondaryMappings`) are copied
//! verbatim, in the same order, so lookups resolve the same way as in C++.
//!
//! Differences from the C++ version:
//!
//! - Platform lookups are not ported. `platform_mime_util_*.cc` asks the OS
//!   (XDG shared-mime-info on Linux, the registry on Windows, and so on). This
//!   crate only uses the built-in tables, which is the behavior the C++ code
//!   has when no platform database is consulted. As a result:
//!   - `get_mime_type_from_extension` and `get_well_known_mime_type_from_extension`
//!     return the same answers, because the only difference between them in C++
//!     is the platform step.
//!   - `get_preferred_extension_for_mime_type` and `get_extensions_for_mime_type`
//!     can return fewer entries than Chromium on a host whose OS adds or
//!     overrides mappings. The `kStandard*Types` lists exist only to query the
//!     platform database, so they are not ported.
//! - The ChromeOS-only `application/x-pkcs12` mapping (`p12,pfx`) is not
//!   included, and neither are its tests. The iOS-only `text/x-markdown`
//!   expectation is not included either. Both are compiled out of a Linux build
//!   of the C++ code.
//! - Functions return values instead of writing through out-parameters.
//!   `bool` plus `std::string*` becomes `Option<String>`. `ParseMimeType` returns
//!   `Option<ParsedMimeType>`. `GetExtensionsForMimeType` returns a `Vec` with no
//!   particular order in C++, and this port returns a sorted, de-duplicated
//!   `Vec`. The multipart helpers return the text to append instead of appending
//!   to a `post_data` buffer.
//! - `MimeTypeValidationLevel` has no default arguments. Callers pass the level
//!   explicitly.
//! - `ScopedOverrideGetMimeTypeForTesting` applies to the thread that created it.
//!   The C++ override is process-wide. Thread-local storage keeps parallel
//!   `cargo test` threads from seeing each other's overrides. It still does not
//!   support nesting, and dropping the guard clears the override.
//! - The C++ code `DCHECK`s that an extension has no leading dot. This port
//!   returns no match for such an extension and does not panic.
//! - `GenerateMimeMultipartBoundary` uses a xorshift generator seeded from
//!   `std`'s randomly keyed hasher, not `base::RandomChoice`. Boundaries only
//!   need to be unique, so this is enough, but the output is not
//!   cryptographically random.
//! - `base::StringPairs` is `Vec<(String, String)>`. Parameter splitting is
//!   delegated to the `string_split` sibling crate, and HTTP token and LWS
//!   checks are delegated to `http_util`. `FilePath::Extension` semantics come
//!   from the `file_path` sibling crate.
//! - Inputs are `&str`, so the C++ rule that strings must be UTF-8 does not
//!   apply. The C++ code works on bytes. Byte-level logic is kept, and the
//!   output is still valid UTF-8 because every split point is an ASCII
//!   character.
//! - `ParseMimeTypeWithoutParameter` in C++ returns `false` for invalid input
//!   and leaves its out-parameters alone. Here it returns `None`.

use std::cell::RefCell;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use file_path::FilePath;
use http_util::{is_lws, is_token as http_is_token, HTTP_LWS};
use string_split::split_string_into_key_value_pairs;

#[cfg(test)]
mod tests;

/// Key/value pairs from a parameter list, in input order, as `base::StringPairs`.
pub type StringPairs = Vec<(String, String)>;

/// Result of [`parse_mime_type`]: the type/subtype and any parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedMimeType {
    /// The `type/subtype` string, with leading and trailing LWS removed.
    pub mime_type: String,
    /// Parameters in input order. Keys keep their case. Invalid parameters are
    /// skipped.
    pub params: StringPairs,
}

/// How strictly wildcard patterns are checked. Mirrors
/// `net::MimeTypeValidationLevel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MimeTypeValidationLevel {
    /// No extra checks.
    #[default]
    None,
    /// For a wildcard pattern, the MIME type must contain exactly one '/'.
    WildcardSlashOnly,
    /// As `WildcardSlashOnly`, and the type and subtype must both be HTTP tokens.
    WildcardSlashAndTokens,
}

/// An (extension or MIME type, comma-separated extensions) row of the tables.
type MimeInfo = (&'static str, &'static str);

// How to use the MIME maps
// ------------------------
// Copied from mime_util.cc. Extensions that appear earlier take precedence.
// A MIME type must not appear more than once in a single list.
//
// Lookups:
// 1) MIME type -> extension: `get_preferred_extension_for_mime_type`.
//    application/octet-stream maps to nothing. Then kPrimaryMappings, then
//    kSecondaryMappings. The platform step is not ported.
// 2) Extension -> MIME type: `get_mime_type_from_extension`. Primary, then the
//    platform (not ported), then secondary.
// 3) Extension -> well-known MIME type: `get_well_known_mime_type_from_extension`.
//    Primary, then secondary.

// See comments above for details on how this list is used.
const PRIMARY_MAPPINGS: &[MimeInfo] = &[
    // Must precede audio/webm .
    ("video/webm", "webm"),
    // Must precede audio/mp3
    ("audio/mpeg", "mp3"),
    ("application/wasm", "wasm"),
    ("application/x-chrome-extension", "crx"),
    ("application/xhtml+xml", "xhtml,xht,xhtm"),
    ("audio/flac", "flac"),
    ("audio/matroska", "mka"),
    ("audio/mp3", "mp3"),
    ("audio/ogg", "ogg,oga,opus"),
    ("audio/wav", "wav"),
    ("audio/webm", "webm"),
    ("audio/x-m4a", "m4a"),
    ("image/avif", "avif"),
    ("image/gif", "gif"),
    ("image/jpeg", "jpeg,jpg,jpe"),
    ("image/jxl", "jxl"),
    ("image/png", "png"),
    ("image/apng", "png,apng"),
    ("image/svg+xml", "svg,svgz"),
    ("image/webp", "webp"),
    ("multipart/related", "mht,mhtml"),
    ("text/css", "css"),
    ("text/html", "html,htm,shtml,shtm"),
    ("text/javascript", "js,mjs"),
    ("text/xml", "xml"),
    ("video/matroska", "mkv"),
    ("video/mp4", "mp4,m4v"),
    ("video/ogg", "ogv,ogm"),
    // This is a primary mapping (overrides the platform) rather than secondary
    // to work around an issue when Excel is installed on Windows. Excel
    // registers csv as application/vnd.ms-excel instead of text/csv from RFC
    // 4180. See https://crbug.com/139105.
    ("text/csv", "csv"),
];

// See comments above for details on how this list is used.
const SECONDARY_MAPPINGS: &[MimeInfo] = &[
    // Must precede image/vnd.microsoft.icon .
    ("image/x-icon", "ico"),
    ("application/epub+zip", "epub"),
    ("application/font-woff", "woff"),
    ("application/gzip", "gz,tgz"),
    ("application/javascript", "js"),
    ("application/json", "json"), // Per http://www.ietf.org/rfc/rfc4627.txt.
    ("application/msword", "doc,dot"),
    ("application/octet-stream", "bin,exe,com"),
    ("application/pdf", "pdf"),
    ("application/pkcs7-mime", "p7m,p7c,p7z"),
    ("application/pkcs7-signature", "p7s"),
    ("application/postscript", "ps,eps,ai"),
    ("application/rdf+xml", "rdf"),
    ("application/rss+xml", "rss"),
    ("application/rtf", "rtf"),
    ("application/vnd.android.package-archive", "apk"),
    ("application/vnd.mozilla.xul+xml", "xul"),
    ("application/vnd.ms-excel", "xls"),
    ("application/vnd.ms-powerpoint", "ppt"),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
    ),
    ("application/x-gzip", "gz,tgz"),
    ("application/x-mpegurl", "m3u8"),
    // ChromeOS adds ("application/x-pkcs12", "p12,pfx") here. That entry is
    // not part of this Linux-equivalent port.
    ("application/x-shockwave-flash", "swf,swl"),
    ("application/x-tar", "tar"),
    ("application/x-x509-ca-cert", "cer,crt"),
    ("application/zip", "zip"),
    // This is the platform mapping on recent versions of Windows 10.
    ("audio/webm", "weba"),
    ("image/bmp", "bmp"),
    ("image/jpeg", "jfif,pjpeg,pjp"),
    ("image/tiff", "tiff,tif"),
    ("image/vnd.microsoft.icon", "ico"),
    ("image/x-png", "png"),
    ("image/x-xbitmap", "xbm"),
    ("message/rfc822", "eml"),
    ("text/calendar", "ics"),
    ("text/html", "ehtml"),
    ("text/markdown", "md"),
    ("text/plain", "txt,text"),
    ("text/vtt", "vtt"),
    ("text/x-sh", "sh"),
    ("text/xml", "xsl,xbl,xslt"),
    ("video/mpeg", "mpeg,mpg,mpe"),
];

// https://www.iana.org/assignments/media-types/media-types.xhtml
const LEGAL_TOP_LEVEL_TYPES: &[&str] = &[
    "application",
    "audio",
    "example",
    "font",
    "image",
    "message",
    "model",
    "multipart",
    "text",
    "video",
];

/// Longest extension accepted, in bytes. Matches the C++ `kMaxFilePathSize`
/// guard for crbug.com/48733.
const MAX_EXTENSION_LEN: usize = 65536;

/// `std::string::npos` in the C++ code.
const NPOS: usize = usize::MAX;

thread_local! {
    /// The override from [`ScopedOverrideGetMimeTypeForTesting`]. Empty means
    /// no override, as in the C++ code.
    static OVERRIDING_MIME_TYPE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn overriding_mime_type() -> Option<String> {
    OVERRIDING_MIME_TYPE.with(|o| {
        let o = o.borrow();
        if o.is_empty() {
            None
        } else {
            Some(o.clone())
        }
    })
}

// ---------------------------------------------------------------------------
// Byte-level helpers (mirror base::StartsWith/EndsWith, find_first_of, ...).
// ---------------------------------------------------------------------------

fn starts_with_ci(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && hay[..needle.len()].eq_ignore_ascii_case(needle)
}

fn ends_with_ci(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && hay[hay.len() - needle.len()..].eq_ignore_ascii_case(needle)
}

/// Like `std::string::find_first_of(set, from)`. Returns `NPOS` if none.
fn find_first_of(s: &[u8], set: &[u8], from: usize) -> usize {
    if from >= s.len() {
        return NPOS;
    }
    s[from..]
        .iter()
        .position(|c| set.contains(c))
        .map_or(NPOS, |p| p + from)
}

/// Like `std::string::find_first_not_of(set, from)`. Returns `NPOS` if none.
fn find_first_not_of(s: &[u8], set: &[u8], from: usize) -> usize {
    if from >= s.len() {
        return NPOS;
    }
    s[from..]
        .iter()
        .position(|c| !set.contains(c))
        .map_or(NPOS, |p| p + from)
}

/// Returns true for ASCII whitespace as in `base::IsAsciiWhitespace`, which
/// includes \v and \f.
fn is_ascii_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn is_token(s: &str) -> bool {
    http_is_token(s.as_bytes())
}

// ---------------------------------------------------------------------------
// Table lookups.
// ---------------------------------------------------------------------------

/// Returns the MIME type for `ext` from the first mapping that lists it.
/// Extensions are compared ASCII case-insensitively.
fn find_mime_type(mappings: &[MimeInfo], ext: &str) -> Option<&'static str> {
    mappings.iter().find_map(|&(mime_type, extensions)| {
        extensions
            .split(',')
            .any(|e| e.eq_ignore_ascii_case(ext))
            .then_some(mime_type)
    })
}

/// Returns the first extension of the first mapping for `mime_type`. The
/// comparison is case-sensitive, as in C++.
fn find_preferred_extension(mappings: &[MimeInfo], mime_type: &str) -> Option<String> {
    // There is no preferred extension for "application/octet-stream".
    if mime_type == "application/octet-stream" {
        return None;
    }
    mappings
        .iter()
        .find(|(m, _)| *m == mime_type)
        .map(|(_, extensions)| extensions.split(',').next().unwrap_or("").to_string())
}

/// Shared body of `GetMimeTypeFromExtension` and
/// `GetWellKnownMimeTypeFromExtension`. See the module docs for why the two
/// functions agree here.
fn mime_type_from_extension_helper(ext: &str) -> Option<String> {
    // Used for tests.
    if let Some(overriding) = overriding_mime_type() {
        return Some(overriding);
    }
    // Avoids crash when unable to handle a long file path. See crbug.com/48733.
    if ext.len() > MAX_EXTENSION_LEN {
        return None;
    }
    // Reject a string which contains a null character.
    if ext.contains('\0') {
        return None;
    }
    find_mime_type(PRIMARY_MAPPINGS, ext)
        .or_else(|| find_mime_type(SECONDARY_MAPPINGS, ext))
        .map(str::to_string)
}

/// Gets the MIME type for a file extension (no leading dot).
pub fn get_mime_type_from_extension(ext: &str) -> Option<String> {
    mime_type_from_extension_helper(ext)
}

/// Gets the well-known MIME type for a file extension (no leading dot). Only
/// the built-in tables are consulted. In this port that is the same as
/// [`get_mime_type_from_extension`].
pub fn get_well_known_mime_type_from_extension(ext: &str) -> Option<String> {
    mime_type_from_extension_helper(ext)
}

fn mime_type_from_file_helper(file_path: &FilePath) -> Option<String> {
    // `extension()` includes the leading dot. An empty result means there is
    // no extension.
    let ext = file_path.extension();
    if ext.is_empty() {
        return None;
    }
    mime_type_from_extension_helper(&ext[1..])
}

/// Gets the MIME type for the extension of `file_path`.
pub fn get_mime_type_from_file(file_path: &FilePath) -> Option<String> {
    mime_type_from_file_helper(file_path)
}

/// Gets the well-known MIME type for the extension of `file_path`. In this port
/// that is the same as [`get_mime_type_from_file`].
pub fn get_well_known_mime_type_from_file(file_path: &FilePath) -> Option<String> {
    mime_type_from_file_helper(file_path)
}

/// Gets the preferred extension for `mime_type`, without a leading dot, such as
/// `"html"`. The platform step is not ported.
pub fn get_preferred_extension_for_mime_type(mime_type: &str) -> Option<String> {
    find_preferred_extension(PRIMARY_MAPPINGS, mime_type)
        .or_else(|| find_preferred_extension(SECONDARY_MAPPINGS, mime_type))
}

// ---------------------------------------------------------------------------
// Wildcard matching.
// ---------------------------------------------------------------------------

/// Returns the part of `s` before the first ';', as `base::` code computes it.
fn base_part(s: &str) -> &str {
    s.split_once(';').map_or(s, |(base, _)| base)
}

fn to_lower_map(pairs: Vec<(String, String)>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (key, value) in pairs {
        // Assignment, so a later duplicate key wins, as in C++ `operator[]`.
        map.insert(key.to_ascii_lowercase(), value);
    }
    map
}

/// Checks that each parameter in `pattern` is present in `mime_type` with the
/// same value. Keys are compared case-insensitively. Values are compared
/// case-sensitively, which may give false negatives, as in C++. If `pattern`
/// has no parameters, the match succeeds.
fn matches_mime_type_parameters(pattern: &str, mime_type: &str) -> bool {
    let Some(semicolon) = pattern.find(';') else {
        return true;
    };
    let Some(test_semicolon) = mime_type.find(';') else {
        return false;
    };

    let pattern_params = to_lower_map(
        split_string_into_key_value_pairs(&pattern[semicolon + 1..], '=', ';').pairs,
    );
    let test_params = to_lower_map(
        split_string_into_key_value_pairs(&mime_type[test_semicolon + 1..], '=', ';').pairs,
    );

    if pattern_params.len() > test_params.len() {
        return false;
    }
    pattern_params
        .iter()
        .all(|(key, value)| test_params.get(key) == Some(value))
}

/// Returns true if `mime_type_pattern` matches `mime_type`. Supports absolute
/// matching and the wildcard forms `*`, `*/*`, `type/*`, `*+suffix`, and a
/// single `*` anywhere. MIME types are case-insensitive. Parameters in the
/// pattern must all appear in `mime_type`.
pub fn matches_mime_type(
    mime_type_pattern: &str,
    mime_type: &str,
    validation_level: MimeTypeValidationLevel,
) -> bool {
    if mime_type_pattern.is_empty() {
        return false;
    }

    let base_pattern = base_part(mime_type_pattern);
    let base_type = base_part(mime_type);

    if validation_level != MimeTypeValidationLevel::None && base_pattern.contains('*') {
        match validation_level {
            MimeTypeValidationLevel::WildcardSlashAndTokens => {
                let Some((top, sub)) = base_type.split_once('/') else {
                    return false;
                };
                if !is_token(top) || !is_token(sub) {
                    return false;
                }
            }
            MimeTypeValidationLevel::WildcardSlashOnly => {
                if base_type.matches('/').count() != 1 {
                    return false;
                }
            }
            MimeTypeValidationLevel::None => {}
        }
    }

    if base_pattern == "*" || base_pattern == "*/*" {
        return matches_mime_type_parameters(mime_type_pattern, mime_type);
    }

    let pattern_bytes = base_pattern.as_bytes();
    let type_bytes = base_type.as_bytes();

    let Some(star) = base_pattern.find('*') else {
        return base_pattern.eq_ignore_ascii_case(base_type)
            && matches_mime_type_parameters(mime_type_pattern, mime_type);
    };

    // Test length to prevent overlap between `left` and `right`.
    if type_bytes.len() < pattern_bytes.len() - 1 {
        return false;
    }

    let left = &pattern_bytes[..star];
    let right = &pattern_bytes[star + 1..];

    if !starts_with_ci(type_bytes, left) {
        return false;
    }
    if !right.is_empty() && !ends_with_ci(type_bytes, right) {
        return false;
    }

    matches_mime_type_parameters(mime_type_pattern, mime_type)
}

// ---------------------------------------------------------------------------
// Parsing.
// ---------------------------------------------------------------------------

/// Parses `type_str` into a MIME type and its parameters.
///
/// Returns `None` if the type has no '/' before the first ';', '(' or LWS. The
/// type is trimmed of LWS. Parameter parsing is lenient, and invalid parameters
/// are skipped. Quoted values keep their whitespace and process backslash
/// escapes. Unquoted values have trailing LWS removed.
///
/// Control characters other than LWS are not rejected here. For example,
/// `"text/html\n"` keeps the newline in the result, as in C++.
pub fn parse_mime_type(type_str: &str) -> Option<ParsedMimeType> {
    let s = type_str.as_bytes();
    let len = s.len();

    // Trim leading and trailing whitespace from type. '(' is in the trailing
    // set to catch media-type comments.
    let type_val = find_first_not_of(s, HTTP_LWS, 0).min(len);
    let mut type_end = find_first_of(s, b" \t;(", type_val);
    if type_end == NPOS {
        type_end = len;
    }

    // Reject a mime-type if it does not include a slash.
    let slash_pos = find_first_of(s, b"/", 0);
    if slash_pos == NPOS || slash_pos > type_end {
        return None;
    }
    let mime_type = type_str[type_val..type_end].to_string();

    // Iterate over parameters. Quoted strings may contain semicolons, so the
    // input is not split on ';' up front.
    let mut params: StringPairs = Vec::new();
    let mut offset = find_first_of(s, b";", type_end);
    while offset < len {
        // Trim off the semicolon.
        offset += 1;

        // Trim off any following spaces.
        offset = find_first_not_of(s, HTTP_LWS, offset);
        let param_name_start = offset;

        // Extend the parameter name until a semicolon or equals sign. Trailing
        // spaces are kept.
        offset = find_first_of(s, b";=", offset);

        // Nothing more to do at the end of the string, or if there is no
        // value, since names without values are not allowed.
        if offset == NPOS || s[offset] == b';' {
            continue;
        }
        let param_name = type_str[param_name_start..offset].to_string();

        // Trim off the '='.
        offset += 1;

        // Remove leading spaces. This violates the spec, as in the C++ code.
        offset = find_first_not_of(s, HTTP_LWS, offset);

        if offset == NPOS || s[offset] == b';' {
            // An unquoted value that is only whitespace is skipped.
            continue;
        }

        let param_value = if s[offset] != b'"' {
            // Not quoted: copy the data directly.
            let value_start = offset;
            offset = find_first_of(s, b";", offset);
            let mut value_end = if offset == NPOS { len } else { offset };
            // Remove terminal whitespace.
            while value_end > value_start && is_lws(s[value_end - 1]) {
                value_end -= 1;
            }
            type_str[value_start..value_end].to_string()
        } else {
            // Quoted: append data, handling backslashes, up to a close quote.
            // Whitespace is not trimmed.
            offset += 1;
            let mut bytes = Vec::new();
            while offset < len && s[offset] != b'"' {
                // Skip over a backslash and append the next byte. At the end of
                // the string, the backslash itself is copied.
                if s[offset] == b'\\' && offset + 1 < len {
                    offset += 1;
                }
                bytes.push(s[offset]);
                offset += 1;
            }
            offset = find_first_of(s, b";", offset);
            // Only whole characters were copied, so this is valid UTF-8.
            String::from_utf8_lossy(&bytes).into_owned()
        };
        params.push((param_name, param_value));
    }

    Some(ParsedMimeType { mime_type, params })
}

/// Parses `type_string` as `top_level/subtype` with no parameters. Surrounding
/// whitespace is allowed, but whitespace around the '/' is not. Both parts
/// must be HTTP tokens. Returns `(top_level_type, subtype)`.
pub fn parse_mime_type_without_parameter(type_string: &str) -> Option<(String, String)> {
    let components: Vec<&str> = type_string.split('/').collect();
    if components.len() != 2 {
        return None;
    }
    let top = components[0].trim_start_matches(is_ascii_ws);
    let sub = components[1].trim_end_matches(is_ascii_ws);
    if !is_token(top) || !is_token(sub) {
        return None;
    }
    Some((top.to_string(), sub.to_string()))
}

/// Extracts `type/subtype` from a Content-Type style value. Parameters are
/// dropped. If `accept_comma_separated` is true, the value is cut at the first
/// ',' too, so `"text/html,text/plain"` gives `"text/html"`.
pub fn extract_mime_type_from_media_type(
    type_string: &str,
    accept_comma_separated: bool,
) -> Option<String> {
    let mut end = type_string.find(';').unwrap_or(type_string.len());
    if accept_comma_separated {
        end = end.min(type_string.find(',').unwrap_or(type_string.len()));
    }
    parse_mime_type_without_parameter(&type_string[..end])
        .map(|(top, sub)| format!("{top}/{sub}"))
}

/// Returns true if `type_string` is an IANA top-level media type, or an
/// experimental `x-` type. The FreeDesktop pseudo-types `x-scheme-handler`,
/// `x-content`, `x-directory` and `x-special` are rejected. The check is
/// ASCII case-insensitive. The input is not checked for token syntax.
pub fn is_valid_top_level_mime_type(type_string: &str) -> bool {
    let lower = type_string.to_ascii_lowercase();
    if LEGAL_TOP_LEVEL_TYPES.contains(&lower.as_str()) {
        return true;
    }
    if matches!(
        lower.as_str(),
        "x-scheme-handler" | "x-content" | "x-directory" | "x-special"
    ) {
        return false;
    }
    lower.len() > 2 && lower.starts_with("x-")
}

// ---------------------------------------------------------------------------
// Extension lists.
// ---------------------------------------------------------------------------

/// Adds to `out` every extension of each mapping whose MIME type matches
/// `mime_type`. With `prefix_match`, `mime_type` is a case-insensitive prefix,
/// otherwise it must be the whole type.
fn collect_extensions_from_mappings(
    mappings: &[MimeInfo],
    mime_type: &str,
    prefix_match: bool,
    out: &mut BTreeSet<String>,
) {
    for &(current, extensions) in mappings {
        let matches = starts_with_ci(current.as_bytes(), mime_type.as_bytes())
            && (prefix_match || current.len() == mime_type.len());
        if matches {
            out.extend(extensions.split(',').map(str::to_string));
        }
    }
}

/// Returns the extensions for `mime_type`, such as `["html", "htm", ...]` for
/// `"text/html"`. Wildcards are supported: `"video/*"` returns every video
/// extension in the built-in tables. `"*"` and `"*/*"` return nothing.
/// The result is sorted and has no duplicates.
pub fn get_extensions_for_mime_type(unsafe_mime_type: &str) -> Vec<String> {
    if unsafe_mime_type == "*/*" || unsafe_mime_type == "*" {
        return Vec::new();
    }

    let mime_type = unsafe_mime_type.to_ascii_lowercase();
    let mut unique = BTreeSet::new();

    if mime_type.ends_with("/*") {
        // Keep the trailing slash, so "video/*" becomes the prefix "video/".
        let leading = &mime_type[..mime_type.len() - 1];
        collect_extensions_from_mappings(PRIMARY_MAPPINGS, leading, true, &mut unique);
        collect_extensions_from_mappings(SECONDARY_MAPPINGS, leading, true, &mut unique);
    } else {
        collect_extensions_from_mappings(PRIMARY_MAPPINGS, &mime_type, false, &mut unique);
        collect_extensions_from_mappings(SECONDARY_MAPPINGS, &mime_type, false, &mut unique);
    }

    unique.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Multipart upload helpers.
// ---------------------------------------------------------------------------

/// Small xorshift64 generator, used for boundaries. It is not cryptographic.
struct Rng(u64);

impl Rng {
    fn from_seed(seed: u64) -> Self {
        // xorshift must not be zero.
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    /// Seeds from `std`'s randomly keyed hasher and the clock.
    fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(nanos);
        Self::from_seed(hasher.finish())
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Returns a value in `0..n`. The modulo bias is negligible for n = 62.
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

fn generate_mime_multipart_boundary_with(rng: &mut Rng) -> String {
    // RFC 1341 allows a boundary of up to 70 characters. Some sites break on
    // several legal characters, so this uses letters and digits only.
    const BOUNDARY_CHARACTERS: &[u8] =
        b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    // Size of the multipart boundary.
    const BOUNDARY_SIZE: usize = 69;

    let mut result = String::with_capacity(BOUNDARY_SIZE);
    result.push_str("----MultipartBoundary--");
    while result.len() < BOUNDARY_SIZE - 4 {
        result.push(BOUNDARY_CHARACTERS[rng.below(BOUNDARY_CHARACTERS.len())] as char);
    }
    result.push_str("----");
    debug_assert_eq!(result.len(), BOUNDARY_SIZE);
    result
}

/// Generates a random multipart boundary of exactly 69 characters, with the
/// same shape as the C++ output.
pub fn generate_mime_multipart_boundary() -> String {
    generate_mime_multipart_boundary_with(&mut Rng::from_entropy())
}

fn push_content_type_and_value(out: &mut String, value: &str, content_type: &str) {
    if !content_type.is_empty() {
        // If Content-Type is specified, the next line is that.
        out.push_str("Content-Type: ");
        out.push_str(content_type);
        out.push_str("\r\n");
    }
    // Leave an empty line and append the value.
    out.push_str("\r\n");
    out.push_str(value);
    out.push_str("\r\n");
}

/// Returns one form-data part for a multipart upload, with the boundary line,
/// the Content-Disposition line, and the optional Content-Type and value. The
/// caller appends the returned text to its body.
pub fn add_multipart_value_for_upload(
    value_name: &str,
    value: &str,
    mime_boundary: &str,
    content_type: &str,
) -> String {
    let mut out = format!(
        "--{mime_boundary}\r\nContent-Disposition: form-data; name=\"{value_name}\"\r\n"
    );
    push_content_type_and_value(&mut out, value, content_type);
    out
}

/// As [`add_multipart_value_for_upload`], but also includes a `filename`
/// parameter in Content-Disposition.
pub fn add_multipart_value_with_file_name_for_upload(
    value_name: &str,
    file_name: &str,
    value: &str,
    mime_boundary: &str,
    content_type: &str,
) -> String {
    let mut out = format!(
        "--{mime_boundary}\r\nContent-Disposition: form-data; name=\"{value_name}\"; filename=\"{file_name}\"\r\n"
    );
    push_content_type_and_value(&mut out, value, content_type);
    out
}

/// Returns the closing delimiter of a multipart body.
pub fn add_multipart_final_delimiter_for_upload(mime_boundary: &str) -> String {
    format!("--{mime_boundary}--\r\n")
}

// ---------------------------------------------------------------------------
// Test-only override.
// ---------------------------------------------------------------------------

/// A test-only helper. While it is alive, every extension lookup on this thread
/// returns `overriding_mime_type`. Other behavior is unaffected. Does not
/// support nesting: dropping any guard clears the override.
pub struct ScopedOverrideGetMimeTypeForTesting {
    _private: (),
}

impl ScopedOverrideGetMimeTypeForTesting {
    /// Starts overriding extension lookups with `overriding_mime_type`.
    pub fn new(overriding_mime_type: &str) -> Self {
        OVERRIDING_MIME_TYPE.with(|o| *o.borrow_mut() = overriding_mime_type.to_string());
        ScopedOverrideGetMimeTypeForTesting { _private: () }
    }
}

impl Drop for ScopedOverrideGetMimeTypeForTesting {
    fn drop(&mut self) {
        let _ = OVERRIDING_MIME_TYPE.try_with(|o| o.borrow_mut().clear());
    }
}
