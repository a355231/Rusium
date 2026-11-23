// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of Chromium's URL component parser
//! (url/third_party/mozilla/url_parse.{h,cc} and url/url_parse_file.cc).
//!
//! The parser splits a URL into components (scheme, username, password, host,
//! port, path, query, reference) and records each one as a `(begin, len)` pair
//! of offsets into the input. It never fails: every input produces a `Parsed`.
//! The algorithm follows the C++ code step by step, so component positions,
//! the `-1` "absent" convention and the empty-but-present cases all match.
//!
//! The parser is generic over [`Unit`], so the same code handles UTF-8 bytes
//! (`&[u8]`) and UTF-16 code units (`&[u16]`). Offsets are counted in units of
//! the input slice, as in the C++ code.
//!
//! Differences from the C++ version:
//! - Inputs are slices (`&[u8]` / `&[u16]`) rather than pointer and length
//!   pairs. The `const char*` / `int url_len` overloads are not ported.
//! - Out-parameters are return values. `ExtractScheme` returns
//!   `Option<Component>`. `ParseAuthority` returns [`Authority`].
//!   `ParsePathInternal` returns [`PathComponents`]. `ExtractQueryKeyValue`
//!   takes `&mut Component` for the query and returns `Option<(key, value)>`.
//! - `ParsePort` returns a [`Port`] enum. [`Port::to_cpp_int`] gives the C++
//!   integer (-1 unspecified, -2 invalid, otherwise the port number).
//! - `Parsed::ref` is named `reference` because `ref` is a Rust keyword.
//! - `Parsed::inner_parsed_` is `Option<Box<Parsed>>`. Copying and dropping
//!   are handled by `Clone` and `Drop` rather than manual copy constructors.
//! - `ParserMode` variants are `Special` and `NonSpecial`.
//! - `operator<<` is replaced by `Display` for [`Component`]. `Parsed` has no
//!   printing helper.
//! - Preconditions that the C++ code only checks with DCHECK (for example a
//!   valid authority component, or a valid path in `ParsePath`) are checked
//!   with `assert!`, so misuse panics instead of reading out of bounds.
//! - `Component::as_view_on` follows `std::basic_string_view::substr`: it
//!   panics when `begin` is past the end of the source and clamps `len` to the
//!   end of the source. The C++ `CHECK` and integer overflow checks in
//!   `Component::Create`, `OffsetBy`, `CheckedEnd` and `MakeRange` become
//!   panics.
//! - Reads that the C++ code would make out of bounds (undefined behavior, for
//!   example for a component that runs past the end of the input) panic.
//! - The standard-scheme registry (`url_util.cc`) is not ported. The default
//!   list is hard-coded: https, http, file, ftp, wss, ws and filesystem.
//!   Runtime registration is not supported.
//! - Windows-only code in `url_parse_file.cc` (`#ifdef WIN32` drive-letter and
//!   UNC rules) is not ported. Only the non-Windows behavior is implemented,
//!   and the non-Windows unit tests are the ones ported.
//! - `Component::Create` and `MakeRange` are `Component::create` and
//!   [`make_range`]. `MakeRange(int, int)` is [`make_range_i32`].
//! - `ParseNonSpecialUrlInternal` and the other `*Internal` entry points
//!   (`ParsePathInternal`, `ParseAfterSpecialScheme`,
//!   `ParseAfterNonSpecialScheme`) are exposed as `pub` functions, because
//!   the C++ code uses them from other translation units.
//! - `Parsed::Length` and `Parsed::CountCharactersBefore` keep their C++
//!   semantics. The int arithmetic is `i32` with overflow checked.
//!
//! Not ported: `EstimateMemoryUsage`, `url_parse_perftest.cc`, and the
//! `CRUBIT_*` / `UNSAFE_BUFFER_USAGE` annotations.

#![forbid(unsafe_code)]

mod file;

use std::fmt;

pub use file::parse_file_url;

/// Sealed trait for the code-unit types the parser accepts: `u8` (UTF-8) and
/// `u16` (UTF-16).
pub trait Unit: Copy + Eq + sealed::Sealed {
    /// The unit's value, zero-extended to `u32`.
    fn code(self) -> u32;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
}

impl Unit for u8 {
    fn code(self) -> u32 {
        u32::from(self)
    }
}

impl Unit for u16 {
    fn code(self) -> u32 {
        u32::from(self)
    }
}

/// A substring of the input, given by its offset and length. `len == -1`
/// means the component is absent. `len == 0` means it is present but empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Component {
    /// Offset of the component in the input.
    pub begin: i32,
    /// Length of the component, or -1 when it is not specified.
    pub len: i32,
}

impl Default for Component {
    /// The absent component: `begin == 0`, `len == -1`.
    fn default() -> Self {
        Component { begin: 0, len: -1 }
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}, {}}}", self.begin, self.len)
    }
}

fn to_i32(v: usize) -> i32 {
    i32::try_from(v).expect("value does not fit in an int")
}

impl Component {
    /// Creates a component with the given offset and length.
    pub const fn new(begin: i32, len: i32) -> Self {
        Component { begin, len }
    }

