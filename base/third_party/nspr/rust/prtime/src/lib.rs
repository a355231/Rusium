// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of Chromium's NSPR date and time helpers
//! (base/third_party/nspr/prtime.{h,cc}). The C++ code is derived from NSPR's
//! prtime.c (MPL 2.0) and from ns/lib/xp/xp_time.c.
//!
//! Ported: `PR_ParseTimeString` ([`parse_time_string`] and
//! [`parse_time_string_with_local_offset`]), `PR_NormalizeTime`
//! ([`normalize_time`]), `PR_ImplodeTime` ([`implode_time`]),
//! `PR_GMTParameters` ([`gmt_parameters`]), and the `PRTime`,
//! `PRExplodedTime` and `PRTimeParameters` types ([`PrTime`],
//! [`ExplodedTime`], [`TimeParameters`]).
//!
//! Differences from the C++ version:
//! - `PRStatus` results are `Option`, and output parameters are return values.
//!   `PR_ParseTimeString` returns `None` where it returned `PR_FAILURE`.
//! - Input is `&[u8]` instead of `const char *`. As with a C string, the input
//!   is cut at the first NUL byte, so bytes after it are ignored.
//! - Time zones: the C++ code calls `mktime` and `localtime_r`. This crate
//!   cannot call them without `unsafe` or FFI, so the local zone is a caller
//!   supplied function `offset_at_utc(utc_seconds) -> offset_seconds` (east
//!   positive). `parse_time_string` uses a zero offset, that is, UTC, as the
//!   local zone. Local-to-UTC conversion uses two lookups of that function.
//!   This is exact for fixed offsets. It may differ from `mktime` near DST
//!   transitions, because `mktime` applies its own gap and overlap rules.
//!   The Windows `localtime_s` path and the Visual C++ 2005 `mktime` guard
//!   are not ported. The `mktime` failure fallback is only reached for years
//!   before 1970, as in the C++ code.
//! - `PR_ExplodeTime` and `PR_LocalTimeParameters` are not ported, because
//!   they are not in this checkout's prtime.{h,cc}. Only the GMT parameter
//!   function is available, as [`gmt_parameters`].
//! - `PR_ASSERT` (DCHECK) checks are not reproduced. They only restate
//!   invariants that normalization guarantees.
//! - Signed 32-bit and 16-bit overflow is undefined behavior in the C++ code.
//!   Here it wraps, so no call panics. Results for in-range inputs are the same.
//! - `PRTimeParamFn` is a generic `Fn(&ExplodedTime) -> TimeParameters` bound.
//! - The 1000-iteration limit in the parser is kept, so inputs that need
//!   more loop iterations are rejected in the same way.
//! - Only the C++ `PR_ParseTimeString` is affected by the local zone. The
//!   normalize and implode functions never consult a time zone.

#![forbid(unsafe_code)]

/// NSPR `PRTime`: microseconds since 1970-01-01 00:00:00 UTC.
pub type PrTime = i64;

const USEC_PER_SEC: i64 = 1_000_000;

/// Offsets from GMT applied to an exploded time (NSPR `PRTimeParameters`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeParameters {
    /// Offset from GMT in seconds (east positive).
    pub gmt_offset: i32,
    /// Daylight saving time contribution in seconds.
    pub dst_offset: i32,
}

/// Broken-down time (NSPR `PRExplodedTime`). Field names drop the `tm_`
/// prefix. The year is absolute (AD), not counted from 1900. The month is
/// 0-based (January = 0), and the day of month is 1-based.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExplodedTime {
    /// Microseconds past `sec`, 0-999999 once normalized.
    pub usec: i32,
    /// Seconds past `min`, 0-59 once normalized.
    pub sec: i32,
    /// Minutes past `hour`, 0-59 once normalized.
    pub min: i32,
    /// Hours past midnight, 0-23 once normalized.
    pub hour: i32,
    /// Day of month, starting at 1.
    pub mday: i32,
    /// Month, 0-11 once normalized (January = 0).
    pub month: i32,
    /// Absolute year, AD.
    pub year: i16,
    /// Day of week, 0-6 (Sunday = 0). Computed by normalization.
    pub wday: i8,
    /// Day of year, 0-365 (January 1 = 0). Computed by normalization.
    pub yday: i16,
    /// Offsets used to convert this time to GMT.
    pub params: TimeParameters,
}

/// Time-zone parameters for GMT (NSPR `PR_GMTParameters`). Both offsets are 0.
pub fn gmt_parameters(_gmt: &ExplodedTime) -> TimeParameters {
    TimeParameters::default()
}

