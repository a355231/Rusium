// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Port of `net/cookies/cookie_options.{h,cc}`.
//!
//! The nested C++ types are hoisted to module level: `ContextType`,
//! `ContextMetadata`, `ContextDowngradeType`, `ContextRedirectTypeBug1221316`
//! and `HttpMethod`.
//!
//! The C++ `PrintTo` gtest helpers are replaced by `Display` impls that
//! produce the same text. Those helpers print enums as integers, and the
//! unit tests depend on that text.
//!
//! The C++ `DCHECK_LE(schemeful_context_, context_)` checks become
//! `debug_assert!`s, so release builds skip them as the C++ release build
//! does.

use std::fmt;

/// Relation between a cookie and the navigational environment. Ordered from
/// least to most trusted. Values are used for histograms; do not renumber.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContextType {
    CrossSite = 0,
    /// Same rules as `SameSiteLax`, but the HTTP method is unsafe.
    SameSiteLaxMethodUnsafe = 1,
    SameSiteLax = 2,
    SameSiteStrict = 3,
}

impl fmt::Display for ContextType {
    /// Prints the integer value, as the C++ `PrintTo` does.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", *self as i32)
    }
}

/// Possible downgrades of the SameSite context type, caused by a behavior
/// change in the same-site calculation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ContextDowngradeType {
    /// Context not downgraded.
    #[default]
    NoDowngrade = 0,
    /// Strictly same-site, downgraded to laxly same-site.
    StrictToLax = 1,
    /// Strictly same-site, downgraded to cross-site.
    StrictToCross = 2,
    /// Laxly same-site, downgraded to cross-site.
    LaxToCross = 3,
}

/// Redirect classification for crbug.com/1221316. Values are persisted to
/// logs. The notation `A->B->C` means a navigation from A to B that was
/// redirected to C.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ContextRedirectTypeBug1221316 {
    /// Unused. For example, a JavaScript access.
    #[default]
    Unset = 0,
    /// No redirects, such as A->B or A->A.
    NoRedirect = 1,
    /// Redirected to a different site than the site-for-cookies, such as
    /// A->B->C.
    CrossSiteRedirect = 2,
    /// Redirected, and the start and end are the same site, such as A->B->A.
    PartialSameSiteRedirect = 3,
    /// Redirected, and all URLs are the same site, such as A->A->A.
    AllSameSiteRedirect = 4,
}

/// HTTP method classification. Values are persisted to logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum HttpMethod {
    /// The enum does not apply in this context.
    Unset = -1,
    /// The method string could not be converted to this enum.
    Unknown = 0,
    Get = 1,
    Head = 2,
    Post = 3,
    /// C++ `KPut`.
    Put = 4,
    Delete = 5,
    Connect = 6,
    Options = 7,
    Trace = 8,
    Patch = 9,
}

/// Metadata describing how a SameSite context was computed. It is used for
/// histograms and warnings only, and must not be relied on for inclusion
/// decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMetadata {
    /// Whether the spec change in
    /// <https://github.com/httpwg/http-extensions/pull/1348> changed the
    /// context calculation for a cross-site redirect.
    pub cross_site_redirect_downgrade: ContextDowngradeType,
    /// Whether the initiator for this request is null.
    pub has_null_initiator: bool,
    pub redirect_type_bug_1221316: ContextRedirectTypeBug1221316,
}

impl fmt::Display for ContextMetadata {
    /// Same text as the C++ `PrintTo` for `ContextMetadata`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ cross_site_redirect_downgrade: {}, redirect_type_bug_1221316: {} }}",
            self.cross_site_redirect_downgrade as i32, self.redirect_type_bug_1221316 as i32
        )
    }
}

/// Relation between a cookie and the navigational environment, with
/// schemeful and schemeless context types.
///
/// Equality compares only the two context types, not the metadata.
#[derive(Clone, Copy, Debug)]
pub struct SameSiteCookieContext {
    context: ContextType,
    schemeful_context: ContextType,
    metadata: ContextMetadata,
    schemeful_metadata: ContextMetadata,
}

impl Default for SameSiteCookieContext {
    /// Both context types are `CrossSite`, the untrusted default.
    fn default() -> Self {
        Self::new(ContextType::CrossSite)
    }
}

impl PartialEq for SameSiteCookieContext {
    fn eq(&self, other: &Self) -> bool {
        self.context == other.context && self.schemeful_context == other.schemeful_context
    }
}

impl Eq for SameSiteCookieContext {}

impl fmt::Display for SameSiteCookieContext {
    /// Same text as the C++ `PrintTo` for `SameSiteCookieContext`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ context: {}, schemeful_context: {}, metadata: {}, schemeful_metadata: {} }}",
            self.context, self.schemeful_context, self.metadata, self.schemeful_metadata
        )
    }
}