    /// Creates a component from `usize` offset and length. Panics if either
    /// does not fit in an `i32`.
    pub fn create(begin: usize, len: usize) -> Self {
        Component::new(to_i32(begin), to_i32(len))
    }

    /// Returns a component that covers all `len` units of an input.
    pub fn covering(len: usize) -> Self {
        Component::new(0, to_i32(len))
    }

    /// Adds `offset` to `begin`. Used when a component is relative to a
    /// substring. Panics on overflow.
    pub fn offset_by(&mut self, offset: usize) {
        let new_begin = i64::from(self.begin) + offset as i64;
        self.begin = i32::try_from(new_begin).expect("component begin overflows an int");
    }

    /// Returns `begin + len`.
    pub fn end(&self) -> i32 {
        self.begin
            .checked_add(self.len)
            .expect("component end overflows an int")
    }

    /// Returns `end()` as `usize`. Panics if it is negative.
    pub fn checked_end(&self) -> usize {
        usize::try_from(self.end()).expect("component end is negative")
    }

    /// True if the component is present. An empty component can still be
    /// present.
    pub fn is_valid(&self) -> bool {
        self.len >= 0
    }

    /// True if the component is absent or has length zero.
    pub fn is_empty(&self) -> bool {
        self.len <= 0
    }

    /// True if the component has a positive length.
    pub fn is_nonempty(&self) -> bool {
        self.len > 0
    }

    /// Makes the component absent (`begin == 0`, `len == -1`).
    pub fn reset(&mut self) {
        *self = Component::default();
    }

    /// Returns the part of `source` the component covers. Follows
    /// `std::basic_string_view::substr`: panics if `begin` is past the end of
    /// `source`, and clamps the end to `source.len()`. An absent component
    /// (`len == -1`) gives the rest of `source` from `begin`.
    pub fn as_view_on<'a, U: Unit>(&self, source: &'a [U]) -> &'a [U] {
        let begin = usize::try_from(self.begin).expect("negative component begin");
        assert!(
            begin <= source.len(),
            "component begins past the end of the source"
        );
        let count = if self.len < 0 {
            usize::MAX
        } else {
            self.len as usize
        };
        let end = begin.saturating_add(count).min(source.len());
        &source[begin..end]
    }

    /// Like `as_view_on`, but returns `None` for an absent component.
    pub fn maybe_as_view_on<'a, U: Unit>(&self, source: &'a [U]) -> Option<&'a [U]> {
        if self.is_valid() {
            Some(self.as_view_on(source))
        } else {
            None
        }
    }
}

/// Returns the component for the range `[begin, end)`. Panics if `begin > end`.
pub fn make_range(begin: usize, end: usize) -> Component {
    assert!(begin <= end, "MakeRange: begin must not exceed end");
    Component::create(begin, end - begin)
}

/// Returns the component for the range `[begin, end)` given as `i32`. The
/// length is `end - begin` and is not checked against `begin > end`, matching
/// the C++ `MakeRange(int, int)`.
pub fn make_range_i32(begin: i32, end: i32) -> Component {
    Component::new(begin, end - begin)
}

/// Which kind of component `Parsed::count_characters_before` looks for.
/// Declaration order matches the C++ enum, so `<=` comparisons work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentType {
    Scheme,
    Username,
    Password,
    Host,
    Port,
    Path,
    Query,
    Ref,
}

/// Whether the parser uses the special-scheme or non-special-scheme authority
/// terminator rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserMode {
    /// Special schemes (https, http, file, ...). A backslash also ends the
    /// authority.
    Special,
    /// Non-special schemes. Only `/`, `?` and `#` end the authority.
    NonSpecial,
}

/// The result of parsing a URL into components. This does not hold the URL
/// text; the caller keeps the input and the `Parsed` together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Scheme without the colon. Absent if there is no colon. Empty if the
    /// input starts with a colon.
    pub scheme: Component,
    /// Username, if given with `@`.
    pub username: Component,
    /// Password, if given after a colon in the userinfo.
    pub password: Component,
    /// Host name. For non-special URLs, absent unless `//` follows the scheme.
    pub host: Component,
    /// Port digits, without the colon.
    pub port: Component,
    /// Path, including the leading slash. Absent when the path is empty.
    pub path: Component,
    /// Query after `?`, without the `?`.
    pub query: Component,
    /// Everything after the first `#`, without the `#`. Present with length 0
    /// for a bare `#`. Absent when there is no `#`.
    pub reference: Component,
    /// True if the input had a raw `<` and whitespace was removed. Only the
    /// canonicalizer sets this, so the parser always leaves it false.
    pub potentially_dangling_markup: bool,
    /// True for non-special URLs whose path is opaque (`git:opaque`).
    pub has_opaque_path: bool,
    inner: Option<Box<Parsed>>,
}

impl Parsed {
    /// Returns the length of the URL: the end of the last component.
    ///
    /// Not always the input length. For `"http://"` only the scheme is
    /// recorded, so the result is 4.
    pub fn length(&self) -> i32 {
        if self.reference.is_valid() {
            return self.reference.end();
        }
        self.count_characters_before(ComponentType::Ref, false)
    }