// Index 1 is January. Index 0 is -1, so `last_day_of_month[leap][month]`
// with 0-based `month` gives the day-of-year offset of the month's start.
const LAST_DAY_OF_MONTH: [[i32; 13]; 2] = [
    [-1, 30, 58, 89, 119, 150, 180, 211, 242, 272, 303, 333, 364],
    [-1, 30, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365],
];

const N_DAYS: [[i32; 12]; 2] = [
    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
    [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31],
];

fn is_leap_year(year: i16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn leap_index(year: i16) -> usize {
    usize::from(is_leap_year(year))
}

/// Days in `month` (0-based). `month` must be in 0..=11, which normalization
/// guarantees before this is called.
fn days_in_month(year: i16, month: i32) -> i32 {
    N_DAYS[leap_index(year)][month as usize]
}

/// Number of days from 01/01/01 to the start of year `y`, as in the C
/// `COUNT_DAYS` macro. Division truncates toward zero, as it does in C.
fn count_days(y: i32) -> i32 {
    let p = y - 1;
    p * 365 + p / 4 - p / 100 + p / 400
}

fn days_between_years(a: i32, b: i32) -> i32 {
    count_days(b).wrapping_sub(count_days(a))
}

/// Moves whole units from `low` into `high` so that `low` is in `0..base`.
/// This is the carry step used for usec, sec, min and hour.
fn carry(low: &mut i32, high: &mut i32, base: i32) {
    if *low < 0 || *low >= base {
        *high = high.wrapping_add(*low / base);
        *low %= base;
        if *low < 0 {
            *low += base;
            *high = high.wrapping_sub(1);
        }
    }
}

/// Carry step for months, which are 16-bit years in the output.
fn carry_month(month: &mut i32, year: &mut i16) {
    if *month < 0 || *month >= 12 {
        *year = year.wrapping_add((*month / 12) as i16);
        *month %= 12;
        if *month < 0 {
            *month += 12;
            *year = year.wrapping_sub(1);
        }
    }
}

/// Normalizes the day of month, moving across month and year boundaries.
/// Expects `month` in 0..=11.
fn normalize_mday(t: &mut ExplodedTime) {
    if t.mday < 1 {
        loop {
            t.month -= 1;
            if t.month < 0 {
                t.month = 11;
                t.year = t.year.wrapping_sub(1);
            }
            t.mday = t.mday.wrapping_add(days_in_month(t.year, t.month));
            if t.mday >= 1 {
                break;
            }
        }
    } else {
        let mut dim = days_in_month(t.year, t.month);
        while t.mday > dim {
            t.mday = t.mday.wrapping_sub(dim);
            t.month += 1;
            if t.month > 11 {
                t.month = 0;
                t.year = t.year.wrapping_add(1);
            }
            dim = days_in_month(t.year, t.month);
        }
    }
}

/// Applies a sub-day offset to a normalized GMT time, moving it to local time.
/// Like the C `ApplySecOffset`, this assumes `sec_offset` is under one day.
/// It moves the hour by at most one day, so larger offsets give wrong results.
fn apply_sec_offset(t: &mut ExplodedTime, sec_offset: i32) {
    t.sec = t.sec.wrapping_add(sec_offset);
    carry(&mut t.sec, &mut t.min, 60);
    carry(&mut t.min, &mut t.hour, 60);

    if t.hour < 0 {
        // Decrement mday, yday, and wday.
        t.hour += 24;
        t.mday = t.mday.wrapping_sub(1);
        t.yday = t.yday.wrapping_sub(1);
        if t.mday < 1 {
            t.month -= 1;
            if t.month < 0 {
                t.month = 11;
                t.year = t.year.wrapping_sub(1);
                t.yday = if is_leap_year(t.year) { 365 } else { 364 };
            }
            t.mday = days_in_month(t.year, t.month);
        }
        t.wday = t.wday.wrapping_sub(1);
        if t.wday < 0 {
            t.wday = 6;
        }
    } else if t.hour > 23 {
        // Increment mday, yday, and wday.
        t.hour -= 24;
        t.mday = t.mday.wrapping_add(1);
        t.yday = t.yday.wrapping_add(1);
        if t.mday > days_in_month(t.year, t.month) {
            t.mday = 1;
            t.month += 1;
            if t.month > 11 {
                t.month = 0;
                t.year = t.year.wrapping_add(1);
                t.yday = 0;
            }
        }
        t.wday = t.wday.wrapping_add(1);
        if t.wday > 6 {
            t.wday = 0;
        }
    }
}

/// Normalizes `time` so every field is in range, and recomputes `wday` and
/// `yday`. `params` gives the offsets for the normalized time. The fields
/// are then moved to local time by those offsets. Any offsets already in
/// `time.params` are undone first. Equivalent to `PR_NormalizeTime`.
pub fn normalize_time(time: &mut ExplodedTime, params: impl Fn(&ExplodedTime) -> TimeParameters) {
    // Get back to GMT.
    let old = time.params;
    time.sec = time
        .sec
        .wrapping_sub(old.gmt_offset.wrapping_add(old.dst_offset));
    time.params = TimeParameters::default();

    // Now normalize GMT. Leap seconds are not counted, as in the C++ code.
    carry(&mut time.usec, &mut time.sec, 1_000_000);
    carry(&mut time.sec, &mut time.min, 60);
    carry(&mut time.min, &mut time.hour, 60);
    carry(&mut time.hour, &mut time.mday, 24);

    // Normalize month and year before mday.
    carry_month(&mut time.month, &mut time.year);

    // Now that month and year are in range, normalize mday.
    normalize_mday(time);

    // Recompute yday and wday.
    time.yday = time
        .mday
        .wrapping_add(LAST_DAY_OF_MONTH[leap_index(time.year)][time.month as usize])
        as i16;
    let num_days = days_between_years(1970, i32::from(time.year)).wrapping_add(i32::from(time.yday));
    let mut wday = num_days.wrapping_add(4) % 7;
    if wday < 0 {
        wday += 7;
    }
    time.wday = wday as i8;

    // Recompute time parameters, then move to local time.
    time.params = params(time);
    let offset = time
        .params
        .gmt_offset
        .wrapping_add(time.params.dst_offset);
    apply_sec_offset(time, offset);
}

/// Converts a broken-down time to `PRTime`, undoing the offsets in
/// `exploded.params`. Equivalent to `PR_ImplodeTime`. Fields outside their
/// normal ranges are normalized first, on a copy.
pub fn implode_time(exploded: &ExplodedTime) -> PrTime {
    let mut copy = *exploded;
    normalize_time(&mut copy, gmt_parameters);

    let num_days = i64::from(days_between_years(1970, i32::from(copy.year)));
    let num_secs = i64::from(copy.yday) * 86400
        + i64::from(copy.hour) * 3600
        + i64::from(copy.min) * 60
        + i64::from(copy.sec);
    let mut num_secs64 = num_secs + num_days * 86400;

    // Apply the GMT and DST offsets. These are zero after normalization.
    num_secs64 -= i64::from(copy.params.gmt_offset);
    num_secs64 -= i64::from(copy.params.dst_offset);

    i64::from(copy.usec) + num_secs64 * USEC_PER_SEC
}

/// Time zone abbreviations and words recognized by the parser. The numeric
/// values are the C enum's, because the parser does arithmetic on them. The
/// numeric month assignments can give values outside the named range.
const TT_UNKNOWN: i32 = 0;
const TT_SUN: i32 = 1;
const TT_MON: i32 = 2;
const TT_TUE: i32 = 3;
const TT_WED: i32 = 4;
const TT_THU: i32 = 5;
const TT_FRI: i32 = 6;
const TT_SAT: i32 = 7;
const TT_JAN: i32 = 8;
const TT_FEB: i32 = 9;
const TT_MAR: i32 = 10;
const TT_APR: i32 = 11;
const TT_MAY: i32 = 12;
const TT_JUN: i32 = 13;
const TT_JUL: i32 = 14;
const TT_AUG: i32 = 15;
const TT_SEP: i32 = 16;
const TT_OCT: i32 = 17;
const TT_NOV: i32 = 18;
const TT_DEC: i32 = 19;
const TT_PST: i32 = 20;
const TT_PDT: i32 = 21;
const TT_MST: i32 = 22;
const TT_MDT: i32 = 23;
const TT_CST: i32 = 24;
const TT_CDT: i32 = 25;
const TT_EST: i32 = 26;
const TT_EDT: i32 = 27;
const TT_AST: i32 = 28;
const TT_NST: i32 = 29;
const TT_GMT: i32 = 30;
const TT_BST: i32 = 31;
const TT_MET: i32 = 32;
const TT_EET: i32 = 33;
const TT_JST: i32 = 34;

/// Parser state. Sentinel -1 means "not seen", as in the C code.
#[derive(Debug)]
struct Fields {
    dotw: i32,
    month: i32,
    zone: i32,
    zone_offset: i32,
    dst_offset: i32,
    date: i32,
    year: i32,
    hour: i32,
    min: i32,
    sec: i32,
    usec: i32,
}

impl Fields {
    fn new() -> Self {
        Fields {
            dotw: TT_UNKNOWN,
            month: TT_UNKNOWN,
            zone: TT_UNKNOWN,
            zone_offset: -1,
            dst_offset: 0,
            date: -1,
            year: -1,
            hour: -1,
            min: -1,
            sec: -1,
            usec: -1,
        }
    }
}

/// The input bytes. Reads past the end return NUL, as C string reads do.
struct Input<'a> {
    bytes: &'a [u8],
}

