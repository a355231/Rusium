// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of url/url_parse_unittest.cc. Every table and TEST case is ported with
//! the same inputs and expected values. Tables that the C++ runs on `char`
//! also run here on UTF-8 bytes (`&[u8]`) and on UTF-16 (`&[u16]`).
//!
//! The C++ file-URL table has separate WIN32 and non-WIN32 halves. Only the
//! non-WIN32 half is ported, because the WIN32 rules are not implemented.

use url_parse::{
    extract_file_name, extract_query_key_value, make_range, parse_authority, parse_file_system_url,
    parse_file_url, parse_mailto_url, parse_non_special_url, parse_path_url, parse_port,
    parse_standard_url, ComponentType, Component, Parsed, ParserMode, Port, Unit,
};

/// `nullptr` in the C++ tables.
type R = Option<&'static str>;
const N: R = None;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Mirrors the C++ `ComponentMatches`. A `None` reference means the component
/// must be absent (`len == -1`). A `Some` reference means it must be present,
/// with the same length and characters.
fn component_matches<U: Unit>(input: &[U], reference: R, component: Component) -> bool {
    // -1 is the only allowed negative length.
    if !component.is_valid() && component.len != -1 {
        return false;
    }
    // The begin offset must not be negative.
    if component.begin < 0 {
        return false;
    }
    match reference {
        None => component.len == -1,
        Some(r) => {
            component.is_valid()
                && r.len() == component.len as usize
                && component
                    .as_view_on(input)
                    .iter()
                    .zip(r.bytes())
                    .all(|(u, b)| u.code() == u32::from(b))
        }
    }
}

fn assert_component<U: Unit>(input: &[U], reference: R, component: Component, what: &str) {
    assert!(
        component_matches(input, reference, component),
        "{what}: expected {reference:?} at {component}"
    );
}

/// Mirrors the C++ `ExpectInvalidComponent`.
fn expect_invalid(component: Component) {
    assert_eq!(0, component.begin);
    assert_eq!(-1, component.len);
}

fn port_int<U: Unit>(input: &[U], component: Component) -> i32 {
    parse_port(input, component).to_cpp_int()
}

/// Regular URL case. Mirrors the C++ `URLParseCase`.
#[derive(Clone, Copy)]
struct Case {
    input: &'static str,
    scheme: R,
    username: R,
    password: R,
    host: R,
    port: i32,
    path: R,
    query: R,
    reference: R,
}

#[allow(clippy::too_many_arguments)]
const fn case(
    input: &'static str,
    scheme: R,
    username: R,
    password: R,
    host: R,
    port: i32,
    path: R,
    query: R,
    reference: R,
) -> Case {
    Case {
        input,
        scheme,
        username,
        password,
        host,
        port,
        path,
        query,
        reference,
    }
}

fn check_case<U: Unit>(expected: &Case, input: &[U], parsed: &Parsed) {
    let what = expected.input;
    let port = port_int(input, parsed.port);
    assert_component(input, expected.scheme, parsed.scheme, what);
    assert_component(input, expected.username, parsed.username, what);
    assert_component(input, expected.password, parsed.password, what);
    assert_component(input, expected.host, parsed.host, what);
    assert_eq!(expected.port, port, "{what}: port");
    assert_component(input, expected.path, parsed.path, what);
    assert_component(input, expected.query, parsed.query, what);
    assert_component(input, expected.reference, parsed.reference, what);
}

// Parsed --------------------------------------------------------------------

#[test]
fn component_predicates() {
    let invalid = Component::default();
    assert!(!invalid.is_valid());
    assert!(invalid.is_empty());
    assert!(!invalid.is_nonempty());

    let empty = Component::new(3, 0);
    assert!(empty.is_valid());
    assert!(empty.is_empty());
    assert!(!empty.is_nonempty());

    let nonempty = Component::new(3, 1);
    assert!(nonempty.is_valid());
    assert!(!nonempty.is_empty());
    assert!(nonempty.is_nonempty());
}

#[test]
fn username_only_user_info_utf16() {
    let url = wide("http://a@c:29/d");
    let parsed = parse_standard_url(&url);
    assert_eq!(Component::new(7, 1), parsed.username);
    expect_invalid(parsed.password);
    assert_eq!(Component::new(9, 1), parsed.host);
    assert_eq!(29, port_int(&url, parsed.port));
    assert_eq!(Component::new(13, 2), parsed.path);
}

#[test]
fn empty_username_user_info_utf16() {
    let url = wide("http://@c:29/d");
    let parsed = parse_standard_url(&url);
    assert_eq!(Component::new(7, 0), parsed.username);
    expect_invalid(parsed.password);
    assert_eq!(Component::new(8, 1), parsed.host);
    assert_eq!(29, port_int(&url, parsed.port));
    assert_eq!(Component::new(12, 2), parsed.path);
}