    /// Returns the number of characters before `component_type`, or where it
    /// would be if it were present. `include_delimiter` controls whether the
    /// `:`, `@`, `?` or `#` before a delimited component is counted.
    pub fn count_characters_before(
        &self,
        component_type: ComponentType,
        include_delimiter: bool,
    ) -> i32 {
        use ComponentType::*;
        let ty = component_type;
        if ty == Scheme {
            return self.scheme.begin;
        }

        let mut cur: i32 = 0;
        if self.scheme.is_valid() {
            cur = self.scheme.end() + 1; // Skip the ':' after the scheme.
        }

        if self.username.is_valid() {
            if ty <= Username {
                return self.username.begin;
            }
            cur = self.username.end() + 1; // Skip the '@' or ':'.
        }

        if self.password.is_valid() {
            if ty <= Password {
                return self.password.begin;
            }
            cur = self.password.end() + 1; // Skip the '@'.
        }

        if self.host.is_valid() {
            if ty <= Host {
                return self.host.begin;
            }
            cur = self.host.end();
        }

        if self.port.is_valid() {
            if ty < Port || (ty == Port && include_delimiter) {
                return self.port.begin - 1; // Back over the ':'.
            }
            if ty == Port {
                return self.port.begin;
            }
            cur = self.port.end();
        }

        if self.path.is_valid() {
            if ty <= Path {
                return self.path.begin;
            }
            cur = self.path.end();
        }

        if self.query.is_valid() {
            if ty < Query || (ty == Query && include_delimiter) {
                return self.query.begin - 1; // Back over the '?'.
            }
            if ty == Query {
                return self.query.begin;
            }
            cur = self.query.end();
        }

        if self.reference.is_valid() {
            if ty == Ref && !include_delimiter {
                return self.reference.begin;
            }
            // The component wanted was before this one, so the fragment start
            // is known. Back over the '#' unless the caller wants Ref without it.
            return self.reference.begin - 1;
        }

        cur
    }

    /// Returns everything after the scheme's colon, through the end of the
    /// URL. Absent when that would be empty.
    pub fn get_content(&self) -> Component {
        let begin = self.count_characters_before(ComponentType::Username, false);
        let len = self.length() - begin;
        if len != 0 {
            Component::new(begin, len)
        } else {
            Component::default()
        }
    }

    /// Returns the parsed inner URL of a filesystem URL, if any.
    pub fn inner_parsed(&self) -> Option<&Parsed> {
        self.inner.as_deref()
    }

    /// Returns a mutable reference to the inner parsed URL, if any.
    pub fn inner_parsed_mut(&mut self) -> Option<&mut Parsed> {
        self.inner.as_deref_mut()
    }

    /// Sets the inner parsed URL, replacing any existing one.
    pub fn set_inner_parsed(&mut self, inner: Parsed) {
        self.inner = Some(Box::new(inner));
    }

    /// Removes the inner parsed URL.
    pub fn clear_inner_parsed(&mut self) {
        self.inner = None;
    }
}

/// The parsed authority: username, password, host and port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Authority {
    pub username: Component,
    pub password: Component,
    pub host: Component,
    pub port: Component,
}

/// The parsed path, query and fragment of a path-style input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathComponents {
    pub path: Component,
    pub query: Component,
    pub reference: Component,
}

/// The result of parsing a port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    /// The port component is absent or empty (C++ `PORT_UNSPECIFIED`).
    Unspecified,
    /// The port is not a number in `0..=65535` (C++ `PORT_INVALID`).
    Invalid,
    /// A valid port number.
    Number(u16),
}

impl Port {
    /// Returns the C++ integer value: -1 for unspecified, -2 for invalid, and
    /// otherwise the port number.
    pub fn to_cpp_int(self) -> i32 {
        match self {
            Port::Unspecified => -1,
            Port::Invalid => -2,
            Port::Number(n) => i32::from(n),
        }
    }
}

// ---------------------------------------------------------------------------
// Character helpers (url_parse_internal.h).

const COLON: u32 = b':' as u32;
const AT: u32 = b'@' as u32;
const SLASH: u32 = b'/' as u32;
const BACKSLASH: u32 = b'\\' as u32;
const QUESTION: u32 = b'?' as u32;
const HASH: u32 = b'#' as u32;
const SEMICOLON: u32 = b';' as u32;
const AMP: u32 = b'&' as u32;
const EQUALS: u32 = b'=' as u32;
const ZERO: u32 = b'0' as u32;

pub(crate) fn is_slash_or_backslash(c: u32) -> bool {
    c == SLASH || c == BACKSLASH
}

/// True for ASCII spaces and control characters (`<= ' '`). These are trimmed
/// from both ends of a URL.
fn should_trim_from_url(c: u32) -> bool {
    c <= u32::from(b' ')
}

/// Returns `(begin, end)` after trimming leading (and, if `trim_path_end`,
/// trailing) spaces and control characters.
pub(crate) fn trim_url<U: Unit>(spec: &[U], trim_path_end: bool) -> (usize, usize) {
    let mut begin = 0;
    let mut end = spec.len();
    while begin < end && should_trim_from_url(spec[begin].code()) {
        begin += 1;
    }
    if trim_path_end {
        while begin < end && should_trim_from_url(spec[end - 1].code()) {
            end -= 1;
        }
    }
    (begin, end)
}