impl Input<'_> {
    fn at(&self, i: usize) -> u8 {
        self.bytes.get(i).copied().unwrap_or(0)
    }

    fn digit(&self, i: usize) -> bool {
        self.at(i).is_ascii_digit()
    }

    /// Value of the digit at `i`. Only meaningful when `digit(i)` is true.
    fn digit_value(&self, i: usize) -> i32 {
        i32::from(self.at(i) - b'0')
    }

    /// Value of a run of one or two digits starting at `start`.
    fn small_number(&self, start: usize, len: usize) -> i32 {
        if len == 2 {
            self.digit_value(start) * 10 + self.digit_value(start + 1)
        } else {
            self.digit_value(start)
        }
    }

    /// True when the bytes at `i` and `i + 1` match the lowercase letters
    /// `a` and `b`, ignoring ASCII case.
    fn pair(&self, i: usize, a: u8, b: u8) -> bool {
        self.at(i).eq_ignore_ascii_case(&a) && self.at(i + 1).eq_ignore_ascii_case(&b)
    }

    /// Characters that end a token, as in the C skip loop.
    fn is_token_delimiter(&self, i: usize) -> bool {
        let c = self.at(i);
        matches!(
            c,
            b' ' | b'\t' | b',' | b';' | b'-' | b'+' | b'/' | b'(' | b')' | b'[' | b']'
        ) || (c == b'T' && self.digit(i + 1))
    }

    /// Characters skipped between tokens.
    fn is_skippable(c: u8) -> bool {
        matches!(c, b' ' | b'\t' | b',' | b';' | b'/' | b'(' | b')' | b'[' | b']')
    }

    /// Handles a token that starts with a letter. Matches the C switch arms
    /// for letters.
    fn letter(&self, f: &mut Fields, rest: usize) {
        let c = self.at(rest).to_ascii_lowercase();
        let unknown_month = f.month == TT_UNKNOWN;
        let unknown_zone = f.zone == TT_UNKNOWN;
        let unknown_dotw = f.dotw == TT_UNKNOWN;
        match c {
            b'a' => {
                if unknown_month && self.pair(rest + 1, b'p', b'r') {
                    f.month = TT_APR;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_AST;
                } else if unknown_month && self.pair(rest + 1, b'u', b'g') {
                    f.month = TT_AUG;
                }
            }
            b'b' => {
                if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_BST;
                }
            }
            b'c' => {
                if unknown_zone && self.pair(rest + 1, b'd', b't') {
                    f.zone = TT_CDT;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_CST;
                }
            }
            b'd' => {
                if unknown_month && self.pair(rest + 1, b'e', b'c') {
                    f.month = TT_DEC;
                }
            }
            b'e' => {
                if unknown_zone && self.pair(rest + 1, b'd', b't') {
                    f.zone = TT_EDT;
                } else if unknown_zone && self.pair(rest + 1, b'e', b't') {
                    f.zone = TT_EET;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_EST;
                }
            }
            b'f' => {
                if unknown_month && self.pair(rest + 1, b'e', b'b') {
                    f.month = TT_FEB;
                } else if unknown_dotw && self.pair(rest + 1, b'r', b'i') {
                    f.dotw = TT_FRI;
                }
            }
            b'g' => {
                if unknown_zone && self.pair(rest + 1, b'm', b't') {
                    f.zone = TT_GMT;
                }
            }
            b'j' => {
                if unknown_month && self.pair(rest + 1, b'a', b'n') {
                    f.month = TT_JAN;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_JST;
                } else if unknown_month && self.pair(rest + 1, b'u', b'l') {
                    f.month = TT_JUL;
                } else if unknown_month && self.pair(rest + 1, b'u', b'n') {
                    f.month = TT_JUN;
                }
            }
            b'm' => {
                if unknown_month && self.pair(rest + 1, b'a', b'r') {
                    f.month = TT_MAR;
                } else if unknown_month && self.pair(rest + 1, b'a', b'y') {
                    f.month = TT_MAY;
                } else if unknown_zone && self.pair(rest + 1, b'd', b't') {
                    f.zone = TT_MDT;
                } else if unknown_zone && self.pair(rest + 1, b'e', b't') {
                    f.zone = TT_MET;
                } else if unknown_dotw && self.pair(rest + 1, b'o', b'n') {
                    f.dotw = TT_MON;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_MST;
                }
            }
            b'n' => {
                if unknown_month && self.pair(rest + 1, b'o', b'v') {
                    f.month = TT_NOV;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_NST;
                }
            }
            b'o' => {
                if unknown_month && self.pair(rest + 1, b'c', b't') {
                    f.month = TT_OCT;
                }
            }
            b'p' => {
                if unknown_zone && self.pair(rest + 1, b'd', b't') {
                    f.zone = TT_PDT;
                } else if unknown_zone && self.pair(rest + 1, b's', b't') {
                    f.zone = TT_PST;
                }
            }
            b's' => {
                if unknown_dotw && self.pair(rest + 1, b'a', b't') {
                    f.dotw = TT_SAT;
                } else if unknown_month && self.pair(rest + 1, b'e', b'p') {
                    f.month = TT_SEP;
                } else if unknown_dotw && self.pair(rest + 1, b'u', b'n') {
                    f.dotw = TT_SUN;
                }
            }
            b't' => {
                if unknown_dotw && self.pair(rest + 1, b'h', b'u') {
                    f.dotw = TT_THU;
                } else if unknown_dotw && self.pair(rest + 1, b'u', b'e') {
                    f.dotw = TT_TUE;
                }
            }
            b'u' => {
                // "UT" is GMT, but "UTx" is not.
                if unknown_zone
                    && self.at(rest + 1).eq_ignore_ascii_case(&b't')
                    && !self.at(rest + 2).is_ascii_alphabetic()
                {
                    f.zone = TT_GMT;
                }
            }
            b'w' => {
                if unknown_dotw && self.pair(rest + 1, b'e', b'd') {
                    f.dotw = TT_WED;
                }
            }
            _ => {}
        }
    }

    /// Handles a '+' or '-'. This is a numeric zone offset when no zone or a
    /// GMT zone has been seen. The `rest` index is advanced as in the C code.
    fn sign(&self, f: &mut Fields, rest: &mut usize) {
        if f.zone_offset != -1 {
            // Already got one.
            *rest += 1;
            return;
        }
        if f.zone != TT_UNKNOWN && f.zone != TT_GMT {
            // GMT+0300 is legal, but PST+0300 is not.
            *rest += 1;
            return;
        }

        let sign = if self.at(*rest) == b'+' { 1 } else { -1 };
        *rest += 1; // Move over the sign.
        let start = *rest;
        let mut end = start;
        while self.digit(end) {
            end += 1;
        }
        if start == end {
            return; // No digits here.
        }

        let value = match end - start {
            4 => {
                // Offset in HHMM.
                self.small_number(start, 2) * 60 + self.small_number(start + 2, 2)
            }
            2 => self.small_number(start, 2) * 60, // Offset in hours.
            1 => self.small_number(start, 1) * 60, // Offset in hours.
            _ => return,                           // 3 or more than 4 digits.
        };
        f.zone_offset = value * sign;
        f.zone = TT_GMT;
    }

    /// Handles a token that starts with a digit. This covers clock times,
    /// numeric dates, years, and days of month. The `rest` index is advanced
    /// as in the C code. Each `return` here is a C `break`.
    fn number(&self, f: &mut Fields, rest: &mut usize) {
        let start = *rest;
        let mut end = start + 1;
        while self.digit(end) {
            end += 1;
        }

        // `end` is now the first character after a run of digits.
        if self.at(end) == b':' {
            if f.hour >= 0 && f.min >= 0 {
                return; // Already got it.
            }
            // We have seen "[0-9]+:", so this is probably HH:MM[:SS].
            if end - start > 2 {
                return; // It is [0-9][0-9][0-9]+:.
            }
            let mut tmp_hour = self.small_number(start, end - start);

            // Move over the colon, and parse minutes.
            end += 1;
            *rest = end;
            while self.digit(end) {
                end += 1;
            }
            if end == *rest {
                return; // No digits after first colon.
            }
            if end - *rest > 2 {
                return; // It is [0-9][0-9][0-9]+:.
            }
            let mut tmp_min = self.small_number(*rest, end - *rest);

            // Now go for seconds.
            *rest = end;
            if self.at(*rest) == b':' {
                *rest += 1;
            }
            end = *rest;
            while self.digit(end) {
                end += 1;
            }
            let mut tmp_sec = -1;
            if end == *rest {
                // No digits after second colon; that's ok.
            } else if end - *rest > 2 {
                return; // It is [0-9][0-9][0-9]+:.
            } else {
                tmp_sec = self.small_number(*rest, end - *rest);
            }

            // Fractional second: use up to 6 digits and skip the rest.
            *rest = end;
            let mut tmp_usec = -1;
            if self.at(*rest) == b'.' {
                *rest += 1;
                end += 1;
                tmp_usec = 0;
                while self.digit(end) {
                    if end - *rest < 6 {
                        tmp_usec = tmp_usec * 10 + self.digit_value(end);
                    }
                    end += 1;
                }
                let mut ndigits = end - *rest;
                while ndigits < 6 {
                    tmp_usec *= 10;
                    ndigits += 1;
                }
                *rest = end;
            }

            if self.at(*rest) == b'Z' {
                f.zone = TT_GMT;
                *rest += 1;
            } else if tmp_hour <= 12 {
                // The time may be followed by AM or PM, with optional blanks.
                let mut s = end;
                while self.at(s) == b' ' || self.at(s) == b'\t' {
                    s += 1;
                }
                if self.pair(s, b'p', b'm') {
                    // 10:05pm == 22:05, and 12:05pm == 12:05.
                    tmp_hour = if tmp_hour == 12 { 12 } else { tmp_hour + 12 };
                } else if tmp_hour == 12 && self.pair(s, b'a', b'm') {
                    // 12:05am == 00:05.
                    tmp_hour = 0;
                }
            }

            f.hour = tmp_hour;
            f.min = tmp_min;
            f.sec = tmp_sec;
            f.usec = tmp_usec;
            tmp_min = 0;
            let _ = tmp_min;
            *rest = end;
            return;
        }

        if (self.at(end) == b'/' || self.at(end) == b'-') && self.digit(end + 1) {
            // Perhaps this is 6/16/95, 16/6/95, 6-16-95, 16-6-95, or even
            // 95-06-05 or 1995-06-22.
            if f.month != TT_UNKNOWN {
                return; // If we saw a month name, this can't be.
            }
            self.numeric_date(f, rest);
            return;
        }

        if self.at(end).is_ascii_alphabetic() {
            // Digits followed by non-punctuation: nothing to do.
        } else if end - start == 5 {
            // Five digits is a year.
            if f.year < 0 {
                f.year = self.digits_value(start, 5);
            }
        } else if end - start == 4 {
            // Four digits is a year.
            if f.year < 0 {
                f.year = self.digits_value(start, 4);
            }
        } else if end - start == 2 {
            // Two digits: a date or a year. If there is no date yet and the
            // number is under 32, it is the date. Otherwise, if there is no
            // year, it is the year: under 70 is 2000s, 70-99 is 1900s.
            let n = self.small_number(start, 2);
            if f.date < 0 && n < 32 {
                f.date = n;
            } else if f.year < 0 {
                f.year = if n < 70 {
                    2000 + n
                } else if n < 100 {
                    1900 + n
                } else {
                    n
                };
            }
        } else if end - start == 1 {
            // One digit: a date.
            if f.date < 0 {
                f.date = self.digit_value(start);
            }
        }
        // Otherwise, three or more than five digits: ignored.
    }

    /// Value of `len` consecutive digits starting at `start`.
    fn digits_value(&self, start: usize, len: usize) -> i32 {
        (0..len).fold(0, |acc, k| acc * 10 + self.digit_value(start + k))
    }

    /// Handles numeric dates such as 6/16/95, 16-6-95, or 1995-06-22. `rest`
    /// is at the first digit. On failure, `rest` is left alone, as in the C
    /// code, unless the C code moved it first. The caller checks that the
    /// separator and digit layout are right.
    fn numeric_date(&self, f: &mut Fields, rest: &mut usize) {
        let mut s = *rest;

        // First one, two, or four digits (not three).
        let mut n1 = self.digit_value(s);
        s += 1;
        if self.digit(s) {
            n1 = n1 * 10 + self.digit_value(s);
            s += 1;
            if self.digit(s) {
                if !self.digit(s + 1) {
                    return;
                }
                n1 = n1 * 10 + self.digit_value(s);
                n1 = n1 * 10 + self.digit_value(s + 1);
                s += 2;
            }
        }

        if self.at(s) != b'/' && self.at(s) != b'-' {
            return;
        }
        s += 1;

        // Second one or two digits.
        if !self.digit(s) {
            return;
        }
        let mut n2 = self.digit_value(s);
        s += 1;
        if self.digit(s) {
            n2 = n2 * 10 + self.digit_value(s);
            s += 1;
        }

        if self.at(s) != b'/' && self.at(s) != b'-' {
            return;
        }
        s += 1;

        // Third one, two, four, or five digits.
        if !self.digit(s) {
            return;
        }
        let mut n3 = self.digit_value(s);
        s += 1;
        if self.digit(s) {
            n3 = n3 * 10 + self.digit_value(s);
            s += 1;
            if self.digit(s) {
                if !self.digit(s + 1) {
                    return;
                }
                n3 = n3 * 10 + self.digit_value(s);
                s += 1;
                n3 = n3 * 10 + self.digit_value(s);
                s += 1;
                if self.digit(s) {
                    n3 = n3 * 10 + self.digit_value(s);
                    s += 1;
                }
            }
        }

        if self.at(s) == b'T' && self.digit(s + 1) {
            // ISO 8601 T delimiter followed by a number: ok.
        } else if self.at(s).is_ascii_alphanumeric() {
            // Other alphanumerics are not ok.
            return;
        }

        // Three multi-digit numbers separated by / or -. Decide whether they
        // are [YY]YY/MM/DD, DD/MM/YY, or MM/DD/YY.
        if n1 > 31 || n1 == 0 {
            // Must be [YY]YY/MM/DD.
            if n2 > 12 || n3 > 31 {
                return;
            }
            f.year = n1;
            if f.year < 70 {
                f.year += 2000;
            } else if f.year < 100 {
                f.year += 1900;
            }
            // Like the C code, n2 == 0 gives a value outside the month range.
            f.month = n2 + TT_JAN - 1;
            f.date = n3;
            *rest = s;
            return;
        }

        if n1 > 12 && n2 > 12 {
            // Illegal.
            *rest = s;
            return;
        }

        if n3 < 70 {
            n3 += 2000;
        } else if n3 < 100 {
            n3 += 1900;
        }

        if n1 > 12 {
            // Must be DD/MM/YY.
            f.date = n1;
            f.month = n2 + TT_JAN - 1;
            f.year = n3;
        } else {
            // Assume MM/DD/YY.
            f.month = n1 + TT_JAN - 1;
            f.date = n2;
            f.year = n3;
        }
        *rest = s;
    }
}

