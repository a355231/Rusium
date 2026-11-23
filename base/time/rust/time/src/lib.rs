// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of the platform-independent parts of `base::Time`
//! (base/time/time.{h,cc}), `base::TimeDelta`, `base::TimeTicks` and
//! `base::ThreadTicks`, plus `TimeDeltaFromString`
//! (base/time/time_delta_from_string.{h,cc}).
//!
//! Types:
//! - [`TimeDelta`]: a signed duration stored in microseconds, with the C++
//!   saturating arithmetic, the `Max()`/`Min()` infinities and the rounding
//!   helpers. The unit constructors are the free functions [`days`],
//!   [`hours`], [`minutes`], [`seconds`], [`milliseconds`], [`microseconds`],
//!   [`nanoseconds`] and [`hertz`].
//! - [`Time`]: wall-clock time in UTC, stored in microseconds since the
//!   Windows epoch (1601-01-01), with [`Exploded`] calendar fields.
//! - [`TimeTicks`] and [`ThreadTicks`]: arithmetic value types. They have no
//!   clock of their own.
//! - [`time_delta_from_string`]: the Go-style duration parser.
//!
//! Differences from the C++ code:
//! - No clock reads. `Time::Now()`, `TimeTicks::Now()`, `ThreadTicks::Now()`,
//!   `LowResolutionNow()`, `NowFromSystemTime()`, the `*NowIgnoringOverride`
//!   functions, the override hooks and `SampleLiveAndRealTicks()` are not
//!   ported. Callers pass in the timestamp they read from their own clock.
//! - No `LiveTicks` or `RealTicks` types.
//! - Unit constructors are free functions with snake_case names. They are
//!   generic over [`Scalar`] (`i32`, `i64`, `f32`, `f64`) instead of C++
//!   templates. Their results are not `const`, so `constexpr` use in C++
//!   becomes runtime use here.
//! - `Exploded`-returning functions return values, not out-parameters.
//!   Conversions that fail return `Option` instead of `bool` plus an out
//!   parameter. C++ sets the out parameter to `Time(0)` on failure, and
//!   `Option::None` is the Rust equivalent.
//! - C++ `CHECK` failures (for example, adding infinities of opposite sign)
//!   are `assert!` panics. C++ `DCHECK`s are not reproduced.
//! - Integer overflow in `int` values is not undefined behavior here: it is
//!   handled explicitly, as the C++ code does with `CheckedNumeric`.
//! - `operator<<` is `Display`. The output strings are the same
//!   (`"0 s"`, `"0.5 s"`, `"0 bogo-microseconds"`, and the
//!   `"YYYY-MM-DD HH:MM:SS.uuuuuu UTC"` form for `Time`). Output is formatted
//!   the same way as `std::ostream` uses `%g` with 6 significant digits for
//!   `double`. Stream flags and stream state have no Rust equivalent.
//!
//! Time zones:
//! - `Time::LocalExplode`, `FromLocalExploded`, `LocalMidnight`,
//!   `LocalMidnightWithOffset` and `NextLocalMidnight` are not ported. They need
//!   a time-zone database, and the C++ code gets one from `mktime` and
//!   `localtime_r`.
//! - `Time::FromString` takes a caller-supplied zone, as `&dyn Fn(i64) -> i64`
//!   (the UTC offset in seconds, east positive, at a UTC instant). The
//!   prtime crate uses the same convention. [`Time::from_utc_string`] assumes
//!   UTC for strings with no zone.
//!
//! Platform-specific pieces that are not ported:
//! - Windows `FILETIME`, WinRT, QPC, `TSCTicksPerSecond`, and high-resolution
//!   timer functions.
//! - Apple `CFAbsoluteTime`, `NSDate`, Mach time.
//! - Fuchsia `zx_time_t`, `zx_duration_t`.
//! - Android uptime and `nanoTime` conversions.
//! - `TimeDelta::ToTimeSpec` and `FromTimeSpec`, and
//!   `Time::FromTimeVal` and `ToTimeVal`, are ported on the POSIX layout as
//!   `(seconds, subsecond)` pairs of `i64` (Linux 64-bit `time_t` and `long`).
//! - ICU-based explode (`ExplodeUsingIcu`) is not ported. It is only used
//!   where `time_t` is 32 bits, and this port always uses 64-bit arithmetic.
//! - `Time::kExplodedMinYear` and `kExplodedMaxYear` are fixed to the Linux
//!   64-bit values, `i32::MIN` and `i32::MAX`.
//! - `EstimateMemoryUsage` and `LocalTimeT`-style tests that depend on the host
//!   zone are not applicable.
//!
//! Explode: the C++ code calls `gmtime_r` and `timegm`. This crate does the
//! same calendar arithmetic with the proleptic Gregorian algorithms in
//! `calendar.rs`, so it covers all `i64` microsecond values (years about
//! -292,277 to 294,276) and never fails, where `gmtime_r` would return
//! `NULL`. `from_utc_exploded` still returns `None` in the same cases as the
//! C++ code: out-of-range fields, microsecond overflow, and days that do not
//! exist (round-trip check).