#[test]
fn parse_authority_does_not_inspect_before_component() {
    let spec = b"@host:29";
    let auth = parse_authority(spec, Component::new(1, 7), ParserMode::Special);

    expect_invalid(auth.username);
    expect_invalid(auth.password);
    assert_eq!(Component::new(1, 4), auth.host);
    assert_eq!(Component::new(6, 2), auth.port);
    assert_eq!(29, port_int(spec, auth.port));
}

#[test]
fn length() {
    let length_cases = [
        // One with everything in it.
        "http://user:pass@host:99/foo?bar#baz",
        // One with nothing in it.
        "",
        // Working backwards, let's start taking off stuff from the full one.
        "http://user:pass@host:99/foo?bar#",
        "http://user:pass@host:99/foo?bar",
        "http://user:pass@host:99/foo?",
        "http://user:pass@host:99/foo",
        "http://user:pass@host:99/",
        "http://user:pass@host:99",
        "http://user:pass@host:",
        "http://user:pass@host",
        "http://host",
        "http://user@",
        "http:",
    ];
    for length_case in length_cases {
        let true_length = length_case.len() as i32;
        let parsed = parse_standard_url(length_case.as_bytes());
        assert_eq!(true_length, parsed.length(), "{length_case}");
        let parsed16 = parse_standard_url(&wide(length_case));
        assert_eq!(true_length, parsed16.length(), "{length_case} (utf16)");
    }
}

#[test]
fn count_characters_before() {
    use ComponentType::*;
    // (url, component, include_delimiter, expected_count)
    let count_cases: &[(&str, ComponentType, bool, i32)] = &[
        // Test each possibility in the case where all components are present.
        //    0         1         2
        //    0123456789012345678901
        ("http://u:p@h:8/p?q#r", Scheme, true, 0),
        ("http://u:p@h:8/p?q#r", Scheme, false, 0),
        ("http://u:p@h:8/p?q#r", Username, true, 7),
        ("http://u:p@h:8/p?q#r", Username, false, 7),
        ("http://u:p@h:8/p?q#r", Password, true, 9),
        ("http://u:p@h:8/p?q#r", Password, false, 9),
        ("http://u:p@h:8/p?q#r", Host, true, 11),
        ("http://u:p@h:8/p?q#r", Host, false, 11),
        ("http://u:p@h:8/p?q#r", Port, true, 12),
        ("http://u:p@h:8/p?q#r", Port, false, 13),
        ("http://u:p@h:8/p?q#r", Path, false, 14),
        ("http://u:p@h:8/p?q#r", Path, true, 14),
        ("http://u:p@h:8/p?q#r", Query, true, 16),
        ("http://u:p@h:8/p?q#r", Query, false, 17),
        ("http://u:p@h:8/p?q#r", Ref, true, 18),
        ("http://u:p@h:8/p?q#r", Ref, false, 19),
        // Now test when the requested component is missing.
        ("http://u:p@h:8/p?", Ref, true, 17),
        ("http://u:p@h:8/p?q", Ref, true, 18),
        ("http://u:p@h:8/p#r", Query, true, 16),
        ("http://u:p@h:8#r", Path, true, 14),
        ("http://u:p@h/", Port, true, 12),
        ("http://u:p@/", Host, true, 11),
        // This case is a little weird. It will report that the password would
        // start where the host begins. This is arguably correct, although you
        // could also argue that it should start at the '@' sign. Doing it
        // starting with the '@' sign is actually harder, so we don't bother.
        ("http://u@h/", Password, true, 9),
        ("http://h/", Username, true, 7),
        ("http:", Username, true, 5),
        ("", Scheme, true, 0),
        // Make sure a random component still works when there's nothing there.
        ("", Ref, true, 0),
        // File URLs are special with no host, so we test those.
        ("file:///c:/foo", Username, true, 7),
        ("file:///c:/foo", Password, true, 7),
        ("file:///c:/foo", Host, true, 7),
        ("file:///c:/foo", Path, true, 7),
    ];
    for &(url, component, include_delimiter, expected) in count_cases {
        // Simple test to distinguish file and standard URLs.
        let parse = |input: &[u8]| {
            if input.first() == Some(&b'f') {
                parse_file_url(input)
            } else {
                parse_standard_url(input)
            }
        };
        let parsed = parse(url.as_bytes());
        assert_eq!(
            expected,
            parsed.count_characters_before(component, include_delimiter),
            "{url} {component:?} {include_delimiter}"
        );
        // The same counts must come out of the UTF-16 parse.
        let parsed16 = if url.as_bytes().first() == Some(&b'f') {
            parse_file_url(&wide(url))
        } else {
            parse_standard_url(&wide(url))
        };
        assert_eq!(
            expected,
            parsed16.count_characters_before(component, include_delimiter),
            "{url} {component:?} {include_delimiter} (utf16)"
        );
    }
}

// Standard ------------------------------------------------------------------

