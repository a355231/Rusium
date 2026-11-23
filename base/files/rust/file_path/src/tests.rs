// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of base/files/file_path_unittest.cc. Each C++ test case runs under the
//! flavor that its `#if` block selects on the C++ side. Cases that are valid on
//! both flavors run under both. Windows-only tables use raw strings, so `\`
//! needs no escaping.

use super::*;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const POSIX: PathFlavor = PathFlavor::Posix;
const WINDOWS: PathFlavor = PathFlavor::Windows;

fn check_unary(flavor: PathFlavor, cases: &[(&str, &str)], op: fn(&FilePath) -> FilePath) {
    for (i, (input, expected)) in cases.iter().enumerate() {
        let observed = op(&FilePath::new(flavor, input));
        assert_eq!(observed.value(), *expected, "{flavor:?} case {i}: input {input:?}");
    }
}

fn check_unary_bool(flavor: PathFlavor, cases: &[(&str, bool)], op: fn(&FilePath) -> bool) {
    for (i, (input, expected)) in cases.iter().enumerate() {
        let observed = op(&FilePath::new(flavor, input));
        assert_eq!(observed, *expected, "{flavor:?} case {i}: input {input:?}");
    }
}

/// Runs `(path, arg, expected)` triples through a binary operation.
fn check_binary(
    flavor: PathFlavor,
    cases: &[(&str, &str, &str)],
    op: fn(&FilePath, &str) -> FilePath,
) {
    for (i, (path, arg, expected)) in cases.iter().enumerate() {
        let observed = op(&FilePath::new(flavor, path), arg);
        assert_eq!(observed.value(), *expected, "{flavor:?} case {i}: path {path:?}, arg {arg:?}");
    }
}

fn check_binary_bool(
    flavor: PathFlavor,
    cases: &[(&str, &str, bool)],
    op: fn(&FilePath, &str) -> bool,
) {
    for (i, (path, arg, expected)) in cases.iter().enumerate() {
        let observed = op(&FilePath::new(flavor, path), arg);
        assert_eq!(observed, *expected, "{flavor:?} case {i}: path {path:?}, arg {arg:?}");
    }
}

/// Swaps `/` and `\`, as the C++ `SwapSeparators` helper does.
fn swap_separators(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' => '\\',
            '\\' => '/',
            other => other,
        })
        .collect()
}

/// Formats components as `|a|b|c`, as the C++ PathComponentsTest does.
fn format_components(path: &FilePath) -> String {
    let mut out = String::new();
    for component in path.components() {
        out.push('|');
        out.push_str(&component);
    }
    out
}

