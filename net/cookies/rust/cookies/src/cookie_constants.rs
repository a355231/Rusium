// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `net/cookies/cookie_constants.{h,cc}`.
//!
//! The enums keep the C++ numeric values, since several are persisted or
//! used as histogram buckets. `CookiePort` and
//! [`reduce_port_range_for_cookie_histogram`] are generated from the C++
//! header and source, so the port table cannot drift from the C++ mapping.
//!
//! Not ported: `GetSchemeNameEnum(const GURL&)`, which needs `GURL`. The
//! `CookieSourceSchemeName` enum itself is ported. Histogram sentinels
//! (`kMaxValue`, `COUNT`) are omitted.

use std::time::Duration;

/// Time threshold for treating a cookie as "short-lived" for allowing unsafe
/// methods on unspecified-SameSite cookies defaulted to Lax.
pub const LAX_ALLOW_UNSAFE_MAX_AGE: Duration = Duration::from_secs(2 * 60);

/// Short version of [`LAX_ALLOW_UNSAFE_MAX_AGE`], for tests.
pub const SHORT_LAX_ALLOW_UNSAFE_MAX_AGE: Duration = Duration::from_secs(10);

/// Probability used to down-sample histogram recording for cookie operations.
pub const HISTOGRAM_SAMPLE_PROBABILITY: f64 = 0.001;

/// Placeholder `partition_key` value for cookies without `Partitioned`. It is
/// the smallest non-null value, so it is the empty string.
pub const EMPTY_COOKIE_PARTITION_KEY: &str = "";

/// `CookiePriority`. The numeric values are used for ordering, so
/// `Low < Medium < High`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CookiePriority {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl CookiePriority {
    /// `COOKIE_PRIORITY_DEFAULT`, which is `Medium`.
    pub const DEFAULT: CookiePriority = CookiePriority::Medium;
}

/// Returns the `Set-Cookie` priority token for `priority`.
pub fn cookie_priority_to_string(priority: CookiePriority) -> &'static str {
    match priority {
        CookiePriority::High => "high",
        CookiePriority::Medium => "medium",
        CookiePriority::Low => "low",
    }
}

/// Converts a `Set-Cookie` priority token (ASCII case-insensitive) to a
/// [`CookiePriority`]. Empty or unrecognized strings give the default.
pub fn string_to_cookie_priority(priority: &str) -> CookiePriority {
    match priority.to_ascii_lowercase().as_str() {
        "high" => CookiePriority::High,
        "medium" => CookiePriority::Medium,
        "low" => CookiePriority::Low,
        _ => CookiePriority::DEFAULT,
    }
}

/// The `SameSite` value of a cookie. Values are persisted to a database, so
/// they must not be renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSameSite {
    Unspecified = -1,
    NoRestriction = 0,
    LaxMode = 1,
    // 3 was EXTENDED_MODE and is reserved.
    StrictMode = 2,
}

/// The same as [`CookieSameSite`] but starting at 0, for standard
/// (non-sparse) histograms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSameSiteForMetrics {
    Unspecified = 0,
    NoRestriction = 1,
    LaxMode = 2,
    StrictMode = 3,
}

/// Enforcement modes applied to a cookie when deciding inclusion/exclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieEffectiveSameSite {
    NoRestriction = 0,
    LaxMode = 1,
    StrictMode = 2,
    LaxModeAllowUnsafe = 3,
    /// No valid cookie object to evaluate on.
    Undefined = 4,
}

/// How a `SameSite` attribute value was parsed, for metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSameSiteString {
    /// No `SameSite` attribute is present.
    Unspecified = 0,
    /// The attribute is present with an empty value.
    EmptyString = 1,
    /// The attribute has an unrecognized value.
    Unrecognized = 2,
    Lax = 3,
    Strict = 4,
    None = 5,
    /// Deprecated, kept for metrics only.
    Extended = 6,
}

/// What `SameSite` rules to apply to a cookie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum CookieAccessSemantics {
    /// Not checked yet, or no way to check.
    Unknown = -1,
    /// Checked, and not subject to legacy access rules.
    NonLegacy = 0,
    /// Checked, and subject to legacy access rules.
    Legacy = 1,
}

/// Whether a cookie is subject to legacy Origin-Bound Cookies scope rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum CookieScopeSemantics {
    Unknown = -1,
    NonLegacy = 0,
    Legacy = 1,
}