// clang-format off
// Input                               Scheme  Usrname  Passwd     Host         Port Path       Query        Ref
static STANDARD_CASES: &[Case] = &[
// Regular URL with all the parts
case("http://user:pass@foo:21/bar;par?b#c", Some("http"), Some("user"), Some("pass"), Some("foo"), 21, Some("/bar;par"), Some("b"), Some("c")),

// Known schemes should lean towards authority identification
case("http:foo.com", Some("http"), N, N, Some("foo.com"), -1, N, N, N),

// Spaces!
case("\t   :foo.com   \n", Some(""), N, N, Some("foo.com"), -1, N, N, N),
case(" foo.com  ", N, N, N, Some("foo.com"), -1, N, N, N),
case("a:\t foo.com", Some("a"), N, N, Some("\t foo.com"), -1, N, N, N),
case("http://f:21/ b ? d # e ", Some("http"), N, N, Some("f"), 21, Some("/ b "), Some(" d "), Some(" e")),

// Invalid port numbers should be identified and turned into -2, empty port
// numbers should be -1. Spaces aren't allowed in port numbers
case("http://f:/c", Some("http"), N, N, Some("f"), -1, Some("/c"), N, N),
case("http://f:0/c", Some("http"), N, N, Some("f"), 0, Some("/c"), N, N),
case("http://f:00000000000000/c", Some("http"), N, N, Some("f"), 0, Some("/c"), N, N),
case("http://f:00000000000000000000080/c", Some("http"), N, N, Some("f"), 80, Some("/c"), N, N),
case("http://f:b/c", Some("http"), N, N, Some("f"), -2, Some("/c"), N, N),
case("http://f: /c", Some("http"), N, N, Some("f"), -2, Some("/c"), N, N),
case("http://f:\n/c", Some("http"), N, N, Some("f"), -2, Some("/c"), N, N),
case("http://f:fifty-two/c", Some("http"), N, N, Some("f"), -2, Some("/c"), N, N),
case("http://f:999999/c", Some("http"), N, N, Some("f"), -2, Some("/c"), N, N),
case("http://f: 21 / b ? d # e ", Some("http"), N, N, Some("f"), -2, Some("/ b "), Some(" d "), Some(" e")),

// Creative URLs missing key elements
case("", N, N, N, N, -1, N, N, N),
case("  \t", N, N, N, N, -1, N, N, N),
case(":foo.com/", Some(""), N, N, Some("foo.com"), -1, Some("/"), N, N),
case(":foo.com\\", Some(""), N, N, Some("foo.com"), -1, Some("\\"), N, N),
case(":", Some(""), N, N, N, -1, N, N, N),
case(":a", Some(""), N, N, Some("a"), -1, N, N, N),
case(":/", Some(""), N, N, N, -1, N, N, N),
case(":\\", Some(""), N, N, N, -1, N, N, N),
case(":#", Some(""), N, N, N, -1, N, N, Some("")),
case("#", N, N, N, N, -1, N, N, Some("")),
case("#/", N, N, N, N, -1, N, N, Some("/")),
case("#\\", N, N, N, N, -1, N, N, Some("\\")),
case("#;?", N, N, N, N, -1, N, N, Some(";?")),
case("?", N, N, N, N, -1, N, Some(""), N),
case("/", N, N, N, N, -1, N, N, N),
case(":23", Some(""), N, N, Some("23"), -1, N, N, N),
case("/:23", Some("/"), N, N, Some("23"), -1, N, N, N),
case("//", N, N, N, N, -1, N, N, N),
case("::", Some(""), N, N, N, -1, N, N, N),
case("::23", Some(""), N, N, N, 23, N, N, N),
case("foo://", Some("foo"), N, N, N, -1, N, N, N),

// Username/passwords and things that look like them
case("http://a:b@c:29/d", Some("http"), Some("a"), Some("b"), Some("c"), 29, Some("/d"), N, N),
case("http://a@c:29/d", Some("http"), Some("a"), N, Some("c"), 29, Some("/d"), N, N),
case("http://@c:29/d", Some("http"), Some(""), N, Some("c"), 29, Some("/d"), N, N),
case("http::@c:29", Some("http"), Some(""), Some(""), Some("c"), 29, N, N, N),
// ... "]" in the password field isn't allowed, but we tolerate it here...
case("http://&a:foo(b]c@d:2/", Some("http"), Some("&a"), Some("foo(b]c"), Some("d"), 2, Some("/"), N, N),
case("http://::@c@d:2", Some("http"), Some(""), Some(":@c"), Some("d"), 2, N, N, N),
case("http://foo.com:b@d/", Some("http"), Some("foo.com"), Some("b"), Some("d"), -1, Some("/"), N, N),

case("http://foo.com/\\@", Some("http"), N, N, Some("foo.com"), -1, Some("/\\@"), N, N),
case("http:\\\\foo.com\\", Some("http"), N, N, Some("foo.com"), -1, Some("\\"), N, N),
case("http:\\\\a\\b:c\\d@foo.com\\", Some("http"), N, N, Some("a"), -1, Some("\\b:c\\d@foo.com\\"), N, N),

// Tolerate different numbers of slashes.
case("foo:/", Some("foo"), N, N, N, -1, N, N, N),
case("foo:/bar.com/", Some("foo"), N, N, Some("bar.com"), -1, Some("/"), N, N),
case("foo://///////", Some("foo"), N, N, N, -1, N, N, N),
case("foo://///////bar.com/", Some("foo"), N, N, Some("bar.com"), -1, Some("/"), N, N),
case("foo:////://///", Some("foo"), N, N, N, -1, Some("/////"), N, N),

// Raw file paths on Windows aren't handled by the parser.
case("c:/foo", Some("c"), N, N, Some("foo"), -1, N, N, N),
case("//foo/bar", N, N, N, Some("foo"), -1, Some("/bar"), N, N),

// Use the first question mark for the query and the ref.
case("http://foo/path;a??e#f#g", Some("http"), N, N, Some("foo"), -1, Some("/path;a"), Some("?e"), Some("f#g")),
case("http://foo/abcd?efgh?ijkl", Some("http"), N, N, Some("foo"), -1, Some("/abcd"), Some("efgh?ijkl"), N),
case("http://foo/abcd#foo?bar", Some("http"), N, N, Some("foo"), -1, Some("/abcd"), N, Some("foo?bar")),

// IPv6, check also interesting uses of colons.
case("[61:24:74]:98", Some("[61"), N, N, Some("24:74]"), 98, N, N, N),
case("http://[61:27]:98", Some("http"), N, N, Some("[61:27]"), 98, N, N, N),
case("http:[61:27]/:foo", Some("http"), N, N, Some("[61:27]"), -1, Some("/:foo"), N, N),
case("http://[1::2]:3:4", Some("http"), N, N, Some("[1::2]:3"), 4, N, N, N),

// Partially-complete IPv6 literals, and related cases.
case("http://2001::1", Some("http"), N, N, Some("2001:"), 1, N, N, N),
case("http://[2001::1", Some("http"), N, N, Some("[2001::1"), -1, N, N, N),
case("http://2001::1]", Some("http"), N, N, Some("2001::1]"), -1, N, N, N),
case("http://2001::1]:80", Some("http"), N, N, Some("2001::1]"), 80, N, N, N),
case("http://[2001::1]", Some("http"), N, N, Some("[2001::1]"), -1, N, N, N),
case("http://[2001::1]:80", Some("http"), N, N, Some("[2001::1]"), 80, N, N, N),
case("http://[[::]]", Some("http"), N, N, Some("[[::]]"), -1, N, N, N),
];
// clang-format on

