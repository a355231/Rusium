// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Reduced stand-in for `net::CookieInclusionStatus`
//! (net/cookies/cookie_inclusion_status.{h,cc}).
//!
//! Only the reasons that the parsed-cookie unit emits are present. The C++
//! class has about 40 exclusion reasons, a SameSite/third-party-phaseout
//! warning model, exemption reasons and `GetDebugString()`. None of those
//! are ported. Each reason is one bit, in the same order as the C++ shim
//! used to generate the golden data.

/// Exclusion reasons emitted by `ParsedCookie`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionReason {
    /// `EXCLUDE_NAME_VALUE_PAIR_EXCEEDS_MAX_SIZE`.
    NameValuePairExceedsMaxSize = 0,
    /// `EXCLUDE_DISALLOWED_CHARACTER`.
    DisallowedCharacter = 1,
    /// `EXCLUDE_NO_COOKIE_CONTENT`.
    NoCookieContent = 2,
}

/// Warning reasons emitted by `ParsedCookie`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningReason {
    /// `WARN_ATTRIBUTE_VALUE_EXCEEDS_MAX_SIZE`.
    AttributeValueExceedsMaxSize = 0,
}

/// Records why a cookie line was excluded, and any warnings about it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CookieInclusionStatus {
    exclusion_reasons: u32,
    warning_reasons: u32,
}

impl CookieInclusionStatus {
    /// Creates a status with no reasons (`IsInclude()` is `true`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if no exclusion reason has been recorded.
    pub fn is_include(&self) -> bool {
        self.exclusion_reasons == 0
    }

    /// Returns `true` if `reason` has been recorded.
    pub fn has_exclusion_reason(&self, reason: ExclusionReason) -> bool {
        self.exclusion_reasons & Self::bit(reason as u32) != 0
    }

    /// Returns `true` if `reason` is the only exclusion reason recorded.
    pub fn has_only_exclusion_reason(&self, reason: ExclusionReason) -> bool {
        self.exclusion_reasons == Self::bit(reason as u32)
    }

    /// Records an exclusion reason.
    pub fn add_exclusion_reason(&mut self, reason: ExclusionReason) {
        self.exclusion_reasons |= Self::bit(reason as u32);
    }

    /// Returns `true` if `reason` has been recorded.
    pub fn has_warning_reason(&self, reason: WarningReason) -> bool {
        self.warning_reasons & Self::bit(reason as u32) != 0
    }

    /// Records a warning reason.
    pub fn add_warning_reason(&mut self, reason: WarningReason) {
        self.warning_reasons |= Self::bit(reason as u32);
    }

    /// The raw exclusion bitmask, for diagnostics and golden tests.
    pub fn exclusion_bits(&self) -> u32 {
        self.exclusion_reasons
    }

    /// The raw warning bitmask, for diagnostics and golden tests.
    pub fn warning_bits(&self) -> u32 {
        self.warning_reasons
    }

    fn bit(index: u32) -> u32 {
        1 << index
    }
}