/// Counts the slashes and backslashes (mixed) starting at `begin_offset`.
pub(crate) fn count_consecutive_slashes_or_backslashes<U: Unit>(
    spec: &[U],
    begin_offset: usize,
) -> usize {
    let mut count = 0;
    let mut i = begin_offset;
    while i < spec.len() && is_slash_or_backslash(spec[i].code()) {
        i += 1;
        count += 1;
    }
    count
}

/// Counts forward slashes only, starting at `begin_offset`.
fn count_consecutive_slashes<U: Unit>(spec: &[U], begin_offset: usize) -> usize {
    let mut count = 0;
    while begin_offset + count < spec.len() && spec[begin_offset + count].code() == SLASH {
        count += 1;
    }
    count
}

fn is_authority_terminator_code(c: u32, mode: ParserMode) -> bool {
    match mode {
        ParserMode::Special => is_slash_or_backslash(c) || c == QUESTION || c == HASH,
        ParserMode::NonSpecial => c == SLASH || c == QUESTION || c == HASH,
    }
}

/// Returns true if `ch` ends the authority section of a URL under `parser_mode`.
pub fn is_authority_terminator(ch: u16, parser_mode: ParserMode) -> bool {
    is_authority_terminator_code(u32::from(ch), parser_mode)
}

/// Returns the offset of the next authority terminator at or after
/// `start_offset`, or `spec.len()` if there is none.
fn find_next_authority_terminator<U: Unit>(
    spec: &[U],
    start_offset: usize,
    mode: ParserMode,
) -> usize {
    (start_offset..spec.len())
        .find(|&i| is_authority_terminator_code(spec[i].code(), mode))
        .unwrap_or(spec.len())
}

/// Returns the offset of `c` in `s[from..]`, if present.
fn find_char<U: Unit>(s: &[U], from: usize, c: u32) -> Option<usize> {
    (from..s.len()).find(|&i| s[i].code() == c)
}

/// Scheme comparison: ASCII case-insensitive equality with a lowercase
/// reference string, as `base::EqualsCaseInsensitiveASCII` does.
fn fold_ascii(c: u32) -> u32 {
    if (u32::from(b'A')..=u32::from(b'Z')).contains(&c) {
        c + 32
    } else {
        c
    }
}

fn eq_ignore_ascii_case_units<U: Unit>(a: &[U], b: &str) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.bytes())
            .all(|(x, y)| fold_ascii(x.code()) == fold_ascii(u32::from(y)))
}

/// Port of `CompareSchemeComponent`: an empty component matches the empty
/// string, and otherwise the case-insensitive comparison applies.
pub(crate) fn compare_scheme_component<U: Unit>(
    spec: &[U],
    component: Component,
    compare_to: &str,
) -> bool {
    if component.is_empty() {
        return compare_to.is_empty();
    }
    eq_ignore_ascii_case_units(component.as_view_on(spec), compare_to)
}

/// Port of `IsStandard` for the default scheme registry: https, http, file,
/// ftp, wss, ws and filesystem. The registry is not mutable here.
pub(crate) fn is_standard_scheme<U: Unit>(scheme: &[U]) -> bool {
    const STANDARD: [&str; 7] = ["https", "http", "file", "ftp", "wss", "ws", "filesystem"];
    if scheme.is_empty() {
        return false;
    }
    STANDARD
        .iter()
        .any(|s| eq_ignore_ascii_case_units(scheme, s))
}

// ---------------------------------------------------------------------------
// Parsing (url_parse.cc).

/// Port of `ParseUserInfo`. Splits `user` into the username and password at the
/// first colon.
fn parse_user_info<U: Unit>(spec: &[U], user: Component) -> (Component, Component) {
    let mut colon_offset: i32 = 0;
    while colon_offset < user.len && spec[(user.begin + colon_offset) as usize].code() != COLON {
        colon_offset += 1;
    }

    if colon_offset < user.len {
        // <username>:<password>
        (
            Component::new(user.begin, colon_offset),
            make_range_i32(user.begin + colon_offset + 1, user.begin + user.len),
        )
    } else {
        // No separator, everything is the username.
        (user, Component::default())
    }
}

/// Splits the server info (`<host>[:<port>]`) into host and port.
fn parse_server_info<U: Unit>(spec: &[U], serverinfo: Component) -> (Component, Component) {
    if serverinfo.len == 0 {
        // No server info: the host is empty.
        return (Component::default(), Component::default());
    }

    // A leading '[' means the whole host is an IPv6 literal until a ']' is seen.
    let mut ipv6_terminator: i64 = if spec[serverinfo.begin as usize].code() == u32::from(b'[') {
        i64::from(serverinfo.end())
    } else {
        -1
    };
    let mut colon: i64 = -1;

    // Find the last ']' and the last ':'.
    for i in serverinfo.begin..serverinfo.end() {
        match spec[i as usize].code() {
            c if c == u32::from(b']') => ipv6_terminator = i64::from(i),
            c if c == COLON => colon = i64::from(i),
            _ => {}
        }
    }

    if colon > ipv6_terminator {
        // <hostname>:<port>
        let mut hostname = make_range_i32(serverinfo.begin, colon as i32);
        if hostname.len == 0 {
            hostname.reset();
        }
        let port = make_range_i32(colon as i32 + 1, serverinfo.end());
        (hostname, port)
    } else {
        // No port: <hostname>
        (serverinfo, Component::default())
    }
}

