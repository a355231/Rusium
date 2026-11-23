// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `uri_template::Expand`
//! (net/third_party/uri_template/uri_template.{h,cc}).
//!
//! Expands RFC 6570 URI templates up to level 3, with the same limitations as
//! the C++ code: variable names are not checked for legal characters, and
//! literal template text is copied without any encoding.
//!
//! Differences from the C++ version:
//! - Inputs and outputs are bytes (`&[u8]` and `Vec<u8>`), not `std::string`.
//!   The C++ tests use values that are not valid UTF-8 (`\x80\xFF`), so a
//!   `&str` API could not express them.
//! - `Expand` takes the template and returns the expansion. The C++ appends to
//!   `*target`, and clears all of `*target` on failure. The Rust version always
//!   starts from an empty output, so it never appends to existing contents.
//! - The C++ returns `bool` and writes to an out-parameter. The Rust version
//!   returns `Result<Vec<u8>, MalformedTemplate>`.
//! - `vars_found` is `Option<&mut BTreeSet<Vec<u8>>>`. `BTreeSet` gives the same
//!   byte-wise ordering as `std::set<std::string>`. As in the C++ code, names
//!   are added as they are expanded, so a malformed template can still leave
//!   names in the set. For example `{x}{` adds `x` and then fails.
//! - Parameters are a `HashMap<Vec<u8>, Vec<u8>, S>`. Any hasher works. The
//!   C++ uses `absl::flat_hash_map`.
//! - The escaping helpers come from the sibling `escape` crate
//!   (`escape_all_except_unreserved` and `escape_external_handler_value`),
//!   which is the port of `base::EscapeAllExceptUnreserved` and
//!   `base::EscapeExternalHandlerValue`.
//! - `base::SplitStringPiece` is replaced by a byte split on `,` that drops
//!   empty pieces. The C++ call uses `KEEP_WHITESPACE`, so names are not
//!   trimmed.
//! - The `COMPONENT_EXPORT` macro and the `fuzzer` are not ported. The fuzzer
//!   is a libFuzzer entry point that only calls `Expand`.
//! - `ValidateUriTemplate` and `ParseUriTemplate` are not in this C++ tree's
//!   header, so there is nothing to port for them.
//!
//! Testing: `src/lib.rs` ports every `TEST_F` from `uri_template_test.cc`. It
//! also adds randomized property tests driven by a small xorshift PRNG.
//! `tests/cpp_golden.rs` checks 498 templates against the
//! unmodified C++ `Expand`, built with stand-in headers (see that file).

#![forbid(unsafe_code)]

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::hash::BuildHasher;

use escape::{escape_all_except_unreserved, escape_external_handler_value};

/// Returned by [`expand`] when the template has an unmatched or nested brace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedTemplate;

impl fmt::Display for MalformedTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed URI template")
    }
}

impl std::error::Error for MalformedTemplate {}

/// Expands a URI template with the given variable values.
///
/// Returns the expansion, or [`MalformedTemplate`] if the template is
/// malformed. Names of variables that were expanded are added to `vars_found`
/// when it is `Some`. The set can contain names even when the result is an
/// error (see the crate documentation).
pub fn expand<S: BuildHasher>(
    template: &[u8],
    parameters: &HashMap<Vec<u8>, Vec<u8>, S>,
    mut vars_found: Option<&mut BTreeSet<Vec<u8>>>,
) -> Result<Vec<u8>, MalformedTemplate> {
    let mut target = Vec::with_capacity(template.len());
    let mut rest = template;
    while !rest.is_empty() {
        let open = rest.iter().position(|&b| b == b'{');
        let close = rest.iter().position(|&b| b == b'}');
        let Some(open) = open else {
            if close.is_some() {
                // Unexpected closing brace.
                return Err(MalformedTemplate);
            }
            // No more variables to process.
            target.extend_from_slice(rest);
            return Ok(target);
        };
        // No closing brace, or the closing brace comes before the opening one.
        let close = match close {
            Some(close) if close > open => close,
            _ => return Err(MalformedTemplate),
        };
        target.extend_from_slice(&rest[..open]);
        let section = &rest[open + 1..close];
        if section.contains(&b'{') {
            return Err(MalformedTemplate);
        }
        rest = &rest[close + 1..];
        process_variable_section(section, parameters, &mut target, vars_found.as_deref_mut());
    }
    Ok(target)
}

/// The operator of a template expression. Mirrors the C++
/// `UriTemplateConfig`.
#[derive(Clone, Copy, Debug)]
struct Operator {
    prefix: &'static [u8],
    joiner: &'static [u8],
    requires_variable_assignment: bool,
    allow_reserved_expansion: bool,
    no_variable_assignment_if_empty: bool,
}