/// Scans the input and fills `Fields`. Returns `None` for inputs that stop
/// the parser. Equivalent to the C switch and token loop.
fn scan(input: &Input<'_>) -> Option<Fields> {
    let mut f = Fields::new();
    let mut rest = 0usize;
    let mut iterations = 0u32;

    while input.at(rest) != 0 {
        if iterations > 1000 {
            return None;
        }
        iterations += 1;

        match input.at(rest) {
            b'+' | b'-' => input.sign(&mut f, &mut rest),
            b'0'..=b'9' => input.number(&mut f, &mut rest),
            _ => input.letter(&mut f, rest),
        }

        // Skip to the end of this token, whether we parsed it or not.
        while input.at(rest) != 0 && !input.is_token_delimiter(rest) {
            rest += 1;
        }

        // Skip over uninteresting characters. A '-' is ignored at the start
        // of a token if a year has not been parsed yet after a letter (as in
        // "30-AUG-1966"), or if it is not followed by a digit.
        loop {
            while Input::is_skippable(input.at(rest)) {
                rest += 1;
            }
            if input.at(rest) == b'-'
                && ((rest > 0 && input.at(rest - 1).is_ascii_alphabetic() && f.year < 0)
                    || !input.digit(rest + 1))
            {
                rest += 1;
                continue;
            }
            break;
        }

        // Skip the T that may precede an ISO 8601 time.
        if input.at(rest) == b'T' && input.digit(rest + 1) {
            rest += 1;
        }
    }

    Some(f)
}

