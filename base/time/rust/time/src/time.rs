// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `Time`: wall-clock time in UTC. Port of `base::Time` (base/time/time.h,
//! time.cc and time_exploded_posix.cc, UTC paths only).

use std::fmt;

use crate::calendar::{civil_from_days, days_from_civil};
use crate::delta::{milliseconds, seconds, MICROSECONDS_PER_SECOND, NANOSECONDS_PER_SECOND};
use crate::TimeDelta;

/// Offset of the Unix epoch (1970-01-01 00:00:00 UTC) from the Windows epoch
/// (1601-01-01 00:00:00 UTC), in microseconds. Port of
/// `Time::kMicrosecondsFromWindowsToUnixEpoch`.
pub const MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH: i64 = 11_644_473_600_000_000;

/// Calendar fields of a `Time`. Port of `Time::Exploded`.
///
/// The fields are in the same ranges as in C++: `day_of_week` is 0 (Sunday)
/// to 6, `month` is 1 to 12, `day_of_month` is 1 to 31, and `second` is 0 to
/// 60 (60 is allowed for leap seconds). See [`Exploded::has_valid_values`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Exploded {
    /// Four digit year, for example 2007.
    pub year: i32,
    /// 1-based month (1 = January).
    pub month: i32,
    /// 0-based day of week (0 = Sunday).
    pub day_of_week: i32,
    /// 1-based day of month.
    pub day_of_month: i32,
    /// Hour of day, 0 to 23.
    pub hour: i32,
    /// Minute of hour, 0 to 59.
    pub minute: i32,
    /// Second of minute, 0 to 59, or 60 for a leap second.
    pub second: i32,
    /// Millisecond of second, 0 to 999.
    pub millisecond: i32,
}

impl Exploded {
    /// Returns true if every field is in its documented range. A true result
    /// does not guarantee that the value converts back to a `Time`. Port of
    /// `HasValidValues`.
    pub fn has_valid_values(&self) -> bool {
        (1..=12).contains(&self.month)
            && (0..=6).contains(&self.day_of_week)
            && (1..=31).contains(&self.day_of_month)
            && (0..=23).contains(&self.hour)
            && (0..=59).contains(&self.minute)
            && (0..=60).contains(&self.second)
            && (0..=999).contains(&self.millisecond)
    }
}

/// Wall-clock time in UTC, stored in microseconds since 1601-01-01 UTC.
///
/// Values are not guaranteed to be monotonic. The OS may adjust the clock.
/// Use [`TimeTicks`](crate::TimeTicks) for measuring durations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    pub(crate) us: i64,
}

impl Time {
    /// Returns the Unix epoch, 1970-01-01 00:00:00 UTC.
    pub const fn unix_epoch() -> Self {
        Self {
            us: MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH,
        }
    }

    /// Creates a time from a delta since 1601-01-01 UTC. Port of
    /// `FromDeltaSinceWindowsEpoch`.
    pub fn from_delta_since_windows_epoch(delta: TimeDelta) -> Self {
        Self {
            us: delta.in_microseconds(),
        }
    }

    /// Returns the delta since 1601-01-01 UTC. Port of
    /// `ToDeltaSinceWindowsEpoch`.
    pub fn to_delta_since_windows_epoch(self) -> TimeDelta {
        TimeDelta::from_internal_value(self.us)
    }

    /// Converts from `time_t` (seconds since the Unix epoch). `0` is the null
    /// time and `i64::MAX` is `max()`. Port of `FromTimeT`.
    pub fn from_time_t(tt: i64) -> Self {
        if tt == 0 {
            return Self::default();
        }
        if tt == i64::MAX {
            return Self::max();
        }
        Self::unix_epoch() + seconds(tt)
    }

    /// Converts to `time_t` (seconds since the Unix epoch, rounded toward
    /// negative infinity). The null time gives 0, and the infinities give the
    /// `time_t` extremes. Port of `ToTimeT`.
    pub fn to_time_t(self) -> i64 {
        if self.is_null() {
            return 0;
        }
        if !self.is_inf() {
            return (self - Self::unix_epoch()).in_seconds_floored();
        }
        if self.us < 0 {
            i64::MIN
        } else {
            i64::MAX
        }
    }

    /// Creates a time from seconds since the Unix epoch. `0` and NaN give the
    /// null time. Port of `FromSecondsSinceUnixEpoch`.
    pub fn from_seconds_since_unix_epoch(dt: f64) -> Self {
        if dt == 0.0 || dt.is_nan() {
            return Self::default();
        }
        Self::unix_epoch() + seconds(dt)
    }

    /// Returns seconds since the Unix epoch as `f64`. The null time gives 0,
    /// and the infinities give `±inf`. Port of `InSecondsFSinceUnixEpoch`.
    pub fn in_seconds_f_since_unix_epoch(self) -> f64 {
        if self.is_null() {
            return 0.0;
        }
        if !self.is_inf() {
            return (self - Self::unix_epoch()).in_seconds_f();
        }
        if self.us < 0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    }