/// Port of `DoParseAuthority`. Splits an already-identified authority range
/// into its username, password, host and port.
pub(crate) fn do_parse_authority<U: Unit>(spec: &[U], auth: Component, mode: ParserMode) -> Authority {
    assert!(auth.is_valid(), "We should always get an authority");
    if auth.len == 0 {
        let host = match mode {
            ParserMode::Special => Component::default(),
            // Non-special URLs can have an empty host, which is distinct from
            // no host ("git:///" vs "git:/").
            ParserMode::NonSpecial => Component::new(auth.begin, 0),
        };
        return Authority {
            username: Component::default(),
            password: Component::default(),
            host,
            port: Component::default(),
        };
    }

    // Search backwards for '@', which separates userinfo from server info.
    let mut i = auth.begin + auth.len - 1;
    while i > auth.begin && spec[i as usize].code() != AT {
        i -= 1;
    }

    if spec[i as usize].code() == AT {
        // <user-info>@<server-info>
        let (username, password) = parse_user_info(spec, Component::new(auth.begin, i - auth.begin));
        let (host, port) = parse_server_info(spec, make_range_i32(i + 1, auth.end()));
        Authority {
            username,
            password,
            host,
            port,
        }
    } else {
        // No userinfo: everything is server info.
        let (host, port) = parse_server_info(spec, auth);
        Authority {
            username: Component::default(),
            password: Component::default(),
            host,
            port,
        }
    }
}

/// Port of `ParseAuthority`. Best-effort split of the authority range `auth`
/// into its components. Components that are not found are absent.
pub fn parse_authority<U: Unit>(spec: &[U], auth: Component, parser_mode: ParserMode) -> Authority {
    do_parse_authority(spec, auth, parser_mode)
}

/// Port of `ParsePort`. Converts the port component to a number. Leading zeros
/// are allowed. The value must have at most five digits and be at most 65535.
pub fn parse_port<U: Unit>(spec: &[U], port: Component) -> Port {
    const MAX_DIGITS: i32 = 5;
    if port.is_empty() {
        return Port::Unspecified;
    }

    // Skip over leading zeros.
    let mut digits = Component::new(port.end(), 0);
    for i in 0..port.len {
        if spec[(port.begin + i) as usize].code() != ZERO {
            digits = make_range_i32(port.begin + i, port.end());
            break;
        }
    }
    if digits.len == 0 {
        return Port::Number(0); // All digits were zero.
    }

    if digits.len > MAX_DIGITS {
        return Port::Invalid;
    }

    let mut value: u32 = 0;
    for i in 0..digits.len {
        let c = spec[(digits.begin + i) as usize].code();
        if !(ZERO..=u32::from(b'9')).contains(&c) {
            return Port::Invalid;
        }
        value = value * 10 + (c - ZERO);
    }
    match u16::try_from(value) {
        Ok(n) => Port::Number(n),
        Err(_) => Port::Invalid,
    }
}

/// Port of `FindQueryAndRefParts`. Returns the offsets of the first `?` and
/// the first `#` inside `path`, as limited by the fragment rule: a `?` counts
/// only if it comes before the `#`.
fn find_query_and_ref_parts<U: Unit>(spec: &[U], path: Component) -> (Option<usize>, Option<usize>) {
    let path_begin = path.begin as usize;
    let path_end = path.checked_end();
    let ref_separator = find_char(&spec[..path_end], path_begin, HASH);
    let len_before_fragment = ref_separator.unwrap_or(path_end);
    let query_separator = find_char(&spec[..len_before_fragment], path_begin, QUESTION);
    (query_separator, ref_separator)
}

/// Port of `ParsePath`. Splits the range `path` (everything after the
/// authority) into path, query and reference.
pub(crate) fn parse_path<U: Unit>(spec: &[U], path: Component) -> PathComponents {
    // path = [/]<segment1>/<segment2>/<...>/<segmentN>;<param>?<query>#<ref>
    assert!(path.is_valid(), "ParsePath requires a valid range");

    let (query_separator, ref_separator) = find_query_and_ref_parts(spec, path);

    // Ends of the parts found so far, searching from the end back to the start.
    let path_end = path.checked_end();
    let (mut file_end, query_end, reference) = match ref_separator {
        Some(r) => (r, r, make_range(r + 1, path_end)),
        None => (path_end, path_end, Component::default()),
    };

    let query = match query_separator {
        Some(q) => {
            file_end = q;
            make_range(q + 1, query_end)
        }
        None => Component::default(),
    };

    let path_begin = path.begin as usize;
    let filepath = if file_end != path_begin {
        make_range(path_begin, file_end)
    } else {
        // An empty file path is treated as no file path.
        Component::default()
    };

    PathComponents {
        path: filepath,
        query,
        reference,
    }
}

