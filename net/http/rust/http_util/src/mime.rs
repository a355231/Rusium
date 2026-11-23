// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `net::ParseMimeType()` (net/base/mime_util.cc). Only the parts used
//! by `http_util` are ported.

/// Linear white space, `HTTP_LWS` in the C++ code.
const HTTP_LWS: &[u8] = b" \t";

fn find_first_not_of(s: &[u8], set: &[u8], from: usize) -> Option<usize> {
    (from..s.len()).find(|&i| !set.contains(&s[i]))
}

fn find_first_of(s: &[u8], set: &[u8], from: usize) -> Option<usize> {
    (from..s.len()).find(|&i| set.contains(&s[i]))
}

/// A parsed `name=value` pair. The value has had quoted-string quotes removed
/// and escapes processed, as in the C++ code.
pub(crate) type MimeParam = (Vec<u8>, Vec<u8>);

/// Parses a MIME type and its parameters. Returns `None` if there is no '/'
/// before the first ';', '(' or LWS. Otherwise returns the type (with LWS
/// trimmed on both ends) and the parameters in order.
///
/// Mostly follows https://mimesniff.spec.whatwg.org/ but, as in the C++ code,
/// it does not validate token characters, and it ignores spaces after '='.
pub(crate) fn parse_mime_type(type_str: &[u8]) -> Option<(Vec<u8>, Vec<MimeParam>)> {
    let n = type_str.len();
    let type_val = find_first_not_of(type_str, HTTP_LWS, 0).unwrap_or(n).min(n);
    let type_end = find_first_of(type_str, b" \t;(", type_val).unwrap_or(n);

    // Reject a mime-type if it does not include a slash.
    let slash_pos = type_str.iter().position(|&c| c == b'/')?;
    if slash_pos > type_end {
        return None;
    }
    let mime_type = type_str[type_val..type_end].to_vec();

    let mut params = Vec::new();
    let mut pos = find_first_of(type_str, b";", type_end);
    while let Some(semi) = pos {
        // Skip the ';' and any following spaces.
        let Some(name_start) = find_first_not_of(type_str, HTTP_LWS, semi + 1) else {
            break;
        };
        // The name runs to the next ';' or '='. Trailing spaces are kept.
        let Some(name_end) = find_first_of(type_str, b";=", name_start) else {
            break;
        };
        if type_str[name_end] == b';' {
            pos = Some(name_end);
            continue;
        }
        let param_name = type_str[name_start..name_end].to_vec();

        // Skip the '=' and leading spaces. This departs from the spec but
        // matches the pre-existing C++ behavior.
        let Some(value_start) = find_first_not_of(type_str, HTTP_LWS, name_end + 1) else {
            break;
        };
        if type_str[value_start] == b';' {
            pos = Some(value_start);
            continue;
        }

        let param_value;
        if type_str[value_start] != b'"' {
            // Unquoted: runs to the next ';' with trailing LWS removed.
            let semi_after = find_first_of(type_str, b";", value_start);
            let mut value_end = semi_after.unwrap_or(n);
            while value_end > value_start && HTTP_LWS.contains(&type_str[value_end - 1]) {
                value_end -= 1;
            }
            param_value = type_str[value_start..value_end].to_vec();
            pos = semi_after;
        } else {
            // Quoted: copy up to the closing quote. Backslash escapes the next
            // byte. Whitespace is kept. Text after the closing quote, up to the
            // next ';', is ignored.
            let mut offset = value_start + 1;
            let mut value = Vec::new();
            while offset < n && type_str[offset] != b'"' {
                if type_str[offset] == b'\\' && offset + 1 < n {
                    offset += 1;
                }
                value.push(type_str[offset]);
                offset += 1;
            }
            param_value = value;
            pos = find_first_of(type_str, b";", offset);
        }
        params.push((param_name, param_value));
    }
    Some((mime_type, params))
}