fn run_standard_cases<U: Unit>(
    to_units: impl Fn(&str) -> Vec<U>,
    cases: &[Case],
) {
    for case in cases {
        let input = to_units(case.input);
        let parsed = parse_standard_url(&input);
        check_case(case, &input, &parsed);
    }
}

#[test]
fn standard() {
    run_standard_cases(|s| s.as_bytes().to_vec(), STANDARD_CASES);
}

#[test]
fn standard_utf16() {
    run_standard_cases(wide, STANDARD_CASES);
}

// PathURL -------------------------------------------------------------------

// Various incarnations of path URLs.
// clang-format off
struct PathCase {
    input: &'static str,
    scheme: R,
    path: R,
}

static PATH_CASES: &[PathCase] = &[
PathCase { input: "", scheme: N, path: N },
PathCase { input: ":", scheme: Some(""), path: N },
PathCase { input: ":/", scheme: Some(""), path: Some("/") },
PathCase { input: "/", scheme: N, path: Some("/") },
PathCase { input: " This is \\interesting// \t", scheme: N, path: Some("This is \\interesting// \t") },
PathCase { input: "about:", scheme: Some("about"), path: N },
PathCase { input: "about:blank", scheme: Some("about"), path: Some("blank") },
PathCase { input: "  about: blank ", scheme: Some("about"), path: Some(" blank ") },
PathCase { input: "javascript :alert(\"He:/l\\l#o?foo\"); ", scheme: Some("javascript "), path: Some("alert(\"He:/l\\l#o?foo\"); ") },
];
// clang-format on

fn check_path_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for (i, case) in PATH_CASES.iter().enumerate() {
        let input = to_units(case.input);
        let parsed = parse_path_url(&input, false);

        assert!(
            component_matches(&input, case.scheme, parsed.scheme),
            "{i}: scheme"
        );
        assert!(
            component_matches(&input, case.path, parsed.get_content()),
            "{i}: path"
        );

        // The remaining components are never used for path URLs.
        expect_invalid(parsed.username);
        expect_invalid(parsed.password);
        expect_invalid(parsed.host);
        expect_invalid(parsed.port);
    }
}