/// The scheme that was used when a cookie was set. Do not renumber.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSourceScheme {
    Unset = 0,
    NonSecure = 1,
    Secure = 2,
}

/// Scheme or trustworthiness used to access or set a cookie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieAccessScheme {
    /// Non-cryptographic, and not potentially trustworthy (or unchecked).
    NonCryptographic = 0,
    /// https or wss.
    Cryptographic = 1,
    /// Non-cryptographic source, but the URL is otherwise potentially
    /// trustworthy.
    Trustworthy = 2,
}

/// Scheme names used for the cookie-source-scheme metrics. Do not renumber.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSourceSchemeName {
    /// Catch-all for schemes not listed here.
    Other = 0,
    AboutBlankUrl = 1,
    AboutSrcdocUrl = 2,
    AboutBlankPath = 3,
    AboutSrcdocPath = 4,
    AboutScheme = 5,
    BlobScheme = 6,
    ContentScheme = 7,
    ContentIdScheme = 8,
    DataScheme = 9,
    FileScheme = 10,
    FileSystemScheme = 11,
    FtpScheme = 12,
    HttpScheme = 13,
    HttpsScheme = 14,
    JavaScriptScheme = 15,
    MailToScheme = 16,
    QuicTransportSchemeObsoleted = 17,
    TelScheme = 18,
    UrnScheme = 19,
    WsScheme = 20,
    WssScheme = 21,
    ChromeExtensionScheme = 22,
}

/// How a cookie was set. Used for the `source_type` column. Do not renumber.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookieSourceType {
    /// Set via an HTTP response header.
    Http = 1,
    /// Set via `document.cookie`.
    Script = 2,
    /// Set via browser login, iOS, WebView, extension or DevTools APIs.
    Other = 3,
}

/// The special cookie name prefixes from RFC 6265bis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookiePrefix {
    None = 0,
    Secure = 1,
    Host = 2,
    Http = 3,
    HostHttp = 4,
}

/// How a cookie line was parsed as a cookie with an empty name, for metrics.
/// The buckets are mutually exclusive. Values are persisted to logs, so they
/// must not be renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamelessCookieLineParseType {
    /// A bare token such as `Foo`, which parses to a nameless cookie with
    /// value `Foo`.
    BareToken = 0,
    /// A bare token whose value matches an attribute name, such as `secure`.
    /// These are most likely configuration mistakes.
    BareTokenMatchingAttributeName = 1,
    /// A line such as `=Foo`, where the first non-whitespace character is `=`.
    EqualsPrecedingToken = 2,
    /// A line such as `=Foo=Bar`, parsed as a nameless cookie with value
    /// `Foo=Bar`.
    NamelessWithAmbiguousValue = 3,
}

/// Usage patterns of `CookiesAllowedForUrls`. Values are persisted to logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookiesAllowedForUrlsUsage {
    ExplicitOnly = 0,
    WildcardPrimaryOnly = 1,
    WildcardSecondaryOnly = 2,
    ExplicitAndPrimaryWildcard = 3,
    ExplicitAndSecondaryWildcard = 4,
    WildcardOnly = 5,
    AllPresent = 6,
}

/// Returns the `SameSite` token for `same_site`, for logging. The string is
/// lowercase and `Unspecified` gives `"unspecified"`.
pub fn cookie_same_site_to_string(same_site: CookieSameSite) -> &'static str {
    match same_site {
        CookieSameSite::LaxMode => "lax",
        CookieSameSite::StrictMode => "strict",
        CookieSameSite::NoRestriction => "none",
        CookieSameSite::Unspecified => "unspecified",
    }
}

/// Converts a `SameSite` attribute value (ASCII case-insensitive) to a
/// [`CookieSameSite`]. Returns the matching [`CookieSameSiteString`] to say
/// what kind of value was parsed. `extended` is accepted but treated as
/// `Unspecified`, since that value is no longer supported.
pub fn string_to_cookie_same_site(same_site: &str) -> (CookieSameSite, CookieSameSiteString) {
    if same_site.eq_ignore_ascii_case("none") {
        (CookieSameSite::NoRestriction, CookieSameSiteString::None)
    } else if same_site.eq_ignore_ascii_case("lax") {
        (CookieSameSite::LaxMode, CookieSameSiteString::Lax)
    } else if same_site.eq_ignore_ascii_case("strict") {
        (CookieSameSite::StrictMode, CookieSameSiteString::Strict)
    } else if same_site.eq_ignore_ascii_case("extended") {
        (CookieSameSite::Unspecified, CookieSameSiteString::Extended)
    } else if same_site.is_empty() {
        (CookieSameSite::Unspecified, CookieSameSiteString::EmptyString)
    } else {
        (CookieSameSite::Unspecified, CookieSameSiteString::Unrecognized)
    }
}

