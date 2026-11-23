// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base::FilePath` (base/files/file_path.{h,cc}) and of
//! `base::SafeBaseName` (base/files/safe_base_name.{h,cc}).
//!
//! A [`FilePath`] is a string plus a [`PathFlavor`] that selects the path
//! rules: `Posix` (separator `/`, no drive letters) or `Windows` (separators
//! `\` and `/`, case-insensitive drive letters, `\\` / `//` network roots).
//! Both flavors are available on every host, so the Windows rules can be
//! tested on Linux.
//!
//! The string operations are pure string manipulation and never touch the
//! file system.
//!
//! Differences from the C++ version:
//! - Paths are stored as UTF-8 `String`s for both flavors. C++ stores `char`
//!   bytes on POSIX and `wchar_t` UTF-16 on Windows. A POSIX name that is not
//!   valid UTF-8 cannot be represented. Byte-slice inputs are validated and
//!   return [`PathError::NotUtf8`] instead of being accepted.
//! - Preconditions that C++ checks with `DCHECK` (ASCII-only and UTF-8-only
//!   inputs) are reported as `Err(PathError)` values. The C++ `DCHECK`s on
//!   absolute components, on `MatchesExtension` arguments, and on the
//!   separator passed to `NormalizePathSeparatorsTo` are `debug_assert!`s.
//! - The flavor is a runtime value carried by each `FilePath`. Comparing two
//!   paths of different flavors gives `false` (and `IsParent` gives `false`).
//!   Ordering puts `Posix` before `Windows`, then compares the strings.
//! - Out-parameters become return values. `AppendRelativePath` returns
//!   `Option<FilePath>`. `CompareIgnoreCase` returns `std::cmp::Ordering`
//!   instead of `-1/0/1`.
//! - Ordering and hashing on `Windows` use the UTF-16 code units of the path,
//!   with the drive letter ASCII-lowercased, which matches the C++ rules.
//! - `IsParent` uses the fast component algorithm, which is the default in
//!   C++ (`kFastFilePathIsParent`). The older algorithm is available as
//!   [`FilePath::is_parent_slow`]. Both are tested against the same cases.
//!   There is no feature flag: the caller picks the algorithm.
//! - `CompareIgnoreCase` on `Windows` uses Rust's simple (one character to one
//!   character) uppercase mapping, not the OS `CharUpperW` table, and it leaves
//!   non-BMP characters unchanged. On `Posix` it folds only ASCII letters, as
//!   the C++ `strncasecmp` path does in a UTF-8 locale.
//! - `LossyDisplayName` returns the stored `String` (no UTF-16 conversion).
//!   `AsUTF8Unsafe` returns the stored `&str` (no locale conversion, because
//!   names are assumed to be UTF-8).
//! - `MaybeAsASCII` returns `None` for non-ASCII paths, where C++ returns "".
//!
//! Not ported:
//! - `InitializeFeatures` and the feature flag (see above).
//! - `WriteToPickle` / `ReadFromPickle` (depend on `base::Pickle`).
//! - `WriteIntoTrace` (depends on perfetto).
//! - Apple: `GetHFSDecomposedForm`, `HFSFastUnicodeCompare`, and the
//!   CoreFoundation-based `CompareIgnoreCase`.
//! - Android: `IsContentUri` and `IsVirtualDocumentPath`.
//! - `file_path_fuzzer.cc` and `file_path_rust_shim` (C++ glue code).
//! - Hashing and ordering use Rust's `Hash` and `Ord` traits, not the
//!   `std::hash<FilePath>` specialization. The values differ from C++.
//!
//! The `SafeBaseName` type is a minimal port: it only checks that a basename
//! is not absolute, does not reference a parent, and does not end in a
//! separator.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[cfg(test)]
mod tests;

/// Error for inputs that the C++ code rejects with a `DCHECK`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    /// The input contains a byte that is not ASCII.
    NotAscii,
    /// The input is not valid UTF-8.
    NotUtf8,
    /// The input is not valid UTF-16.
    NotUtf16,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            PathError::NotAscii => "input is not ASCII",
            PathError::NotUtf8 => "input is not valid UTF-8",
            PathError::NotUtf16 => "input is not valid UTF-16",
        };
        f.write_str(text)
    }
}

impl std::error::Error for PathError {}

/// The path rules that a [`FilePath`] follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PathFlavor {
    /// POSIX: `/` is the only separator, and there are no drive letters.
    Posix,
    /// Windows: `\` and `/` are separators, `X:` is a drive letter, and a
    /// leading `\\` or `//` starts a network path.
    Windows,
}

