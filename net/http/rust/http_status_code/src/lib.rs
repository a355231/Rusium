// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `net::HttpStatusCode`, `net::GetHttpReasonPhrase` and
//! `net::TryToGetHttpStatusCode` (net/http/http_status_code.{h,cc} and
//! net/http/http_status_code_list.h).
//!
//! The C++ X-macro list is turned into one `macro_rules!` table. From that one
//! table this generates the enum, the reason-phrase lookup and the integer
//! lookup, so they cannot disagree. The codes and reason phrases are copied
//! from the IANA-based list, in the same order.
//!
//! Differences from the C++ version:
//! - Enum variants use CamelCase (`HTTP_OK` becomes `HttpStatusCode::Ok`). The
//!   numeric values are unchanged, and `code as i32` gives the status code.
//! - C++ can `static_cast` any `int` to `HttpStatusCode`. Rust enums cannot
//!   hold non-enumerator values, so the lookups that accept raw integers are
//!   `get_http_reason_phrase_for_code` and `get_http_reason_phrase_for_code_or`.
//!   The C++ overload for `int` and the `static_cast` path are both covered by
//!   those.
//! - C++ default arguments become separate functions. The C++
//!   `default_value` argument of the `HttpStatusCode` overload is dropped
//!   because every enumerator has a reason phrase, so it could never be used.
//! - `TryToGetHttpStatusCode` returns `Option<HttpStatusCode>`. The C++ return
//!   type is `const std::optional<HttpStatusCode>`, and `const` is dropped.
//! - `NET_EXPORT` has no meaning here and is not ported.
//! - `HTTP_STATUS_CODE_MAX` (600) is kept, as in the C++ list. It is a
//!   histogram bound and not a real status code.
//! - Nothing else is left unported.

#![forbid(unsafe_code)]

/// Reason phrase used for unrecognized codes when no default is given.
pub const UNKNOWN_STATUS_CODE_REASON: &str = "Unknown Status Code";

macro_rules! http_status_codes {
    ($( $variant:ident = $code:literal, $reason:literal; )*) => {
        /// HTTP status codes, from the IANA HTTP Status Code Registry.
        ///
        /// These values are persisted to logs in the C++ code. Entries should
        /// not be renumbered and numeric values should never be reused.
        #[repr(i32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum HttpStatusCode {
            $( #[doc = $reason] $variant = $code, )*
        }

        /// Returns the reason phrase for `code`, as used in the Reason-Phrase
        /// field of an HTTP response.
        pub fn get_http_reason_phrase(code: HttpStatusCode) -> &'static str {
            match code {
                $( HttpStatusCode::$variant => $reason, )*
            }
        }

        /// Returns the enum value for a numeric status code, or `None` if the
        /// code is not in the list.
        pub fn try_to_get_http_status_code(code: i32) -> Option<HttpStatusCode> {
            match code {
                $( $code => Some(HttpStatusCode::$variant), )*
                _ => None,
            }
        }
    };
}

http_status_codes! {
    Continue = 100, "Continue";
    SwitchingProtocols = 101, "Switching Protocols";
    Processing = 102, "Processing";
    EarlyHints = 103, "Early Hints";
    Ok = 200, "OK";
    Created = 201, "Created";
    Accepted = 202, "Accepted";
    NonAuthoritativeInformation = 203, "Non-Authoritative Information";
    NoContent = 204, "No Content";
    ResetContent = 205, "Reset Content";
    PartialContent = 206, "Partial Content";
    MultiStatus = 207, "Multi-Status";
    AlreadyReported = 208, "Already Reported";
    ImUsed = 226, "IM Used";
    MultipleChoices = 300, "Multiple Choices";
    MovedPermanently = 301, "Moved Permanently";
    Found = 302, "Found";
    SeeOther = 303, "See Other";
    NotModified = 304, "Not Modified";
    UseProxy = 305, "Use Proxy";
    TemporaryRedirect = 307, "Temporary Redirect";
    PermanentRedirect = 308, "Permanent Redirect";
    BadRequest = 400, "Bad Request";
    Unauthorized = 401, "Unauthorized";
    PaymentRequired = 402, "Payment Required";
    Forbidden = 403, "Forbidden";
    NotFound = 404, "Not Found";
    MethodNotAllowed = 405, "Method Not Allowed";
    NotAcceptable = 406, "Not Acceptable";
    ProxyAuthenticationRequired = 407, "Proxy Authentication Required";
    RequestTimeout = 408, "Request Timeout";
    Conflict = 409, "Conflict";
    Gone = 410, "Gone";
    LengthRequired = 411, "Length Required";
    PreconditionFailed = 412, "Precondition Failed";
    RequestEntityTooLarge = 413, "Request Entity Too Large";
    RequestUriTooLong = 414, "Request-URI Too Long";
    UnsupportedMediaType = 415, "Unsupported Media Type";
    RequestedRangeNotSatisfiable = 416, "Requested Range Not Satisfiable";
    ExpectationFailed = 417, "Expectation Failed";
    InvalidXprivetToken = 418, "Invalid XPrivet Token";
    MisdirectedRequest = 421, "Misdirected Request";
    UnprocessableContent = 422, "Unprocessable Content";
    Locked = 423, "Locked";
    FailedDependency = 424, "Failed Dependency";
    TooEarly = 425, "Too Early";
    UpgradeRequired = 426, "Upgrade Required";
    PreconditionRequired = 428, "Precondition Required";
    TooManyRequests = 429, "Too Many Requests";
    RequestHeaderFieldsTooLarge = 431, "Request Header Fields Too Large";
    UnavailableForLegalReasons = 451, "Unavailable For Legal Reasons";
    InternalServerError = 500, "Internal Server Error";
    NotImplemented = 501, "Not Implemented";
    BadGateway = 502, "Bad Gateway";
    ServiceUnavailable = 503, "Service Unavailable";
    GatewayTimeout = 504, "Gateway Timeout";
    VersionNotSupported = 505, "HTTP Version Not Supported";
    VariantAlsoNegotiates = 506, "Variant Also Negotiates";
    InsufficientStorage = 507, "Insufficient Storage";
    LoopDetected = 508, "Loop Detected";
    NotExtendedObsoleted = 510, "Not Extended (Obsoleted)";
    NetworkAuthenticationRequired = 511, "Network Authentication Required";
    StatusCodeMax = 600, "HTTP Status Code Max";
}