/// Potentially interesting ports for cookie histograms. Mirrors `net::CookiePort`.
/// The values are used for histograms, so they must not be renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CookiePort {
    /// Any port not listed here, including invalid ports.
    Other = 0,
    /// Port 80.
    K80 = 1,
    /// Port 81.
    K81 = 2,
    /// Port 82.
    K82 = 3,
    /// Port 83.
    K83 = 4,
    /// Port 84.
    K84 = 5,
    /// Port 85.
    K85 = 6,
    /// Port 443.
    K443 = 7,
    /// Port 444.
    K444 = 8,
    /// Port 445.
    K445 = 9,
    /// Port 446.
    K446 = 10,
    /// Port 447.
    K447 = 11,
    /// Port 448.
    K448 = 12,
    /// Port 3000.
    K3000 = 13,
    /// Port 3001.
    K3001 = 14,
    /// Port 3002.
    K3002 = 15,
    /// Port 3003.
    K3003 = 16,
    /// Port 3004.
    K3004 = 17,
    /// Port 3005.
    K3005 = 18,
    /// Port 4200.
    K4200 = 19,
    /// Port 4201.
    K4201 = 20,
    /// Port 4202.
    K4202 = 21,
    /// Port 4203.
    K4203 = 22,
    /// Port 4204.
    K4204 = 23,
    /// Port 4205.
    K4205 = 24,
    /// Port 5000.
    K5000 = 25,
    /// Port 5001.
    K5001 = 26,
    /// Port 5002.
    K5002 = 27,
    /// Port 5003.
    K5003 = 28,
    /// Port 5004.
    K5004 = 29,
    /// Port 5005.
    K5005 = 30,
    /// Port 7000.
    K7000 = 31,
    /// Port 7001.
    K7001 = 32,
    /// Port 7002.
    K7002 = 33,
    /// Port 7003.
    K7003 = 34,
    /// Port 7004.
    K7004 = 35,
    /// Port 7005.
    K7005 = 36,
    /// Port 8000.
    K8000 = 37,
    /// Port 8001.
    K8001 = 38,
    /// Port 8002.
    K8002 = 39,
    /// Port 8003.
    K8003 = 40,
    /// Port 8004.
    K8004 = 41,
    /// Port 8005.
    K8005 = 42,
    /// Port 8080.
    K8080 = 43,
    /// Port 8081.
    K8081 = 44,
    /// Port 8082.
    K8082 = 45,
    /// Port 8083.
    K8083 = 46,
    /// Port 8084.
    K8084 = 47,
    /// Port 8085.
    K8085 = 48,
    /// Port 8090.
    K8090 = 49,
    /// Port 8091.
    K8091 = 50,
    /// Port 8092.
    K8092 = 51,
    /// Port 8093.
    K8093 = 52,
    /// Port 8094.
    K8094 = 53,
    /// Port 8095.
    K8095 = 54,
    /// Port 8100.
    K8100 = 55,
    /// Port 8101.
    K8101 = 56,
    /// Port 8102.
    K8102 = 57,
    /// Port 8103.
    K8103 = 58,
    /// Port 8104.
    K8104 = 59,
    /// Port 8105.
    K8105 = 60,
    /// Port 8200.
    K8200 = 61,
    /// Port 8201.
    K8201 = 62,
    /// Port 8202.
    K8202 = 63,
    /// Port 8203.
    K8203 = 64,
    /// Port 8204.
    K8204 = 65,
    /// Port 8205.
    K8205 = 66,
    /// Port 8443.
    K8443 = 67,
    /// Port 8444.
    K8444 = 68,
    /// Port 8445.
    K8445 = 69,
    /// Port 8446.
    K8446 = 70,
    /// Port 8447.
    K8447 = 71,
    /// Port 8448.
    K8448 = 72,
    /// Port 8888.
    K8888 = 73,
    /// Port 8889.
    K8889 = 74,
    /// Port 8890.
    K8890 = 75,
    /// Port 8891.
    K8891 = 76,
    /// Port 8892.
    K8892 = 77,
    /// Port 8893.
    K8893 = 78,
    /// Port 9000.
    K9000 = 79,
    /// Port 9001.
    K9001 = 80,
    /// Port 9002.
    K9002 = 81,
    /// Port 9003.
    K9003 = 82,
    /// Port 9004.
    K9004 = 83,
    /// Port 9005.
    K9005 = 84,
    /// Port 9090.
    K9090 = 85,
    /// Port 9091.
    K9091 = 86,
    /// Port 9092.
    K9092 = 87,
    /// Port 9093.
    K9093 = 88,
    /// Port 9094.
    K9094 = 89,
    /// Port 9095.
    K9095 = 90,
}