/// Port of `ParsePathInternal`: splits the range `path` into path, query and
/// reference.
pub fn parse_path_internal<U: Unit>(spec: &[U], path: Component) -> PathComponents {
    parse_path(spec, path)
}

/// Port of `DoExtractScheme`. Skips leading spaces and control characters,
/// then takes everything up to the first colon as the scheme.
pub(crate) fn do_extract_scheme<U: Unit>(url: &[U]) -> Option<Component> {
    let mut begin = 0;
    while begin < url.len() && should_trim_from_url(url[begin].code()) {
        begin += 1;
    }
    if begin == url.len() {
        return None; // Empty or all whitespace.
    }

    (begin..url.len())
        .find(|&i| url[i].code() == COLON)
        .map(|i| make_range(begin, i))
}

/// Port of `ExtractScheme`. Returns the scheme range, or `None` if there is no
/// colon. A leading colon gives an empty scheme.
pub fn extract_scheme<U: Unit>(url: &[U]) -> Option<Component> {
    do_extract_scheme(url)
}

/// Port of `DoParseAfterSpecialScheme`. Fills in the parsed components after
/// a special scheme. `after_scheme` is the offset just past the colon.
pub fn parse_after_special_scheme<U: Unit>(spec: &[U], after_scheme: usize, parsed: &mut Parsed) {
    let num_slashes = count_consecutive_slashes_or_backslashes(spec, after_scheme);
    let mut after_slashes = after_scheme + num_slashes;

    // The authority runs to the next slash (or backslash, '?' or '#').
    let end_auth = find_next_authority_terminator(spec, after_slashes, ParserMode::Special);

    // Workaround for crbug.com/484253361: `after_slashes` can exceed `end_auth`.
    after_slashes = after_slashes.min(end_auth);

    let authority = make_range(after_slashes, end_auth);
    // Everything from the authority terminator to the end is the path part.
    let full_path = make_range(end_auth, spec.len());

    let auth = do_parse_authority(spec, authority, ParserMode::Special);
    parsed.username = auth.username;
    parsed.password = auth.password;
    parsed.host = auth.host;
    parsed.port = auth.port;
    set_path_parts(parsed, parse_path(spec, full_path));
}

pub(crate) fn set_path_parts(parsed: &mut Parsed, parts: PathComponents) {
    parsed.path = parts.path;
    parsed.query = parts.query;
    parsed.reference = parts.reference;
}

/// Port of `DoParseStandardUrl`. Trims the input, then parses it as a
/// special-scheme URL.
fn do_parse_standard_url<U: Unit>(url: &[U]) -> Parsed {
    let (begin, url_len) = trim_url(url, true);
    let url = &url[..url_len];

    let mut parsed = Parsed::default();
    let after_scheme = match do_extract_scheme(url) {
        Some(scheme) => {
            parsed.scheme = scheme;
            scheme.checked_end() + 1 // Skip past the colon.
        }
        None => {
            // No colon means no scheme. This is less wrong than treating the
            // whole input as a scheme.
            parsed.scheme.reset();
            begin
        }
    };
    parse_after_special_scheme(url, after_scheme, &mut parsed);
    parsed
}

/// Parses `url` as a standard (special-scheme) URL, such as `https://...`.
///
/// The query starts at the first `?` before the first `#`. The reference is
/// everything after the first `#`, so a later `#` stays in the reference.
/// Backslashes count as slashes.
pub fn parse_standard_url<U: Unit>(url: &[U]) -> Parsed {
    do_parse_standard_url(url)
}

/// Port of `DoParseAfterNonSpecialScheme`.
pub fn parse_after_non_special_scheme<U: Unit>(
    spec: &[U],
    after_scheme: usize,
    parsed: &mut Parsed,
) {
    // Only forward slashes count here, unlike the special-scheme parser.
    let num_slashes = count_consecutive_slashes(spec, after_scheme);
    let spec_len = spec.len();

    if num_slashes >= 2 {
        // "//<some data>": an authority section follows.
        parsed.has_opaque_path = false;

        let after_slashes = after_scheme + 2;
        let end_auth = find_next_authority_terminator(spec, after_slashes, ParserMode::NonSpecial);
        let authority = make_range(after_slashes, end_auth);

        let auth = do_parse_authority(spec, authority, ParserMode::NonSpecial);
        parsed.username = auth.username;
        parsed.password = auth.password;
        parsed.host = auth.host;
        parsed.port = auth.port;

        let full_path = make_range(end_auth, spec_len);
        set_path_parts(parsed, parse_path(spec, full_path));
        return;
    }

    // One slash is a hierarchical path. No slash is an opaque path.
    parsed.has_opaque_path = num_slashes == 0;

    parsed.username.reset();
    parsed.password.reset();
    // Reset the host to tell "host is empty" from "host does not exist".
    parsed.host.reset();
    parsed.port.reset();

    // Everything from after the scheme to the end is the path.
    set_path_parts(parsed, parse_path(spec, make_range(after_scheme, spec_len)));
}