impl PathFlavor {
    /// The separators of this flavor. The first one is canonical.
    pub const fn separators(self) -> &'static [char] {
        match self {
            PathFlavor::Posix => &['/'],
            PathFlavor::Windows => &['\\', '/'],
        }
    }

    /// Returns true if `c` is a separator in this flavor.
    pub fn is_separator(self, c: char) -> bool {
        self.separators().contains(&c)
    }

    /// The separator used when composing paths.
    pub fn canonical_separator(self) -> char {
        self.separators()[0]
    }

    /// Returns true if `path` begins with a drive letter specification.
    /// Only `Windows` has drive letters.
    pub fn has_drive_letter(self, path: &str) -> bool {
        let b = path.as_bytes();
        self == PathFlavor::Windows && b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()
    }

    /// Compares two strings the way this flavor's file system does, ignoring
    /// case. See the crate documentation for the exact rules.
    pub fn compare_ignore_case(self, a: &str, b: &str) -> Ordering {
        match self {
            PathFlavor::Posix => a
                .bytes()
                .map(|c| c.to_ascii_lowercase())
                .cmp(b.bytes().map(|c| c.to_ascii_lowercase())),
            PathFlavor::Windows => windows_upper_units(a).cmp(&windows_upper_units(b)),
        }
    }

    /// Returns true if the two strings are equal ignoring case.
    pub fn compare_equal_ignore_case(self, a: &str, b: &str) -> bool {
        self.compare_ignore_case(a, b) == Ordering::Equal
    }

    /// Returns true if `a` sorts before `b`, ignoring case.
    pub fn compare_less_ignore_case(self, a: &str, b: &str) -> bool {
        self.compare_ignore_case(a, b) == Ordering::Less
    }

    fn is_sep_byte(self, b: u8) -> bool {
        match self {
            PathFlavor::Posix => b == b'/',
            PathFlavor::Windows => b == b'/' || b == b'\\',
        }
    }

    fn canonical_sep_byte(self) -> u8 {
        match self {
            PathFlavor::Posix => b'/',
            PathFlavor::Windows => b'\\',
        }
    }

    /// Length of `s` in the units the C++ code counts: bytes on POSIX and
    /// UTF-16 code units on Windows.
    fn unit_len(self, s: &str) -> usize {
        match self {
            PathFlavor::Posix => s.len(),
            PathFlavor::Windows => s.encode_utf16().count(),
        }
    }
}