    /// Creates a time from a POSIX `timespec` value (`tv_sec`, `tv_nsec`).
    /// Port of `FromTimeSpec`.
    pub fn from_time_spec(tv_sec: i64, tv_nsec: i64) -> Self {
        Self::from_seconds_since_unix_epoch(
            tv_sec as f64 + (tv_nsec as f64) / NANOSECONDS_PER_SECOND as f64,
        )
    }

    /// Creates a time from a POSIX `timeval` value (`tv_sec`, `tv_usec`).
    /// `(i64::MAX, 999_999)` gives `max()`. Port of `FromTimeVal`.
    pub fn from_time_val(tv_sec: i64, tv_usec: i64) -> Self {
        if tv_usec == MICROSECONDS_PER_SECOND - 1 && tv_sec == i64::MAX {
            return Self::max();
        }
        Self::unix_epoch() + seconds(tv_sec) + microseconds_i64(tv_usec)
    }

    /// Returns `(tv_sec, tv_usec)` for a POSIX `timeval`. The null time gives
    /// `(0, 0)`, and `max()` gives `(i64::MAX, 999_999)`. Port of
    /// `ToTimeVal`.
    pub fn to_time_val(self) -> (i64, i64) {
        if self.is_null() {
            return (0, 0);
        }
        let us = self.us.saturating_sub(MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH);
        if self.is_max() {
            return (i64::MAX, MICROSECONDS_PER_SECOND - 1);
        }
        (us / MICROSECONDS_PER_SECOND, us % MICROSECONDS_PER_SECOND)
    }

    /// Creates a time from milliseconds since the Unix epoch. Port of
    /// `FromMillisecondsSinceUnixEpoch` for integers. The integer form does
    /// not treat 0 as null.
    pub fn from_milliseconds_since_unix_epoch(ms: i64) -> Self {
        Self::unix_epoch() + milliseconds(ms)
    }

    /// Creates a time from fractional milliseconds since the Unix epoch. NaN
    /// gives the null time. Port of `FromMillisecondsSinceUnixEpoch(double)`.
    pub fn from_fractional_milliseconds_since_unix_epoch(ms: f64) -> Self {
        if ms.is_nan() {
            return Self::default();
        }
        Self::unix_epoch() + milliseconds(ms)
    }

    /// Returns milliseconds since the Unix epoch, rounded toward negative
    /// infinity. The null time gives 0. Port of `InMillisecondsSinceUnixEpoch`.
    pub fn in_milliseconds_since_unix_epoch(self) -> i64 {
        if self.is_null() {
            return 0;
        }
        if !self.is_inf() {
            return (self - Self::unix_epoch()).in_milliseconds();
        }
        if self.us < 0 {
            i64::MIN
        } else {
            i64::MAX
        }
    }

    /// Returns milliseconds since the Unix epoch as `f64`. The null time gives
    /// 0. Port of `InMillisecondsFSinceUnixEpoch`.
    pub fn in_milliseconds_f_since_unix_epoch(self) -> f64 {
        if self.is_null() {
            0.0
        } else {
            self.in_milliseconds_f_since_unix_epoch_ignoring_null()
        }
    }

    /// Returns milliseconds since the Unix epoch as `f64`, without special
    /// handling for the null time. Port of
    /// `InMillisecondsFSinceUnixEpochIgnoringNull`.
    pub fn in_milliseconds_f_since_unix_epoch_ignoring_null(self) -> f64 {
        if !self.is_inf() {
            return (self - Self::unix_epoch()).in_milliseconds_f();
        }
        if self.us < 0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    }

    /// Returns the value in milliseconds since the Unix epoch. Rounds
    /// microseconds toward negative infinity. Port of
    /// `ToRoundedDownMillisecondsSinceUnixEpoch`.
    pub(crate) fn to_rounded_down_milliseconds_since_unix_epoch(self) -> i64 {
        self.us.div_euclid(1000) - MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH / 1000
    }

    /// Converts a `Time` to calendar fields in UTC. Port of `UTCExplode`.
    ///
    /// Every `Time` converts, including `min()` and `max()`. The C++ code
    /// relies on `gmtime_r`, which fails outside the range of `int` years.
    pub fn utc_explode(self) -> Exploded {
        let millis = self.to_rounded_down_milliseconds_since_unix_epoch();
        let secs = millis.div_euclid(1000);
        let millisecond = millis.rem_euclid(1000);
        let days = secs.div_euclid(86_400);
        let second_of_day = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Exploded {
            year: year as i32,
            month: month as i32,
            day_of_week: (days + 4).rem_euclid(7) as i32,
            day_of_month: day as i32,
            hour: (second_of_day / 3600) as i32,
            minute: ((second_of_day % 3600) / 60) as i32,
            second: (second_of_day % 60) as i32,
            millisecond: millisecond as i32,
        }
    }