#[test]
fn path_url() {
    check_path_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn path_url_utf16() {
    check_path_cases(wide);
}

// File URLs -----------------------------------------------------------------

// Various incarnations of file URLs. Only the non-WIN32 half of the C++ table
// is ported.
// clang-format off
static FILE_CASES: &[Case] = &[
case("file:",                    Some("file"), N, N, N,              -1, N,            N, N),
case("file:path",                Some("file"), N, N, N,              -1, Some("path"), N, N),
case("file:path/",               Some("file"), N, N, N,              -1, Some("path/"), N, N),
case("file:path/f.txt",          Some("file"), N, N, N,              -1, Some("path/f.txt"), N, N),
// One slash.
case("file:/",                   Some("file"), N, N, N,              -1, Some("/"), N, N),
case("file:/path",               Some("file"), N, N, N,              -1, Some("/path"), N, N),
case("file:/path/",              Some("file"), N, N, N,              -1, Some("/path/"), N, N),
case("file:/path/f.txt",         Some("file"), N, N, N,              -1, Some("/path/f.txt"), N, N),
// Two slashes.
case("file://",                  Some("file"), N, N, N,              -1, N, N, N),
case("file://server",            Some("file"), N, N, Some("server"), -1, N, N, N),
case("file://server/",           Some("file"), N, N, Some("server"), -1, Some("/"), N, N),
case("file://server/f.txt",      Some("file"), N, N, Some("server"), -1, Some("/f.txt"), N, N),
// Three slashes.
case("file:///",                 Some("file"), N, N, N,              -1, Some("/"), N, N),
case("file:///path",             Some("file"), N, N, N,              -1, Some("/path"), N, N),
case("file:///path/",            Some("file"), N, N, N,              -1, Some("/path/"), N, N),
case("file:///path/f.txt",       Some("file"), N, N, N,              -1, Some("/path/f.txt"), N, N),
// More than three slashes.
case("file:////",                Some("file"), N, N, N,              -1, Some("/"), N, N),
case("file:////path",            Some("file"), N, N, N,              -1, Some("/path"), N, N),
case("file:////path/",           Some("file"), N, N, N,              -1, Some("/path/"), N, N),
case("file:////path/f.txt",      Some("file"), N, N, N,              -1, Some("/path/f.txt"), N, N),
// Schemeless URLs
case("path/f.txt",               N, N, N, N,                         -1, Some("path/f.txt"), N, N),
case("path:80/f.txt",            Some("path"), N, N, N,              -1, Some("80/f.txt"), N, N),
case("path/f.txt:80",            Some("path/f.txt"), N, N, N,        -1, Some("80"), N, N), // Wrong.
case("/path/f.txt",              N, N, N, N,                         -1, Some("/path/f.txt"), N, N),
case("/path:80/f.txt",           N, N, N, N,                         -1, Some("/path:80/f.txt"), N, N),
case("/path/f.txt:80",           N, N, N, N,                         -1, Some("/path/f.txt:80"), N, N),
case("//server/f.txt",           N, N, N, Some("server"),            -1, Some("/f.txt"), N, N),
case("//server:80/f.txt",        N, N, N, Some("server:80"),         -1, Some("/f.txt"), N, N),
case("//server/f.txt:80",        N, N, N, Some("server"),            -1, Some("/f.txt:80"), N, N),
case("///path/f.txt",            N, N, N, N,                         -1, Some("/path/f.txt"), N, N),
case("///path:80/f.txt",         N, N, N, N,                         -1, Some("/path:80/f.txt"), N, N),
case("///path/f.txt:80",         N, N, N, N,                         -1, Some("/path/f.txt:80"), N, N),
case("////path/f.txt",           N, N, N, N,                         -1, Some("/path/f.txt"), N, N),
case("////path:80/f.txt",        N, N, N, N,                         -1, Some("/path:80/f.txt"), N, N),
case("////path/f.txt:80",        N, N, N, N,                         -1, Some("/path/f.txt:80"), N, N),
// Queries and refs are valid for file URLs as well.
case("file:///foo.html?#",       Some("file"), N, N, N,              -1, Some("/foo.html"), Some(""), Some("")),
case("file:///foo.html?q=y#ref", Some("file"), N, N, N,              -1, Some("/foo.html"), Some("q=y"), Some("ref")),
];
// clang-format on

fn check_file_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for case in FILE_CASES {
        let input = to_units(case.input);
        let parsed = parse_file_url(&input);
        check_case(case, &input, &parsed);
        assert!(!parsed.has_opaque_path, "{}", case.input);
    }
}