/// Returns the (gmt, dst) offsets in minutes for a named zone, as in the C
/// switch. Returns `None` for tokens that are not zones.
fn named_zone_offsets(zone: i32) -> Option<(i32, i32)> {
    Some(match zone {
        TT_PST => (-8 * 60, 0),
        TT_PDT => (-8 * 60, 60),
        TT_MST => (-7 * 60, 0),
        TT_MDT => (-7 * 60, 60),
        TT_CST => (-6 * 60, 0),
        TT_CDT => (-6 * 60, 60),
        TT_EST => (-5 * 60, 0),
        TT_EDT => (-5 * 60, 60),
        TT_AST => (-4 * 60, 0),
        TT_NST => (-3 * 60 - 30, 0),
        TT_GMT => (0, 0),
        TT_BST => (0, 60),
        TT_MET => (60, 0),
        TT_EET => (2 * 60, 0),
        TT_JST => (9 * 60, 0),
        _ => return None,
    })
}

/// Converts a local wall-clock time, given as seconds, to UTC seconds using
/// `offset_at_utc`. The first lookup is an estimate, and the second refines it.
fn local_to_utc(wall_secs: i64, offset_at_utc: &dyn Fn(i64) -> i64) -> i64 {
    let guess = wall_secs - offset_at_utc(wall_secs);
    wall_secs - offset_at_utc(guess)
}