#[test]
fn dir_name() {
    const COMMON: &[(&str, &str)] = &[
        ("", "."),
        (".", "."),
        ("aa", "."),
        ("/aa/bb", "/aa"),
        ("/aa/bb/", "/aa"),
        ("/aa/bb//", "/aa"),
        ("/aa/bb/ccc", "/aa/bb"),
        ("/aa", "/"),
        ("/aa/", "/"),
        ("/", "/"),
        ("//", "//"),
        ("///", "/"),
        ("aa/", "."),
        ("aa/bb", "aa"),
        ("aa/bb/", "aa"),
        ("aa/bb//", "aa"),
        ("aa//bb//", "aa"),
        ("aa//bb/", "aa"),
        ("aa//bb", "aa"),
        ("//aa/bb", "//aa"),
        ("//aa/", "//"),
        ("//aa", "//"),
        ("///aa/", "/"),
        ("///aa", "/"),
        ("///", "/"),
        ("0:", "."),
        ("@:", "."),
        ("[:", "."),
        ("`:", "."),
        ("{:", "."),
        ("\u{B3}:", "."),
        ("\u{C5}:", "."),
        ("/aa/../bb/cc", "/aa/../bb"),
    ];
    const WINDOWS_ONLY: &[(&str, &str)] = &[
        ("\u{143}:", "."),
        ("c:", "c:"),
        ("C:", "C:"),
        ("A:", "A:"),
        ("Z:", "Z:"),
        ("a:", "a:"),
        ("z:", "z:"),
        ("c:aa", "c:"),
        ("c:/", "c:/"),
        ("c://", "c://"),
        ("c:///", "c:/"),
        ("c:/aa", "c:/"),
        ("c:/aa/", "c:/"),
        ("c:/aa/bb", "c:/aa"),
        ("c:aa/bb", "c:aa"),
        (r"\aa\bb", r"\aa"),
        (r"\aa\bb\", r"\aa"),
        (r"\aa\bb\\", r"\aa"),
        (r"\aa\bb\ccc", r"\aa\bb"),
        (r"\aa", r"\"),
        (r"\aa\", r"\"),
        (r"\", r"\"),
        (r"\\", r"\\"),
        (r"\\\", r"\"),
        (r"aa\", "."),
        (r"aa\bb", "aa"),
        (r"aa\bb\", "aa"),
        (r"aa\bb\\", "aa"),
        (r"aa\\bb\\", "aa"),
        (r"aa\\bb\", "aa"),
        (r"aa\\bb", "aa"),
        (r"\\aa\bb", r"\\aa"),
        (r"\\aa\", r"\\"),
        (r"\\aa", r"\\"),
        (r"aa\..\bb\c", r"aa\..\bb"),
        (r"c:\", r"c:\"),
        (r"c:\\", r"c:\\"),
        (r"c:\\\", r"c:\"),
        (r"c:\\aa", r"c:\\"),
        (r"c:\\\aa", r"c:\"),
        (r"c:\aa", r"c:\"),
        (r"c:\aa\", r"c:\"),
        (r"c:\aa\bb", r"c:\aa"),
        (r"c:aa\bb", "c:aa"),
    ];
    check_unary(POSIX, COMMON, FilePath::dir_name);
    check_unary(WINDOWS, COMMON, FilePath::dir_name);
    check_unary(WINDOWS, WINDOWS_ONLY, FilePath::dir_name);
}

#[test]
fn base_name() {
    const COMMON: &[(&str, &str)] = &[
        ("", ""),
        ("aa", "aa"),
        ("/aa/bb", "bb"),
        ("/aa/bb/", "bb"),
        ("/aa/bb//", "bb"),
        ("/aa/bb/ccc", "ccc"),
        ("/aa", "aa"),
        ("/", "/"),
        ("//", "//"),
        ("///", "/"),
        ("aa/", "aa"),
        ("aa/bb", "bb"),
        ("aa/bb/", "bb"),
        ("aa/bb//", "bb"),
        ("aa//bb//", "bb"),
        ("aa//bb/", "bb"),
        ("aa//bb", "bb"),
        ("//aa/bb", "bb"),
        ("//aa/", "aa"),
        ("//aa", "aa"),
        ("0:", "0:"),
        ("@:", "@:"),
        ("[:", "[:"),
        ("`:", "`:"),
        ("{:", "{:"),
        ("\u{B3}:", "\u{B3}:"),
        ("\u{C5}:", "\u{C5}:"),
    ];
    const WINDOWS_ONLY: &[(&str, &str)] = &[
        ("\u{143}:", "\u{143}:"),
        ("c:", ""),
        ("C:", ""),
        ("A:", ""),
        ("Z:", ""),
        ("a:", ""),
        ("z:", ""),
        ("c:aa", "aa"),
        ("c:/", "/"),
        ("c://", "//"),
        ("c:///", "/"),
        ("c:/aa", "aa"),
        ("c:/aa/", "aa"),
        ("c:/aa/bb", "bb"),
        ("c:aa/bb", "bb"),
        (r"\aa\bb", "bb"),
        (r"\aa\bb\", "bb"),
        (r"\aa\bb\\", "bb"),
        (r"\aa\bb\ccc", "ccc"),
        (r"\aa", "aa"),
        (r"\", r"\"),
        (r"\\", r"\\"),
        (r"\\\", r"\"),
        (r"aa\", "aa"),
        (r"aa\bb", "bb"),
        (r"aa\bb\", "bb"),
        (r"aa\bb\\", "bb"),
        (r"aa\\bb\\", "bb"),
        (r"aa\\bb\", "bb"),
        (r"aa\\bb", "bb"),
        (r"\\aa\bb", "bb"),
        (r"\\aa\", "aa"),
        (r"\\aa", "aa"),
        (r"c:\", r"\"),
        (r"c:\\", r"\\"),
        (r"c:\\\", r"\"),
        (r"c:\aa", "aa"),
        (r"c:\aa\", "aa"),
        (r"c:\aa\bb", "bb"),
        (r"c:aa\bb", "bb"),
    ];
    check_unary(POSIX, COMMON, FilePath::base_name);
    check_unary(WINDOWS, COMMON, FilePath::base_name);
    check_unary(WINDOWS, WINDOWS_ONLY, FilePath::base_name);
}

#[test]
fn append() {
    // (root, leaf, expected). These are ASCII, so the ASCII variant can be
    // tested too.
    const COMMON: &[(&str, &str, &str)] = &[
        ("", "cc", "cc"),
        (".", "ff", "ff"),
        (".", "", "."),
        ("/", "cc", "/cc"),
        ("/aa", "", "/aa"),
        ("/aa/", "", "/aa"),
        ("//aa", "", "//aa"),
        ("//aa/", "", "//aa"),
        ("//", "aa", "//aa"),
    ];
    const POSIX_ONLY: &[(&str, &str, &str)] = &[
        ("/aa/bb", "cc", "/aa/bb/cc"),
        ("/aa/bb/", "cc", "/aa/bb/cc"),
        ("aa/bb/", "cc", "aa/bb/cc"),
        ("aa/bb", "cc", "aa/bb/cc"),
        ("a/b", "c", "a/b/c"),
        ("a/b/", "c", "a/b/c"),
        ("//aa", "bb", "//aa/bb"),
        ("//aa/", "bb", "//aa/bb"),
    ];
    const WINDOWS_DRIVE: &[(&str, &str, &str)] = &[
        ("c:", "a", "c:a"),
        ("c:", "", "c:"),
        ("c:/", "a", "c:/a"),
        ("c://", "a", "c://a"),
        ("c:///", "a", "c:/a"),
    ];
    const WINDOWS_SEPARATORS: &[(&str, &str, &str)] = &[
        (r"\", "cc", r"\cc"),
        (r"\aa", "", r"\aa"),
        (r"\aa\", "", r"\aa"),
        (r"\\aa", "", r"\\aa"),
        (r"\\aa\", "", r"\\aa"),
        (r"\\", "aa", r"\\aa"),
        (r"/aa/bb", "cc", r"/aa/bb\cc"),
        (r"/aa/bb/", "cc", r"/aa/bb\cc"),
        (r"aa/bb/", "cc", r"aa/bb\cc"),
        (r"aa/bb", "cc", r"aa/bb\cc"),
        (r"a/b", "c", r"a/b\c"),
        (r"a/b/", "c", r"a/b\c"),
        (r"//aa", "bb", r"//aa\bb"),
        (r"//aa/", "bb", r"//aa\bb"),
        (r"\aa\bb", "cc", r"\aa\bb\cc"),
        (r"\aa\bb\", "cc", r"\aa\bb\cc"),
        (r"aa\bb\", "cc", r"aa\bb\cc"),
        (r"aa\bb", "cc", r"aa\bb\cc"),
        (r"a\b", "c", r"a\b\c"),
        (r"a\b\", "c", r"a\b\c"),
        (r"\\aa", "bb", r"\\aa\bb"),
        (r"\\aa\", "bb", r"\\aa\bb"),
    ];
    const WINDOWS_DRIVE_SEPARATORS: &[(&str, &str, &str)] = &[
        (r"c:\", "a", r"c:\a"),
        (r"c:\\", "a", r"c:\\a"),
        (r"c:\\\", "a", r"c:\a"),
        (r"c:\", "", r"c:\"),
        (r"c:\a", "b", r"c:\a\b"),
        (r"c:\a\", "b", r"c:\a\b"),
    ];

    for (flavor, table) in [
        (POSIX, COMMON),
        (POSIX, POSIX_ONLY),
        (WINDOWS, COMMON),
        (WINDOWS, WINDOWS_DRIVE),
        (WINDOWS, WINDOWS_SEPARATORS),
        (WINDOWS, WINDOWS_DRIVE_SEPARATORS),
    ] {
        for (i, (root, leaf, expected)) in table.iter().enumerate() {
            let root = FilePath::new(flavor, root);
            let observed = root.append(leaf);
            assert_eq!(observed.value(), *expected, "{flavor:?} case {i}, append");
            let observed = root.append_path(&FilePath::new(flavor, leaf));
            assert_eq!(observed.value(), *expected, "{flavor:?} case {i}, append_path");
            let observed = root.append_ascii(leaf).unwrap();
            assert_eq!(observed.value(), *expected, "{flavor:?} case {i}, append_ascii");
        }
    }
}

/// Non-ASCII valid UTF-8 names. These are the same values as the C++
/// FilePathUTF8TestCases instantiations.
const UTF8_VALID_NAMES: &[&str] =
    &["خطأ في عملية التحقق من التحديث 5555", "अपडेट जांच में यह गड़बड़ी है 5555", "更新確認エラー 5555。"];

#[test]
fn utf8_names() {
    // C++ FilePathUTF8Test.Test, one instance per value in UTF_VALID_NAMES.
    for flavor in [POSIX, WINDOWS] {
        for name in UTF8_VALID_NAMES {
            let root = FilePath::new(flavor, "");
            assert_eq!(root.append(name).value(), *name);
            assert_eq!(root.append_path(&FilePath::new(flavor, name)).value(), *name);

            // The ASCII variants must reject non-ASCII input.
            assert_eq!(root.append_ascii(name), Err(PathError::NotAscii));
            assert_eq!(root.insert_before_extension_ascii(name), Err(PathError::NotAscii));
            assert_eq!(root.add_extension_ascii(name), Err(PathError::NotAscii));

            assert_eq!(root.append_utf8(name.as_bytes()).unwrap().value(), *name);
            assert!(root.insert_before_extension_utf8(name.as_bytes()).unwrap().is_empty());
            assert!(root.add_extension_utf8(name.as_bytes()).unwrap().is_empty());
        }
    }
}

#[test]
fn utf8_invalid_names() {
    // C++ FilePathUTF8InvalidTest.Test, one instance per byte string below.
    // Every one is non-ASCII and not valid UTF-8, so every checked entry
    // point must reject it.
    let invalid: &[&[u8]] = &[
        // Invalid encoding of U+1FFFE (0x8F instead of 0x9F).
        b"\xF0\x8F\xBF\xBE",
        // Surrogate code points.
        b"\xED\xA0\x80\xED\xBF\xBF",
        b"\xED\xA0\x8F",
        b"\xED\xBF\xBF",
        // Overlong sequences.
        b"\xC0\x80",
        b"\xC1\x80\xC1\x81",
        b"\xE0\x80\x80",
        b"\xE0\x82\x80",
        b"\xE0\x9F\xBF",
        b"\xF0\x80\x80\x8D",
        b"\xF0\x80\x82\x91",
        b"\xF0\x80\xA0\x80",
        b"\xF0\x8F\xBB\xBF",
        b"\xF8\x80\x80\x80\xBF",
        b"\xFC\x80\x80\x80\xA0\xA5",
        // Beyond U+10FFFF.
        b"\xF4\x90\x80\x80",
        b"\xF8\xA0\xBF\x80\xBF",
        b"\xFC\x9C\xBF\x80\xBF\x80",
        // BOM in UTF-16 BE and LE.
        b"\xFE\xFF",
        b"\xFF\xFE",
        // Legacy encodings: ISO-8859-1, EUC-KR, Big5, windows-125x, ISO-8859-6
        // and ISO-8859-7.
        b"caf\xE9",
        b"\xB0\xA1\xB0\xA2",
        b"\xA7\x41\xA6\x6E",
        b"\x93abc\x94",
        b"\xD9\xEE\xE4\xEE",
        b"\xE3\xE5\xE9\xDC",
    ];
    for flavor in [POSIX, WINDOWS] {
        let root = FilePath::new(flavor, "");
        for bytes in invalid {
            assert!(
                std::str::from_utf8(bytes).is_err(),
                "fixture should be invalid UTF-8: {bytes:?}"
            );
            assert!(bytes.iter().any(|&b| b >= 0x80), "fixture: {bytes:?}");
            assert!(root.append_utf8(bytes).is_err(), "{bytes:?}");
            assert!(root.insert_before_extension_utf8(bytes).is_err());
            assert!(root.add_extension_utf8(bytes).is_err());
            assert!(FilePath::from_utf8_unsafe(flavor, bytes).is_err());
        }
    }
}

#[test]
fn strip_trailing_separators() {
    const COMMON: &[(&str, &str)] = &[
        ("", ""),
        ("/", "/"),
        ("//", "//"),
        ("///", "/"),
        ("////", "/"),
        ("a/", "a"),
        ("a//", "a"),
        ("a///", "a"),
        ("a////", "a"),
        ("/a", "/a"),
        ("/a/", "/a"),
        ("/a//", "/a"),
        ("/a///", "/a"),
        ("/a////", "/a"),
    ];
    const WINDOWS_DRIVE: &[(&str, &str)] = &[
        ("c:", "c:"),
        ("c:/", "c:/"),
        ("c://", "c://"),
        ("c:///", "c:/"),
        ("c:////", "c:/"),
        ("c:/a", "c:/a"),
        ("c:/a/", "c:/a"),
        ("c:/a//", "c:/a"),
        ("c:/a///", "c:/a"),
        ("c:/a////", "c:/a"),
    ];
    const WINDOWS_SEPARATORS: &[(&str, &str)] = &[
        (r"\", r"\"),
        (r"\\", r"\\"),
        (r"\\\", r"\"),
        (r"\\\\", r"\"),
        (r"a\", "a"),
        (r"a\\", "a"),
        (r"a\\\", "a"),
        (r"a\\\\", "a"),
        (r"\a", r"\a"),
        (r"\a\", r"\a"),
        (r"\a\\", r"\a"),
        (r"\a\\\", r"\a"),
        (r"\a\\\\", r"\a"),
    ];
    const WINDOWS_DRIVE_SEPARATORS: &[(&str, &str)] = &[
        (r"c:\", r"c:\"),
        (r"c:\\", r"c:\\"),
        (r"c:\\\", r"c:\"),
        (r"c:\\\\", r"c:\"),
        (r"c:\a", r"c:\a"),
        (r"c:\a\", r"c:\a"),
        (r"c:\a\\", r"c:\a"),
        (r"c:\a\\\", r"c:\a"),
        (r"c:\a\\\\", r"c:\a"),
    ];
    check_unary(POSIX, COMMON, FilePath::strip_trailing_separators);
    check_unary(WINDOWS, COMMON, FilePath::strip_trailing_separators);
    check_unary(WINDOWS, WINDOWS_DRIVE, FilePath::strip_trailing_separators);
    check_unary(WINDOWS, WINDOWS_SEPARATORS, FilePath::strip_trailing_separators);
    check_unary(WINDOWS, WINDOWS_DRIVE_SEPARATORS, FilePath::strip_trailing_separators);
}

#[test]
fn is_absolute_or_network() {
    // (input, is_absolute, is_network)
    const COMMON: &[(&str, bool, bool)] = &[
        ("", false, false),
        ("a", false, false),
        ("c:", false, false),
        ("c:a", false, false),
        ("a/b", false, false),
        ("//", true, true),
        ("//a", true, true),
        ("c:a/b", false, false),
        ("?:/a", false, false),
    ];
    const POSIX_ONLY: &[(&str, bool, bool)] = &[
        ("/", true, false),
        ("/a", true, false),
        ("/.", true, false),
        ("/..", true, false),
        ("c:/", false, false),
    ];
    const WINDOWS_ONLY: &[(&str, bool, bool)] = &[
        ("/", false, false),
        ("/a", false, false),
        ("/.", false, false),
        ("/..", false, false),
        ("c:/", true, false),
        ("c:/a", true, false),
        ("c:/.", true, false),
        ("c:/..", true, false),
        ("C:/a", true, false),
        ("d:/a", true, false),
        (r"a\b", false, false),
        (r"\\", true, true),
        (r"\\a", true, true),
        ("//a", true, true),
        (r"c:a\b", false, false),
        (r"?:\a", false, false),
        (r"\", false, false),
        (r"\a", false, false),
        (r"\.", false, false),
        (r"\..", false, false),
        (r"c:\", true, false),
        (r"c:\a", true, false),
        (r"c:\.", true, false),
        (r"c:\..", true, false),
        (r"C:\a", true, false),
        (r"d:\a", true, false),
    ];
    for (flavor, table) in
        [(POSIX, COMMON), (POSIX, POSIX_ONLY), (WINDOWS, COMMON), (WINDOWS, WINDOWS_ONLY)]
    {
        for (i, (input, abs, net)) in table.iter().enumerate() {
            let path = FilePath::new(flavor, input);
            assert_eq!(path.is_absolute(), *abs, "{flavor:?} case {i}: {input:?}");
            assert_eq!(path.is_network(), *net, "{flavor:?} case {i}: {input:?}");
        }
    }
}

#[test]
fn path_components() {
    const COMMON: &[(&str, &str)] = &[
        ("//foo/bar/baz/", "|//|foo|bar|baz"),
        ("///", "|/"),
        ("///foo//bar/baz", "|/|foo|bar|baz"),
        ("/foo//bar//baz/", "|/|foo|bar|baz"),
        ("/foo/bar/baz/", "|/|foo|bar|baz"),
        ("/foo/bar/baz//", "|/|foo|bar|baz"),
        ("/foo/bar/baz///", "|/|foo|bar|baz"),
        ("/foo/bar/baz", "|/|foo|bar|baz"),
        ("/foo/bar.bot/baz.txt", "|/|foo|bar.bot|baz.txt"),
        ("//foo//bar/baz", "|//|foo|bar|baz"),
        ("/", "|/"),
        ("foo", "|foo"),
        ("", ""),
    ];
    const WINDOWS_ONLY: &[(&str, &str)] = &[
        ("e:/foo", "|e:|/|foo"),
        ("e:/", "|e:|/"),
        ("e:foo", "|e:|foo"),
        ("e:", "|e:"),
        ("e://foo", "|e:|//|foo"),
        ("e://", "|e:|//"),
        ("e:///foo", "|e:|/|foo"),
        ("e:///", "|e:|/"),
        ("../foo", "|..|foo"),
        ("./foo", "|foo"),
        ("../foo/bar/", "|..|foo|bar"),
        (r"\\foo\bar\baz\", r"|\\|foo|bar|baz"),
        (r"\\\", r"|\"),
        (r"\foo\\bar\\baz\", r"|\|foo|bar|baz"),
        (r"\foo\bar\baz\", r"|\|foo|bar|baz"),
        (r"\foo\bar\baz\\", r"|\|foo|bar|baz"),
        (r"\foo\bar\baz\\\", r"|\|foo|bar|baz"),
        (r"\foo\bar\baz", r"|\|foo|bar|baz"),
        (r"\foo\bar/baz\\\", r"|\|foo|bar|baz"),
        (r"/foo\bar\baz", "|/|foo|bar|baz"),
        (r"\foo\bar.bot\baz.txt", r"|\|foo|bar.bot|baz.txt"),
        (r"\\foo\\bar\baz", r"|\\|foo|bar|baz"),
        (r"\", r"|\"),
    ];
    for (input, expected) in COMMON {
        assert_eq!(format_components(&FilePath::new(POSIX, input)), *expected, "posix {input:?}");
        assert_eq!(
            format_components(&FilePath::new(WINDOWS, input)),
            *expected,
            "windows {input:?}"
        );
    }
    for (input, expected) in WINDOWS_ONLY {
        assert_eq!(
            format_components(&FilePath::new(WINDOWS, input)),
            *expected,
            "windows {input:?}"
        );
    }
}

/// Parent/child cases for IsParent. Each entry is `(parent, child, expected)`.
/// The same expectations hold for `is_parent` and `is_parent_slow`.
const IS_PARENT_COMMON: &[(&str, &str, bool)] = &[
    ("", "", false),
    ("", ".", false),
    ("", "foo", false),
    (".", "", false),
    (".", ".", false),
    // The next two are incorrect, but they pin the old behavior of IsParent.
    (".", "./foo", false),
    ("./", "./foo", false),
    ("./", "/", false),
    ("./", "//", false),
    ("./", "/foo", false),
    ("./", "//foo", false),
    ("./foo", "foo/bar", true),
    ("./foo", "./foo/bar", true),
    ("./foo", "/foo/bar", false),
    ("./foo", "//foo/bar", false),
    ("/", "/", false),
    ("/", "//foo", false),
    ("/", "///foo", true),
    ("/", "/foo/bar/baz", true),
    ("//////host/bar/", "//////HOST/bar/baz", true),
    ("//host/bar/", "//host/bar/baz", true),
    ("//host/bar/", "//host2/bar/baz", false),
    ("//host/bar/", "/HOST/bar/baz", false),
    ("//host/bar/", "/host/bar/baz", false),
    ("//HOST/foo", "//host/foo/bar", true),
    ("/foo", "/foo/bar/baz", true),
    ("/foo/bar.txt", "/foo/bar/baz", false),
    ("/foo/bar", "/foo//bar/baz", true),
    ("/foo/bar", "/FOO//bar/baz/", false),
    ("/foo/bar", "/foo//bar/baz/", true),
    ("/foo/bar", "/foo/bar", false),
    ("/foo/bar", "/foo/bar/baz", true),
    ("/foo/bar", "/foo/bar2/baz", false),
    ("/foo/bar", "/foo2/bar/baz", false),
    ("/foo/bar/", "/foo/bar/baz", true),
    ("/foo/bar/baz", "/foo/bar", false),
    ("foo", "./foo/bar/baz", true),
    ("foo", "", false),
    ("foo", ".", false),
    ("foo", "boo/bar/baz", false),
    ("foo", "foo/bar/baz", true),
    ("foo/b:", "foo/B:/baz", false),
    ("foo/bar", "/foo/bar/baz", false),
    ("foo/bar", "foo/bar/baz", true),
    ("foo/bar", "foo/bar2/baz", false),
    ("foo/bar", "foo2/bar/baz", false),
    ("foo/bar/../baz", "foo/baz/aaa", false),
];

const IS_PARENT_WINDOWS_DRIVE: &[(&str, &str, bool)] = &[
    ("./c:", "C:foo", true),
    ("c:", "*:/foo/bar/baz", false),
    ("c:", "C:/foo", true),
    ("c:", "c:/foo/bar/baz", true),
    ("c:", "C:foo", true),
    ("c:/", "C:/foo", true),
    ("c:/", "c:/foo/bar/baz", true),
    // The components of "c:/" are ["c:", "/"], and those of "C:foo" are
    // ["C:", "foo"]. They are not prefix-related, so this is false.
    ("c:/", "C:foo", false),
    ("c:/foo", "C:foo/bar", false),
    ("c:/foo/bar", "c:///foo/bar/baz", true),
    ("c:/foo/bar", "c://foo/bar/baz", false),
    ("c:/foo/bar", "c:/foo/bar/baz", true),
    ("c:/foo/bar", "c:/foo/bar2/baz", false),
    ("c:/foo/bar", "c:/foo2/bar/baz", false),
    ("c:/foo/bar", "d:/foo/bar/baz", false),
    ("c:/foo/bar", "D:/foo/bar/baz", false),
    ("C:/foo/bar", "d:/foo/bar/baz", false),
    ("c:foo", "C:/foo/bar", false),
    ("c:foo", "C:foo/bar", true),
    ("c:foo/bar", "C:foo/bar/baz", true),
    ("E:/Foo/bar", "e:/foo/bar/baz", false),
    ("E:/foo/bar", "e:/foo/bar/baz", true),
    ("e:/foo/bar", "E:/foo2/bar/baz", false),
    ("f:/foo/bar", "F:/foo/Bar/baz", false),
    ("f:/foo/bar", "F:/foo/bar/baz", true),
    ("F:/foo/bar", "f:/foo2/bar/baz", false),
];

const IS_PARENT_WINDOWS_SEPARATORS: &[(&str, &str, bool)] = &[
    (r"\foo/bar", r"\foo/bar/baz", true),
    (r"\foo/bar", r"\foo\bar/baz", true),
    (r"\foo/bar", r"\foo\/\/bar/baz", true),
    // A root can mix separator types.
    (r"//host/foo", r"/\host\foo\bar", true),
    (r"\/host", r"/\host/foo", true),
    ("/", r"\/foo", false),
];

fn run_is_parent_cases(flavor: PathFlavor, cases: &[(&str, &str, bool)], is_fast: bool) {
    let check = |parent: &FilePath, child: &FilePath, expected: bool, label: &str| {
        let observed = if is_fast { parent.is_parent(child) } else { parent.is_parent_slow(child) };
        assert_eq!(
            observed,
            expected,
            "{flavor:?} {label}: parent {:?}, child {:?}",
            parent.value(),
            child.value()
        );
    };
    for (parent, child, expected) in cases {
        let parent = FilePath::new(flavor, parent);
        let child = FilePath::new(flavor, child);
        check(&parent, &child, *expected, "plain");
        if flavor == WINDOWS {
            // Swapping the separators of either path must not change the
            // result.
            let parent_swapped = FilePath::new(flavor, &swap_separators(parent.value()));
            let child_swapped = FilePath::new(flavor, &swap_separators(child.value()));
            check(&parent_swapped, &child_swapped, *expected, "swapped");
            check(&parent_swapped, &child, *expected, "parent swapped");
            check(&parent, &child_swapped, *expected, "child swapped");
        }
    }
}

#[test]
fn is_parent_fast() {
    run_is_parent_cases(POSIX, IS_PARENT_COMMON, true);
    run_is_parent_cases(WINDOWS, IS_PARENT_COMMON, true);
    run_is_parent_cases(WINDOWS, IS_PARENT_WINDOWS_DRIVE, true);
    run_is_parent_cases(WINDOWS, IS_PARENT_WINDOWS_SEPARATORS, true);
}

#[test]
fn is_parent_slow() {
    run_is_parent_cases(POSIX, IS_PARENT_COMMON, false);
    run_is_parent_cases(WINDOWS, IS_PARENT_COMMON, false);
    run_is_parent_cases(WINDOWS, IS_PARENT_WINDOWS_DRIVE, false);
    run_is_parent_cases(WINDOWS, IS_PARENT_WINDOWS_SEPARATORS, false);
}

#[test]
fn append_relative_path() {
    // (parent, child, expected). `expected` is "" when the path is not a
    // parent. With base "blah", the result must be base + expected.
    const POSIX_ONLY: &[(&str, &str, &str)] = &[("/", "/foo/bar/baz", "foo/bar/baz")];
    const WINDOWS_ONLY: &[(&str, &str, &str)] = &[("/", "/foo/bar/baz", r"foo\bar\baz")];
    const COMMON: &[(&str, &str, &str)] = &[
        ("/foo/bar", "/foo/bar/baz", "baz"),
        ("/foo/bar/", "/foo/bar/baz", "baz"),
        ("//foo/bar/", "//foo/bar/baz", "baz"),
        ("/foo/bar", "/foo2/bar/baz", ""),
        ("/foo/bar.txt", "/foo/bar/baz", ""),
        ("/foo/bar", "/foo/bar2/baz", ""),
        ("/foo/bar", "/foo/bar", ""),
        ("/foo/bar/baz", "/foo/bar", ""),
        ("foo/bar", "foo/bar/baz", "baz"),
        ("foo/bar", "foo2/bar/baz", ""),
        ("foo/bar", "foo/bar2/baz", ""),
        ("", "foo", ""),
        // For network paths, the host is compared case-insensitively.
        ("//FOO/bar/", "//foo/bar/baz", "baz"),
        ("//foo/BAR/", "//foo/bar/baz", ""),
        // For non-network paths, the first component is compared exactly.
        ("/FOO/bar/", "/foo/bar/baz", ""),
        // Degenerate case: the parent has no host.
        ("//", "//foo", "foo"),
    ];
    const WINDOWS_DRIVE: &[(&str, &str, &str)] = &[
        ("c:/foo/bar", "c:/foo/bar/baz", "baz"),
        ("E:/foo/bar", "e:/foo/bar/baz", "baz"),
        ("f:/foo/bar", "F:/foo/bar/baz", "baz"),
        ("E:/Foo/bar", "e:/foo/bar/baz", ""),
        ("f:/foo/bar", "F:/foo/Bar/baz", ""),
        ("c:/foo/bar", "d:/foo/bar/baz", ""),
        ("c:/foo/bar", "D:/foo/bar/baz", ""),
        ("C:/foo/bar", "d:/foo/bar/baz", ""),
        ("c:/foo/bar", "c:/foo2/bar/baz", ""),
        ("e:/foo/bar", "E:/foo2/bar/baz", ""),
        ("F:/foo/bar", "f:/foo2/bar/baz", ""),
        ("c:/foo/bar", "c:/foo/bar2/baz", ""),
        ("c:/", "c:/foo/bar/baz", r"foo\bar\baz"),
    ];
    const WINDOWS_SEPARATORS: &[(&str, &str, &str)] = &[
        (r"\foo\bar", r"\foo\bar\baz", "baz"),
        (r"\foo/bar", r"\foo\bar\baz", "baz"),
        (r"\foo/bar", r"\foo/bar/baz", "baz"),
        (r"\", r"\foo\bar\baz", r"foo\bar\baz"),
        ("", r"\foo\bar\baz", ""),
        (r"\foo\bar", r"\foo2\bar\baz", ""),
        (r"\foo\bar", r"\foo\bar2\baz", ""),
        (r"\\FOO\bar", r"\\foo\bar\baz", "baz"),
        (r"\\fOO\Bar", r"\\foo\bar\baz", ""),
        (r"\FOO\bar", r"\foo\bar\baz", ""),
        (r"\\", r"\\foo", "foo"),
    ];

    let base = FilePath::new(POSIX, "blah");
    let windows_base = FilePath::new(WINDOWS, "blah");
    for (flavor, table) in [
        (POSIX, POSIX_ONLY),
        (POSIX, COMMON),
        (WINDOWS, WINDOWS_ONLY),
        (WINDOWS, COMMON),
        (WINDOWS, WINDOWS_DRIVE),
        (WINDOWS, WINDOWS_SEPARATORS),
    ] {
        let base = if flavor == POSIX { &base } else { &windows_base };
        for (parent, child, expected) in table {
            let mut variants = vec![(parent.to_string(), child.to_string())];
            if flavor == WINDOWS {
                let (ps, cs) = (swap_separators(parent), swap_separators(child));
                variants.push((ps.clone(), cs.clone()));
                variants.push((ps, child.to_string()));
                variants.push((parent.to_string(), cs));
            }
            for (p, c) in variants {
                let parent_path = FilePath::new(flavor, &p);
                let child_path = FilePath::new(flavor, &c);
                let success = parent_path.append_relative_path(&child_path, base);
                assert_eq!(
                    success.is_some(),
                    !expected.is_empty(),
                    "{flavor:?}: parent {p:?}, child {c:?}"
                );
                let result = success.unwrap_or_else(|| base.clone());
                assert_eq!(
                    result.value(),
                    base.append(expected).value(),
                    "{flavor:?}: parent {p:?}, child {c:?}"
                );
            }
        }
    }
}

fn hash_of(path: &FilePath) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn equality() {
    // (a, b, expected_equal)
    const COMMON: &[(&str, &str, bool)] = &[
        ("/foo/bar/baz", "/foo/bar/baz", true),
        ("/foo/bar", "/foo/bar/baz", false),
        ("/foo/bar/baz", "/foo/bar", false),
        ("//foo/bar/", "//foo/bar/", true),
        ("/foo/bar", "/foo2/bar", false),
        ("/foo/bar.txt", "/foo/bar", false),
        ("foo/bar", "foo/bar", true),
        ("foo/bar", "foo/bar/baz", false),
        ("", "foo", false),
    ];
    const WINDOWS_DRIVE: &[(&str, &str, bool)] = &[
        ("c:/foo/bar", "c:/foo/bar", true),
        ("E:/foo/bar", "e:/foo/bar", true),
        ("f:/foo/bar", "F:/foo/bar", true),
        ("E:/Foo/bar", "e:/foo/bar", false),
        ("f:/foo/bar", "F:/foo/Bar", false),
        ("c:/", "c:/", true),
        ("c:", "c:", true),
        ("c:/foo/bar", "d:/foo/bar", false),
        ("c:/foo/bar", "D:/foo/bar", false),
        ("C:/foo/bar", "d:/foo/bar", false),
        ("c:/foo/bar", "c:/foo2/bar", false),
    ];
    const WINDOWS_SEPARATORS: &[(&str, &str, bool)] = &[
        (r"\foo\bar", r"\foo\bar", true),
        (r"\foo/bar", r"\foo/bar", true),
        (r"\foo/bar", r"\foo\bar", false),
        (r"\", r"\", true),
        (r"\", "/", false),
        ("", r"\", false),
        (r"\foo\bar", r"\foo2\bar", false),
        (r"\foo\bar", r"\foo\bar2", false),
        (r"c:\foo\bar", r"c:\foo\bar", true),
        (r"E:\foo\bar", r"e:\foo\bar", true),
        (r"f:\foo\bar", r"F:\foo/bar", false),
    ];

    let mut cases: Vec<(PathFlavor, &str, &str, bool)> = Vec::new();
    cases.extend(COMMON.iter().map(|&(a, b, e)| (POSIX, a, b, e)));
    cases.extend(COMMON.iter().map(|&(a, b, e)| (WINDOWS, a, b, e)));
    cases.extend(WINDOWS_DRIVE.iter().map(|&(a, b, e)| (WINDOWS, a, b, e)));
    cases.extend(WINDOWS_SEPARATORS.iter().map(|&(a, b, e)| (WINDOWS, a, b, e)));

    for (flavor, a_str, b_str, expected) in &cases {
        let a = FilePath::new(*flavor, a_str);
        let b = FilePath::new(*flavor, b_str);
        let label = format!("{flavor:?}: {a_str:?} vs {b_str:?}");
        assert_eq!(a == b, *expected, "equality {label}");
        assert_eq!(a.cmp(&b) == Ordering::Equal, *expected, "three-way {label}");
        assert_eq!(a != b, !*expected, "inequality {label}");
        if *expected {
            assert!(!(a < b), "less {label}");
            assert!(!(b < a), "greater {label}");
            assert_eq!(hash_of(&a), hash_of(&b), "hash {label}");
        } else {
            assert_ne!(a < b, b < a, "strict weak ordering {label}");
        }
    }

    // Leading drive letters compare case-insensitively. The rest of the
    // path compares case-sensitively.
    let w = |s: &str| FilePath::new(WINDOWS, s);
    assert!(w("C:/foo/bar") < w("d:/foo/bar"));
    assert!(w("c:/foo/bar") < w("D:/foo/bar"));
    assert!(w("c:/Foo/bar") < w("C:/foo/bar"));
}

#[test]
fn macro_expansion() {
    // The C++ test checks FILE_PATH_LITERAL, a preprocessor macro with no
    // Rust counterpart. This checks that a string literal of the same text
    // builds the same path.
    assert_eq!(FilePath::new(POSIX, "TestFile").value(), "TestFile");
    assert_eq!(FilePath::new(WINDOWS, "TestFile").value(), "TestFile");
}

#[test]
fn extension() {
    for flavor in [POSIX, WINDOWS] {
        let base_dir = FilePath::new(flavor, "base_dir");

        let jpg = base_dir.append("foo.jpg");
        assert_eq!(jpg.extension(), ".jpg");
        assert_eq!(jpg.final_extension(), ".jpg");

        let base = jpg.base_name().remove_extension();
        assert_eq!(base.value(), "foo");

        let path_no_ext = base_dir.append(base.value());
        assert_eq!(path_no_ext.value(), jpg.remove_extension().value());

        assert_eq!(path_no_ext.value(), path_no_ext.remove_extension().value());
        assert_eq!(path_no_ext.extension(), "");
        assert_eq!(path_no_ext.final_extension(), "");
    }
}

#[test]
fn extension2() {
    // (input, extension and final extension). Windows-only cases come first.
    const WINDOWS_ONLY: &[(&str, &str)] = &[
        (r"C:\a\b\c.ext", ".ext"),
        (r"C:\a\b\c.", "."),
        (r"C:\a\b\c", ""),
        (r"C:\a\b\", ""),
        (r"C:\a\b.\", "."),
        (r"C:\a\b\c.ext1.ext2", ".ext2"),
        (r"C:\foo.bar\\\", ".bar"),
        (r"C:\foo.bar\..", ""),
        (r"C:\foo.bar\..\\", ""),
    ];
    const COMMON: &[(&str, &str)] = &[
        ("/foo/bar/baz.EXT", ".EXT"),
        ("/foo/bar/baz.Ext", ".Ext"),
        ("/foo/bar/baz.ext", ".ext"),
        ("/foo/bar/baz.", "."),
        ("/foo/bar/baz..", "."),
        ("/foo/bar/baz", ""),
        ("/foo/bar/", ""),
        ("/foo/bar./", "."),
        ("/foo/bar/baz.ext1.ext2", ".ext2"),
        ("/subversion-1.6.12.zip", ".zip"),
        ("/foo.12345.gz", ".gz"),
        ("/foo..gz", ".gz"),
        (".", ""),
        ("..", ""),
        ("./foo", ""),
        ("./foo.ext", ".ext"),
        ("/foo.ext1/bar.ext2", ".ext2"),
        ("/foo.bar////", ".bar"),
        ("/foo.bar/..", ""),
        ("/foo.bar/..////", ""),
        ("/foo.1234.luser.js", ".js"),
        ("/user.js", ".js"),
    ];
    // Double extensions. Only Extension() is checked for these.
    const DOUBLE: &[(&str, &str)] = &[
        // Suffixes on the allow-list. Membership ignores ASCII case.
        ("/foo.TAR.bz2", ".TAR.bz2"),
        ("/foo.tar.Z", ".tar.Z"),
        ("/foo.tar.blah", ".blah"),
        ("/foo.tar.bz", ".tar.bz"),
        ("/foo.tar.bz2", ".tar.bz2"),
        ("/foo.tar.gz", ".tar.gz"),
        ("/foo.tar.lz", ".tar.lz"),
        ("/foo.tar.lzma", ".tar.lzma"),
        ("/foo.tar.lzo", ".tar.lzo"),
        ("/foo.tar.xz", ".tar.xz"),
        ("/foo.tar.z", ".tar.z"),
        ("/foo.tar.zst", ".tar.zst"),
        // Double extensions on the allow-list.
        ("/foo.1234.user.js", ".user.js"),
        ("foo.user.js", ".user.js"),
        // Other cases.
        ("/foo.1234.gz", ".1234.gz"),
        ("/foo.1234.gz.", "."),
        ("/foo.1234.tar.gz", ".tar.gz"),
        ("/foo.tar.tar.gz", ".tar.gz"),
        ("/foo.tar.gz.gz", ".gz.gz"),
    ];

    for (flavor, table) in [(POSIX, COMMON), (WINDOWS, COMMON), (WINDOWS, WINDOWS_ONLY)] {
        for (input, expected) in table {
            let path = FilePath::new(flavor, input);
            assert_eq!(path.extension(), *expected, "{flavor:?} extension {input:?}");
            assert_eq!(path.final_extension(), *expected, "{flavor:?} final {input:?}");
        }
    }
    for flavor in [POSIX, WINDOWS] {
        for (input, expected) in DOUBLE {
            let path = FilePath::new(flavor, input);
            assert_eq!(path.extension(), *expected, "{flavor:?} double {input:?}");
        }
    }
}

#[test]
fn insert_before_extension() {
    // (path, suffix, expected)
    const COMMON: &[(&str, &str, &str)] = &[
        ("", "", ""),
        ("", "txt", ""),
        (".", "txt", ""),
        ("..", "txt", ""),
        ("foo.dll", "txt", "footxt.dll"),
        (".", "", "."),
        ("foo.dll", ".txt", "foo.txt.dll"),
        ("foo", "txt", "footxt"),
        ("foo", ".txt", "foo.txt"),
        ("foo.baz.dll", "txt", "foo.baztxt.dll"),
        ("foo.baz.dll", ".txt", "foo.baz.txt.dll"),
        ("foo.dll", "", "foo.dll"),
        ("foo.dll", ".", "foo..dll"),
        ("foo", "", "foo"),
        ("foo", ".", "foo."),
        ("foo.baz.dll", "", "foo.baz.dll"),
        ("foo.baz.dll", ".", "foo.baz..dll"),
    ];
    const WINDOWS_ONLY: &[(&str, &str, &str)] = &[
        (r"\", "", r"\"),
        (r"\", "txt", r"\txt"),
        (r"\.", "txt", ""),
        (r"\..", "txt", ""),
        (r"\.", "", r"\."),
        (r"C:\bar\foo.dll", "txt", r"C:\bar\footxt.dll"),
        (r"C:\bar.baz\foodll", "txt", r"C:\bar.baz\foodlltxt"),
        (r"C:\bar.baz\foo.dll", "txt", r"C:\bar.baz\footxt.dll"),
        (r"C:\bar.baz\foo.dll.exe", "txt", r"C:\bar.baz\foo.dlltxt.exe"),
        (r"C:\bar.baz\foo", "", r"C:\bar.baz\foo"),
        (r"C:\bar.baz\foo.exe", "", r"C:\bar.baz\foo.exe"),
        (r"C:\bar.baz\foo.dll.exe", "", r"C:\bar.baz\foo.dll.exe"),
        (r"C:\bar\baz\foo.exe", " (1)", r"C:\bar\baz\foo (1).exe"),
        (r"C:\foo.baz\\", " (1)", r"C:\foo (1).baz"),
        (r"C:\foo.baz\..\", " (1)", ""),
    ];
    const POSIX_ONLY_OR_COMMON: &[(&str, &str, &str)] = &[
        ("/", "", "/"),
        ("/", "txt", "/txt"),
        ("/.", "txt", ""),
        ("/..", "txt", ""),
        ("/.", "", "/."),
        ("/bar/foo.dll", "txt", "/bar/footxt.dll"),
        ("/bar.baz/foodll", "txt", "/bar.baz/foodlltxt"),
        ("/bar.baz/foo.dll", "txt", "/bar.baz/footxt.dll"),
        ("/bar.baz/foo.dll.exe", "txt", "/bar.baz/foo.dlltxt.exe"),
        ("/bar.baz/foo", "", "/bar.baz/foo"),
        ("/bar.baz/foo.exe", "", "/bar.baz/foo.exe"),
        ("/bar.baz/foo.dll.exe", "", "/bar.baz/foo.dll.exe"),
        ("/bar/baz/foo.exe", " (1)", "/bar/baz/foo (1).exe"),
        ("/bar/baz/..////", " (1)", ""),
    ];
    for (flavor, table) in [
        (POSIX, COMMON),
        (POSIX, POSIX_ONLY_OR_COMMON),
        (WINDOWS, COMMON),
        (WINDOWS, POSIX_ONLY_OR_COMMON),
        (WINDOWS, WINDOWS_ONLY),
    ] {
        check_binary(flavor, table, FilePath::insert_before_extension);
    }
}

#[test]
fn remove_extension() {
    const COMMON: &[(&str, &str)] = &[
        ("", ""),
        (".", "."),
        ("..", ".."),
        ("foo.dll", "foo"),
        ("./foo.dll", "./foo"),
        ("foo..dll", "foo."),
        ("foo", "foo"),
        ("foo.", "foo"),
        ("foo..", "foo."),
        ("foo.baz.dll", "foo.baz"),
        ("/foo.bar/foo", "/foo.bar/foo"),
        ("/foo.bar/..////", "/foo.bar/..////"),
    ];
    const WINDOWS_ONLY: &[(&str, &str)] =
        &[(r"C:\foo.bar\foo", r"C:\foo.bar\foo"), (r"C:\foo.bar\..\\", r"C:\foo.bar\..\\")];
    for flavor in [POSIX, WINDOWS] {
        for (input, expected) in COMMON {
            let path = FilePath::new(flavor, input);
            assert_eq!(path.remove_extension().value(), *expected, "{flavor:?} {input:?}");
            assert_eq!(
                path.remove_final_extension().value(),
                *expected,
                "{flavor:?} final {input:?}"
            );
        }
    }
    for (input, expected) in WINDOWS_ONLY {
        let path = FilePath::new(WINDOWS, input);
        assert_eq!(path.remove_extension().value(), *expected);
        assert_eq!(path.remove_final_extension().value(), *expected);
    }

    // The C++ test loops over tarballs but always builds "foo.tar.gz". The
    // loop below checks each tarball, which the C++ loop meant to do.
    let tarballs = ["foo.tar.gz", "foo.tar.xz", "foo.tar.bz2", "foo.tar.Z", "foo.tar.bz"];
    for flavor in [POSIX, WINDOWS] {
        let path = FilePath::new(flavor, "foo.tar.gz");
        assert_eq!(path.remove_extension().value(), "foo");
        assert_eq!(path.remove_final_extension().value(), "foo.tar");
        for name in tarballs {
            let path = FilePath::new(flavor, name);
            assert_eq!(path.remove_extension().value(), "foo", "{name}");
            assert_eq!(path.remove_final_extension().value(), "foo.tar", "{name}");
        }
    }
}

#[test]
fn replace_extension() {
    // (path, new extension, expected)
    const COMMON: &[(&str, &str, &str)] = &[
        ("", "", ""),
        ("", "txt", ""),
        (".", "txt", ""),
        ("..", "txt", ""),
        (".", "", ""),
        ("foo.dll", "txt", "foo.txt"),
        ("./foo.dll", "txt", "./foo.txt"),
        ("foo..dll", "txt", "foo..txt"),
        ("foo.dll", ".txt", "foo.txt"),
        ("foo", "txt", "foo.txt"),
        ("foo.", "txt", "foo.txt"),
        ("foo..", "txt", "foo..txt"),
        ("foo", ".txt", "foo.txt"),
        ("foo.baz.dll", "txt", "foo.baz.txt"),
        ("foo.baz.dll", ".txt", "foo.baz.txt"),
        ("foo.dll", "", "foo"),
        ("foo.dll", ".", "foo"),
        ("foo", "", "foo"),
        ("foo", ".", "foo"),
        ("foo.baz.dll", "", "foo.baz"),
        ("foo.baz.dll", ".", "foo.baz"),
        ("/foo.bar/foo", "baz", "/foo.bar/foo.baz"),
        ("/foo.bar/..////", "baz", ""),
    ];
    const WINDOWS_ONLY: &[(&str, &str, &str)] =
        &[(r"C:\foo.bar\foo", "baz", r"C:\foo.bar\foo.baz"), (r"C:\foo.bar\..\\", "baz", "")];
    for flavor in [POSIX, WINDOWS] {
        check_binary(flavor, COMMON, FilePath::replace_extension);
    }
    check_binary(WINDOWS, WINDOWS_ONLY, FilePath::replace_extension);
}

#[test]
fn add_extension() {
    // (path, extension to add, expected)
    const COMMON: &[(&str, &str, &str)] = &[
        ("", "", ""),
        ("", "txt", ""),
        (".", "txt", ""),
        ("..", "txt", ""),
        (".", "", ""),
        ("foo.dll", "txt", "foo.dll.txt"),
        ("./foo.dll", "txt", "./foo.dll.txt"),
        ("foo..dll", "txt", "foo..dll.txt"),
        ("foo.dll", ".txt", "foo.dll.txt"),
        ("foo", "txt", "foo.txt"),
        ("foo.", "txt", "foo.txt"),
        ("foo..", "txt", "foo..txt"),
        ("foo", ".txt", "foo.txt"),
        ("foo.baz.dll", "txt", "foo.baz.dll.txt"),
        ("foo.baz.dll", ".txt", "foo.baz.dll.txt"),
        ("foo.dll", "", "foo.dll"),
        ("foo.dll", ".", "foo.dll"),
        ("foo", "", "foo"),
        ("foo", ".", "foo"),
        ("foo.baz.dll", "", "foo.baz.dll"),
        ("foo.baz.dll", ".", "foo.baz.dll"),
        ("/foo.bar/foo", "baz", "/foo.bar/foo.baz"),
        ("/foo.bar/..////", "baz", ""),
    ];
    const WINDOWS_ONLY: &[(&str, &str, &str)] =
        &[(r"C:\foo.bar\foo", "baz", r"C:\foo.bar\foo.baz"), (r"C:\foo.bar\..\\", "baz", "")];
    for flavor in [POSIX, WINDOWS] {
        check_binary(flavor, COMMON, FilePath::add_extension);
    }
    check_binary(WINDOWS, WINDOWS_ONLY, FilePath::add_extension);
}

/// (path, extension, expected). Umlaut and accent cases are Windows-only,
/// since the C++ ones are compiled only on Windows and macOS.
const MATCHES_COMMON: &[(&str, &str, bool)] = &[
    ("foo", "", true),
    ("foo", ".", false),
    ("foo.", "", false),
    ("foo.", ".", true),
    ("foo.txt", ".dll", false),
    ("foo.txt", ".txt", true),
    ("foo.txt.dll", ".txt", false),
    ("foo.txt.dll", ".dll", true),
    ("foo.tar.gz", ".gz", false),
    ("foo.tar.lzma", ".tar.lzma", true),
    ("foo.TXT", ".txt", true),
    ("foo.txt", ".TXT", true),
    ("foo.tXt", ".txt", true),
    ("foo.txt", ".tXt", true),
    ("foo.tXt", ".TXT", true),
    ("foo.tXt", ".tXt", true),
    ("/bar/foo.txt.dll", ".txt", false),
    ("/bar/foo.txt", ".txt", true),
];

const MATCHES_WINDOWS_ONLY: &[(&str, &str, bool)] = &[
    ("c:/foo.txt.dll", ".txt", false),
    ("c:/foo.txt", ".txt", true),
    (r"c:\bar\foo.txt.dll", ".txt", false),
    (r"c:\bar\foo.txt", ".txt", true),
    ("foo.\u{E4}\u{F6}\u{FC}", ".\u{E4}\u{F6}\u{FC}", true),
    ("foo.\u{C4}\u{D6}\u{DC}", ".\u{E4}\u{F6}\u{FC}", true),
    ("foo.\u{109}", ".\u{109}", true),
    ("foo.\u{108}", ".\u{109}", true),
];

#[test]
fn matches_extension() {
    for flavor in [POSIX, WINDOWS] {
        check_binary_bool(flavor, MATCHES_COMMON, FilePath::matches_extension);
    }
    check_binary_bool(WINDOWS, MATCHES_WINDOWS_ONLY, FilePath::matches_extension);
}

#[test]
fn matches_final_extension() {
    const COMMON: &[(&str, &str, bool)] = &[
        ("foo", "", true),
        ("foo", ".", false),
        ("foo.", "", false),
        ("foo.", ".", true),
        ("foo.txt", ".dll", false),
        ("foo.txt", ".txt", true),
        ("foo.txt.dll", ".txt", false),
        ("foo.txt.dll", ".dll", true),
        // Unlike MatchesExtension, the final extension of a double extension
        // is only the last part.
        ("foo.tar.gz", ".gz", true),
        ("foo.tar.lzma", ".lzma", true),
        ("foo.tar.lzma", ".tar.lzma", false),
        ("foo.tlzma", ".tlzma", true),
        ("foo.TXT", ".txt", true),
        ("foo.txt", ".TXT", true),
        ("foo.tXt", ".txt", true),
        ("foo.txt", ".tXt", true),
        ("foo.tXt", ".TXT", true),
        ("foo.tXt", ".tXt", true),
        ("/bar/foo.txt.dll", ".txt", false),
        ("/bar/foo.txt", ".txt", true),
    ];
    const WINDOWS_ONLY: &[(&str, &str, bool)] = &[
        ("c:/foo.txt.dll", ".txt", false),
        ("c:/foo.txt", ".txt", true),
        (r"c:\bar\foo.txt.dll", ".txt", false),
        (r"c:\bar\foo.txt", ".txt", true),
        ("foo.\u{E4}\u{F6}\u{FC}", ".\u{E4}\u{F6}\u{FC}", true),
        ("foo.\u{C4}\u{D6}\u{DC}", ".\u{E4}\u{F6}\u{FC}", true),
        ("foo.\u{109}", ".\u{109}", true),
        ("foo.\u{108}", ".\u{109}", true),
    ];
    check_binary_bool(POSIX, COMMON, FilePath::matches_final_extension);
    check_binary_bool(WINDOWS, COMMON, FilePath::matches_final_extension);
    check_binary_bool(WINDOWS, WINDOWS_ONLY, FilePath::matches_final_extension);
}

#[test]
fn compare_ignore_case() {
    // (a, b, expected). Cases valid on every flavor first.
    const COMMON: &[(&str, &str, Ordering)] = &[
        ("foo", "foo", Ordering::Equal),
        ("FOO", "foo", Ordering::Equal),
        ("foo.ext", "foo.ext", Ordering::Equal),
        ("FOO.EXT", "foo.ext", Ordering::Equal),
        ("Foo.Ext", "foo.ext", Ordering::Equal),
        ("foO", "foo", Ordering::Equal),
        ("foo", "foO", Ordering::Equal),
        ("fOo", "foo", Ordering::Equal),
        ("foo", "fOo", Ordering::Equal),
        ("bar", "foo", Ordering::Less),
        ("foo", "bar", Ordering::Greater),
        ("BAR", "foo", Ordering::Less),
        ("FOO", "bar", Ordering::Greater),
        ("bar", "FOO", Ordering::Less),
        ("foo", "BAR", Ordering::Greater),
        ("BAR", "FOO", Ordering::Less),
        ("FOO", "BAR", Ordering::Greater),
        // German Eszett: lowercase and the new-style uppercase. The uppercase
        // of U+00DF is "SS" in full mapping, but neither Windows nor macOS
        // maps it.
        ("\u{DF}", "\u{DF}", Ordering::Equal),
        ("\u{1E9E}", "\u{1E9E}", Ordering::Equal),
        ("\u{DF}", "\u{1E9E}", Ordering::Less),
        ("SS", "\u{DF}", Ordering::Less),
        ("SS", "\u{1E9E}", Ordering::Less),
    ];
    const WINDOWS_ONLY: &[(&str, &str, Ordering)] = &[
        // Umlauts A, O, U: direct comparison, then upper case against lower.
        ("\u{E4}\u{F6}\u{FC}", "\u{E4}\u{F6}\u{FC}", Ordering::Equal),
        ("\u{C4}\u{D6}\u{DC}", "\u{E4}\u{F6}\u{FC}", Ordering::Equal),
        // C with circumflex.
        ("\u{109}", "\u{109}", Ordering::Equal),
        ("\u{108}", "\u{109}", Ordering::Equal),
        // Cyrillic SHA.
        ("\u{428}", "\u{428}", Ordering::Equal),
        ("\u{428}", "\u{448}", Ordering::Equal),
        // Greek DELTA.
        ("\u{394}", "\u{394}", Ordering::Equal),
        ("\u{394}", "\u{3B4}", Ordering::Equal),
        // Full-width A. Full-width and ASCII characters are different.
        ("\u{FF21}", "\u{FF21}", Ordering::Equal),
        ("\u{FF21}", "\u{FF41}", Ordering::Equal),
        ("A", "\u{FF21}", Ordering::Less),
        ("A", "\u{FF41}", Ordering::Less),
        ("a", "\u{FF21}", Ordering::Less),
        ("a", "\u{FF41}", Ordering::Less),
    ];
    for (a, b, expected) in COMMON {
        assert_eq!(PathFlavor::Posix.compare_ignore_case(a, b), *expected, "posix {a:?} {b:?}");
        assert_eq!(PathFlavor::Windows.compare_ignore_case(a, b), *expected, "windows {a:?} {b:?}");
    }
    for (a, b, expected) in WINDOWS_ONLY {
        assert_eq!(PathFlavor::Windows.compare_ignore_case(a, b), *expected, "windows {a:?} {b:?}");
    }
}

#[test]
fn references_parent() {
    const COMMON: &[(&str, bool)] = &[
        (".", false),
        ("..", true),
        ("a..", false),
        ("..a", false),
        ("../", true),
        ("/..", true),
        ("/../", true),
        ("/a../", false),
        ("/..a/", false),
        ("//..", true),
        ("..//", true),
        ("//..//", true),
        ("a//..//c", true),
        ("../b/c", true),
        ("/../b/c", true),
        ("a/b/..", true),
        ("a/b/../", true),
        ("a/../c", true),
        ("a/b/c", false),
    ];
    const POSIX_ONLY: &[(&str, bool)] = &[(".. ", false), (" ..", false), ("...", false)];
    const WINDOWS_ONLY: &[(&str, bool)] = &[(".. ", true), (" ..", true), ("...", true)];
    check_unary_bool(POSIX, COMMON, FilePath::references_parent);
    check_unary_bool(WINDOWS, COMMON, FilePath::references_parent);
    check_unary_bool(POSIX, POSIX_ONLY, FilePath::references_parent);
    check_unary_bool(WINDOWS, WINDOWS_ONLY, FilePath::references_parent);
}

#[test]
fn from_ascii() {
    let cases: &[(&str, &str)] = &[("foo.txt", "foo.txt"), ("!#$%&'()", "!#$%&'()")];
    for flavor in [POSIX, WINDOWS] {
        for (input, expected) in cases {
            let path = FilePath::from_ascii(flavor, input).unwrap();
            assert_eq!(path.value(), *expected);
        }
        assert_eq!(FilePath::from_ascii(flavor, "caf\u{E9}"), Err(PathError::NotAscii));
    }
}

#[test]
fn from_utf8_unsafe_and_as_utf8_unsafe() {
    // (native, UTF-8 bytes)
    let cases: &[(&str, &[u8])] = &[
        ("foo.txt", b"foo.txt"),
        // "aeo" with accents.
        ("\u{E0}\u{E8}\u{F2}.txt", "\u{E0}\u{E8}\u{F2}.txt".as_bytes()),
        // Full-width "ABC".
        ("\u{FF21}\u{FF22}\u{FF23}.txt", "\u{FF21}\u{FF22}\u{FF23}.txt".as_bytes()),
    ];
    for flavor in [POSIX, WINDOWS] {
        for (i, (native, utf8)) in cases.iter().enumerate() {
            // FromUTF8 gives the native form, and AsUTF8 gives it back.
            let from_utf8 = FilePath::from_utf8_unsafe(flavor, utf8).unwrap();
            assert_eq!(from_utf8.value(), *native, "case {i}");
            let from_native = FilePath::new(flavor, native);
            assert_eq!(from_native.as_utf8_unsafe().as_bytes(), *utf8, "case {i}");
            assert_eq!(from_utf8.value(), from_native.value(), "case {i}");
        }
    }
}

#[test]
fn construct_with_nul() {
    // The constructor cuts the path at the first NUL.
    let path = FilePath::new(POSIX, "a\0b");
    assert_eq!(path.value().len(), 1);
    assert_eq!(path.value(), "a");
}

#[test]
fn append_with_nul() {
    // Append also cuts the component at the first NUL.
    for (flavor, expected) in [(POSIX, "a/b"), (WINDOWS, r"a\b")] {
        let path = FilePath::new(flavor, "a").append("b\0b");
        assert_eq!(path.value().len(), 3);
        assert_eq!(path.value(), expected);
    }
}

#[test]
fn append_base_name() {
    for (flavor, expected) in [(POSIX, "foo/bar.txt"), (WINDOWS, r"foo\bar.txt")] {
        let dir = FilePath::new(flavor, "foo");
        let file = SafeBaseName::create_from_str(flavor, "bar.txt").unwrap();
        assert_eq!(dir.append_base_name(&file), FilePath::new(flavor, expected));
    }
}

#[test]
fn references_parent_with_nul() {
    // The NUL cut leaves "..", and that still references the parent.
    let path = FilePath::new(POSIX, "..\0");
    assert_eq!(path.value().len(), 2);
    assert!(path.references_parent());
}

#[test]
fn normalize_path_separators() {
    // Windows-only in C++. The POSIX version does nothing.
    const CASES: &[(&str, &str)] = &[
        ("foo/bar", r"foo\bar"),
        (r"foo/bar\betz", r"foo\bar\betz"),
        (r"foo\bar", r"foo\bar"),
        (r"foo\bar/betz", r"foo\bar\betz"),
        ("foo", "foo"),
        // Trailing separators are kept. StripTrailingSeparators removes them.
        (r"foo\", r"foo\"),
        ("foo/", r"foo\"),
        (r"foo/bar\", r"foo\bar\"),
        (r"foo\bar/", r"foo\bar\"),
        ("foo/bar/", r"foo\bar\"),
        (r"foo\bar\", r"foo\bar\"),
        (r"\foo/bar", r"\foo\bar"),
        (r"/foo\bar", r"\foo\bar"),
        ("c:/foo/bar/", r"c:\foo\bar\"),
        ("/foo/bar/", r"\foo\bar\"),
        (r"\foo\bar\", r"\foo\bar\"),
        (r"c:\foo/bar", r"c:\foo\bar"),
        ("//foo\\bar\\", r"\\foo\bar\"),
        (r"\\foo\bar\", r"\\foo\bar\"),
        (r"//foo\bar\", r"\\foo\bar\"),
        // The number of separators is not normalized.
        (r"foo\\bar", r"foo\\bar"),
        ("foo//bar", r"foo\\bar"),
        (r"foo/\bar", r"foo\\bar"),
        (r"foo\/bar", r"foo\\bar"),
        (r"///foo\\bar", r"\\\foo\\bar"),
        ("foo//bar///", r"foo\\bar\\\"),
        (r"foo/\bar/\", r"foo\\bar\\"),
        (r"/\foo\/bar", r"\\foo\\bar"),
    ];
    for (input, expected) in CASES {
        let observed = FilePath::new(WINDOWS, input).normalize_path_separators();
        assert_eq!(observed.value(), *expected, "input {input:?}");
    }
    // POSIX: nothing to normalize.
    assert_eq!(FilePath::new(POSIX, "foo\\bar").normalize_path_separators().value(), "foo\\bar");
}

#[test]
fn ends_with_separator() {
    const CASES: &[(&str, bool)] =
        &[("", false), ("/", true), ("foo/", true), ("bar", false), ("/foo/bar", false)];
    for flavor in [POSIX, WINDOWS] {
        for (input, expected) in CASES {
            let path = FilePath::new(flavor, input).normalize_path_separators();
            assert_eq!(path.ends_with_separator(), *expected, "{flavor:?} {input:?}");
        }
    }
}

#[test]
fn as_ending_with_separator() {
    const CASES: &[(&str, &str)] = &[("", ""), ("/", "/"), ("foo", "foo/"), ("foo/", "foo/")];
    for flavor in [POSIX, WINDOWS] {
        for (input, expected) in CASES {
            let path = FilePath::new(flavor, input).normalize_path_separators();
            let expected = FilePath::new(flavor, expected).normalize_path_separators();
            assert_eq!(path.as_ending_with_separator().value(), expected.value());
        }
    }
}

#[test]
fn print_to_stream() {
    // The C++ test writes to an ostream. Here Display is used.
    assert_eq!(FilePath::new(POSIX, "foo").to_string(), "foo");
}

#[test]
fn is_separator() {
    assert!(!POSIX.is_separator('x'));
    assert!(!POSIX.is_separator('\0'));
    assert!(POSIX.is_separator('/'));
    assert!(!POSIX.is_separator('\\'));
    assert!(WINDOWS.is_separator('/'));
    assert!(WINDOWS.is_separator('\\'));
}

// ---- Extra tests that go beyond the C++ file ----

/// xorshift64 PRNG, so the tests need no external crate.
struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn string(&mut self, alphabet: &[&str], max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| alphabet[self.below(alphabet.len())]).collect()
    }
}

const RANDOM_ALPHABET: &[&str] = &["a", "B", ".", "/", "\\", ":", "c", "C", " ", "x"];

#[test]
fn random_is_parent_matches_slow_algorithm() {
    // The fast and slow IsParent algorithms are independent. They must agree.
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for flavor in [POSIX, WINDOWS] {
        for _ in 0..20_000 {
            let parent = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 6));
            let child = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 8));
            assert_eq!(
                parent.is_parent(&child),
                parent.is_parent_slow(&child),
                "{flavor:?}: parent {:?}, child {:?}",
                parent.value(),
                child.value()
            );
        }
    }
}

#[test]
fn random_extension_split_round_trips() {
    // The header documents that RemoveExtension() + Extension() gives back the
    // original path. That holds only without a trailing separator: C++ finds
    // the extension on the base name, but removes it from the full path.
    let mut rng = XorShift(0xD1B5_4A32_D192_ED03);
    for flavor in [POSIX, WINDOWS] {
        for _ in 0..20_000 {
            let path = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 8));
            if path.ends_with_separator() {
                continue;
            }
            let rebuilt = format!("{}{}", path.remove_extension().value(), path.extension());
            assert_eq!(rebuilt, path.value(), "{flavor:?}: {:?}", path.value());
            let rebuilt =
                format!("{}{}", path.remove_final_extension().value(), path.final_extension());
            assert_eq!(rebuilt, path.value(), "{flavor:?} final: {:?}", path.value());
        }
    }
}

#[test]
fn remove_extension_with_trailing_separator_is_a_documented_quirk() {
    // Matches the C++ code: Extension() is found on the base name "a.b", but
    // RemoveExtension() cuts the full path "a.b/" at its last dot.
    let path = FilePath::new(POSIX, "a.b/");
    assert_eq!(path.extension(), ".b");
    assert_eq!(path.remove_extension().value(), "a");
}

#[test]
fn random_base_name_has_no_separator() {
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    for flavor in [POSIX, WINDOWS] {
        for _ in 0..20_000 {
            let path = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 8));
            let base = path.base_name();
            // A base that is all separators is the root, and that is allowed.
            if !base.value().chars().all(|c| flavor.is_separator(c)) {
                assert!(
                    !base.value().chars().any(|c| flavor.is_separator(c)),
                    "{flavor:?}: {:?} -> base {:?}",
                    path.value(),
                    base.value()
                );
            }
        }
    }
}

#[test]
fn random_strip_trailing_separators_is_idempotent() {
    let mut rng = XorShift(0xA076_1D64_78BD_642F);
    for flavor in [POSIX, WINDOWS] {
        for _ in 0..20_000 {
            let path = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 8));
            let once = path.strip_trailing_separators();
            assert_eq!(once.strip_trailing_separators(), once);
            // It must be a prefix of the original. It can end in a separator
            // only when it is a root, such as "/" or "c:/".
            assert!(path.value().starts_with(once.value()));
            if once.ends_with_separator() {
                let root = once.base_name();
                assert!(
                    root.value().chars().all(|c| flavor.is_separator(c)),
                    "{flavor:?}: {:?} -> {:?}",
                    path.value(),
                    once.value()
                );
            }
        }
    }
}

#[test]
fn random_append_then_base_name_round_trips() {
    let mut rng = XorShift(0x5851_F42D_4C95_7F2D);
    let names = ["a", "B.txt", "x y", "..x", "c.d.e"];
    for flavor in [POSIX, WINDOWS] {
        for _ in 0..5_000 {
            let dir = FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 6));
            let name = names[rng.below(names.len())];
            let joined = dir.append(name);
            assert_eq!(joined.base_name().value(), name, "{flavor:?}: dir {:?}", dir.value());
            // The joined path is a parent of anything appended to it.
            assert!(joined.is_parent(&joined.append("z")), "{flavor:?}: {:?}", joined.value());
        }
    }
}

#[test]
fn random_ordering_and_hash_are_consistent() {
    let mut rng = XorShift(0x0123_4567_89AB_CDEF);
    for flavor in [POSIX, WINDOWS] {
        let items: Vec<FilePath> =
            (0..200).map(|_| FilePath::new(flavor, &rng.string(RANDOM_ALPHABET, 5))).collect();
        for a in &items {
            for b in &items {
                let eq = a == b;
                assert_eq!(eq, a.cmp(b) == Ordering::Equal);
                assert_eq!(eq, b.cmp(a) == Ordering::Equal);
                if eq {
                    assert_eq!(hash_of(a), hash_of(b), "{:?} vs {:?}", a.value(), b.value());
                }
            }
        }
        // Sorting must give a non-decreasing sequence.
        let mut sorted = items.clone();
        sorted.sort();
        for pair in sorted.windows(2) {
            assert!(pair[0] <= pair[1]);
        }
    }
}

#[test]
fn random_compare_ignore_case_matches_ascii_reference() {
    // For ASCII, POSIX folds to lowercase. Windows folds to uppercase, so
    // the ordering of characters between 'Z' and 'a' differs.
    let mut rng = XorShift(0xBF58_476D_1CE4_E5B9);
    let alphabet = ["a", "Z", "z", "A", "_", "[", "`", "{", "0", "@", "^", "\\"];
    for _ in 0..20_000 {
        let a = rng.string(&alphabet, 5);
        let b = rng.string(&alphabet, 5);
        let posix_expected = a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase());
        assert_eq!(PathFlavor::Posix.compare_ignore_case(&a, &b), posix_expected, "{a:?} {b:?}");
        let windows_expected = a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase());
        assert_eq!(
            PathFlavor::Windows.compare_ignore_case(&a, &b),
            windows_expected,
            "{a:?} {b:?}"
        );
    }
}

#[test]
fn windows_drive_letter_is_not_absolute_without_separator() {
    assert!(!FilePath::new(WINDOWS, "C:").is_absolute());
    assert!(FilePath::new(WINDOWS, r"C:\").is_absolute());
    // On POSIX, "C:\" is a relative name that contains a backslash.
    assert!(!FilePath::new(POSIX, r"C:\").is_absolute());
    assert!(FilePath::new(POSIX, "/").is_absolute());
}

#[test]
fn flavors_do_not_compare_equal() {
    assert_ne!(FilePath::new(POSIX, "a"), FilePath::new(WINDOWS, "a"));
    assert!(!FilePath::new(POSIX, "/a").is_parent(&FilePath::new(WINDOWS, "/a/b")));
}

#[test]
fn safe_base_name_rejects_unsafe_names() {
    assert!(SafeBaseName::create_from_str(POSIX, "..").is_none());
    assert!(SafeBaseName::create_from_str(POSIX, "a/..").is_none());
    assert!(SafeBaseName::create_from_str(POSIX, "/").is_none());
    // BaseName drops the leading "/", so "/abs" is safe and its base is "abs".
    assert_eq!(SafeBaseName::create_from_str(POSIX, "/abs").unwrap().path().value(), "abs");
    assert!(SafeBaseName::create_from_str(POSIX, "dir/name").is_some());
    // BaseName drops the trailing separator, so "dir/" is safe and its base is
    // "dir". The C++ check runs on the base name, so this matches it.
    let safe = SafeBaseName::create_from_str(POSIX, "dir/").unwrap();
    assert_eq!(safe.path().value(), "dir");
    assert!(SafeBaseName::create_from_str(WINDOWS, r"c:\x").is_some());
    assert!(SafeBaseName::create_from_str(WINDOWS, "c:/").is_none());
}