#[test]
fn file_url_cases() {
    check_file_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn file_url_cases_utf16() {
    check_file_cases(wide);
}

#[test]
fn extract_file_name_cases() {
    // (input, expected file name)
    let extract_cases: &[(&str, R)] = &[
        ("http://www.google.com", N),
        ("http://www.google.com/", Some("")),
        ("http://www.google.com/search", Some("search")),
        ("http://www.google.com/search/", Some("")),
        ("http://www.google.com/foo/bar.html?baz=22", Some("bar.html")),
        ("http://www.google.com/foo/bar.html#ref", Some("bar.html")),
        ("http://www.google.com/search/;param", Some("")),
        ("http://www.google.com/foo/bar.html;param#ref", Some("bar.html")),
        ("http://www.google.com/foo/bar.html;foo;param#ref", Some("bar.html")),
        ("http://www.google.com/foo/bar.html?query#ref", Some("bar.html")),
        ("http://www.google.com/foo;/bar.html", Some("bar.html")),
        ("http://www.google.com/foo;/", Some("")),
        ("http://www.google.com/foo;", Some("foo")),
        ("http://www.google.com/;", Some("")),
        ("http://www.google.com/foo;bar;html", Some("foo")),
    ];

    for &(url, expected) in extract_cases {
        let input = url.as_bytes();
        let parsed = parse_standard_url(input);
        let file_name = extract_file_name(input, parsed.path);
        assert!(component_matches(input, expected, file_name), "{url}");

        let input16 = wide(url);
        let parsed16 = parse_standard_url(&input16);
        let file_name16 = extract_file_name(&input16, parsed16.path);
        assert!(component_matches(&input16, expected, file_name16), "{url} (utf16)");
    }
}

/// Returns true if the `parameter`th (1-based) key/value in the URL's query
/// has the expected key and value. A `None` key means no such parameter exists.
/// Mirrors the C++ `NthParameterIs`.
fn nth_parameter_is(
    url: &str,
    parameter: i32,
    expected_key: R,
    expected_value: R,
) -> bool {
    let input = url.as_bytes();
    let parsed = parse_standard_url(input);
    let mut query = parsed.query;

    for i in 1..=parameter {
        let Some((key, value)) = extract_query_key_value(input, &mut query) else {
            if parameter >= i && expected_key.is_none() {
                return true; // Expected nonexistent key, got one.
            }
            return false; // Not enough keys.
        };

        if i == parameter {
            let Some(want_key) = expected_key else {
                return false;
            };
            let want_value = expected_value.unwrap_or("");
            return component_bytes_eq(input, key, want_key)
                && component_bytes_eq(input, value, want_value);
        }
    }
    expected_key.is_none() // We didn't find that many parameters.
}

fn component_bytes_eq(input: &[u8], c: Component, expected: &str) -> bool {
    c.as_view_on(input) == expected.as_bytes()
}

#[test]
fn query_key_value_cases() {
    assert!(nth_parameter_is("http://www.google.com", 1, N, N));

    // Basic case.
    let a = "http://www.google.com?arg1=1&arg2=2&bar";
    assert!(nth_parameter_is(a, 1, Some("arg1"), Some("1")));
    assert!(nth_parameter_is(a, 2, Some("arg2"), Some("2")));
    assert!(nth_parameter_is(a, 3, Some("bar"), Some("")));
    assert!(nth_parameter_is(a, 4, N, N));

    // Empty param at the end.
    let b = "http://www.google.com?foo=bar&";
    assert!(nth_parameter_is(b, 1, Some("foo"), Some("bar")));
    assert!(nth_parameter_is(b, 2, N, N));

    // Empty param at the beginning.
    let c = "http://www.google.com?&foo=bar";
    assert!(nth_parameter_is(c, 1, Some(""), Some("")));
    assert!(nth_parameter_is(c, 2, Some("foo"), Some("bar")));
    assert!(nth_parameter_is(c, 3, N, N));

    // Empty key with value.
    let d = "http://www.google.com?=foo";
    assert!(nth_parameter_is(d, 1, Some(""), Some("foo")));
    assert!(nth_parameter_is(d, 2, N, N));

    // Empty value with key.
    let e = "http://www.google.com?foo=";
    assert!(nth_parameter_is(e, 1, Some("foo"), Some("")));
    assert!(nth_parameter_is(e, 2, N, N));

    // Empty key and values.
    let f = "http://www.google.com?&&==&=";
    assert!(nth_parameter_is(f, 1, Some(""), Some("")));
    assert!(nth_parameter_is(f, 2, Some(""), Some("")));
    assert!(nth_parameter_is(f, 3, Some(""), Some("=")));
    assert!(nth_parameter_is(f, 4, Some(""), Some("")));
    assert!(nth_parameter_is(f, 5, N, N));
}

#[test]
fn query_key_value_cases_utf16() {
    // The same iteration over a UTF-16 input.
    let url = wide("http://www.google.com?&&==&=");
    let parsed = parse_standard_url(&url);
    let mut query = parsed.query;
    let expected: &[(&str, &str)] = &[("", ""), ("", ""), ("", "="), ("", "")];
    for &(k, v) in expected {
        let (key, value) = extract_query_key_value(&url, &mut query).expect("pair");
        assert!(component_matches(&url, Some(k), key));
        assert!(component_matches(&url, Some(v), value));
    }
    assert!(extract_query_key_value(&url, &mut query).is_none());
}

// MailtoURL -----------------------------------------------------------------

struct MailtoCase {
    input: &'static str,
    scheme: R,
    path: R,
    query: R,
}

// clang-format off
static MAILTO_CASES: &[MailtoCase] = &[
//|input                       |scheme   |path               |query
MailtoCase { input: "mailto:foo@gmail.com",        scheme: Some("mailto"), path: Some("foo@gmail.com"),    query: N },
MailtoCase { input: "  mailto: to  \t",            scheme: Some("mailto"), path: Some(" to"),              query: N },
MailtoCase { input: "mailto:addr1%2C%20addr2 ",    scheme: Some("mailto"), path: Some("addr1%2C%20addr2"), query: N },
MailtoCase { input: "Mailto:addr1, addr2 ",         scheme: Some("Mailto"), path: Some("addr1, addr2"),     query: N },
MailtoCase { input: "mailto:addr1:addr2 ",          scheme: Some("mailto"), path: Some("addr1:addr2"),      query: N },
MailtoCase { input: "mailto:?to=addr1,addr2",       scheme: Some("mailto"), path: N,                        query: Some("to=addr1,addr2") },
MailtoCase { input: "mailto:?to=addr1%2C%20addr2",  scheme: Some("mailto"), path: N,                        query: Some("to=addr1%2C%20addr2") },
MailtoCase { input: "mailto:addr1?to=addr2",        scheme: Some("mailto"), path: Some("addr1"),            query: Some("to=addr2") },
MailtoCase { input: "mailto:?body=#foobar#",        scheme: Some("mailto"), path: N,                        query: Some("body=#foobar#") },
MailtoCase { input: "mailto:#?body=#foobar#",       scheme: Some("mailto"), path: Some("#"),                query: Some("body=#foobar#") },
];
// clang-format on

fn check_mailto_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for case in MAILTO_CASES {
        let input = to_units(case.input);
        let parsed = parse_mailto_url(&input);
        let port = port_int(&input, parsed.port);

        assert!(component_matches(&input, case.scheme, parsed.scheme), "{}", case.input);
        assert!(component_matches(&input, case.path, parsed.path), "{}", case.input);
        assert!(component_matches(&input, case.query, parsed.query), "{}", case.input);
        assert_eq!(-1, port, "{}", case.input); // PORT_UNSPECIFIED
        assert_eq!(Port::Unspecified, parse_port(&input, parsed.port));
        assert!(!parsed.has_opaque_path, "{}", case.input);

        // The remaining components are never used for mailto URLs.
        expect_invalid(parsed.username);
        expect_invalid(parsed.password);
        expect_invalid(parsed.port);
        expect_invalid(parsed.reference);
    }
}