/// Parses a date and time string into a `PrTime`, assuming the local time
/// zone is UTC. Equivalent to `PR_ParseTimeString`. If the string has no
/// zone and `default_to_gmt` is false, the local zone (UTC here) is used.
/// If `default_to_gmt` is true, a missing zone means GMT.
///
/// Returns `None` where the C++ code returns `PR_FAILURE`.
///
/// Accepted formats include the following examples:
///
/// ```text
/// 14 Apr 89 03:20:12
/// 14 Apr 89 03:20 GMT
/// Fri, 17 Mar 89 4:01:33
/// Mon Jan 16 16:12 PDT 1989
/// Mon Jan 16 16:12 +0130 1989
/// 6 May 1992 16:41-JST (Wednesday)
/// 22-AUG-1993 10:59:12.82
/// 22-AUG-1993 10:59pm
/// Friday, August 04, 1995 3:54 PM
/// 06/21/95 04:24:34 PM
/// 20/06/95 21:07
/// 95-06-08 19:32:48 EDT
/// 1995-06-17T23:11:25.342156Z
/// ```
pub fn parse_time_string(input: &[u8], default_to_gmt: bool) -> Option<PrTime> {
    parse_time_string_with_local_offset(input, default_to_gmt, &|_| 0)
}

/// Like [`parse_time_string`], but with a caller-supplied local time zone.
/// `offset_at_utc(utc_seconds)` returns the local UTC offset in seconds
/// (east positive) in effect at that UTC instant. It stands in for the
/// `mktime` and `localtime_r` calls in the C++ code.
pub fn parse_time_string_with_local_offset(
    input: &[u8],
    default_to_gmt: bool,
    offset_at_utc: &dyn Fn(i64) -> i64,
) -> Option<PrTime> {
    // Like a C string, stop at the first NUL.
    let len = input.iter().position(|&b| b == 0).unwrap_or(input.len());
    let input = Input {
        bytes: &input[..len],
    };

    let mut f = scan(&input)?;

    if f.month == TT_UNKNOWN || f.date == -1 || f.year == -1 || f.year > i32::from(i16::MAX) {
        // No year, month, or day of month: the string cannot be parsed.
        return None;
    }

    let mut result = ExplodedTime::default();
    if f.usec != -1 {
        result.usec = f.usec;
    }
    if f.sec != -1 {
        result.sec = f.sec;
    }
    if f.min != -1 {
        result.min = f.min;
    }
    if f.hour != -1 {
        result.hour = f.hour;
    }
    if f.date != -1 {
        result.mday = f.date;
    }
    if f.month != TT_UNKNOWN {
        result.month = f.month - TT_JAN;
    }
    if f.year != -1 {
        result.year = f.year as i16;
    }
    if f.dotw != TT_UNKNOWN {
        result.wday = (f.dotw - TT_SUN) as i8;
    }
    // Compute wday and yday, and normalize for the checks below.
    normalize_time(&mut result, gmt_parameters);

    // Set the GMT and DST offsets in tm_params.
    if f.zone == TT_UNKNOWN && default_to_gmt {
        // No zone was given, so pretend the zone was GMT.
        f.zone = TT_GMT;
        f.zone_offset = 0;
    }

    if f.zone != TT_UNKNOWN && f.zone_offset == -1 {
        if let Some((gmt, dst)) = named_zone_offsets(f.zone) {
            f.zone_offset = gmt;
            f.dst_offset = dst;
        }
    }

    if f.zone_offset == -1 {
        // No zone was given, so everything is local.
        if i32::from(result.year) >= 1970 {
            // The wall-clock time as seconds. The offsets are zero after
            // normalization, so this is the wall-clock value.
            let wall_usec = implode_time(&result);
            let wall_secs = wall_usec.div_euclid(USEC_PER_SEC);
            let utc_secs = local_to_utc(wall_secs, offset_at_utc);
            return Some(utc_secs * USEC_PER_SEC + i64::from(result.usec));
        }

        // Before 1970 the C++ code uses the zone offset at 00:00:00 on
        // 2 Jan 1970 GMT for the whole date. It reads that from localtime_r
        // and truncates to minutes. This is the same as floor division.
        let offset = offset_at_utc(86_400);
        f.zone_offset = offset.div_euclid(60) as i32;
    }

    result.params = TimeParameters {
        gmt_offset: f.zone_offset.wrapping_mul(60),
        dst_offset: f.dst_offset.wrapping_mul(60),
    };
    Some(implode_time(&result))
}

#[cfg(test)]
mod tests;