const SIMPLE: Operator = Operator {
    prefix: b"",
    joiner: b",",
    requires_variable_assignment: false,
    allow_reserved_expansion: false,
    no_variable_assignment_if_empty: false,
};

impl Operator {
    fn escape(&self, value: &[u8]) -> String {
        if self.allow_reserved_expansion {
            // Reserved expansion passes through reserved and pct-encoded
            // characters.
            escape_external_handler_value(value)
        } else {
            escape_all_except_unreserved(value)
        }
    }

    fn append_value(&self, variable: &[u8], value: &[u8], use_prefix: bool, target: &mut Vec<u8>) {
        let joiner = if use_prefix { self.prefix } else { self.joiner };
        target.extend_from_slice(joiner);
        if self.requires_variable_assignment {
            target.extend_from_slice(self.escape(variable).as_bytes());
            if value.is_empty() && self.no_variable_assignment_if_empty {
                return;
            }
            target.push(b'=');
        }
        target.extend_from_slice(self.escape(value).as_bytes());
    }
}

/// Splits the operator character off the front of a variable section. Returns
/// the operator and the comma-separated variable names. Mirrors the C++
/// `MakeConfig`.
fn parse_operator(section: &[u8]) -> (Operator, &[u8]) {
    let Some((&first, names)) = section.split_first() else {
        return (SIMPLE, section);
    };
    let op = match first {
        // Reserved expansion.
        b'+' => Operator {
            prefix: b"",
            joiner: b",",
            requires_variable_assignment: false,
            allow_reserved_expansion: true,
            no_variable_assignment_if_empty: false,
        },
        // Fragment expansion.
        b'#' => Operator {
            prefix: b"#",
            joiner: b",",
            requires_variable_assignment: false,
            allow_reserved_expansion: true,
            no_variable_assignment_if_empty: false,
        },
        // Label with dot-prefix.
        b'.' => Operator {
            prefix: b".",
            joiner: b".",
            requires_variable_assignment: false,
            allow_reserved_expansion: false,
            no_variable_assignment_if_empty: false,
        },
        // Path segment expansion.
        b'/' => Operator {
            prefix: b"/",
            joiner: b"/",
            requires_variable_assignment: false,
            allow_reserved_expansion: false,
            no_variable_assignment_if_empty: false,
        },
        // Path segment parameter expansion.
        b';' => Operator {
            prefix: b";",
            joiner: b";",
            requires_variable_assignment: true,
            allow_reserved_expansion: false,
            no_variable_assignment_if_empty: true,
        },
        // Form-style query expansion.
        b'?' => Operator {
            prefix: b"?",
            joiner: b"&",
            requires_variable_assignment: true,
            allow_reserved_expansion: false,
            no_variable_assignment_if_empty: false,
        },
        // Form-style query continuation.
        b'&' => Operator {
            prefix: b"&",
            joiner: b"&",
            requires_variable_assignment: true,
            allow_reserved_expansion: false,
            no_variable_assignment_if_empty: false,
        },
        // Simple expansion. The operator character is not removed.
        _ => return (SIMPLE, section),
    };
    (op, names)
}