impl SameSiteCookieContext {
    /// Both the schemeful and the schemeless context are `same_site_context`.
    pub fn new(same_site_context: ContextType) -> Self {
        Self::with_schemeful(same_site_context, same_site_context)
    }

    /// Creates a context with separate schemeless and schemeful types, and
    /// default metadata.
    pub fn with_schemeful(same_site_context: ContextType, schemeful_context: ContextType) -> Self {
        Self::with_metadata(
            same_site_context,
            schemeful_context,
            ContextMetadata::default(),
            ContextMetadata::default(),
        )
    }

    /// Creates a context with metadata. The context types are checked for
    /// consistency (`schemeful_context <= context`), but the metadata is
    /// stored as given, even if it is inconsistent with the types.
    pub fn with_metadata(
        same_site_context: ContextType,
        schemeful_context: ContextType,
        metadata: ContextMetadata,
        schemeful_metadata: ContextMetadata,
    ) -> Self {
        debug_assert!(schemeful_context <= same_site_context);
        Self {
            context: same_site_context,
            schemeful_context,
            metadata,
            schemeful_metadata,
        }
    }

    /// The most inclusive context for access: `SameSiteStrict` for both.
    pub fn make_inclusive() -> Self {
        Self::new(ContextType::SameSiteStrict)
    }

    /// The most inclusive context for set: `SameSiteLax` for both.
    pub fn make_inclusive_for_set() -> Self {
        Self::new(ContextType::SameSiteLax)
    }

    /// The context used for SameSite cookie inclusion. This is the schemeful
    /// context.
    pub fn get_context_for_cookie_inclusion(&self) -> ContextType {
        debug_assert!(self.schemeful_context <= self.context);
        self.schemeful_context
    }

    /// The metadata for the current schemeful mode. This is always the
    /// schemeful metadata.
    pub fn get_metadata_for_current_schemeful_mode(&self) -> &ContextMetadata {
        &self.schemeful_metadata
    }

    /// The schemeless context type. Prefer
    /// [`Self::get_context_for_cookie_inclusion`] for access checks.
    pub fn context(&self) -> ContextType {
        self.context
    }

    /// The schemeful context type.
    pub fn schemeful_context(&self) -> ContextType {
        self.schemeful_context
    }

    pub fn metadata(&self) -> &ContextMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut ContextMetadata {
        &mut self.metadata
    }

    pub fn schemeful_metadata(&self) -> &ContextMetadata {
        &self.schemeful_metadata
    }

    pub fn schemeful_metadata_mut(&mut self) -> &mut ContextMetadata {
        &mut self.schemeful_metadata
    }

    /// Sets the context types without checking consistency, and without
    /// touching the metadata.
    pub fn set_context_types_for_testing(
        &mut self,
        context_type: ContextType,
        schemeful_context_type: ContextType,
    ) {
        self.context = context_type;
        self.schemeful_context = schemeful_context_type;
    }

    /// Returns `true` if the context types and all metadata fields are equal.
    pub fn complete_equivalence_for_testing(&self, other: &Self) -> bool {
        self == other
            && self.metadata == other.metadata
            && self.schemeful_metadata == other.schemeful_metadata
    }
}

/// Options for getting and setting cookies.
///
/// The defaults exclude HttpOnly and SameSite cookies, update the access
/// time, and do not report excluded cookies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieOptions {
    exclude_httponly: bool,
    same_site_cookie_context: SameSiteCookieContext,
    update_access_time: bool,
    return_excluded_cookies: bool,
}

impl Default for CookieOptions {
    fn default() -> Self {
        Self {
            exclude_httponly: true,
            same_site_cookie_context: SameSiteCookieContext::new(ContextType::CrossSite),
            update_access_time: true,
            return_excluded_cookies: false,
        }
    }
}

impl CookieOptions {
    /// Same as `CookieOptions::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Excludes HttpOnly cookies.
    pub fn set_exclude_httponly(&mut self) {
        self.exclude_httponly = true;
    }

    /// Includes HttpOnly cookies.
    pub fn set_include_httponly(&mut self) {
        self.exclude_httponly = false;
    }

    pub fn exclude_httponly(&self) -> bool {
        self.exclude_httponly
    }

    /// Sets how trusted the browser environment is for accessing SameSite
    /// cookies. The default is not trusted (`CrossSite`).
    pub fn set_same_site_cookie_context(&mut self, context: SameSiteCookieContext) {
        self.same_site_cookie_context = context;
    }

