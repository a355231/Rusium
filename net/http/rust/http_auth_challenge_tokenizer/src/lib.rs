// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![forbid(unsafe_code)]

//! Rust port of `net::HttpAuthChallengeTokenizer`
//! (net/http/http_auth_challenge_tokenizer.{h,cc}).
//!
//! Splits an auth challenge into the auth scheme and the parameter list, per
//! RFC 2617 section 1.2: `challenge = auth-scheme 1*SP 1#auth-param`. The
//! parameters can be read as `name=value` pairs with `param_pairs()`, or as a
//! base64 token with `base64_param()`.
//!
//! Differences from the C++ version:
//! - The challenge is borrowed as `&[u8]`, not `std::string_view`, so
//!   non-UTF-8 bytes are allowed as they are in C++.
//! - `auth_scheme()` returns `&[u8]`, not `const std::string&`. It is
//!   ASCII-lowercased, as in C++.
//! - The scheme split uses `http_util::HTTP_LWS` as the delimiter set. This is
//!   the same set `base::StringViewTokenizer` used in C++.
//! - `param_pairs()` returns `http_util::NameValuePairsIterator` with `,` as
//!   the delimiter. This is the same iterator and defaults C++ uses.
//! - `NET_EXPORT_PRIVATE` and the `net` namespace have no equivalent.

use http_util::{trim_lws, NameValuePairsIterator, HTTP_LWS};

/// Splits an auth challenge into its scheme and parameters.
///
/// For `"Basic realm=\"foo\""`, `auth_scheme()` is `b"basic"` and `params()`
/// is `b"realm=\"foo\""`. A challenge with no scheme gives an empty scheme and
/// empty parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAuthChallengeTokenizer<'a> {
    challenge: &'a [u8],
    params: &'a [u8],
    lower_case_scheme: Vec<u8>,
}

impl<'a> HttpAuthChallengeTokenizer<'a> {
    /// Parses `challenge` into its scheme and parameter text.
    pub fn new(challenge: &'a [u8]) -> Self {
        // The first LWS-delimited token is the auth-scheme. This is more
        // permissive than RFC 2617, which requires 1*SP.
        let (lower_case_scheme, params) =
            match challenge.iter().position(|c| !HTTP_LWS.contains(c)) {
                None => (Vec::new(), &challenge[..0]),
                Some(start) => {
                    let end = challenge[start..]
                        .iter()
                        .position(|c| HTTP_LWS.contains(c))
                        .map_or(challenge.len(), |p| start + p);
                    (
                        challenge[start..end].to_ascii_lowercase(),
                        trim_lws(&challenge[end..]),
                    )
                }
            };
        Self {
            challenge,
            params,
            lower_case_scheme,
        }
    }

    /// The original challenge text.
    pub fn challenge_text(&self) -> &'a [u8] {
        self.challenge
    }

    /// The auth scheme, ASCII-lowercased.
    pub fn auth_scheme(&self) -> &[u8] {
        &self.lower_case_scheme
    }

    /// The text after the scheme, with leading and trailing LWS removed.
    pub fn params(&self) -> &'a [u8] {
        self.params
    }

    /// Iterates the parameters as comma-separated `name=value` pairs.
    pub fn param_pairs(&self) -> NameValuePairsIterator<'a> {
        NameValuePairsIterator::new(self.params, b',')
    }

    /// The parameters as a base64 token, with trailing `=` padding removed
    /// when the length is not a multiple of 4. This matches the C++ behavior,
    /// which works around https://bugzilla.mozilla.org/show_bug.cgi?id=230351.
    pub fn base64_param(&self) -> &'a [u8] {
        // Our base64 decoder requires a length that is a multiple of 4.
        let mut encoded_length = self.params.len();
        while encoded_length > 0
            && encoded_length % 4 != 0
            && self.params[encoded_length - 1] == b'='
        {
            encoded_length -= 1;
        }
        &self.params[..encoded_length]
    }
}

#[cfg(test)]
mod tests {
    use super::HttpAuthChallengeTokenizer;