    /// Converts UTC calendar fields to a `Time`. Port of `FromUTCExploded`.
    ///
    /// Returns `None` if `month - 1` or `year - 1900` overflows `int`, if the
    /// result overflows the microsecond range, or if the fields do not
    /// round-trip. Fields outside their ranges are normalized first, as
    /// `timegm` does, so for example February 31 becomes March 3. The
    /// normalized time then differs from the input, and the round-trip check
    /// fails.
    pub fn from_utc_exploded(exploded: &Exploded) -> Option<Self> {
        // CheckedNumeric<int> in the C++ code: month - 1 and year - 1900 must
        // fit in int.
        if exploded.month == i32::MIN {
            return None;
        }
        if i64::from(exploded.year) - 1900 < i64::from(i32::MIN) {
            return None;
        }

        // Normalize the month as timegm does. Then add the day, hour, minute
        // and second linearly, which is what timegm's normalization computes.
        let month0 = i64::from(exploded.month) - 1;
        let year = i64::from(exploded.year) + month0.div_euclid(12);
        let month = month0.rem_euclid(12) + 1;
        let days = days_from_civil(year, month, 1) + i64::from(exploded.day_of_month) - 1;
        let seconds_since_epoch = days * 86_400
            + i64::from(exploded.hour) * 3600
            + i64::from(exploded.minute) * 60
            + i64::from(exploded.second);

        // A result of -1 is ambiguous. In the C++ code it is an error unless
        // the year is 1969 or 1970, so the year is clamped to the range of
        // int32 seconds. 64-bit time_t makes this the only way to get -1.
        let milliseconds_since_unix_epoch = if seconds_since_epoch == -1
            && (exploded.year < 1969 || exploded.year > 1970)
        {
            if exploded.year < 1969 {
                i64::from(i32::MIN) * 1000
            } else {
                i64::from(i32::MAX) * 1000 + 999
            }
        } else {
            seconds_since_epoch
                .checked_mul(1000)?
                .checked_add(i64::from(exploded.millisecond))?
        };

        let us = milliseconds_since_unix_epoch
            .checked_mul(1000)?
            .checked_add(MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH)?;
        let converted = Self { us };

        // Round-trip check. February 31 normalizes to March 3, for example,
        // so the fields differ and the conversion fails. day_of_week is
        // ignored.
        let back = converted.utc_explode();
        if exploded_mostly_equal(&back, exploded) {
            Some(converted)
        } else {
            None
        }
    }

    /// Rounds down to midnight UTC. Port of `UTCMidnight`.
    ///
    /// C++ falls back to 01:00 when midnight fails to convert, because that
    /// case is only expected for local time. The same fallback is kept here.
    pub fn utc_midnight(self) -> Self {
        let mut exploded = self.utc_explode();
        exploded.hour = 0;
        exploded.minute = 0;
        exploded.second = 0;
        exploded.millisecond = 0;
        if let Some(midnight) = Self::from_utc_exploded(&exploded) {
            return midnight;
        }
        exploded.hour = 1;
        Self::from_utc_exploded(&exploded).unwrap_or_default()
    }

    /// Parses a time string. Time zones are taken from the string when it has
    /// one. Otherwise the local zone given by `offset_at_utc` is used, which
    /// returns the offset in seconds (east positive) at a UTC instant. Port of
    /// `FromString`.
    pub fn from_string(time_string: &str, offset_at_utc: &dyn Fn(i64) -> i64) -> Option<Self> {
        parse_time(time_string, false, offset_at_utc)
    }

    /// Parses a time string. Strings with no zone are read as UTC. Port of
    /// `FromUTCString`.
    pub fn from_utc_string(time_string: &str) -> Option<Self> {
        parse_time(time_string, true, &|_| 0)
    }

    /// Writes `YYYY-MM-DD HH:MM:SS.uuuuuu UTC`, as C++ `operator<<` does.
    fn write_utc(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let e = self.utc_explode();
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06} UTC",
            e.year,
            e.month,
            e.day_of_month,
            e.hour,
            e.minute,
            e.second,
            self.us % MICROSECONDS_PER_SECOND
        )
    }
}

impl_time_base!(Time);

impl fmt::Display for Time {
    /// Matches C++ `operator<<` for `Time`:
    /// `YYYY-MM-DD HH:MM:SS.uuuuuu UTC`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_utc(f)
    }
}

/// Returns true if the fields match. The day of week is ignored, as in the
/// C++ `ExplodedMostlyEquals`.
fn exploded_mostly_equal(lhs: &Exploded, rhs: &Exploded) -> bool {
    lhs.year == rhs.year
        && lhs.month == rhs.month
        && lhs.day_of_month == rhs.day_of_month
        && lhs.hour == rhs.hour
        && lhs.minute == rhs.minute
        && lhs.second == rhs.second
        && lhs.millisecond == rhs.millisecond
}

/// Shared implementation of `FromStringInternal`.
fn parse_time(time_string: &str, default_to_gmt: bool, offset_at_utc: &dyn Fn(i64) -> i64) -> Option<Time> {
    // The C++ code rejects the empty string before calling PR_ParseTimeString.
    if time_string.is_empty() {
        return None;
    }
    let result_time = prtime::parse_time_string_with_local_offset(
        time_string.as_bytes(),
        default_to_gmt,
        offset_at_utc,
    )?;
    Some(Time::unix_epoch() + microseconds_i64(result_time))
}