/// Reduces a TCP port to one of the histogram buckets in [`CookiePort`], so
/// that cookie port metrics stay within the range UMA can handle.
pub fn reduce_port_range_for_cookie_histogram(port: i32) -> CookiePort {
    match port {
        80 => CookiePort::K80,
        81 => CookiePort::K81,
        82 => CookiePort::K82,
        83 => CookiePort::K83,
        84 => CookiePort::K84,
        85 => CookiePort::K85,
        443 => CookiePort::K443,
        444 => CookiePort::K444,
        445 => CookiePort::K445,
        446 => CookiePort::K446,
        447 => CookiePort::K447,
        448 => CookiePort::K448,
        3000 => CookiePort::K3000,
        3001 => CookiePort::K3001,
        3002 => CookiePort::K3002,
        3003 => CookiePort::K3003,
        3004 => CookiePort::K3004,
        3005 => CookiePort::K3005,
        4200 => CookiePort::K4200,
        4201 => CookiePort::K4201,
        4202 => CookiePort::K4202,
        4203 => CookiePort::K4203,
        4204 => CookiePort::K4204,
        4205 => CookiePort::K4205,
        5000 => CookiePort::K5000,
        5001 => CookiePort::K5001,
        5002 => CookiePort::K5002,
        5003 => CookiePort::K5003,
        5004 => CookiePort::K5004,
        5005 => CookiePort::K5005,
        7000 => CookiePort::K7000,
        7001 => CookiePort::K7001,
        7002 => CookiePort::K7002,
        7003 => CookiePort::K7003,
        7004 => CookiePort::K7004,
        7005 => CookiePort::K7005,
        8000 => CookiePort::K8000,
        8001 => CookiePort::K8001,
        8002 => CookiePort::K8002,
        8003 => CookiePort::K8003,
        8004 => CookiePort::K8004,
        8005 => CookiePort::K8005,
        8080 => CookiePort::K8080,
        8081 => CookiePort::K8081,
        8082 => CookiePort::K8082,
        8083 => CookiePort::K8083,
        8084 => CookiePort::K8084,
        8085 => CookiePort::K8085,
        8090 => CookiePort::K8090,
        8091 => CookiePort::K8091,
        8092 => CookiePort::K8092,
        8093 => CookiePort::K8093,
        8094 => CookiePort::K8094,
        8095 => CookiePort::K8095,
        8100 => CookiePort::K8100,
        8101 => CookiePort::K8101,
        8102 => CookiePort::K8102,
        8103 => CookiePort::K8103,
        8104 => CookiePort::K8104,
        8105 => CookiePort::K8105,
        8200 => CookiePort::K8200,
        8201 => CookiePort::K8201,
        8202 => CookiePort::K8202,
        8203 => CookiePort::K8203,
        8204 => CookiePort::K8204,
        8205 => CookiePort::K8205,
        8443 => CookiePort::K8443,
        8444 => CookiePort::K8444,
        8445 => CookiePort::K8445,
        8446 => CookiePort::K8446,
        8447 => CookiePort::K8447,
        8448 => CookiePort::K8448,
        8888 => CookiePort::K8888,
        8889 => CookiePort::K8889,
        8890 => CookiePort::K8890,
        8891 => CookiePort::K8891,
        8892 => CookiePort::K8892,
        8893 => CookiePort::K8893,
        9000 => CookiePort::K9000,
        9001 => CookiePort::K9001,
        9002 => CookiePort::K9002,
        9003 => CookiePort::K9003,
        9004 => CookiePort::K9004,
        9005 => CookiePort::K9005,
        9090 => CookiePort::K9090,
        9091 => CookiePort::K9091,
        9092 => CookiePort::K9092,
        9093 => CookiePort::K9093,
        9094 => CookiePort::K9094,
        9095 => CookiePort::K9095,
        _ => CookiePort::Other,
    }
}