/// Uppercases each character with its simple mapping, then returns the UTF-16
/// units. Characters outside the BMP are left unchanged.
fn windows_upper_units(s: &str) -> Vec<u16> {
    let mut out = Vec::with_capacity(s.len());
    let mut buf = [0u16; 2];
    for c in s.chars() {
        let upper = if (c as u32) > 0xFFFF {
            c
        } else {
            let mut it = c.to_uppercase();
            match (it.next(), it.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        };
        out.extend_from_slice(upper.encode_utf16(&mut buf));
    }
    out
}

/// The key used for Windows ordering and equality: UTF-16 units, with the
/// first unit ASCII-lowercased when the path starts with a drive letter.
fn windows_key(s: &str) -> Vec<u16> {
    let mut units: Vec<u16> = s.encode_utf16().collect();
    if PathFlavor::Windows.has_drive_letter(s) {
        units[0] = u16::from((units[0] as u8).to_ascii_lowercase());
    }
    units
}

// Private helpers that work on a string slice. Separators, drive letters and
// dots are ASCII, so slicing at their positions is always on a char boundary.

fn find_first_sep(f: PathFlavor, s: &str) -> Option<usize> {
    s.bytes().position(|b| f.is_sep_byte(b))
}

fn rfind_sep(f: PathFlavor, s: &str) -> Option<usize> {
    s.bytes().rposition(|b| f.is_sep_byte(b))
}

fn all_separators(f: PathFlavor, s: &str) -> bool {
    s.bytes().all(|b| f.is_sep_byte(b))
}

fn is_path_absolute(f: PathFlavor, s: &str) -> bool {
    let b = s.as_bytes();
    if f.has_drive_letter(s) {
        // A drive letter must be followed by a separator.
        return b.len() > 2 && f.is_sep_byte(b[2]);
    }
    if f == PathFlavor::Windows {
        return b.len() > 1 && f.is_sep_byte(b[0]) && f.is_sep_byte(b[1]);
    }
    !b.is_empty() && f.is_sep_byte(b[0])
}

fn is_network_path(f: PathFlavor, s: &str) -> bool {
    let b = s.as_bytes();
    b.len() > 1 && f.is_sep_byte(b[0]) && f.is_sep_byte(b[1])
}

/// Removes trailing separators, keeping the root. A leading `//` is never
/// stripped, and `c://` keeps both separators, as in the C++ code.
fn strip_trailing(f: PathFlavor, s: &str) -> &str {
    // With no drive letter, `start` is 1 so the leading separator survives.
    // With a drive letter, it is 3, which keeps the separator after `X:`.
    let start = if f.has_drive_letter(s) { 3 } else { 1 };
    let b = s.as_bytes();
    let mut new_len = b.len();
    let mut last_stripped: Option<usize> = None;
    let mut pos = b.len();
    while pos > start && f.is_sep_byte(b[pos - 1]) {
        if pos != start + 1 || last_stripped == Some(start + 2) || !f.is_sep_byte(b[start - 1]) {
            new_len = pos - 1;
            last_stripped = Some(pos);
        }
        pos -= 1;
    }
    &s[..new_len]
}

fn base_name_of(f: PathFlavor, s: &str) -> &str {
    let mut s = strip_trailing(f, s);
    if f.has_drive_letter(s) {
        s = &s[2..];
    }
    match rfind_sep(f, s) {
        Some(last) if last + 1 < s.len() => &s[last + 1..],
        _ => s,
    }
}

fn is_empty_or_special(s: &str) -> bool {
    s.is_empty() || s == FilePath::CURRENT_DIRECTORY || s == FilePath::PARENT_DIRECTORY
}

/// Position of the last dot that separates the extension, or `None`. The
/// argument is a base name.
fn final_extension_separator_position(s: &str) -> Option<usize> {
    if s == FilePath::CURRENT_DIRECTORY || s == FilePath::PARENT_DIRECTORY {
        return None;
    }
    s.rfind('.')
}

const COMMON_DOUBLE_EXTENSION_SUFFIXES: &[&str] =
    &["bz", "bz2", "gz", "lz", "lzma", "lzo", "xz", "z", "zst"];
const COMMON_DOUBLE_EXTENSIONS: &[&str] = &["user.js"];

/// Position of the dot that starts the extension, or `None`. A common double
/// extension such as `.tar.gz` starts at the penultimate dot.
fn extension_separator_position(f: PathFlavor, s: &str) -> Option<usize> {
    let last_dot = final_extension_separator_position(s)?;
    if last_dot == 0 {
        return Some(0);
    }

    let before = &s[..last_dot];
    let penultimate = before.rfind('.');
    let last_sep = rfind_sep(f, before);
    let penultimate = match penultimate {
        None => return Some(last_dot),
        Some(p) => {
            if let Some(sep) = last_sep {
                if p < sep {
                    return Some(last_dot);
                }
            }
            p
        }
    };

    let tail = &s[penultimate + 1..];
    if COMMON_DOUBLE_EXTENSIONS.iter().any(|e| tail.eq_ignore_ascii_case(e)) {
        return Some(penultimate);
    }

    let extension = &s[last_dot + 1..];
    if COMMON_DOUBLE_EXTENSION_SUFFIXES.iter().any(|e| extension.eq_ignore_ascii_case(e)) {
        // Count the units between the two dots, as the C++ code does.
        let diff = f.unit_len(&s[penultimate..last_dot]);
        if diff > 1 && diff <= 5 {
            return Some(penultimate);
        }
    }

    Some(last_dot)
}

/// Splits `path` into its first component and the remainder. Leading
/// separators form one component. A leading drive letter is its own
/// component when `can_be_drive_letter` is set.
fn extract_first_component(f: PathFlavor, path: &str, can_be_drive_letter: bool) -> (&str, &str) {
    if path.is_empty() {
        return ("", "");
    }

    if f.is_sep_byte(path.as_bytes()[0]) {
        // One separator is the root. Two are a network root. Three or more
        // are treated as one root separator.
        let (mut first, remainder) = match path.bytes().position(|b| !f.is_sep_byte(b)) {
            Some(i) => (&path[..i], &path[i..]),
            None => (path, ""),
        };
        if first.len() >= 3 {
            first = &first[..1];
        }
        if f == PathFlavor::Windows {
            // Canonicalize the separators, so that "/" and "\" compare equal.
            let canonical_root: &'static str = "\\\\";
            first = &canonical_root[..first.len()];
        }
        return (first, remainder);
    }

    if can_be_drive_letter && f.has_drive_letter(path) {
        let remainder = if path.len() > 2 { &path[2..] } else { "" };
        return (&path[..2], remainder);
    }

    match find_first_sep(f, path) {
        None => (path, ""),
        Some(i) => {
            let first = &path[..i];
            let rest = &path[i..];
            let remainder = match rest.bytes().position(|b| !f.is_sep_byte(b)) {
                Some(j) => &rest[j..],
                None => "",
            };
            (first, remainder)
        }
    }
}

/// Returns true if both are drive letter components with the same letter.
fn are_drive_letter_components_equal(left: &str, right: &str) -> bool {
    let (l, r) = (left.as_bytes(), right.as_bytes());
    l.len() == 2
        && r.len() == 2
        && l[1] == b':'
        && r[1] == b':'
        && l[0].is_ascii_alphabetic()
        && l[0].eq_ignore_ascii_case(&r[0])
}

/// A path in the native format of a [`PathFlavor`].
#[derive(Clone)]
pub struct FilePath {
    flavor: PathFlavor,
    path: String,
}

impl FilePath {
    /// The special path component meaning "this directory".
    pub const CURRENT_DIRECTORY: &'static str = ".";
    /// The special path component meaning "the parent directory".
    pub const PARENT_DIRECTORY: &'static str = "..";
    /// The character that starts a file extension.
    pub const EXTENSION_SEPARATOR: char = '.';

    /// Creates a path. Like the C++ constructor, the path is cut at the first
    /// NUL character.
    pub fn new(flavor: PathFlavor, path: &str) -> FilePath {
        let path = match path.find('\0') {
            Some(nul) => &path[..nul],
            None => path,
        };
        FilePath { flavor, path: path.to_owned() }
    }

    /// Returns the flavor of this path.
    pub fn flavor(&self) -> PathFlavor {
        self.flavor
    }

    /// Returns the path string.
    pub fn value(&self) -> &str {
        &self.path
    }

    /// Returns true if the path is empty.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Empties the path.
    pub fn clear(&mut self) {
        self.path.clear();
    }

    fn same_flavor(&self, path: String) -> FilePath {
        FilePath { flavor: self.flavor, path }
    }

    /// Returns true if `c` is a separator for this path's flavor.
    pub fn is_separator(&self, c: char) -> bool {
        self.flavor.is_separator(c)
    }

    /// Returns the path's components. The root separator is kept as its own
    /// component, so the result distinguishes absolute from relative paths.
    ///
    /// POSIX: `/foo/bar` gives `["/", "foo", "bar"]`.
    /// Windows: `C:\foo\bar` gives `["C:", "\\", "foo", "bar"]`.
    pub fn components(&self) -> Vec<String> {
        let f = self.flavor;
        let mut ret = Vec::new();
        if self.path.is_empty() {
            return ret;
        }

        let mut current = self.clone();
        // Capture path components.
        loop {
            let parent = current.dir_name();
            if current == parent {
                break;
            }
            let base = current.base_name();
            if !all_separators(f, base.value()) {
                ret.push(base.path);
            }
            current = parent;
        }

        // Capture the root, if any.
        let base = current.base_name();
        if !base.value().is_empty() && base.value() != FilePath::CURRENT_DIRECTORY {
            ret.push(base.path);
        }

        // Capture the drive letter, if any.
        let dir = current.dir_name();
        if f.has_drive_letter(dir.value()) {
            ret.push(dir.path[..2].to_owned());
        }

        ret.reverse();
        ret
    }

    /// Returns true if this path is a parent or ancestor of `child`.
    ///
    /// Absolute and relative paths both work (`/foo` is a parent of
    /// `/foo/bar`, and `foo` is a parent of `foo/bar`). Any ancestor counts,
    /// so `/a` is a parent of `/a/b/c`. A path is not its own parent. No
    /// conversion to absolute, no symlink resolution, and no handling of
    /// `..`.
    ///
    /// This is the fast component-by-component algorithm, which is the
    /// default in C++. See [`FilePath::is_parent_slow`] for the other one.
    pub fn is_parent(&self, child: &FilePath) -> bool {
        if self.flavor != child.flavor {
            return false;
        }
        let f = self.flavor;
        let mut parent_view: &str = &self.path;
        let mut child_view: &str = &child.path;

        let mut index = 0usize;
        loop {
            // The first component that is not "." can be a drive letter.
            let can_be_drive_letter = index == 0;

            let (mut pc, mut pr) = extract_first_component(f, parent_view, can_be_drive_letter);
            let (mut cc, mut cr) = extract_first_component(f, child_view, can_be_drive_letter);

            if index == 0 {
                if pc.is_empty() {
                    // `self` has no component, so it is not a parent of anything.
                    return false;
                }
                // "./a" counts as a parent of "a/b", but a lone "." does not.
                if !pr.is_empty() && pc == FilePath::CURRENT_DIRECTORY {
                    (pc, pr) = extract_first_component(f, pr, can_be_drive_letter);
                }
                if !cr.is_empty() && cc == FilePath::CURRENT_DIRECTORY {
                    (cc, cr) = extract_first_component(f, cr, can_be_drive_letter);
                }
            }

            if pc.is_empty() {
                // All components of `self` matched. It is a parent only if the
                // child has more components.
                return !cc.is_empty();
            }

            let equal = pc == cc
                || (index == 0
                    && f == PathFlavor::Windows
                    && are_drive_letter_components_equal(pc, cc))
                || (index == 1 && is_network_path(f, &self.path) && pc.eq_ignore_ascii_case(cc));
            if !equal {
                return false;
            }

            parent_view = pr;
            child_view = cr;
            index += 1;
        }
    }

    /// Returns true if this path is a parent of `child`, computed by comparing
    /// the components from [`FilePath::components`]. It gives the same answers
    /// as [`FilePath::is_parent`].
    pub fn is_parent_slow(&self, child: &FilePath) -> bool {
        self.relative_components(child).is_some()
    }

    /// Returns the components of `child` below this path. `None` if this path
    /// is not a parent of `child`.
    fn relative_components(&self, child: &FilePath) -> Option<Vec<String>> {
        if self.flavor != child.flavor {
            return None;
        }
        let f = self.flavor;
        let parent = self.components();
        let child_components = child.components();

        if parent.is_empty() || parent.len() >= child_components.len() {
            return None;
        }

        let mut p = 0usize;
        let mut c = 0usize;

        // On Windows, the drive letters match case-insensitively. Component
        // comparisons after that are case-sensitive.
        if f == PathFlavor::Windows
            && f.has_drive_letter(&parent[p])
            && f.has_drive_letter(&child_components[c])
        {
            let n = child_components[c].len();
            if parent[p].len() < n
                || !parent[p].as_bytes()[..n].eq_ignore_ascii_case(child_components[c].as_bytes())
            {
                return None;
            }
            p += 1;
            c += 1;
        }

        // The root is the only component made of separators. Roots of the
        // same length match even if written with different separators.
        if p < parent.len() && all_separators(f, &parent[p]) {
            let child_root = child_components.get(c)?;
            if f.unit_len(&parent[p]) != f.unit_len(child_root) || !all_separators(f, child_root) {
                return None;
            }
            p += 1;
            c += 1;

            // Network paths: the host is compared case-insensitively.
            if is_network_path(f, &self.path) && p < parent.len() {
                let child_host = child_components.get(c)?;
                if !parent[p].eq_ignore_ascii_case(child_host) {
                    return None;
                }
                p += 1;
                c += 1;
            }
        }

        while p < parent.len() {
            let child_component = child_components.get(c)?;
            if parent[p] != *child_component {
                return None;
            }
            p += 1;
            c += 1;
        }

        Some(child_components[c..].to_vec())
    }

    /// If this path is a parent of `child`, returns `base` with the relative
    /// path to `child` appended. Otherwise returns `None`.
    ///
    /// For example, with parent `/Users/x/Library` and child
    /// `/Users/x/Library/Google/Chrome`, and `base` = `/tmp/cache`, the result
    /// is `/tmp/cache/Google/Chrome`.
    pub fn append_relative_path(&self, child: &FilePath, base: &FilePath) -> Option<FilePath> {
        let rest = self.relative_components(child)?;
        let mut out = base.clone();
        for component in &rest {
            out = out.append(component);
        }
        Some(out)
    }

    /// Returns the directory that holds this path. A single component gives
    /// `"."`. The root stays the root. Directory navigation is not resolved,
    /// so the parent of `../a` is `..`.
    pub fn dir_name(&self) -> FilePath {
        let f = self.flavor;
        let s = strip_trailing(f, &self.path);
        let dir: &str = match rfind_sep(f, s) {
            None if f.has_drive_letter(s) => &s[..2],
            None => FilePath::CURRENT_DIRECTORY,
            Some(last) => strip_trailing(f, &s[..=last]),
        };
        self.same_flavor(dir.to_owned())
    }

    /// Returns the last component of this path, which can be a file or a
    /// directory. A root returns itself, and this is the only case in which
    /// the result is absolute. The drive letter is always removed.
    pub fn base_name(&self) -> FilePath {
        self.same_flavor(base_name_of(self.flavor, &self.path).to_owned())
    }

    /// Returns the extension, including the dot. A common double extension
    /// such as `.tar.gz` or `.user.js` is returned whole, and the match
    /// against the allow-list ignores ASCII case. The case of the result is
    /// not normalized. Returns an empty string if there is no extension.
    pub fn extension(&self) -> String {
        let base = base_name_of(self.flavor, &self.path);
        match extension_separator_position(self.flavor, base) {
            None => String::new(),
            Some(dot) => base[dot..].to_owned(),
        }
    }

    /// Returns the last extension, including the dot, ignoring double
    /// extensions. Returns an empty string if there is none. `"."` and `".."`
    /// have no extension.
    pub fn final_extension(&self) -> String {
        let base = base_name_of(self.flavor, &self.path);
        match final_extension_separator_position(base) {
            None => String::new(),
            Some(dot) => base[dot..].to_owned(),
        }
    }

    /// Removes the extension, honoring double extensions. `C:\pics\jojo.jpg`
    /// becomes `C:\pics\jojo`.
    pub fn remove_extension(&self) -> FilePath {
        if self.extension().is_empty() {
            return self.clone();
        }
        match extension_separator_position(self.flavor, &self.path) {
            None => self.clone(),
            Some(dot) => self.same_flavor(self.path[..dot].to_owned()),
        }
    }

    /// Removes only the last extension, so `foo.tar.gz` becomes `foo.tar`.
    pub fn remove_final_extension(&self) -> FilePath {
        if self.final_extension().is_empty() {
            return self.clone();
        }
        match final_extension_separator_position(&self.path) {
            None => self.clone(),
            Some(dot) => self.same_flavor(self.path[..dot].to_owned()),
        }
    }

    /// Inserts `suffix` between the base name and the extension. Returns an
    /// empty path if the base name is `"."` or `".."`. An empty `suffix` gives
    /// a copy of this path.
    ///
    /// `C:\pics\jojo.jpg` with `" (1)"` gives `C:\pics\jojo (1).jpg`.
    pub fn insert_before_extension(&self, suffix: &str) -> FilePath {
        if suffix.is_empty() {
            return self.clone();
        }
        if is_empty_or_special(base_name_of(self.flavor, &self.path)) {
            return self.same_flavor(String::new());
        }
        let joined = format!("{}{}{}", self.remove_extension().value(), suffix, self.extension());
        self.build_same_flavor(&joined)
    }

    /// [`FilePath::insert_before_extension`] with an ASCII-only suffix.
    pub fn insert_before_extension_ascii(&self, suffix: &str) -> Result<FilePath, PathError> {
        if !suffix.is_ascii() {
            return Err(PathError::NotAscii);
        }
        Ok(self.insert_before_extension(suffix))
    }

    /// [`FilePath::insert_before_extension`] with a UTF-8 suffix.
    pub fn insert_before_extension_utf8(&self, suffix: &[u8]) -> Result<FilePath, PathError> {
        let suffix = std::str::from_utf8(suffix).map_err(|_| PathError::NotUtf8)?;
        Ok(self.insert_before_extension(suffix))
    }

    /// Adds `extension` to the file name. A missing leading dot is added. An
    /// empty extension or `"."` gives a copy of this path. Returns an empty
    /// path if the base name is `"."` or `".."`.
    pub fn add_extension(&self, extension: &str) -> FilePath {
        if is_empty_or_special(base_name_of(self.flavor, &self.path)) {
            return self.same_flavor(String::new());
        }
        if extension.is_empty() || extension == "." {
            return self.clone();
        }
        let needs_separator =
            !extension.starts_with('.') && !self.path.is_empty() && !self.path.ends_with('.');
        let mut out = self.path.clone();
        if needs_separator {
            out.push('.');
        }
        out.push_str(extension);
        self.build_same_flavor(&out)
    }

    /// [`FilePath::add_extension`] with an ASCII-only extension.
    pub fn add_extension_ascii(&self, extension: &str) -> Result<FilePath, PathError> {
        if !extension.is_ascii() {
            return Err(PathError::NotAscii);
        }
        Ok(self.add_extension(extension))
    }

    /// [`FilePath::add_extension`] with a UTF-8 extension.
    pub fn add_extension_utf8(&self, extension: &[u8]) -> Result<FilePath, PathError> {
        let extension = std::str::from_utf8(extension).map_err(|_| PathError::NotUtf8)?;
        Ok(self.add_extension(extension))
    }

    /// Replaces the extension with `extension`. A missing leading dot is
    /// added. An empty extension or `"."` removes the extension. Returns an
    /// empty path if the base name is `"."` or `".."`.
    pub fn replace_extension(&self, extension: &str) -> FilePath {
        if is_empty_or_special(base_name_of(self.flavor, &self.path)) {
            return self.same_flavor(String::new());
        }
        let no_ext = self.remove_extension();
        if extension.is_empty() || extension == "." {
            return no_ext;
        }
        let mut out = no_ext.path;
        if !extension.starts_with('.') {
            out.push('.');
        }
        out.push_str(extension);
        self.build_same_flavor(&out)
    }

    /// Returns true if `extension()` equals `extension`, ignoring case. The
    /// argument must be empty or start with a dot.
    pub fn matches_extension(&self, extension: &str) -> bool {
        debug_assert!(extension.is_empty() || extension.starts_with('.'));
        self.matches_ignore_case(extension, &self.extension())
    }

    /// Returns true if `final_extension()` equals `extension`, ignoring case.
    /// The argument must be empty or start with a dot.
    pub fn matches_final_extension(&self, extension: &str) -> bool {
        debug_assert!(extension.is_empty() || extension.starts_with('.'));
        self.matches_ignore_case(extension, &self.final_extension())
    }

    fn matches_ignore_case(&self, expected: &str, actual: &str) -> bool {
        self.flavor.unit_len(expected) == self.flavor.unit_len(actual)
            && self.flavor.compare_equal_ignore_case(expected, actual)
    }

    /// Appends `component` with one separator, avoiding doubled separators.
    /// If this path is `"."`, the result is `component`. The component cannot
    /// be absolute, and it is cut at the first NUL.
    pub fn append(&self, component: &str) -> FilePath {
        let appended = match component.find('\0') {
            Some(nul) => &component[..nul],
            None => component,
        };
        debug_assert!(!is_path_absolute(self.flavor, appended));

        if self.path == FilePath::CURRENT_DIRECTORY && !appended.is_empty() {
            // Appending to "." would only lengthen the path, so return the
            // component itself.
            return FilePath::new(self.flavor, appended);
        }

        let mut new_path = strip_trailing(self.flavor, &self.path).to_owned();

        // No separator is added for an empty path or an empty component.
        if !appended.is_empty() && !new_path.is_empty() {
            let last = new_path.as_bytes()[new_path.len() - 1];
            if !self.flavor.is_sep_byte(last) {
                // Don't add a separator right after a drive letter.
                let after_drive =
                    if self.flavor.has_drive_letter(&new_path) { Some(2) } else { None };
                if after_drive != Some(new_path.len()) {
                    new_path.push(char::from(self.flavor.canonical_sep_byte()));
                }
            }
        }

        new_path.push_str(appended);
        self.same_flavor(new_path)
    }

    /// [`FilePath::append`] with another path as the component.
    pub fn append_path(&self, component: &FilePath) -> FilePath {
        self.append(component.value())
    }

    /// [`FilePath::append`] with a [`SafeBaseName`] as the component.
    pub fn append_base_name(&self, component: &SafeBaseName) -> FilePath {
        self.append(component.path().value())
    }

    /// [`FilePath::append`] with an ASCII-only component.
    pub fn append_ascii(&self, component: &str) -> Result<FilePath, PathError> {
        if !component.is_ascii() {
            return Err(PathError::NotAscii);
        }
        Ok(self.append(component))
    }

    /// [`FilePath::append`] with a UTF-8 component.
    pub fn append_utf8(&self, component: &[u8]) -> Result<FilePath, PathError> {
        let component = std::str::from_utf8(component).map_err(|_| PathError::NotUtf8)?;
        Ok(self.append(component))
    }

    /// Returns true if the path is absolute. Windows: a drive letter followed
    /// by a separator, or two separators at the start. POSIX: a leading
    /// separator.
    pub fn is_absolute(&self) -> bool {
        is_path_absolute(self.flavor, &self.path)
    }

    /// Returns true if the path starts with two separators (a network path).
    pub fn is_network(&self) -> bool {
        is_network_path(self.flavor, &self.path)
    }

    /// Returns true if the last character is a separator. An empty path is
    /// false.
    pub fn ends_with_separator(&self) -> bool {
        self.path.as_bytes().last().is_some_and(|&b| self.flavor.is_sep_byte(b))
    }

    /// Returns a copy that ends with a separator. An empty path stays empty.
    pub fn as_ending_with_separator(&self) -> FilePath {
        if self.ends_with_separator() || self.path.is_empty() {
            return self.clone();
        }
        let mut out = self.path.clone();
        out.push(self.flavor.canonical_separator());
        self.same_flavor(out)
    }

    /// Returns a copy without trailing separators. A root stays a root, so
    /// `////` becomes `/`.
    pub fn strip_trailing_separators(&self) -> FilePath {
        self.same_flavor(strip_trailing(self.flavor, &self.path).to_owned())
    }

    /// Returns true if a component is `..`. On Windows, a component made only
    /// of dots and whitespace that contains `..` also counts.
    pub fn references_parent(&self) -> bool {
        if !self.path.contains(FilePath::PARENT_DIRECTORY) {
            // GetComponents is relatively costly, so skip it when there is no
            // ".." at all.
            return false;
        }
        self.components().iter().any(|component| match self.flavor {
            PathFlavor::Posix => component == FilePath::PARENT_DIRECTORY,
            PathFlavor::Windows => {
                component.chars().all(|c| ". \n\r\t".contains(c))
                    && component.contains(FilePath::PARENT_DIRECTORY)
            }
        })
    }

    /// Returns the path for display. The C++ version returns UTF-16 and may
    /// fail to convert; here the stored UTF-8 string is returned. Do not use
    /// this to get a real path back.
    pub fn lossy_display_name(&self) -> String {
        self.path.clone()
    }

    /// Returns the path as ASCII, or `None` if it is not ASCII.
    pub fn maybe_as_ascii(&self) -> Option<&str> {
        if self.path.is_ascii() {
            Some(&self.path)
        } else {
            None
        }
    }

    /// Returns the path as UTF-8. This is always the stored string, because
    /// paths are stored as UTF-8.
    pub fn as_utf8_unsafe(&self) -> &str {
        &self.path
    }

    /// Returns the path as UTF-16 code units.
    pub fn as_utf16_unsafe(&self) -> Vec<u16> {
        self.path.encode_utf16().collect()
    }

    /// Creates a path from an ASCII string. Returns `Err` for non-ASCII input.
    pub fn from_ascii(flavor: PathFlavor, ascii: &str) -> Result<FilePath, PathError> {
        if !ascii.is_ascii() {
            return Err(PathError::NotAscii);
        }
        Ok(FilePath::new(flavor, ascii))
    }

    /// Creates a path from UTF-8 bytes. Returns `Err` for invalid UTF-8.
    pub fn from_utf8_unsafe(flavor: PathFlavor, utf8: &[u8]) -> Result<FilePath, PathError> {
        let s = std::str::from_utf8(utf8).map_err(|_| PathError::NotUtf8)?;
        Ok(FilePath::new(flavor, s))
    }

    /// Creates a path from UTF-16 code units. Returns `Err` for invalid
    /// UTF-16.
    pub fn from_utf16_unsafe(flavor: PathFlavor, utf16: &[u16]) -> Result<FilePath, PathError> {
        let s = String::from_utf16(utf16).map_err(|_| PathError::NotUtf16)?;
        Ok(FilePath::new(flavor, &s))
    }

    /// On Windows, replaces every `/` with `\`. On POSIX, returns a copy.
    /// Does not change the number of separators.
    pub fn normalize_path_separators(&self) -> FilePath {
        self.normalize_path_separators_to(self.flavor.canonical_separator())
    }

    /// Replaces every separator with `normalized_separator`. On POSIX, returns
    /// a copy.
    pub fn normalize_path_separators_to(&self, normalized_separator: char) -> FilePath {
        if self.flavor == PathFlavor::Posix {
            return self.clone();
        }
        debug_assert!(self.flavor.is_separator(normalized_separator));
        let out: String = self
            .path
            .chars()
            .map(|c| if self.flavor.is_separator(c) { normalized_separator } else { c })
            .collect();
        self.same_flavor(out)
    }

    /// Builds a path with this flavor from `s`, cutting at the first NUL.
    fn build_same_flavor(&self, s: &str) -> FilePath {
        FilePath::new(self.flavor, s)
    }
}

impl fmt::Display for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

impl fmt::Debug for FilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePath").field("flavor", &self.flavor).field("path", &self.path).finish()
    }
}

