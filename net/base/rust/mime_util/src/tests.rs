// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Ports of net/base/mime_util_unittest.cc, plus seeded randomized checks.
//
// Not ported (compiled out of a Linux build of the C++ tests): the ChromeOS-only
// p12/pfx expectations, the iOS-only text/x-markdown expectation, and the
// Windows wide-string branch of TestParseMimeTypeWithoutParameter.

use file_path::{FilePath, PathFlavor};

use super::*;

fn pairs(p: &[(&str, &str)]) -> StringPairs {
    p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn posix(path: &str) -> FilePath {
    FilePath::new(PathFlavor::Posix, path)
}

// ---------------------------------------------------------------------------
// Ported from mime_util_unittest.cc.
// ---------------------------------------------------------------------------

#[test]
fn test_well_known_extension() {
    // String: png\0css
    let contains_null_byte = "png\0css";

    let tests: &[(&str, Option<&str>)] = &[
        ("png", Some("image/png")),
        ("PNG", Some("image/png")),
        ("css", Some("text/css")),
        ("pjp", Some("image/jpeg")),
        ("pjpeg", Some("image/jpeg")),
        ("json", Some("application/json")),
        ("js", Some("text/javascript")),
        ("webm", Some("video/webm")),
        ("weba", Some("audio/webm")),
        ("avif", Some("image/avif")),
        ("epub", Some("application/epub+zip")),
        ("apk", Some("application/vnd.android.package-archive")),
        ("cer", Some("application/x-x509-ca-cert")),
        ("crt", Some("application/x-x509-ca-cert")),
        ("zip", Some("application/zip")),
        ("ics", Some("text/calendar")),
        ("m3u8", Some("application/x-mpegurl")),
        ("csv", Some("text/csv")),
        ("mkv", Some("video/matroska")),
        ("mka", Some("audio/matroska")),
        ("md", Some("text/markdown")),
        ("not an extension / for sure", None),
        (contains_null_byte, None),
    ];

    for (ext, expected) in tests {
        assert_eq!(
            get_well_known_mime_type_from_extension(ext).as_deref(),
            *expected,
            "extension {ext:?}"
        );
    }
}

#[test]
fn extension_test() {
    // String: png\0css
    let contains_null_byte = "png\0css";

    // Expected MIME types. Each entry lists what the built-in tables must
    // contain. The C++ test also accepts platform overrides, which are not
    // consulted here.
    let tests: &[(&str, &[&str])] = &[
        ("png", &["image/png"]),
        ("PNG", &["image/png"]),
        ("css", &["text/css"]),
        ("pjp", &["image/jpeg"]),
        ("pjpeg", &["image/jpeg"]),
        ("json", &["application/json"]),
        ("js", &["text/javascript"]),
        ("webm", &["video/webm"]),
        ("weba", &["audio/webm"]),
        ("avif", &["image/avif"]),
        // Listed as ChromeOS-only test cases in C++. The built-in tables give
        // the same answers on Linux, so they are checked here too.
        ("epub", &["application/epub+zip"]),
        ("apk", &["application/vnd.android.package-archive"]),
        ("zip", &["application/zip"]),
        ("ics", &["text/calendar"]),
        ("m3u8", &["application/x-mpegurl"]),
        ("csv", &["text/csv"]),
        ("mkv", &["video/matroska"]),
        ("mka", &["audio/matroska"]),
        ("md", &["text/markdown"]),
        ("not an extension / for sure", &[]),
        (contains_null_byte, &[]),
    ];

    for (ext, mime_types) in tests {
        match get_mime_type_from_extension(ext) {
            Some(mime_type) => assert!(
                mime_types.contains(&mime_type.as_str()),
                "extension {ext:?} gave {mime_type:?}, expected one of {mime_types:?}"
            ),
            None => assert!(mime_types.is_empty(), "extension {ext:?}"),
        }
    }
}

// Behavior of GetPreferredExtensionForMimeType() is platform-dependent in C++.
// These cases only hold when the platform does not override the built-in
// mappings. This port has no platform step, so they always hold.
#[test]
fn test_preferred_extension() {
    let cases: &[(&str, &str)] = &[
        ("application/wasm", "wasm"),     // Primary
        ("application/javascript", "js"), // Secondary
        ("text/javascript", "js"),        // Primary
        ("video/webm", "webm"),           // Primary
    ];

    for (mime_type, expected_extension) in cases {
        assert_eq!(
            get_preferred_extension_for_mime_type(mime_type).as_deref(),
            Some(*expected_extension),
            "{mime_type}"
        );
    }
}

#[test]
fn file_test() {
    let tests: &[(&str, &str, bool)] = &[
        ("c:\\foo\\bar.css", "text/css", true),
        ("c:\\foo\\bar.CSS", "text/css", true),
        ("c:\\blah", "", false),
        ("/usr/local/bin/mplayer", "", false),
        ("/home/foo/bar.css", "text/css", true),
        ("/blah.", "", false),
        ("c:\\blah.", "", false),
    ];

    for (file_path, mime_type, valid) in tests {
        let path = posix(file_path);
        let got = get_mime_type_from_file(&path);
        assert_eq!(got.is_some(), *valid, "{file_path:?}");
        if let Some(got) = got {
            assert_eq!(got, *mime_type, "{file_path:?}");
        }

        let got = get_well_known_mime_type_from_file(&path);
        assert_eq!(got.is_some(), *valid, "{file_path:?} (well-known)");
        if let Some(got) = got {
            assert_eq!(got, *mime_type, "{file_path:?} (well-known)");
        }
    }
}

#[test]
fn test_matches_mime_type() {
    use MimeTypeValidationLevel::{None as Lvl0, WildcardSlashAndTokens, WildcardSlashOnly};

    // (pattern, mime_type, without validation, slash-only, strict).
    let cases: &[(&str, &str, bool, bool, bool)] = &[
        // MIME types are case insensitive.
        ("VIDEO/*", "video/x-mpeg", true, true, true),
        ("video/*", "VIDEO/X-MPEG", true, true, true),
        ("*", "video/x-mpeg", true, true, true),
        ("video/*", "video/x-mpeg", true, true, true),
        ("video/*", "video/*", true, true, true),
        ("video/x-mpeg", "video/x-mpeg", true, true, true),
        ("application/*+xml", "application/html+xml", true, true, true),
        ("application/*+xml", "application/+xml", true, true, true),
        ("application/*+json", "application/x-myformat+json", true, true, true),
        ("aaa*aaa", "aaaaaa", true, false, false),
        ("*", "", true, false, false),
        ("video/", "video/x-mpeg", false, false, false),
        ("VIDEO/", "Video/X-MPEG", false, false, false),
        ("", "video/x-mpeg", false, false, false),
        ("", "", false, false, false),
        ("video/x-mpeg", "", false, false, false),
        ("application/*+xml", "application/xml", false, false, false),
        ("application/*+xml", "application/html+xmlz", false, false, false),
        ("application/*+xml", "applcation/html+xml", false, false, false),
        ("aaa*aaa", "aaaaa", false, false, false),
        ("*", "video/x-mpeg;param=val", true, true, true),
        ("*", "Video/X-MPEG;PARAM=VAL", true, true, true),
        ("video/*", "video/x-mpeg;param=val", true, true, true),
        ("video/*;param=val", "video/mpeg", false, false, false),
        ("Video/*;PARAM=VAL", "VIDEO/Mpeg", false, false, false),
        ("video/*;param=val", "video/mpeg;param=other", false, false, false),
        ("video/*;param=val", "video/mpeg;param=val", true, true, true),
        ("Video/*;PARAM=Val", "VIDEO/Mpeg;Param=Val", true, true, true),
        ("Video/*;PARAM=VAL", "VIDEO/Mpeg;Param=Val", false, false, false),
        ("video/x-mpeg", "video/x-mpeg;param=val", true, true, true),
        ("video/x-mpeg;param=val", "video/x-mpeg;param=val", true, true, true),
        ("video/x-mpeg;param2=val2", "video/x-mpeg;param=val", false, false, false),
        ("video/x-mpeg;param2=val2", "video/x-mpeg;param2=val", false, false, false),
        ("video/x-mpeg;param=val", "video/x-mpeg;param=val;param2=val2", true, true, true),
        ("Video/X-Mpeg;Param=Val", "VIDEO/X-MPEG;PARAM=Val;PARAM2=val2", true, true, true),
        ("Video/X-Mpeg;Param=VAL", "VIDEO/X-MPEG;PARAM=VAL;PARAM2=val2", true, true, true),
        ("Video/X-Mpeg;Param=val", "VIDEO/X-MPEG;PARAM=VAL;PARAM2=val2", false, false, false),
        (
            "video/x-mpeg;param=VAL;param2=val2",
            "video/x-mpeg;param=val;param2=val2",
            false,
            false,
            false,
        ),
        (
            "video/x-mpeg;param2=val2;param=val",
            "video/x-mpeg;param=val;param2=val2",
            true,
            true,
            true,
        ),
        (
            "video/x-mpeg;param3=val3;param=val",
            "video/x-mpeg;param=val;param2=val2",
            false,
            false,
            false,
        ),
        (
            "video/x-mpeg;param=val ;param2=val2 ",
            "video/x-mpeg;param=val;param2=val2",
            true,
            true,
            true,
        ),
        ("*/*;param=val", "video/x-mpeg;param=val", true, true, true),
        ("*/*;param=val", "video/x-mpeg;param=val2", false, false, false),
        ("*", "*", true, false, false),
        ("*", "*/*", true, true, true),
        ("*/*", "*/*", true, true, true),
        ("*/*", "*", true, false, false),
        ("video/*", "video/*", true, true, true),
        ("video/*", "*/*", false, false, false),
        ("video/*;param=val", "video/*", false, false, false),
        ("video/*;param=val", "video/*;param=val", true, true, true),
        ("video/*;param=val", "video/*;param=val2", false, false, false),
        ("ab*cd", "abxxxcd", true, false, false),
        ("ab*cd", "abx/xcd", true, true, true),
        ("ab/*cd", "ab/xxxcd", true, true, true),
        ("*+json", "application/hal+json", true, true, true),
        ("*+json", "application/vnd.collection+json", true, true, true),
        ("*+json", "invalidmimetype+json", true, false, false),
        ("*+json", "applic\x09ation/vnd.api+json", true, true, false),
        ("*+json", "application/vnd\x09.api+json", true, true, false),
        ("*+json", "applic<ation/vnd.api+json", true, true, false),
        ("*+json", "application/vnd<api+json", true, true, false),
        ("*+json", "applic ation/vnd api+json", true, true, false),
        ("*+json", "\"application/vnd.api+json", true, true, false),
        ("*+json", "application/vnd.api +json", true, true, false),
        ("*+json", "\"application\"/\"vnd.api\"+json", true, true, false),
        ("*", "invalid", true, false, false),
        ("*/*", "invalid", true, false, false),
        ("*", "valid/mime", true, true, true),
        ("*/*", "valid/mime", true, true, true),
        ("text", "text/plain/extra", false, false, false),
        ("text/*", "text/plain/extra", true, false, false),
        ("text/plain/extra", "text/plain/extra", true, true, true),
        ("text/*", "text;charset=utf-8/plain", false, false, false),
        ("text/*;charset=utf-8/extra", "text/plain;charset=utf-8/extra", true, true, true),
        ("image*jpeg", "image/jpeg", true, true, true),
        ("image*jpeg", "image/jpeg/extra", false, false, false),
    ];

    for &(pattern, mime_type, without, slash_only, strict) in cases {
        assert_eq!(
            matches_mime_type(pattern, mime_type, Lvl0),
            without,
            "Pattern: '{pattern}', MIME type: '{mime_type}', no validation"
        );
        assert_eq!(
            matches_mime_type(pattern, mime_type, WildcardSlashOnly),
            slash_only,
            "Pattern: '{pattern}', MIME type: '{mime_type}', slash-only validation"
        );
        assert_eq!(
            matches_mime_type(pattern, mime_type, WildcardSlashAndTokens),
            strict,
            "Pattern: '{pattern}', MIME type: '{mime_type}', strict validation"
        );
    }
}

#[test]
fn test_parse_mime_type() {
    // (input, expected mime type, expected params).
    type ParseCase = (&'static str, &'static str, &'static [(&'static str, &'static str)]);
    let tests: &[ParseCase] = &[
        // Simple tests.
        ("image/jpeg", "image/jpeg", &[]),
        (
            "application/octet-stream;foo=bar;name=\"test.jpg\"",
            "application/octet-stream",
            &[("foo", "bar"), ("name", "test.jpg")],
        ),
        // Quoted string parsing.
        ("t/s;name=\"t\\\\est\\\".jpg\"", "t/s", &[("name", "t\\est\".jpg")]),
        ("t/s;name=\"test.jpg\"", "t/s", &[("name", "test.jpg")]),
        ("t/s;name=\"test;jpg\"", "t/s", &[("name", "test;jpg")]),
        // Lenient for no closing quote.
        ("t/s;name=\"test.jpg", "t/s", &[("name", "test.jpg")]),
        ("t/s;name=\"ab\\\"", "t/s", &[("name", "ab\"")]),
        // Strip whitespace from start/end of mime_type.
        (" t/s", "t/s", &[]),
        ("t/s ", "t/s", &[]),
        (" t/s ", "t/s", &[]),
        ("t/=", "t/=", &[]),
        // Generally ignore whitespace.
        ("t/s;a=1;b=2", "t/s", &[("a", "1"), ("b", "2")]),
        ("t/s ;a=1;b=2", "t/s", &[("a", "1"), ("b", "2")]),
        ("t/s; a=1;b=2", "t/s", &[("a", "1"), ("b", "2")]),
        // Special case, include whitespace after param name until equals.
        ("t/s;a =1;b=2", "t/s", &[("a ", "1"), ("b", "2")]),
        ("t/s;a= 1;b=2", "t/s", &[("a", "1"), ("b", "2")]),
        ("t/s;a=1 ;b=2", "t/s", &[("a", "1"), ("b", "2")]),
        ("t/s;a=1; b=2", "t/s", &[("a", "1"), ("b", "2")]),
        ("t/s; a = 1;b=2", "t/s", &[("a ", "1"), ("b", "2")]),
        // Do not trim whitespace from quoted-string param values.
        ("t/s;a=\" 1\";b=2", "t/s", &[("a", " 1"), ("b", "2")]),
        ("t/s;a=\"1 \";b=2", "t/s", &[("a", "1 "), ("b", "2")]),
        ("t/s;a=\" 1 \";b=2", "t/s", &[("a", " 1 "), ("b", "2")]),
        // Ignore incomplete params.
        ("t/s;a", "t/s", &[]),
        ("t/s;a=", "t/s", &[]),
        ("t/s;a=1;", "t/s", &[("a", "1")]),
        ("t/s;a=1;b", "t/s", &[("a", "1")]),
        ("t/s;a=1;b=", "t/s", &[("a", "1")]),
        // Allow empty subtype.
        ("t/", "t/", &[]),
        ("ts/", "ts/", &[]),
        ("t/;", "t/", &[]),
        ("t/ s", "t/", &[]),
        // Questionable: allow anything as long as there is a slash somewhere.
        ("/ts", "/ts", &[]),
        ("/s", "/s", &[]),
        ("/", "/", &[]),
    ];
    for (input, mime_type, params) in tests {
        let parsed = parse_mime_type(input).unwrap_or_else(|| panic!("parse {input:?}"));
        assert_eq!(parsed.mime_type, *mime_type, "{input:?}");
        assert_eq!(parsed.params, pairs(params), "{input:?}");
    }
    for input in [
        // Must have slash in mime type.
        "", "ts", "t / s",
    ] {
        assert!(parse_mime_type(input).is_none(), "{input:?}");
    }
}

#[test]
fn test_parse_mime_type_without_parameter() {
    let ok = |s: &str| assert!(parse_mime_type_without_parameter(s).is_some(), "{s:?}");
    let bad = |s: &str| assert!(parse_mime_type_without_parameter(s).is_none(), "{s:?}");

    let non_ascii = "application/nonutf8\u{2603}"; // unicode snowman
    assert!(parse_mime_type_without_parameter("application/nonutf8").is_some());
    bad(non_ascii);

    assert_eq!(
        parse_mime_type_without_parameter("application/mime"),
        Some(("application".to_string(), "mime".to_string()))
    );

    // Various allowed subtype forms.
    ok("application/json");
    ok("application/x-suggestions+json");
    ok("application/+json");

    // Upper case letters are allowed.
    ok("text/mime");
    ok("TEXT/mime");
    ok("Text/mime");
    ok("TeXt/mime");

    // Experimental types are also considered to be valid.
    ok("x-video/mime");
    ok("X-Video/mime");

    bad("text");
    bad("text/");
    bad("text/ ");
    bad("te(xt/ ");
    bad("text/()plain");

    bad("x-video");
    bad("x-video/");

    bad("application/a/b/c");

    // Test leading and trailing whitespace.
    ok(" text/plain");
    ok("text/plain ");
    bad("text /plain");
    bad("text/ plain ");

    ok("\ttext/plain");
    ok("text/plain\t");
    bad("text\t/plain");
    bad("text/\tplain ");

    ok("\u{0B}text/plain");
    ok("text/plain\u{0B}");
    bad("text\u{0B}/plain");
    bad("text/\u{0B}plain ");

    ok("\rtext/plain");
    ok("text/plain\r");
    bad("text\r/plain");
    bad("text/\rplain ");

    ok("\ntext/plain");
    ok("text/plain\n");
    bad("text\n/plain");
    bad("text/\nplain ");
}

#[test]
fn extract_mime_type_invalid() {
    // Each of these must fail.
    let invalid = [
        // Fails because it doesn't contain '/'.
        "a",
        "application",
        // Space is not HTTP token code point.
        // U+2003, EM SPACE (UTF-8: E2 80 83).
        "\u{2003}text/html",
        "text\u{2003}/html",
        "text / html",
        "t e x t / h t m l",
        "text\r\n/\nhtml",
        "text\n/\nhtml",
        ", text/html",
        "; text/html",
    ];
    for input in invalid {
        assert_eq!(extract_mime_type_from_media_type(input, true), None, "{input:?}");
    }
}

#[test]
fn extract_mime_type_valid() {
    let valid = [
        "text/html",
        "text/html; charset=iso-8859-1",
        // Quoted charset parameter.
        "text/html; charset=\"quoted\"",
        // Multiple parameters.
        "text/html; charset=x; foo=bar",
        // OWSes are trimmed.
        " text/html   ",
        "\ttext/html \t",
        "text/html ; charset=iso-8859-1",
        // Non-standard multiple type/subtype listing using a comma as a
        // separator is accepted.
        "text/html,text/plain",
        "text/html , text/plain",
        "text/html\t,\ttext/plain",
        "text/html,text/plain;charset=iso-8859-1",
        "\r\ntext/html\r\n",
        "text/html;wow",
        "text/html;;;;;;",
        "text/html; = = = ",
    ];
    for input in valid {
        assert_eq!(
            extract_mime_type_from_media_type(input, true).as_deref(),
            Some("text/html"),
            "{input:?}"
        );
    }
}

#[test]
fn test_is_valid_top_level_mime_type() {
    for ok in [
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
        "TEXT",
        "Text",
        "TeXt",
        "x-video",
        "X-video",
        "x-custom",
        "X-Custom",
    ] {
        assert!(is_valid_top_level_mime_type(ok), "{ok}");
    }

    for bad in [
        "mime",
        "",
        "/",
        " ",
        "x-",
        "x-scheme-handler",
        "X-Scheme-Handler",
        "x-content",
        "X-Content",
        "x-directory",
        "X-Directory",
        "x-special",
        "X-Special",
    ] {
        assert!(!is_valid_top_level_mime_type(bad), "{bad}");
    }
}

#[test]
fn test_get_extensions_for_mime_type() {
    // (mime type, min expected size, contained result, no matches).
    let tests: &[(&str, usize, Option<&str>, bool)] = &[
        ("text/plain", 2, Some("txt"), false),
        ("text/pl", 0, None, true),
        ("*", 0, None, false),
        ("", 0, None, true),
        ("message/*", 1, Some("eml"), false),
        ("MeSsAge/*", 1, Some("eml"), false),
        ("message/", 0, None, true),
        ("image/avif", 1, Some("avif"), false),
        ("image/bmp", 1, Some("bmp"), false),
        ("video/matroska", 1, Some("mkv"), false),
        ("audio/matroska", 1, Some("mka"), false),
        ("video/*", 6, Some("mp4"), false),
        ("video/*", 6, Some("mpeg"), false),
        ("audio/*", 6, Some("oga"), false),
        ("aUDIo/*", 6, Some("wav"), false),
    ];

    for (mime_type, min_expected_size, contained_result, no_matches) in tests {
        let extensions = get_extensions_for_mime_type(mime_type);
        assert!(*min_expected_size <= extensions.len(), "{mime_type}: got {extensions:?}");

        if *no_matches {
            assert_eq!(extensions.len(), 0, "{mime_type}");
        }

        if let Some(contained) = contained_result {
            assert!(
                extensions.iter().any(|e| e == contained),
                "Must find at least the contained result within {mime_type}"
            );
        }
    }
}

#[test]
fn test_generate_mime_multipart_boundary() {
    let boundary1 = generate_mime_multipart_boundary();
    let boundary2 = generate_mime_multipart_boundary();

    // RFC 1341 says: the boundary parameter [...] consists of 1 to 70 characters.
    assert!(boundary1.len() <= 70);
    assert!(boundary2.len() <= 70);

    // RFC 1341 asks to: exercise care to choose a unique boundary.
    assert_ne!(boundary1, boundary2);
    assert!(boundary1.len() >= 16);
    assert!(boundary2.len() >= 16);

    // The boundary must not contain '\0'.
    assert!(!boundary1.contains('\0'));
    assert!(!boundary2.contains('\0'));

    // Asserts below are not RFC 1341 requirements, but are here to improve
    // readability of generated MIME documents.
    assert_eq!(&boundary1[..2], "--");
    assert_eq!(&boundary2[..2], "--");
    assert!(boundary1.contains("MultipartBoundary"));
    assert!(boundary2.contains("MultipartBoundary"));
    assert_eq!(&boundary1[boundary1.len() - 2..], "--");
    assert_eq!(&boundary2[boundary2.len() - 2..], "--");
}

#[test]
fn test_add_multipart_value_for_upload() {
    let ref_output = concat!(
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"value name\"\r\n",
        "Content-Type: content type\r\n\r\nvalue\r\n",
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"value name\"\r\n\r\nvalue\r\n",
        "--boundary--\r\n",
    );
    let mut post_data = String::new();
    post_data.push_str(&add_multipart_value_for_upload(
        "value name",
        "value",
        "boundary",
        "content type",
    ));
    post_data.push_str(&add_multipart_value_for_upload("value name", "value", "boundary", ""));
    post_data.push_str(&add_multipart_final_delimiter_for_upload("boundary"));
    assert_eq!(ref_output, post_data);
}

#[test]
fn test_add_multipart_value_for_upload_with_file_name() {
    let ref_output = concat!(
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"value name\"; filename=\"file name\"\r\n",
        "Content-Type: content type\r\n\r\nvalue\r\n",
        "--boundary\r\n",
        "Content-Disposition: form-data; name=\"value name\"; filename=\"file name\"\r\n\r\nvalue\r\n",
        "--boundary--\r\n",
    );
    let mut post_data = String::new();
    post_data.push_str(&add_multipart_value_with_file_name_for_upload(
        "value name",
        "file name",
        "value",
        "boundary",
        "content type",
    ));
    post_data.push_str(&add_multipart_value_with_file_name_for_upload(
        "value name",
        "file name",
        "value",
        "boundary",
        "",
    ));
    post_data.push_str(&add_multipart_final_delimiter_for_upload("boundary"));
    assert_eq!(ref_output, post_data);
}

#[test]
fn scoped_override_get_mime_type_for_testing() {
    // Checks the behavior for a png file.
    let verify_expectations = |expected_mime_type: &str| {
        assert_eq!(
            get_well_known_mime_type_from_extension("png").as_deref(),
            Some(expected_mime_type)
        );
        assert_eq!(
            get_well_known_mime_type_from_file(&posix("c:\\foo\\bar.png")).as_deref(),
            Some(expected_mime_type)
        );
        assert_eq!(get_mime_type_from_extension("png").as_deref(), Some(expected_mime_type));
        assert_eq!(
            get_mime_type_from_file(&posix("c:\\foo\\bar.png")).as_deref(),
            Some(expected_mime_type)
        );

        // Behavior other than "get a mime type" should be unaffected by the
        // override.
        assert_eq!(get_preferred_extension_for_mime_type("text/javascript").as_deref(), Some("js"));
        assert_eq!(
            extract_mime_type_from_media_type("text/html", true).as_deref(),
            Some("text/html")
        );
    };

    // Normal state, without override.
    verify_expectations("image/png");

    {
        let overriding_mime_type = "text/not-a-real-mime-type";
        let _override = ScopedOverrideGetMimeTypeForTesting::new(overriding_mime_type);
        verify_expectations(overriding_mime_type);
    }

    // Reset after override is destroyed.
    verify_expectations("image/png");
}

// ---------------------------------------------------------------------------
// Extra tests (not in the C++ file).
// ---------------------------------------------------------------------------

/// Standard glob matcher with `*` matching any run of bytes. Used as an
/// independent reference for `matches_mime_type` when there are no parameters
/// and at most one `*`.
fn glob_reference(p: &[u8], t: &[u8]) -> bool {
    match p.split_first() {
        None => t.is_empty(),
        Some((b'*', rest)) => (0..=t.len()).any(|i| glob_reference(rest, &t[i..])),
        Some((&c, rest)) => {
            !t.is_empty() && t[0].eq_ignore_ascii_case(&c) && glob_reference(rest, &t[1..])
        }
    }
}

#[test]
fn matches_mime_type_matches_glob_reference() {
    let mut rng = Rng::from_seed(0x1234_5678_9abc_def1);
    for _ in 0..20_000 {
        // Pattern: 1..=6 chars from {a, b}, with at most one '*' placed at random.
        let len = 1 + rng.below(6);
        let mut pattern: Vec<u8> =
            (0..len).map(|_| if rng.below(2) == 0 { b'a' } else { b'b' }).collect();
        if rng.below(2) == 0 {
            let at = rng.below(len + 1);
            pattern.insert(at, b'*');
        }
        // Type: 0..=8 chars from {a, b, A, B}.
        let tlen = rng.below(9);
        let typ: Vec<u8> = (0..tlen).map(|_| *b"abAB".get(rng.below(4)).unwrap()).collect();

        let p = std::str::from_utf8(&pattern).unwrap();
        let t = std::str::from_utf8(&typ).unwrap();
        assert_eq!(
            matches_mime_type(p, t, MimeTypeValidationLevel::None),
            glob_reference(&pattern, &typ),
            "pattern {p:?} type {t:?}"
        );
    }
}

#[test]
fn extension_lookup_is_case_insensitive_and_follows_table_order() {
    let mut rng = Rng::from_seed(0xdead_beef_0001);
    let all: Vec<(&str, &str)> = PRIMARY_MAPPINGS
        .iter()
        .chain(SECONDARY_MAPPINGS.iter())
        .flat_map(|&(mime, exts)| exts.split(',').map(move |e| (e, mime)))
        .collect();

    for (ext, _) in &all {
        // Expected: the first table entry (primary before secondary) that lists it.
        let expected = all.iter().find(|(e, _)| e.eq_ignore_ascii_case(ext)).map(|(_, m)| *m);
        for _ in 0..4 {
            let mixed: String = ext
                .chars()
                .map(|c| if rng.below(2) == 0 { c.to_ascii_uppercase() } else { c })
                .collect();
            assert_eq!(get_mime_type_from_extension(&mixed).as_deref(), expected, "{mixed}");
        }
    }
    assert_eq!(get_mime_type_from_extension("no-such-ext"), None);
}

#[test]
fn preferred_extension_is_first_listed() {
    for &(mime, exts) in PRIMARY_MAPPINGS {
        let first = exts.split(',').next().unwrap();
        assert_eq!(get_preferred_extension_for_mime_type(mime).as_deref(), Some(first), "{mime}");
    }
    // octet-stream never has a preferred extension, even though it is listed.
    assert_eq!(get_preferred_extension_for_mime_type("application/octet-stream"), None);
    // Lookups are case-sensitive for the MIME type, as in C++.
    assert_eq!(get_preferred_extension_for_mime_type("Text/HTML"), None);
}

#[test]
fn extensions_for_mime_type_are_sorted_and_unique() {
    for mime in ["video/*", "audio/*", "text/*", "image/jpeg", "text/plain"] {
        let exts = get_extensions_for_mime_type(mime);
        let mut sorted = exts.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(exts, sorted, "{mime}");
    }
    assert!(get_extensions_for_mime_type("*/*").is_empty());
}

#[test]
fn override_is_per_thread() {
    let _override = ScopedOverrideGetMimeTypeForTesting::new("text/override");
    assert_eq!(get_mime_type_from_extension("png").as_deref(), Some("text/override"));
    let other = std::thread::spawn(|| get_mime_type_from_extension("png")).join();
    assert_eq!(other.unwrap().as_deref(), Some("image/png"));
}

#[test]
fn extension_guards_reject_nul_and_long_input() {
    assert_eq!(get_mime_type_from_extension("png\0"), None);
    let long = "a".repeat(MAX_EXTENSION_LEN + 1);
    assert_eq!(get_mime_type_from_extension(&long), None);
    // A bare "." has no extension to look up.
    assert_eq!(get_mime_type_from_file(&posix("/x/.")), None);
}

#[test]
fn boundary_shape_over_many_seeds() {
    let mut rng = Rng::from_seed(0xfeed_face);
    for _ in 0..500 {
        let b = generate_mime_multipart_boundary_with(&mut rng);
        assert_eq!(b.len(), 69);
        assert!(b.starts_with("----MultipartBoundary--"));
        assert!(b.ends_with("----"));
        let random_part = &b[23..65];
        assert!(random_part.bytes().all(|c| c.is_ascii_alphanumeric()), "{b}");
    }
}

#[test]
fn parsers_do_not_panic_on_random_input() {
    let tokens = [
        "a", "b", "/", ";", "=", "\"", "\\", " ", "\t", "(", "*", "é", "☃", "text", "\n", "\r",
        "+", ",",
    ];
    let mut rng = Rng::from_seed(0x0bad_cafe);
    for _ in 0..20_000 {
        let n = rng.below(12);
        let s: String = (0..n).map(|_| tokens[rng.below(tokens.len())]).collect();

        if let Some(parsed) = parse_mime_type(&s) {
            assert!(parsed.mime_type.contains('/'), "{s:?}");
        }

        if let Some((top, sub)) = parse_mime_type_without_parameter(&s) {
            assert!(http_util::is_token(top.as_bytes()), "{s:?}");
            assert!(http_util::is_token(sub.as_bytes()), "{s:?}");
        }

        let _ = extract_mime_type_from_media_type(&s, rng.below(2) == 0);
        let _ = is_valid_top_level_mime_type(&s);
        let _ = get_extensions_for_mime_type(&s);
        let _ = get_mime_type_from_extension(&s);
        for level in [
            MimeTypeValidationLevel::None,
            MimeTypeValidationLevel::WildcardSlashOnly,
            MimeTypeValidationLevel::WildcardSlashAndTokens,
        ] {
            let _ = matches_mime_type(&s, "text/plain;charset=utf-8", level);
            let _ = matches_mime_type("text/*;a=b", &s, level);
        }
    }
}

#[test]
fn parse_mime_type_escape_handling() {
    // A trailing backslash inside an open quote is kept.
    let parsed = parse_mime_type("t/s;a=\"x\\").unwrap();
    assert_eq!(parsed.params, pairs(&[("a", "x\\")]));
    // A backslash before a multi-byte character is dropped, and the character
    // is kept whole.
    let parsed = parse_mime_type("t/s;a=\"\\é\"").unwrap();
    assert_eq!(parsed.params, pairs(&[("a", "é")]));
}
