// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Minimal stand-ins for `base::Time` and `base::TimeDelta`, covering only what
//! `http_util` needs: UTC calendar conversion, HTTP-date parsing and
//! microsecond arithmetic.

use std::ops::Sub;

const MICROS_PER_SECOND: i64 = 1_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// A signed duration with microsecond resolution, like `base::TimeDelta`.
/// Arithmetic saturates instead of overflowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TimeDelta {
    micros: i64,
}

impl TimeDelta {
    /// The zero-length delta.
    pub const ZERO: TimeDelta = TimeDelta { micros: 0 };

    /// Creates a delta of `seconds` seconds, saturating on overflow.
    pub const fn seconds(seconds: i64) -> TimeDelta {
        TimeDelta { micros: seconds.saturating_mul(MICROS_PER_SECOND) }
    }

    /// Creates a delta from a raw microsecond count.
    pub const fn from_micros(micros: i64) -> TimeDelta {
        TimeDelta { micros }
    }

    /// Returns the raw microsecond count.
    pub const fn as_micros(self) -> i64 {
        self.micros
    }

    /// Returns the whole number of seconds, truncated toward zero, like
    /// `base::TimeDelta::InSeconds()`.
    pub const fn in_seconds(self) -> i64 {
        self.micros / MICROS_PER_SECOND
    }
}

/// A point in time, stored as microseconds since the Unix epoch (UTC). This is
/// a simplified `base::Time`: there is no local time zone and no clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    micros: i64,
}

/// Broken-down UTC time, like `base::Time::Exploded`. Weekdays are 0 for
/// Sunday through 6 for Saturday. Months are 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Exploded {
    pub year: i64,
    pub month: u32,
    pub day_of_week: u32,
    pub day_of_month: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}

impl Sub for Time {
    type Output = TimeDelta;

    fn sub(self, rhs: Time) -> TimeDelta {
        TimeDelta { micros: self.micros.saturating_sub(rhs.micros) }
    }
}

const MONTH_FULL_NAMES: [&[u8]; 12] = [
    b"january",
    b"february",
    b"march",
    b"april",
    b"may",
    b"june",
    b"july",
    b"august",
    b"september",
    b"october",
    b"november",
    b"december",
];
const WEEKDAY_FULL_NAMES: [&[u8]; 7] =
    [b"sunday", b"monday", b"tuesday", b"wednesday", b"thursday", b"friday", b"saturday"];

/// Returns true if `year` is a Gregorian leap year.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        _ => 28,
    }
}

/// Days from 1970-01-01 to the given proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`). `month` is 1-based.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of `days_from_civil`. Returns (year, 1-based month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl Time {
    /// The Unix epoch, 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Time = Time { micros: 0 };

    /// Creates a time from microseconds since the Unix epoch.
    pub const fn from_unix_micros(micros: i64) -> Time {
        Time { micros }
    }

    /// Returns microseconds since the Unix epoch.
    pub const fn unix_micros(self) -> i64 {
        self.micros
    }

    /// Adds a delta, saturating on overflow.
    pub fn saturating_add(self, delta: TimeDelta) -> Time {
        Time { micros: self.micros.saturating_add(delta.micros) }
    }

    /// Builds a UTC time from broken-down fields. Returns `None` if any field
    /// is out of range. Equivalent to `base::Time::FromUTCExploded`.
    pub fn from_utc_exploded(exploded: &Exploded) -> Option<Time> {
        if !(1..=12).contains(&exploded.month)
            || exploded.day_of_month == 0
            || exploded.day_of_month > days_in_month(exploded.year, exploded.month)
            || exploded.hour > 23
            || exploded.minute > 59
            || exploded.second > 59
            || exploded.millisecond > 999
        {
            return None;
        }
        let days = days_from_civil(exploded.year, exploded.month, exploded.day_of_month);
        let seconds = days * SECONDS_PER_DAY
            + exploded.hour as i64 * 3600
            + exploded.minute as i64 * 60
            + exploded.second as i64;
        let micros = seconds
            .checked_mul(MICROS_PER_SECOND)?
            .checked_add(exploded.millisecond as i64 * 1000)?;
        Some(Time { micros })
    }

    /// Breaks the time down into UTC fields. Equivalent to
    /// `base::Time::UTCExplode`.
    pub fn to_utc_exploded(self) -> Exploded {
        let days = self.micros.div_euclid(SECONDS_PER_DAY * MICROS_PER_SECOND);
        let day_micros = self.micros.rem_euclid(SECONDS_PER_DAY * MICROS_PER_SECOND);
        let (year, month, day_of_month) = civil_from_days(days);
        let secs_of_day = day_micros / MICROS_PER_SECOND;
        Exploded {
            year,
            month,
            day_of_week: (days + 4).rem_euclid(7) as u32,
            day_of_month,
            hour: (secs_of_day / 3600) as u32,
            minute: ((secs_of_day / 60) % 60) as u32,
            second: (secs_of_day % 60) as u32,
            millisecond: ((day_micros % MICROS_PER_SECOND) / 1000) as u32,
        }
    }

    /// Parses an HTTP-date-like string as UTC. See `parse_date` for the
    /// accepted forms. Equivalent in spirit to `base::Time::FromUTCString`,
    /// with a narrower grammar.
    pub fn from_utc_string(text: &[u8]) -> Option<Time> {
        parse_date(text)
    }

    /// Parses a date string. The C++ `base::Time::FromString` interprets
    /// zone-less input as local time. This port has no local time zone, so the
    /// result is the same as `from_utc_string`.
    pub fn from_string(text: &[u8]) -> Option<Time> {
        parse_date(text)
    }
}