#![forbid(unsafe_code)]

// Stringification of time types and the internal `Time`/`TimeTicks` base
// operations, shared by the modules below. This macro is defined before the
// modules so that they can use it.
macro_rules! impl_time_base {
    ($name:ident) => {
        impl $name {
            /// Creates a value from the raw microsecond count. Prefer the
            /// typed constructors. See the C++ `FromInternalValue`.
            pub const fn from_internal_value(us: i64) -> Self {
                Self { us }
            }

            /// Returns the raw microsecond count.
            pub const fn to_internal_value(self) -> i64 {
                self.us
            }

            /// The largest value. Adding a finite `TimeDelta` saturates here.
            pub const fn max() -> Self {
                Self { us: i64::MAX }
            }

            /// The smallest value. Adding a finite `TimeDelta` saturates here.
            pub const fn min() -> Self {
                Self { us: i64::MIN }
            }

            /// Returns true for the zero value, which is also the
            /// "not initialized" value for `Time`.
            pub const fn is_null(self) -> bool {
                self.us == 0
            }

            /// Returns true if this is `max()`.
            pub const fn is_max(self) -> bool {
                self.us == i64::MAX
            }

            /// Returns true if this is `min()`.
            pub const fn is_min(self) -> bool {
                self.us == i64::MIN
            }

            /// Returns true if this is `min()` or `max()`.
            pub const fn is_inf(self) -> bool {
                self.is_min() || self.is_max()
            }

            /// Returns the time since the origin (zero) of this type.
            pub const fn since_origin(self) -> $crate::TimeDelta {
                $crate::TimeDelta::from_internal_value(self.us)
            }

            /// Returns `self` snapped to the next tick of a repeating interval
            /// that starts at `tick_phase`. `self` may be before, after or
            /// equal to `tick_phase`.
            pub fn snapped_to_next_tick(
                self,
                tick_phase: Self,
                tick_interval: $crate::TimeDelta,
            ) -> Self {
                let mut interval_offset = (tick_phase - self) % tick_interval;
                if !interval_offset.is_zero() && tick_phase < self {
                    interval_offset += tick_interval;
                }
                self + interval_offset
            }
        }

        impl ::core::ops::Sub for $name {
            type Output = $crate::TimeDelta;

            /// Saturates at the ends of `TimeDelta`.
            fn sub(self, other: Self) -> $crate::TimeDelta {
                $crate::TimeDelta::from_internal_value(self.us.saturating_sub(other.us))
            }
        }

        impl ::core::ops::Add<$crate::TimeDelta> for $name {
            type Output = Self;

            fn add(self, delta: $crate::TimeDelta) -> Self {
                Self {
                    us: ($crate::TimeDelta::from_internal_value(self.us) + delta).to_internal_value(),
                }
            }
        }

        impl ::core::ops::Sub<$crate::TimeDelta> for $name {
            type Output = Self;

            fn sub(self, delta: $crate::TimeDelta) -> Self {
                Self {
                    us: ($crate::TimeDelta::from_internal_value(self.us) - delta).to_internal_value(),
                }
            }
        }

        impl ::core::ops::AddAssign<$crate::TimeDelta> for $name {
            fn add_assign(&mut self, delta: $crate::TimeDelta) {
                *self = *self + delta;
            }
        }

        impl ::core::ops::SubAssign<$crate::TimeDelta> for $name {
            fn sub_assign(&mut self, delta: $crate::TimeDelta) {
                *self = *self - delta;
            }
        }
    };
}

mod calendar;
mod delta;
mod delta_from_string;
mod format;
mod ticks;
mod time;

pub use delta::{
    days, hertz, hours, microseconds, milliseconds, minutes, nanoseconds, seconds, Scalar,
    TimeDelta, MICROSECONDS_PER_DAY, MICROSECONDS_PER_HOUR, MICROSECONDS_PER_MILLISECOND,
    MICROSECONDS_PER_MINUTE, MICROSECONDS_PER_SECOND, MICROSECONDS_PER_WEEK,
    MILLISECONDS_PER_DAY, MILLISECONDS_PER_SECOND, NANOSECONDS_PER_MICROSECOND,
    NANOSECONDS_PER_MILLISECOND, NANOSECONDS_PER_SECOND,
};
pub use delta_from_string::time_delta_from_string;
pub use ticks::{ThreadTicks, TimeTicks};
pub use time::{Exploded, Time, MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH};

#[cfg(test)]
mod tests;
