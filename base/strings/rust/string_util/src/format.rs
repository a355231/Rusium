// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! IsWprintfFormatPortable from base/strings/string_util.h.

/// C++ `IsWprintfFormatPortable(const wchar_t*)`. Checks that the conversion
/// specifiers in a wprintf format string behave the same on all platforms:
/// `%s`/`%c` need the `l` modifier, and `%S`, `%C`, `%F`, `%D`, `%O`, and `%U`
/// are not portable. A format that ends in the middle of a specifier counts as
/// portable, as in the C++ code.
///
/// Takes `&str`, so each `char` is one code unit. The C++ `wchar_t` is 16 bits
/// on Windows and 32 bits elsewhere. This port treats the input as Unicode
/// scalar values, which matches the 32-bit behavior for ASCII formats.
pub fn is_wprintf_format_portable(format: &str) -> bool {
    const PORTABLE_SPECIFIERS: &str = "diouxXeEfgGaAcspn%";
    let chars: Vec<char> = format.chars().collect();
    let mut pos = 0;
    while pos < chars.len() {
        if chars[pos] == '%' {
            let mut in_specification = true;
            let mut modifier_l = false;
            while in_specification {
                pos += 1;
                if pos >= chars.len() {
                    // Ended mid-specification. Portable, as in the C++ code.
                    return true;
                }
                let c = chars[pos];
                if c == 'l' {
                    // 'l' is the only modifier that makes 's' and 'c' portable.
                    modifier_l = true;
                } else if ((c == 's' || c == 'c') && !modifier_l)
                    || matches!(c, 'S' | 'C' | 'F' | 'D' | 'O' | 'U')
                {
                    return false;
                }
                if PORTABLE_SPECIFIERS.contains(c) {
                    in_specification = false;
                }
            }
        }
        pos += 1;
    }
    true
}