#[test]
fn mailto_url() {
    check_mailto_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn mailto_url_utf16() {
    check_mailto_cases(wide);
}

// FileSystemURL ------------------------------------------------------------

struct FileSystemCase {
    input: &'static str,
    inner_scheme: R,
    inner_username: R,
    inner_password: R,
    inner_host: R,
    inner_port: i32,
    inner_path: R,
    path: R,
    query: R,
    reference: R,
}

// Various incarnations of filesystem URLs.
static FILESYSTEM_CASES: &[FileSystemCase] = &[
    // Regular URL with all the parts
    FileSystemCase { input: "filesystem:http://user:pass@foo:21/temporary/bar;par?b#c", inner_scheme: Some("http"), inner_username: Some("user"), inner_password: Some("pass"), inner_host: Some("foo"), inner_port: 21, inner_path: Some("/temporary"), path: Some("/bar;par"), query: Some("b"), reference: Some("c") },
    FileSystemCase { input: "filesystem:https://foo/persistent/bar;par/", inner_scheme: Some("https"), inner_username: N, inner_password: N, inner_host: Some("foo"), inner_port: -1, inner_path: Some("/persistent"), path: Some("/bar;par/"), query: N, reference: N },
    FileSystemCase { input: "filesystem:file:///persistent/bar;par/", inner_scheme: Some("file"), inner_username: N, inner_password: N, inner_host: N, inner_port: -1, inner_path: Some("/persistent"), path: Some("/bar;par/"), query: N, reference: N },
    FileSystemCase { input: "filesystem:file:///persistent/bar;par/?query#ref", inner_scheme: Some("file"), inner_username: N, inner_password: N, inner_host: N, inner_port: -1, inner_path: Some("/persistent"), path: Some("/bar;par/"), query: Some("query"), reference: Some("ref") },
    FileSystemCase { input: "filesystem:file:///persistent", inner_scheme: Some("file"), inner_username: N, inner_password: N, inner_host: N, inner_port: -1, inner_path: Some("/persistent"), path: Some(""), query: N, reference: N },
    FileSystemCase { input: "filesystem:", inner_scheme: N, inner_username: N, inner_password: N, inner_host: N, inner_port: -1, inner_path: N, path: N, query: N, reference: N },
];

fn check_filesystem_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for case in FILESYSTEM_CASES {
        let input = to_units(case.input);
        let parsed = parse_file_system_url(&input);

        assert!(component_matches(&input, Some("filesystem"), parsed.scheme), "{}", case.input);
        assert_eq!(case.inner_scheme.is_none(), parsed.inner_parsed().is_none(), "{}", case.input);
        // Only check the inner_parsed if there is one.
        if let Some(inner) = parsed.inner_parsed() {
            assert!(component_matches(&input, case.inner_scheme, inner.scheme), "{}", case.input);
            assert!(component_matches(&input, case.inner_username, inner.username), "{}", case.input);
            assert!(component_matches(&input, case.inner_password, inner.password), "{}", case.input);
            assert!(component_matches(&input, case.inner_host, inner.host), "{}", case.input);
            assert!(component_matches(&input, case.inner_path, inner.path), "{}", case.input);
            assert_eq!(case.inner_port, port_int(&input, inner.port), "{}", case.input);

            // The remaining components are never used for filesystem URLs.
            expect_invalid_inner(inner.query);
            expect_invalid_inner(inner.reference);
        }

        assert!(component_matches(&input, case.path, parsed.path), "{}", case.input);
        assert!(component_matches(&input, case.query, parsed.query), "{}", case.input);
        assert!(component_matches(&input, case.reference, parsed.reference), "{}", case.input);
        assert!(!parsed.has_opaque_path, "{}", case.input);

        // The remaining components are never used for filesystem URLs.
        expect_invalid(parsed.username);
        expect_invalid(parsed.password);
        expect_invalid(parsed.host);
        expect_invalid(parsed.port);
    }
}