/// Returns the reason phrase for a numeric status code, or
/// `default_value` if the code is not recognized. The C++ `int` overload.
pub fn get_http_reason_phrase_for_code_or(code: i32, default_value: &str) -> &str {
    match try_to_get_http_status_code(code) {
        Some(status) => get_http_reason_phrase(status),
        None => default_value,
    }
}

/// Like [`get_http_reason_phrase_for_code_or`], with
/// [`UNKNOWN_STATUS_CODE_REASON`] as the default.
pub fn get_http_reason_phrase_for_code(code: i32) -> &'static str {
    get_http_reason_phrase_for_code_or(code, UNKNOWN_STATUS_CODE_REASON)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Number of entries in net/http/http_status_code_list.h.
    const LIST_LENGTH: usize = 63;

    #[test]
    fn get_http_reason_phrase_cpp_test() {
        assert_eq!("OK", get_http_reason_phrase(HttpStatusCode::Ok));
        assert_eq!("OK", get_http_reason_phrase_for_code(200));
        assert_eq!("OK", get_http_reason_phrase_for_code_or(200, "Overridden Default"));

        assert_eq!("Not Found", get_http_reason_phrase(HttpStatusCode::NotFound));
        assert_eq!("Not Found", get_http_reason_phrase_for_code(404));
        assert_eq!("Not Found", get_http_reason_phrase_for_code_or(404, "Overridden Default"));

        // The C++ test casts 599, 1, 12345 and -1 to HttpStatusCode. Those
        // values are not enumerators, so they go through the integer lookup.
        assert_eq!("Unknown Status Code", get_http_reason_phrase_for_code(599));
        assert_eq!(
            "Overridden Default",
            get_http_reason_phrase_for_code_or(599, "Overridden Default")
        );
        assert_eq!("Unknown Status Code", get_http_reason_phrase_for_code(1));
        assert_eq!("Unknown Status Code", get_http_reason_phrase_for_code(12345));
        assert_eq!("Unknown Status Code", get_http_reason_phrase_for_code(-1));
    }

    #[test]
    fn enum_values_match_status_codes() {
        assert_eq!(200, HttpStatusCode::Ok as i32);
        assert_eq!(404, HttpStatusCode::NotFound as i32);
        assert_eq!(600, HttpStatusCode::StatusCodeMax as i32);
    }

    #[test]
    fn try_to_get_http_status_code_known_codes() {
        assert_eq!(Some(HttpStatusCode::Continue), try_to_get_http_status_code(100));
        assert_eq!(Some(HttpStatusCode::Ok), try_to_get_http_status_code(200));
        assert_eq!(Some(HttpStatusCode::InvalidXprivetToken), try_to_get_http_status_code(418));
        assert_eq!(Some(HttpStatusCode::StatusCodeMax), try_to_get_http_status_code(600));
    }

    #[test]
    fn try_to_get_http_status_code_unknown_codes() {
        // 306 is no longer used, and the list has no entry for it.
        assert_eq!(None, try_to_get_http_status_code(306));
        assert_eq!(None, try_to_get_http_status_code(599));
        assert_eq!(None, try_to_get_http_status_code(1));
        assert_eq!(None, try_to_get_http_status_code(12345));
        assert_eq!(None, try_to_get_http_status_code(-1));
        assert_eq!(None, try_to_get_http_status_code(i32::MIN));
        assert_eq!(None, try_to_get_http_status_code(i32::MAX));
    }

    #[test]
    fn exhaustive_lookup_over_small_range() {
        // The domain is small, so check every value instead of sampling.
        let mut found = 0;
        for code in -1000..=1000 {
            if let Some(status) = try_to_get_http_status_code(code) {
                found += 1;
                assert_eq!(code, status as i32);
                // The reason phrase comes from the same table, so it is never
                // the unknown-code fallback.
                assert_ne!(UNKNOWN_STATUS_CODE_REASON, get_http_reason_phrase(status));
                assert_eq!(get_http_reason_phrase(status), get_http_reason_phrase_for_code(code));
            } else {
                assert_eq!(UNKNOWN_STATUS_CODE_REASON, get_http_reason_phrase_for_code(code));
            }
        }
        assert_eq!(LIST_LENGTH, found);
    }

    #[test]
    fn spot_check_reason_phrases() {
        assert_eq!(
            "Non-Authoritative Information",
            get_http_reason_phrase(HttpStatusCode::NonAuthoritativeInformation)
        );
        assert_eq!(
            "Request-URI Too Long",
            get_http_reason_phrase(HttpStatusCode::RequestUriTooLong)
        );
        assert_eq!(
            "HTTP Version Not Supported",
            get_http_reason_phrase(HttpStatusCode::VersionNotSupported)
        );
        assert_eq!(
            "Not Extended (Obsoleted)",
            get_http_reason_phrase(HttpStatusCode::NotExtendedObsoleted)
        );
        assert_eq!(
            "Invalid XPrivet Token",
            get_http_reason_phrase(HttpStatusCode::InvalidXprivetToken)
        );
    }
}