    pub fn same_site_cookie_context(&self) -> &SameSiteCookieContext {
        &self.same_site_cookie_context
    }

    pub fn set_update_access_time(&mut self) {
        self.update_access_time = true;
    }

    pub fn set_do_not_update_access_time(&mut self) {
        self.update_access_time = false;
    }

    pub fn update_access_time(&self) -> bool {
        self.update_access_time
    }

    pub fn set_return_excluded_cookies(&mut self) {
        self.return_excluded_cookies = true;
    }

    pub fn unset_return_excluded_cookies(&mut self) {
        self.return_excluded_cookies = false;
    }

    pub fn return_excluded_cookies(&self) -> bool {
        self.return_excluded_cookies
    }

    /// Options that work for getting and setting all cookies, including
    /// HttpOnly and SameSite cookies. The access time is not updated, so
    /// that stored creation times are not disturbed. This is the opposite of
    /// the default options.
    pub fn make_all_inclusive() -> Self {
        let mut options = Self::default();
        options.set_include_httponly();
        options.set_same_site_cookie_context(SameSiteCookieContext::make_inclusive());
        options.set_do_not_update_access_time();
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from net/cookies/cookie_options_unittest.cc.

    #[test]
    fn same_site_cookie_context_type() {
        // gtest PrintToString of the enum gives its integer value.
        assert_eq!("0", ContextType::CrossSite.to_string());
        assert_eq!("1", ContextType::SameSiteLaxMethodUnsafe.to_string());
        assert_eq!("2", ContextType::SameSiteLax.to_string());
        assert_eq!("3", ContextType::SameSiteStrict.to_string());
    }

    #[test]
    fn same_site_cookie_context() {
        let cross_cross = SameSiteCookieContext::new(ContextType::CrossSite);
        let lax_lax = SameSiteCookieContext::new(ContextType::SameSiteLax);
        let strict_strict = SameSiteCookieContext::new(ContextType::SameSiteStrict);
        let strict_cross =
            SameSiteCookieContext::with_schemeful(ContextType::SameSiteStrict, ContextType::CrossSite);
        let strict_lax =
            SameSiteCookieContext::with_schemeful(ContextType::SameSiteStrict, ContextType::SameSiteLax);
        let lax_cross =
            SameSiteCookieContext::with_schemeful(ContextType::SameSiteLax, ContextType::CrossSite);

        let metadata1 = ContextMetadata {
            cross_site_redirect_downgrade: ContextDowngradeType::StrictToLax,
            redirect_type_bug_1221316: ContextRedirectTypeBug1221316::PartialSameSiteRedirect,
            ..Default::default()
        };
        let metadata2 = metadata1;
        let context_with_metadata = SameSiteCookieContext::with_metadata(
            ContextType::SameSiteStrict,
            ContextType::SameSiteStrict,
            metadata1,
            metadata2,
        );

        assert_eq!(
            "{ context: 0, schemeful_context: 0, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            cross_cross.to_string()
        );
        assert_eq!(
            "{ context: 2, schemeful_context: 2, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            lax_lax.to_string()
        );
        assert_eq!(
            "{ context: 3, schemeful_context: 3, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            strict_strict.to_string()
        );
        assert_eq!(
            "{ context: 3, schemeful_context: 0, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            strict_cross.to_string()
        );
        assert_eq!(
            "{ context: 3, schemeful_context: 2, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            strict_lax.to_string()
        );
        assert_eq!(
            "{ context: 2, schemeful_context: 0, \
             metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 0, redirect_type_bug_1221316: 0 } }",
            lax_cross.to_string()
        );
        assert_eq!(
            "{ context: 3, schemeful_context: 3, \
             metadata: { cross_site_redirect_downgrade: 1, redirect_type_bug_1221316: 3 }, \
             schemeful_metadata: { cross_site_redirect_downgrade: 1, redirect_type_bug_1221316: 3 } }",
            context_with_metadata.to_string()
        );
    }

    #[test]
    fn enum_numeric_values_match_cpp() {
        assert_eq!(ContextDowngradeType::NoDowngrade as i32, 0);
        assert_eq!(ContextDowngradeType::StrictToLax as i32, 1);
        assert_eq!(ContextDowngradeType::StrictToCross as i32, 2);
        assert_eq!(ContextDowngradeType::LaxToCross as i32, 3);
        assert_eq!(ContextRedirectTypeBug1221316::Unset as i32, 0);
        assert_eq!(ContextRedirectTypeBug1221316::AllSameSiteRedirect as i32, 4);
        assert_eq!(HttpMethod::Unset as i32, -1);
        assert_eq!(HttpMethod::Unknown as i32, 0);
        assert_eq!(HttpMethod::Put as i32, 4);
        assert_eq!(HttpMethod::Patch as i32, 9);
    }

    #[test]
    fn default_context_is_cross_site() {
        let ctx = SameSiteCookieContext::default();
        assert_eq!(ctx.context(), ContextType::CrossSite);
        assert_eq!(ctx.schemeful_context(), ContextType::CrossSite);
        assert_eq!(*ctx.metadata(), ContextMetadata::default());
        assert_eq!(*ctx.schemeful_metadata(), ContextMetadata::default());
    }

    #[test]
    fn context_equality_ignores_metadata() {
        let plain = SameSiteCookieContext::new(ContextType::SameSiteLax);
        let mut with_meta = plain;
        with_meta.metadata_mut().redirect_type_bug_1221316 =
            ContextRedirectTypeBug1221316::NoRedirect;
        with_meta.schemeful_metadata_mut().has_null_initiator = true;
        assert_eq!(plain, with_meta);
        assert!(!plain.complete_equivalence_for_testing(&with_meta));
        assert!(plain.complete_equivalence_for_testing(&plain));
    }

    #[test]
    fn schemeful_mode_uses_schemeful_metadata() {
        let mut ctx = SameSiteCookieContext::with_schemeful(
            ContextType::SameSiteStrict,
            ContextType::SameSiteLax,
        );
        ctx.metadata_mut().cross_site_redirect_downgrade = ContextDowngradeType::LaxToCross;
        ctx.schemeful_metadata_mut().cross_site_redirect_downgrade = ContextDowngradeType::StrictToLax;
        assert_eq!(
            ctx.get_metadata_for_current_schemeful_mode().cross_site_redirect_downgrade,
            ContextDowngradeType::StrictToLax
        );
        assert_eq!(ctx.get_context_for_cookie_inclusion(), ContextType::SameSiteLax);
    }

    #[test]
    fn context_type_ordering_is_least_to_most_trusted() {
        assert!(ContextType::CrossSite < ContextType::SameSiteLaxMethodUnsafe);
        assert!(ContextType::SameSiteLaxMethodUnsafe < ContextType::SameSiteLax);
        assert!(ContextType::SameSiteLax < ContextType::SameSiteStrict);
    }

    #[test]
    fn set_context_types_for_testing_does_not_touch_metadata() {
        let mut ctx = SameSiteCookieContext::new(ContextType::CrossSite);
        ctx.metadata_mut().has_null_initiator = true;
        ctx.set_context_types_for_testing(ContextType::SameSiteStrict, ContextType::CrossSite);
        assert_eq!(ctx.context(), ContextType::SameSiteStrict);
        assert_eq!(ctx.schemeful_context(), ContextType::CrossSite);
        assert!(ctx.metadata().has_null_initiator);
    }

    #[test]
    fn make_inclusive_contexts() {
        assert_eq!(
            SameSiteCookieContext::make_inclusive().get_context_for_cookie_inclusion(),
            ContextType::SameSiteStrict
        );
        assert_eq!(
            SameSiteCookieContext::make_inclusive_for_set().get_context_for_cookie_inclusion(),
            ContextType::SameSiteLax
        );
    }

    #[test]
    fn cookie_options_defaults() {
        let options = CookieOptions::default();
        assert!(options.exclude_httponly());
        assert_eq!(
            options.same_site_cookie_context().context(),
            ContextType::CrossSite
        );
        assert!(options.update_access_time());
        assert!(!options.return_excluded_cookies());
        assert_eq!(CookieOptions::new(), options);
    }

    #[test]
    fn cookie_options_setters() {
        let mut options = CookieOptions::default();
        options.set_include_httponly();
        assert!(!options.exclude_httponly());
        options.set_exclude_httponly();
        assert!(options.exclude_httponly());

        options.set_do_not_update_access_time();
        assert!(!options.update_access_time());
        options.set_update_access_time();
        assert!(options.update_access_time());

        options.set_return_excluded_cookies();
        assert!(options.return_excluded_cookies());
        options.unset_return_excluded_cookies();
        assert!(!options.return_excluded_cookies());

        options.set_same_site_cookie_context(SameSiteCookieContext::make_inclusive());
        assert_eq!(
            options.same_site_cookie_context().context(),
            ContextType::SameSiteStrict
        );
    }

    #[test]
    fn make_all_inclusive_is_opposite_of_default() {
        let options = CookieOptions::make_all_inclusive();
        assert!(!options.exclude_httponly());
        assert!(!options.update_access_time());
        assert_eq!(
            options.same_site_cookie_context().get_context_for_cookie_inclusion(),
            ContextType::SameSiteStrict
        );
        assert!(!options.return_excluded_cookies());
    }
}