impl PartialEq for FilePath {
    /// On Windows, drive letters compare case-insensitively. Paths of
    /// different flavors are never equal.
    fn eq(&self, other: &FilePath) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for FilePath {}

impl PartialOrd for FilePath {
    fn partial_cmp(&self, other: &FilePath) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FilePath {
    /// Orders by flavor, then by the path. On Windows, the path is compared as
    /// UTF-16 units with the drive letter ASCII-lowercased.
    fn cmp(&self, other: &FilePath) -> Ordering {
        match self.flavor.cmp(&other.flavor) {
            Ordering::Equal => match self.flavor {
                PathFlavor::Posix => self.path.cmp(&other.path),
                PathFlavor::Windows => windows_key(&self.path).cmp(&windows_key(&other.path)),
            },
            ordering => ordering,
        }
    }
}

impl Hash for FilePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.flavor.hash(state);
        match self.flavor {
            PathFlavor::Posix => self.path.hash(state),
            PathFlavor::Windows => windows_key(&self.path).hash(state),
        }
    }
}

/// A basename that is safe to append to a directory: it is not absolute, it
/// does not reference the parent, and it does not end with a separator.
/// Port of `base::SafeBaseName`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeBaseName {
    path: FilePath,
}

impl SafeBaseName {
    /// Returns the safe basename of `path`, or `None` if it is not safe. The
    /// basename is taken with [`FilePath::base_name`].
    pub fn create(path: &FilePath) -> Option<SafeBaseName> {
        let basename = path.base_name();
        if !basename.is_absolute()
            && !basename.references_parent()
            && !basename.ends_with_separator()
        {
            Some(SafeBaseName { path: basename })
        } else {
            None
        }
    }

    /// Like [`SafeBaseName::create`], from a string.
    pub fn create_from_str(flavor: PathFlavor, path: &str) -> Option<SafeBaseName> {
        SafeBaseName::create(&FilePath::new(flavor, path))
    }

    /// Returns the basename as a path.
    pub fn path(&self) -> &FilePath {
        &self.path
    }
}