/// Port of `DoParseNonSpecialUrl`. `trim_path_end` controls whether trailing
/// spaces and control characters are removed. The public API always trims.
fn do_parse_non_special_url<U: Unit>(url: &[U], trim_path_end: bool) -> Parsed {
    let (_, url_len) = trim_url(url, trim_path_end);
    let url = &url[..url_len];

    let mut parsed = Parsed::default();
    let after_scheme = match do_extract_scheme(url) {
        Some(scheme) => {
            parsed.scheme = scheme;
            scheme.checked_end() + 1
        }
        None => {
            parsed.scheme.reset();
            // Unlike the special parser, this starts at 0, not at the trimmed
            // beginning.
            0
        }
    };
    parse_after_non_special_scheme(url, after_scheme, &mut parsed);
    parsed
}

/// Parses `url` as a non-special URL, such as `git://host/path` or
/// `git:opaque`. Trailing spaces are trimmed.
pub fn parse_non_special_url<U: Unit>(url: &[U]) -> Parsed {
    do_parse_non_special_url(url, true)
}

/// Like [`parse_non_special_url`], but `trim_path_end` controls whether
/// trailing spaces and control characters are removed.
pub fn parse_non_special_url_internal<U: Unit>(url: &[U], trim_path_end: bool) -> Parsed {
    do_parse_non_special_url(url, trim_path_end)
}

/// Port of `DoParsePathUrl`. For schemes with no authority and no special file
/// rules, such as `about:blank` or `javascript:...`. Everything after the
/// colon is the path.
fn do_parse_path_url<U: Unit>(url: &[U], trim_path_end: bool) -> Parsed {
    let (scheme_begin, url_len) = trim_url(url, trim_path_end);

    if scheme_begin == url_len {
        return Parsed::default(); // Empty or whitespace only.
    }

    let mut parsed = Parsed::default();
    let path_begin = match do_extract_scheme(&url[scheme_begin..url_len]) {
        Some(mut scheme) => {
            scheme.offset_by(scheme_begin);
            parsed.scheme = scheme;
            scheme.checked_end() + 1
        }
        None => {
            parsed.scheme.reset();
            scheme_begin
        }
    };

    if path_begin == url_len {
        return parsed;
    }

    set_path_parts(
        &mut parsed,
        parse_path(&url[..url_len], make_range(path_begin, url_len)),
    );
    parsed
}

/// Parses `url` as a path URL: a scheme followed by a path, with no authority.
/// `trim_path_end` controls whether trailing whitespace is removed.
pub fn parse_path_url<U: Unit>(url: &[U], trim_path_end: bool) -> Parsed {
    do_parse_path_url(url, trim_path_end)
}

/// Port of `DoParseMailtoUrl`. Splits a `mailto:` URL into scheme, path and
/// query. Everything after the first `?` is the query.
fn do_parse_mailto_url<U: Unit>(url: &[U]) -> Parsed {
    let (begin, url_len) = trim_url(url, true);

    if begin == url_len {
        return Parsed::default();
    }

    let mut parsed = Parsed::default();

    // (path_begin, path_end) is None when there is no path range at all.
    let range: Option<(usize, usize)> = match do_extract_scheme(&url[begin..url_len]) {
        Some(mut scheme) => {
            scheme.offset_by(begin);
            parsed.scheme = scheme;
            let scheme_end = scheme.checked_end();
            if scheme_end != url_len - 1 {
                Some((scheme_end + 1, url_len))
            } else {
                None // "mailto:" with nothing after the colon.
            }
        }
        None => {
            // No scheme: everything is the path.
            parsed.scheme.reset();
            Some((begin, url_len))
        }
    };

    let Some((path_begin, mut path_end)) = range else {
        parsed.path.reset();
        return parsed;
    };

    // Split [path_begin, path_end) into path and query at the first '?'.
    for i in path_begin..path_end {
        if url[i].code() == QUESTION {
            parsed.query = make_range(i + 1, path_end);
            path_end = i;
            break;
        }
    }

    // Following the standard parser's convention, no path is absent (-1).
    if path_begin == path_end {
        parsed.path.reset();
    } else {
        parsed.path = make_range(path_begin, path_end);
    }
    parsed
}

/// Parses `url` as a `mailto:` URL.
pub fn parse_mailto_url<U: Unit>(url: &[U]) -> Parsed {
    do_parse_mailto_url(url)
}