/// The C++ checks begin == 0 and len == -1 for the inner query and reference,
/// which are reset after being moved to the outer URL.
fn expect_invalid_inner(component: Component) {
    expect_invalid(component);
}

#[test]
fn file_system_url() {
    check_filesystem_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn file_system_url_utf16() {
    check_filesystem_cases(wide);
}

// NonSpecial ----------------------------------------------------------------

// Non-special URLs which don't have an opaque path.
static NON_SPECIAL_CASES: &[Case] = &[
    case("git://user:pass@foo:21/bar;par?b#c", Some("git"), Some("user"), Some("pass"), Some("foo"), 21, Some("/bar;par"), Some("b"), Some("c")),
    case("git://host", Some("git"), N, N, Some("host"), -1, N, N, N),
    case("git://host/a/../b", Some("git"), N, N, Some("host"), -1, Some("/a/../b"), N, N),
    case("git://host/a b", Some("git"), N, N, Some("host"), -1, Some("/a b"), N, N),
    case("git://ho\\st/", Some("git"), N, N, Some("ho\\st"), -1, Some("/"), N, N),
    // Empty users
    case("git://@host", Some("git"), Some(""), N, Some("host"), -1, N, N, N),
    // Empty user and invalid host. "git://@" is an invalid URL.
    case("git://@", Some("git"), Some(""), N, N, -1, N, N, N),
    // Invalid host and non-empty port. "git://:80" is an invalid URL.
    case("git://:80", Some("git"), N, N, N, 80, N, N, N),
    // Empty host cases
    case("git://", Some("git"), N, N, Some(""), -1, N, N, N),
    case("git:///", Some("git"), N, N, Some(""), -1, Some("/"), N, N),
    case("git:////", Some("git"), N, N, Some(""), -1, Some("//"), N, N),
    // Null host cases
    case("git:/", Some("git"), N, N, N, -1, Some("/"), N, N),
    case("git:/trailing-space ", Some("git"), N, N, N, -1, Some("/trailing-space"), N, N),
];

fn check_non_special_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for case in NON_SPECIAL_CASES {
        let input = to_units(case.input);
        let parsed = parse_non_special_url(&input);
        check_case(case, &input, &parsed);
        assert!(!parsed.has_opaque_path, "url: {}", case.input);
    }
}

#[test]
fn non_special() {
    check_non_special_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn non_special_utf16() {
    check_non_special_cases(wide);
}

// Non-special URLs which have an opaque path.
static NON_SPECIAL_OPAQUE_CASES: &[Case] = &[
    case("git:", Some("git"), N, N, N, -1, N, N, N),
    case("git:opaque", Some("git"), N, N, N, -1, Some("opaque"), N, N),
    case("git:opaque?a=b#c", Some("git"), N, N, N, -1, Some("opaque"), Some("a=b"), Some("c")),
    case("git: o p a q u e ", Some("git"), N, N, N, -1, Some(" o p a q u e"), N, N),
    case("git:opa\\que", Some("git"), N, N, N, -1, Some("opa\\que"), N, N),
];

fn check_non_special_opaque_cases<U: Unit>(to_units: impl Fn(&str) -> Vec<U>) {
    for case in NON_SPECIAL_OPAQUE_CASES {
        let input = to_units(case.input);
        let parsed = parse_non_special_url(&input);
        check_case(case, &input, &parsed);
        assert!(parsed.has_opaque_path, "url: {}", case.input);
    }
}

#[test]
fn non_special_opaque_path() {
    check_non_special_opaque_cases(|s| s.as_bytes().to_vec());
}

#[test]
fn non_special_opaque_path_utf16() {
    check_non_special_opaque_cases(wide);
}

#[test]
fn make_range_matches_component_constructor() {
    assert_eq!(Component::new(3, 4), make_range(3, 7));
    assert_eq!(Component::new(5, 0), make_range(5, 5));
}