/// Parses a date in the forms used by HTTP (RFC 1123, RFC 850 and asctime),
/// plus other loose variants with the same tokens:
///
/// - tokens are separated by ASCII whitespace, ',' or '-';
/// - a token containing ':' is a time of day `hh:mm[:ss]`;
/// - an all-digit token is a number; of the two numbers, one is the day and
///   one is the year. A number with 3 or more digits is the year;
/// - a 1 or 2 digit year is 19xx for 50..=99 and 20xx for 0..=49;
/// - an alphabetic token starting with a month name (at least 3 letters, a
///   prefix of the full name) sets the month. Weekday names are ignored;
/// - "am"/"pm" adjust the hour, "gmt", "utc", "ut" and "z" are ignored and the
///   result is always UTC.
///
/// A missing time of day means midnight. Anything else fails.
fn parse_date(text: &[u8]) -> Option<Time> {
    let mut month: Option<u32> = None;
    let mut numbers: Vec<(i64, usize)> = Vec::new();
    let mut time_of_day: Option<(u32, u32, u32)> = None;
    let mut pm: Option<bool> = None;

    for token in
        text.split(|&c| c.is_ascii_whitespace() || c == b',' || c == b'-').filter(|t| !t.is_empty())
    {
        if token.contains(&b':') {
            if time_of_day.is_some() {
                return None;
            }
            time_of_day = Some(parse_time_of_day(token)?);
        } else if token.iter().all(u8::is_ascii_digit) {
            if token.len() > 9 {
                return None;
            }
            let value = token.iter().fold(0i64, |acc, &c| acc * 10 + (c - b'0') as i64);
            numbers.push((value, token.len()));
        } else if token.iter().all(u8::is_ascii_alphabetic) {
            let lower = token.to_ascii_lowercase();
            if lower == b"am" || lower == b"pm" {
                if pm.is_some() {
                    return None;
                }
                pm = Some(lower == b"pm");
            } else if matches_name(&lower, &MONTH_FULL_NAMES) {
                if month.is_some() {
                    return None;
                }
                let index = MONTH_FULL_NAMES.iter().position(|full| full.starts_with(&lower))?;
                month = Some(index as u32 + 1);
            } else if matches_name(&lower, &WEEKDAY_FULL_NAMES) {
                // Weekday names are accepted and ignored.
            } else if matches!(lower.as_slice(), b"gmt" | b"utc" | b"ut" | b"z") {
                // Zone names are accepted and ignored: the result is UTC.
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    let month = month?;
    if numbers.len() != 2 {
        return None;
    }
    let ((year, year_digits), (day, _)) =
        if numbers[0].1 >= 3 { (numbers[0], numbers[1]) } else { (numbers[1], numbers[0]) };
    let year = if year_digits <= 2 {
        if year < 50 {
            2000 + year
        } else {
            1900 + year
        }
    } else {
        year
    };

    let (mut hour, minute, second) = time_of_day.unwrap_or((0, 0, 0));
    if let Some(is_pm) = pm {
        if !(1..=12).contains(&hour) {
            return None;
        }
        if is_pm && hour < 12 {
            hour += 12;
        } else if !is_pm && hour == 12 {
            hour = 0;
        }
    }

    Time::from_utc_exploded(&Exploded {
        year,
        month,
        day_of_week: 0,
        day_of_month: u32::try_from(day).ok()?,
        hour,
        minute,
        second,
        millisecond: 0,
    })
}

/// True if `lower` is a prefix of one of `names` and has at least 3 letters.
fn matches_name(lower: &[u8], names: &[&[u8]]) -> bool {
    lower.len() >= 3 && names.iter().any(|full| full.starts_with(lower))
}

/// Parses `hh:mm` or `hh:mm:ss` into (hour, minute, second).
fn parse_time_of_day(token: &[u8]) -> Option<(u32, u32, u32)> {
    let parts: Vec<&[u8]> = token.split(|&c| c == b':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut values = [0u32; 3];
    for (slot, part) in values.iter_mut().zip(parts.iter()) {
        if part.is_empty() || part.len() > 2 || !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        *slot = part.iter().fold(0u32, |acc, &c| acc * 10 + (c - b'0') as u32);
    }
    let (hour, minute, second) = (values[0], values[1], values[2]);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some((hour, minute, second))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
    }
}