fn process_variable_section<S: BuildHasher>(
    section: &[u8],
    parameters: &HashMap<Vec<u8>, Vec<u8>, S>,
    target: &mut Vec<u8>,
    mut vars_found: Option<&mut BTreeSet<Vec<u8>>>,
) {
    let (op, names) = parse_operator(section);
    let mut first_var = true;
    for name in names.split(|&b| b == b',').filter(|name| !name.is_empty()) {
        if let Some(value) = parameters.get(name) {
            op.append_value(name, value, first_var, target);
            first_var = false;
            if let Some(set) = vars_found.as_deref_mut() {
                set.insert(name.to_vec());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parameter map from `uri_template_test.cc`.
    fn parameters() -> HashMap<Vec<u8>, Vec<u8>> {
        let pairs: &[(&[u8], &[u8])] = &[
            (b"var", b"value"),
            (b"hello", b"Hello World!"),
            (b"path", b"/foo/bar"),
            (b"empty", b""),
            (b"x", b"1024"),
            (b"y", b"768"),
            (b"percent", b"%31"),
            (b"bad_percent", b"%1"),
            (b"escaped", b" !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~\x80\xFF"),
        ];
        pairs.iter().map(|(k, v)| (k.to_vec(), v.to_vec())).collect()
    }

    fn vars(names: &[&str]) -> BTreeSet<Vec<u8>> {
        names.iter().map(|n| n.as_bytes().to_vec()).collect()
    }

    /// Mirrors `CheckExpansion` in uri_template_test.cc.
    fn check_expansion(
        template: &str,
        expected_expansion: &str,
        expected_validity: bool,
        expected_vars: Option<&BTreeSet<Vec<u8>>>,
    ) {
        let mut found = BTreeSet::new();
        let result = expand(template.as_bytes(), &parameters(), Some(&mut found));
        assert_eq!(expected_validity, result.is_ok(), "validity of {template:?}");
        let output = result.unwrap_or_default();
        assert_eq!(expected_expansion.as_bytes(), output.as_slice(), "expansion of {template:?}");
        if let Some(expected_vars) = expected_vars {
            assert_eq!(expected_vars, &found, "vars of {template:?}");
        }
    }

    // TEST_F(UriTemplateTest, TestLevel1Templates)
    #[test]
    fn level1_templates() {
        check_expansion("", "", true, None);
        check_expansion("{var}", "value", true, None);
        check_expansion("{hello}", "Hello%20World%21", true, None);
        check_expansion("{percent}", "%2531", true, None);
        check_expansion(
            "{escaped}",
            "%20%21%22%23%24%25%26%27%28%29%2A%2B%2C-.%2F%3A%3B%3C%3D%3E%\
             3F%40%5B%5C%5D%5E_%60%7B%7C%7D~%80%FF",
            true,
            None,
        );
    }

    // TEST_F(UriTemplateTest, TestLevel2Templates)
    #[test]
    fn level2_templates() {
        // Reserved string expansion
        check_expansion("{+var}", "value", true, None);
        check_expansion("{+hello}", "Hello%20World!", true, None);
        check_expansion("{+percent}", "%31", true, None);
        check_expansion("{+bad_percent}", "%251", true, None);
        check_expansion(
            "{+escaped}",
            "%20!%22#$%25&'()*+,-./:;%3C=%3E?@[%5C]%5E_%60%7B%7C%7D~%80%FF",
            true,
            None,
        );
        check_expansion("{+path}/here", "/foo/bar/here", true, None);
        check_expansion("here?ref={+path}", "here?ref=/foo/bar", true, None);
        // Fragment expansion
        check_expansion("X{#var}", "X#value", true, None);
        check_expansion("X{#hello}", "X#Hello%20World!", true, None);
    }

    // TEST_F(UriTemplateTest, TestLevel3Templates)
    #[test]
    fn level3_templates() {
        // String expansion with multiple variables
        check_expansion("map?{x,y}", "map?1024,768", true, None);
        check_expansion("{x,hello,y}", "1024,Hello%20World%21,768", true, None);
        // Reserved expansion with multiple variables
        check_expansion("{+x,hello,y}", "1024,Hello%20World!,768", true, None);
        check_expansion("{+path,x}/here", "/foo/bar,1024/here", true, None);
        // Fragment expansion with multiple variables
        check_expansion("{#x,hello,y}", "#1024,Hello%20World!,768", true, None);
        check_expansion("{#path,x}/here", "#/foo/bar,1024/here", true, None);
        // Label expansion, dot-prefixed
        check_expansion("X{.var}", "X.value", true, None);
        check_expansion("X{.x,y}", "X.1024.768", true, None);
        // Path segments, slash-prefixed
        check_expansion("{/var}", "/value", true, None);
        check_expansion("{/var,x}/here", "/value/1024/here", true, None);
        // Path-style parameters, semicolon-prefixed
        check_expansion("{;x,y}", ";x=1024;y=768", true, None);
        check_expansion("{;x,y,empty}", ";x=1024;y=768;empty", true, None);
        // Form-style query, ampersand-separated
        check_expansion("{?x,y}", "?x=1024&y=768", true, None);
        check_expansion("{?x,y,empty}", "?x=1024&y=768&empty=", true, None);
        // Form-style query continuation
        check_expansion("?fixed=yes{&x}", "?fixed=yes&x=1024", true, None);
        check_expansion("{&x,y,empty}", "&x=1024&y=768&empty=", true, None);
    }

    // TEST_F(UriTemplateTest, TestMalformed)
    #[test]
    fn malformed() {
        check_expansion("{", "", false, None);
        check_expansion("}{", "", false, None);
        check_expansion("map?{x", "", false, None);
        check_expansion("map?{x,{y}", "", false, None);
        check_expansion("map?{x,y}}", "", false, None);
        check_expansion("map?{{x,y}}", "", false, None);
    }

    // TEST_F(UriTemplateTest, TestVariableSet)
    #[test]
    fn variable_set() {
        let mut expected_vars = vars(&[]);
        check_expansion("map?{z}", "map?", true, Some(&expected_vars));
        check_expansion("map{?z}", "map", true, Some(&expected_vars));
        expected_vars = vars(&["empty"]);
        check_expansion("{empty}", "", true, Some(&expected_vars));
        expected_vars = vars(&["x", "y"]);
        check_expansion("map?{x,y}", "map?1024,768", true, Some(&expected_vars));
        check_expansion("map?{x,z,y}", "map?1024,768", true, Some(&expected_vars));
        check_expansion("map{?x,z,y}", "map?x=1024&y=768", true, Some(&expected_vars));
        expected_vars = vars(&["y", "path"]);
        check_expansion("{+path}{/z}{?y}&k=24", "/foo/bar?y=768&k=24", true, Some(&expected_vars));
        check_expansion("map?{}", "map?", true, None);
        check_expansion("{y}{+path}", "768/foo/bar", true, Some(&expected_vars));
    }

    // TEST_F(UriTemplateTest, TestNullVarsFound)
    #[test]
    fn null_vars_found() {
        let result = expand(b"{var}", &parameters(), None);
        assert_eq!(result.as_deref(), Ok(&b"value"[..]));
    }

    // Not in the C++ tests. Documents the behavior of `vars_found` on failure,
    // which the C++ code also shows (`{x}{` adds `x` before it fails).
    #[test]
    fn malformed_template_keeps_names_expanded_before_failure() {
        let mut found = BTreeSet::new();
        let result = expand(b"{x}{", &parameters(), Some(&mut found));
        assert_eq!(result, Err(MalformedTemplate));
        assert_eq!(found, vars(&["x"]));
    }

    // Not in the C++ tests. Names are not trimmed, and empty names between
    // commas are skipped.
    #[test]
    fn names_are_not_trimmed_and_empty_names_are_skipped() {
        let mut found = BTreeSet::new();
        let result = expand(b"{x, y}", &parameters(), Some(&mut found));
        assert_eq!(result.as_deref(), Ok(&b"1024"[..]));
        assert_eq!(found, vars(&["x"]));

        check_expansion("{x,,y}", "1024,768", true, None);
        check_expansion("{ x}", "", true, None);
    }

    // Not in the C++ tests. Operator characters are removed only from the
    // front, and only the first found variable gets the prefix.
    #[test]
    fn operator_only_at_front_and_prefix_on_first_found() {
        check_expansion("{?z,x,y}", "?x=1024&y=768", true, None);
        check_expansion("{#z,path}", "#/foo/bar", true, None);
        // "x+y" is one name, and it is not in the map.
        check_expansion("{x+y}", "", true, None);
        check_expansion("{+}", "", true, None);
        check_expansion("{}", "", true, None);
    }

    // Not in the C++ tests. Output is byte-exact for non-UTF-8 values.
    #[test]
    fn non_utf8_values_are_escaped_byte_wise() {
        let result = expand(b"{escaped}", &parameters(), None).unwrap();
        assert!(result.ends_with(b"%80%FF"));
        let mut params = HashMap::new();
        params.insert(b"v".to_vec(), vec![0xC3, 0xA9]);
        let result = expand(b"{v}", &params, None).unwrap();
        assert_eq!(result, b"%C3%A9");
    }

    /// xorshift64, a small deterministic PRNG for the randomized tests.
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
    }

    // Randomized: templates with no braces are copied unchanged.
    #[test]
    fn random_literal_templates_are_copied() {
        let alphabet = b"abcXYZ019 -._~%/?#&=+,;:@!$'()*[]\\\"`^|<>\x80\xFF";
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..2000 {
            let len = rng.below(40);
            let template: Vec<u8> = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
            let result = expand(&template, &parameters(), None);
            assert_eq!(result.as_deref(), Ok(template.as_slice()));
        }
    }

    // Randomized: arbitrary token soup never panics, and every name reported
    // in `vars_found` is one that appears in the template between braces.
    #[test]
    fn random_token_soup_never_panics_and_reports_real_names() {
        const TOKENS: &[&[u8]] = &[
            b"{",
            b"}",
            b"{var}",
            b"{x,y}",
            b"{+path}",
            b"{#hello}",
            b"{.x}",
            b"{/y}",
            b"{;empty}",
            b"{?x,empty}",
            b"{&y}",
            b"{z}",
            b"{ x}",
            b"{}",
            b"{{",
            b"}}",
            b"a",
            b"%",
            b" ",
            b",",
            b"+",
            b"\xC3\xA9",
            b"\x80",
            b"{x,",
            b"x}",
        ];
        let params = parameters();
        let mut rng = XorShift(0x0123_4567_89AB_CDEF);
        for _ in 0..3000 {
            let count = 1 + rng.below(8);
            let mut template = Vec::new();
            for _ in 0..count {
                template.extend_from_slice(TOKENS[rng.below(TOKENS.len())]);
            }
            let mut found = BTreeSet::new();
            let _ = expand(&template, &params, Some(&mut found));
            for name in &found {
                assert!(params.contains_key(name), "unknown name {name:?}");
                assert!(
                    template.windows(name.len()).any(|w| w == name.as_slice()),
                    "name {name:?} not in template {template:?}"
                );
            }
        }
    }
}