    // Use a name=value property with no quote marks.
    #[test]
    fn basic() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Basic realm=\"foobar\"");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"basic");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"foobar");
        assert!(!parameters.get_next());
    }

    #[test]
    fn no_quotes() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Basic realm=foobar@baz.com");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"basic");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"foobar@baz.com");
        assert!(!parameters.get_next());
    }

    // Use a name=value property with mismatching quote marks.
    #[test]
    fn mismatched_quotes() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Basic realm=\"foobar@baz.com");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"basic");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"foobar@baz.com");
        assert!(!parameters.get_next());
    }

    // Use a name= property without a value and with mismatching quote marks.
    #[test]
    fn mismatched_quotes_no_value() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Basic realm=\"");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"basic");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"");
        assert!(!parameters.get_next());
    }

    // Use a name=value property with mismatching quote marks and spaces in the
    // value.
    #[test]
    fn mismatched_quotes_spaces() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Basic realm=\"foo bar");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"basic");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"foo bar");
        assert!(!parameters.get_next());
    }

    // Use multiple name=value properties with mismatching quote marks in the
    // last value.
    #[test]
    fn mismatched_quotes_multiple() {
        let challenge = HttpAuthChallengeTokenizer::new(
            b"Digest qop=auth-int, algorithm=md5, realm=\"foo",
        );
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"digest");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"qop");
        assert_eq!(parameters.value(), b"auth-int");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"algorithm");
        assert_eq!(parameters.value(), b"md5");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"foo");
        assert!(!parameters.get_next());
    }

    // Use a name= property which has no value.
    #[test]
    fn no_value() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Digest qop=");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"digest");
        assert!(!parameters.get_next());
        assert!(!parameters.valid());
    }

    // Specify multiple properties, comma separated.
    #[test]
    fn multiple() {
        let challenge = HttpAuthChallengeTokenizer::new(
            b"Digest algorithm=md5, realm=\"Oblivion\", qop=auth-int",
        );
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"digest");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"algorithm");
        assert_eq!(parameters.value(), b"md5");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"realm");
        assert_eq!(parameters.value(), b"Oblivion");
        assert!(parameters.get_next());
        assert!(parameters.valid());
        assert_eq!(parameters.name(), b"qop");
        assert_eq!(parameters.value(), b"auth-int");
        assert!(!parameters.get_next());
        assert!(parameters.valid());
    }

    // Use a challenge which has no property.
    #[test]
    fn no_property() {
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM");
        let mut parameters = challenge.param_pairs();

        assert!(parameters.valid());
        assert_eq!(challenge.auth_scheme(), b"ntlm");
        assert!(!parameters.get_next());
    }

    // Use a challenge with Base64 encoded token.
    #[test]
    fn base64() {
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM  SGVsbG8sIFdvcmxkCg===");

        assert_eq!(challenge.auth_scheme(), b"ntlm");
        // The padding removal makes the base64 parameter equal the value
        // below, which is the expected token for the padded input above.
        assert_eq!(challenge.base64_param(), b"SGVsbG8sIFdvcmxkCg==");
    }

    #[test]
    fn empty_challenge() {
        let challenge = HttpAuthChallengeTokenizer::new(b"");
        assert_eq!(challenge.auth_scheme(), b"");
        assert_eq!(challenge.params(), b"");
        assert_eq!(challenge.base64_param(), b"");
        assert!(challenge.challenge_text().is_empty());
        let mut parameters = challenge.param_pairs();
        assert!(parameters.valid());
        assert!(!parameters.get_next());
    }

    #[test]
    fn whitespace_only_challenge() {
        let challenge = HttpAuthChallengeTokenizer::new(b" \t  ");
        assert_eq!(challenge.auth_scheme(), b"");
        assert_eq!(challenge.params(), b"");
    }

    #[test]
    fn leading_whitespace_and_tabs_are_skipped() {
        let challenge = HttpAuthChallengeTokenizer::new(b"\t Digest\t realm=x ");
        assert_eq!(challenge.challenge_text(), b"\t Digest\t realm=x ");
        assert_eq!(challenge.auth_scheme(), b"digest");
        assert_eq!(challenge.params(), b"realm=x");
    }

    #[test]
    fn scheme_is_lowercased_and_params_are_trimmed() {
        let challenge = HttpAuthChallengeTokenizer::new(b"NeGoTiAtE   token  ");
        assert_eq!(challenge.auth_scheme(), b"negotiate");
        assert_eq!(challenge.params(), b"token");
    }

    #[test]
    fn scheme_only_has_no_params() {
        let challenge = HttpAuthChallengeTokenizer::new(b"Negotiate\t");
        assert_eq!(challenge.auth_scheme(), b"negotiate");
        assert_eq!(challenge.params(), b"");
    }

    #[test]
    fn base64_without_padding_is_unchanged() {
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM SGVsbG8sIFdvcmxkCg");
        assert_eq!(challenge.base64_param(), b"SGVsbG8sIFdvcmxkCg");
    }

    #[test]
    fn base64_padding_is_trimmed_to_multiple_of_four() {
        // Length 6 with padding: trims to 4.
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM abc===");
        assert_eq!(challenge.base64_param(), b"abc=");
        // Length 4 is already a multiple of 4, so padding is kept.
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM ab==");
        assert_eq!(challenge.base64_param(), b"ab==");
        // Only padding in the token: trims to nothing.
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM =");
        assert_eq!(challenge.base64_param(), b"");
    }

    #[test]
    fn base64_keeps_inner_equals_signs() {
        let challenge = HttpAuthChallengeTokenizer::new(b"NTLM a=b");
        assert_eq!(challenge.base64_param(), b"a=b");
    }
}