/// Port of `DoParseFileSystemUrl`. A `filesystem:` URL wraps an inner URL
/// (for example `filesystem:file:///temporary/foo`). The outer components are
/// `scheme`, `path` (the part after the filesystem type), `query` and `ref`.
/// The inner URL's parse is stored in `inner_parsed()`, with every offset
/// relative to the whole input.
fn do_parse_file_system_url<U: Unit>(url: &[U]) -> Parsed {
    let (begin, url_len) = trim_url(url, true);

    // Empty input, or only whitespace and control characters.
    if begin == url_len {
        return Parsed::default();
    }

    let mut parsed = Parsed::default();
    let inner_start: usize = match do_extract_scheme(&url[begin..url_len]) {
        Some(mut scheme) => {
            scheme.offset_by(begin);
            parsed.scheme = scheme;
            let scheme_end = scheme.checked_end();
            if scheme_end == url_len - 1 {
                return parsed; // "filesystem:" with nothing after it.
            }
            scheme_end + 1
        }
        None => {
            // No scheme is not valid for filesystem URLs.
            return Parsed::default();
        }
    };

    let inner_url = &url[inner_start..url_len];
    let inner_scheme = match do_extract_scheme(inner_url) {
        Some(mut s) => {
            s.offset_by(inner_start);
            if s.checked_end() == url_len - 1 {
                return parsed;
            }
            s
        }
        None => {
            // No inner scheme. The best we can do is "filesystem://".
            return parsed;
        }
    };

    // The inner URL is parsed with the same rules as its own scheme. Filesystem
    // URLs do not nest.
    let mut inner = if compare_scheme_component(url, inner_scheme, "file") {
        file::parse_file_url(inner_url)
    } else if compare_scheme_component(url, inner_scheme, "filesystem") {
        return parsed;
    } else if is_standard_scheme(inner_scheme.as_view_on(url)) {
        do_parse_standard_url(inner_url)
    } else {
        return parsed;
    };

    // Shift every inner component by inner_start, including absent ones, as
    // the C++ does.
    shift(&mut inner.scheme, inner_start);
    shift(&mut inner.username, inner_start);
    shift(&mut inner.password, inner_start);
    shift(&mut inner.host, inner_start);
    shift(&mut inner.port, inner_start);
    shift(&mut inner.query, inner_start);
    shift(&mut inner.reference, inner_start);
    shift(&mut inner.path, inner_start);

    // The query and reference move from the inner URL to the outer one.
    parsed.query = inner.query;
    inner.query.reset();
    parsed.reference = inner.reference;
    inner.reference.reset();

    let inner_scheme_valid = inner.scheme.is_valid();
    let inner_path = inner.path;
    let inner_has_nested = inner.inner_parsed().is_some();
    parsed.set_inner_parsed(inner);
    if !inner_scheme_valid || !inner_path.is_valid() || inner_has_nested {
        return parsed;
    }

    // The inner path should start with a slash, then the filesystem type, then
    // a slash. The outer path is what follows the type. If the path ends before
    // the second slash, that is still accepted.
    if !is_slash_or_backslash(url[inner_path.begin as usize].code()) {
        return parsed;
    }
    let mut inner_path_end = inner_path.begin as usize + 1; // Skip the leading slash.
    while inner_path_end < url_len && !is_slash_or_backslash(url[inner_path_end].code()) {
        inner_path_end += 1;
    }

    parsed.path.begin = inner_path_end as i32;
    let new_inner_path_length = inner_path_end as i32 - inner_path.begin;
    parsed.path.len = inner_path.len - new_inner_path_length;
    if let Some(inner) = parsed.inner_parsed_mut() {
        inner.path.len = new_inner_path_length;
    }
    parsed
}

fn shift(component: &mut Component, by: usize) {
    component.begin += to_i32(by);
}

/// Parses `url` as a `filesystem:` URL.
pub fn parse_file_system_url<U: Unit>(url: &[U]) -> Parsed {
    do_parse_file_system_url(url)
}

/// Port of `ExtractFileName`. Returns the file name at the end of `path`: the
/// text after the last slash or backslash, up to the first `;` after that
/// slash. Absent if `path` is absent.
pub fn extract_file_name<U: Unit>(url: &[U], path: Component) -> Component {
    if path.is_empty() {
        return Component::default();
    }

    // Scan backwards from the end of the path.
    let mut file_end = path.end();
    let mut i = path.end() - 1;
    while i >= path.begin {
        let c = url[i as usize].code();
        if c == SEMICOLON {
            file_end = i;
        } else if is_slash_or_backslash(c) {
            // The name is everything after this slash, up to the ';' (if any).
            return make_range_i32(i + 1, file_end);
        }
        i -= 1;
    }

    // No slash: the input is degenerate. Treat the whole path as the name.
    make_range_i32(path.begin, file_end)
}

/// Port of `ExtractQueryKeyValue`. Returns the next key and value in `query`,
/// and moves `query` to start after them, so repeated calls iterate over all
/// the pairs. Pairs can be empty (`"?&"` gives two empty pairs). A trailing
/// empty pair (`"a=1&"`) is not returned, since it is the same as "done".
/// Returns `None` if no pair is left, and then `query` is unchanged.
pub fn extract_query_key_value<U: Unit>(
    url: &[U],
    query: &mut Component,
) -> Option<(Component, Component)> {
    if !query.is_nonempty() {
        return None;
    }

    let mut cur = query.begin;
    let end = query.end();

    // The key runs up to '&' or '='.
    let key_begin = cur;
    while cur < end && url[cur as usize].code() != AMP && url[cur as usize].code() != EQUALS {
        cur += 1;
    }
    let key = Component::new(key_begin, cur - key_begin);

    // Skip the '=' after the key (if any).
    if cur < end && url[cur as usize].code() == EQUALS {
        cur += 1;
    }

    // The value runs up to '&'.
    let value_begin = cur;
    while cur < end && url[cur as usize].code() != AMP {
        cur += 1;
    }
    let value = Component::new(value_begin, cur - value_begin);

    // Skip the '&' after the value (if any).
    if cur < end && url[cur as usize].code() == AMP {
        cur += 1;
    }

    *query = make_range_i32(cur, end);
    Some((key, value))
}
