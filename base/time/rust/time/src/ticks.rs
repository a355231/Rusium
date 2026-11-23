// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `TimeTicks` and `ThreadTicks`: arithmetic value types for monotonic and
//! per-thread CPU clocks. Ports of `base::TimeTicks` and `base::ThreadTicks`.
//! The clocks themselves are not ported: callers supply the values.

use std::fmt;

/// A monotonically non-decreasing clock reading, in microseconds from an
/// origin that may differ between runs. Port of `base::TimeTicks`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeTicks {
    us: i64,
}

impl_time_base!(TimeTicks);

impl fmt::Display for TimeTicks {
    /// Matches C++ `operator<<`: the raw count followed by
    /// " bogo-microseconds".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bogo-microseconds", self.us)
    }
}

/// CPU time consumed by the current thread, in microseconds. Port of
/// `base::ThreadTicks`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreadTicks {
    us: i64,
}

impl_time_base!(ThreadTicks);

impl fmt::Display for ThreadTicks {
    /// Matches C++ `operator<<`: the raw count followed by
    /// " bogo-thread-microseconds".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} bogo-thread-microseconds", self.us)
    }
}
