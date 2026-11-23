// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Unit tests. The `parse_time_test*` tests are ports of the C++ TEST_F cases
//! in base/time/pr_time_unittest.cc. The C++ tests compare against the
//! machine's local zone. Here the local zone is either UTC (the default) or
//! a fixed PDT zone (UTC-7), so the expected values are the same on every
//! machine.

use super::*;

const USEC: i64 = 1_000_000;

/// 15 Oct 2007 19:45:00 UTC, which is 12:45 PDT.
const COMPARISON_TIME_PDT: PrTime = 1_192_477_500 * USEC;

/// 2013-07-08 09:28:12.441381 UTC.
const COMPARISON_TIME_2: PrTime = 1_373_275_692_441_381;

/// 2013-07-08 11:28:12.441381 local time at UTC-7, which is 18:28:12 UTC.
const COMPARISON_TIME_LOCAL_2: PrTime = COMPARISON_TIME_2 + 9 * 3600 * USEC;

/// Local zone used by the C++ tests: PDT, a fixed UTC-7 offset.
fn pdt_local(_utc: i64) -> i64 {
    -7 * 3600
}

fn parse_local(s: &str) -> Option<PrTime> {
    parse_time_string_with_local_offset(s.as_bytes(), false, &pdt_local)
}

fn parse_utc(s: &str) -> Option<PrTime> {
    parse_time_string(s.as_bytes(), false)
}

// ---------------------------------------------------------------------------
// Reference helpers, written independently of the code under test.
// ---------------------------------------------------------------------------

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// algorithm). `m` is 1-based.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of `days_from_civil`. Returns (year, 1-based month, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Total microseconds for fields that may be out of range, with `offset`
/// seconds east of UTC removed. `month` is 0-based.
#[allow(clippy::too_many_arguments)]
fn ref_usec(
    year: i64,
    month: i64,
    mday: i64,
    hour: i64,
    min: i64,
    sec: i64,
    usec: i64,
    offset: i64,
) -> i64 {
    let months = year * 12 + month;
    let y = months.div_euclid(12);
    let m = months.rem_euclid(12) + 1;
    let days = days_from_civil(y, m, 1) + mday - 1;
    let secs = days * 86_400 + hour * 3600 + min * 60 + sec - offset;
    secs * USEC + usec
}

/// (weekday with Sunday = 0, day of year with January 1 = 0) for a date.
fn ref_wday_yday(year: i64, month0: i64, mday: i64) -> (i64, i64) {
    let days = days_from_civil(year, month0 + 1, mday);
    let jan1 = days_from_civil(year, 1, 1);
    ((days + 4).rem_euclid(7), days - jan1)
}

/// Small deterministic PRNG (xorshift64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform-ish value in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
}

/// asctime-style string in UTC, with the trailing newline that asctime adds.
fn asctime_utc(secs: i64) -> String {
    const WDAY: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MON: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let wday = (days + 4).rem_euclid(7) as usize;
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}\n",
        WDAY[wday],
        MON[(m - 1) as usize],
        d,
        rem / 3600,
        (rem / 60) % 60,
        rem % 60,
        y
    )
}

// ---------------------------------------------------------------------------
// Ported from base/time/pr_time_unittest.cc (PRTimeTest fixture).
// ---------------------------------------------------------------------------

#[test]
fn parse_time_test1() {
    // The C++ test uses the current local time, formatted with asctime_r.
    // Here the current time is formatted in UTC, because the local zone is
    // UTC. The second is fixed before formatting, so there is no race.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs() as i64;
    let text = asctime_utc(now);
    let parsed = parse_utc(&text);
    assert_eq!(parsed, Some(now * USEC), "input: {text:?}");
}

#[test]
fn parse_time_test2() {
    assert_eq!(parse_utc("Mon, 15 Oct 2007 19:45:00 GMT"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test3() {
    // Local 12:45 in PDT.
    assert_eq!(parse_local("15 Oct 07 12:45:00"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test4() {
    assert_eq!(parse_utc("15 Oct 07 19:45 GMT"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test5() {
    assert_eq!(parse_utc("Mon Oct 15 12:45 PDT 2007"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test6() {
    assert_eq!(parse_local("Monday, Oct 15, 2007 12:45 PM"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test7() {
    assert_eq!(parse_local("10/15/07 12:45:00 PM"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test8() {
    assert_eq!(parse_local("10/15/07 12:45:00. PM"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test9() {
    assert_eq!(parse_local("10/15/07 12:45:00.0 PM"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test10() {
    assert_eq!(parse_local("15-OCT-2007 12:45pm"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test11() {
    // 4:45 JST on 16 Oct is 19:45 UTC on 15 Oct.
    assert_eq!(parse_utc("16 Oct 2007 4:45-JST (Tuesday)"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test12() {
    // hh:mm timezone offset.
    assert_eq!(parse_utc("2013-07-08T11:28:12.441381+02:00"), Some(COMPARISON_TIME_2));
}

#[test]
fn parse_time_test13() {
    // hhmm timezone offset.
    assert_eq!(parse_utc("2013-07-08T11:28:12.441381+0200"), Some(COMPARISON_TIME_2));
}

#[test]
fn parse_time_test14() {
    // hh timezone offset, with 7 fractional digits.
    assert_eq!(parse_utc("2013-07-08T11:28:12.4413819+02"), Some(COMPARISON_TIME_2));
}

#[test]
fn parse_time_test15() {
    // 5 digits fractional second.
    assert_eq!(parse_utc("2013-07-08T09:28:12.44138Z"), Some(COMPARISON_TIME_2 - 1));
}

#[test]
fn parse_time_test16() {
    // Fractional seconds, local timezone.
    assert_eq!(parse_local("2013-07-08T11:28:12.441381"), Some(COMPARISON_TIME_LOCAL_2));
}

#[test]
fn parse_time_test17() {
    // "Z" (=GMT) timezone.
    assert_eq!(parse_utc("2013-07-08T09:28:12.441381Z"), Some(COMPARISON_TIME_2));
}

#[test]
fn parse_time_test18() {
    // "T" delimiter replaced by space.
    assert_eq!(parse_utc("2013-07-08 09:28:12.441381Z"), Some(COMPARISON_TIME_2));
}

#[test]
fn parse_time_test_invalid1() {
    assert_eq!(parse_utc("201-07-08T09:28:12.441381Z"), None);
}

#[test]
fn parse_time_test_invalid2() {
    assert_eq!(parse_utc("2013-007-08T09:28:12.441381Z"), None);
}

#[test]
fn parse_time_test_invalid3() {
    assert_eq!(parse_utc("2013-07-008T09:28:12.441381Z"), None);
}

#[test]
fn parse_time_test_out_of_range() {
    // Year 3001 without a zone. It must still parse, as the C++ test expects.
    assert!(parse_utc("Sun Jan  2 00:00:00 3001").is_some());
    assert!(parse_local("Sun Jan  2 00:00:00 3001").is_some());
}

#[test]
fn parse_time_test_not_normalized1() {
    assert_eq!(parse_utc("Mon Oct 15 12:44:60 PDT 2007"), Some(COMPARISON_TIME_PDT));
}

#[test]
fn parse_time_test_not_normalized2() {
    // Hour 36 is one day later, so 36:45 on the 14th is 12:45 on the 15th.
    assert_eq!(parse_utc("Sun Oct 14 36:45 PDT 2007"), Some(COMPARISON_TIME_PDT));
}

// ---------------------------------------------------------------------------
// Extra tests.
// ---------------------------------------------------------------------------

#[test]
fn gmt_parameters_are_zero() {
    assert_eq!(gmt_parameters(&ExplodedTime::default()), TimeParameters::default());
}

#[test]
fn implode_epoch_and_known_values() {
    let epoch = ExplodedTime { mday: 1, year: 1970, ..Default::default() };
    assert_eq!(implode_time(&epoch), 0);

    // 2000-03-01 00:00:00 UTC. 2000 is a leap year, so February has 29 days.
    let t = ExplodedTime { mday: 1, month: 2, year: 2000, ..Default::default() };
    assert_eq!(implode_time(&t), 951_868_800 * USEC);
}

#[test]
fn normalize_sets_wday_and_yday() {
    // 2007-10-15 was a Monday (wday 1), day 287 of the year (0-based).
    let mut t = ExplodedTime { mday: 15, month: 9, year: 2007, ..Default::default() };
    normalize_time(&mut t, gmt_parameters);
    assert_eq!(t.wday, 1);
    assert_eq!(t.yday, 287);
}

#[test]
fn normalize_handles_negative_and_large_overflow() {
    // Day 0 of March is the last day of February (2008 is a leap year).
    let mut t = ExplodedTime { mday: 0, month: 2, year: 2008, ..Default::default() };
    normalize_time(&mut t, gmt_parameters);
    assert_eq!((t.year, t.month, t.mday), (2008, 1, 29));

    // Month 13 of year 2000 is January 2001.
    let mut t = ExplodedTime { mday: 1, month: 12, year: 2000, ..Default::default() };
    normalize_time(&mut t, gmt_parameters);
    assert_eq!((t.year, t.month, t.mday), (2001, 0, 1));
}

#[test]
fn implode_matches_reference_random() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let year = rng.range(1900, 2100);
        let month = rng.range(-20, 30);
        let mday = rng.range(-40, 80);
        let hour = rng.range(-50, 80);
        let min = rng.range(-100, 200);
        let sec = rng.range(-200, 200);
        let usec = rng.range(-2_000_000, 2_000_000);
        let t = ExplodedTime {
            usec: usec as i32,
            sec: sec as i32,
            min: min as i32,
            hour: hour as i32,
            mday: mday as i32,
            month: month as i32,
            year: year as i16,
            ..Default::default()
        };
        let expected = ref_usec(year, month, mday, hour, min, sec, usec, 0);
        assert_eq!(implode_time(&t), expected, "fields: {t:?}");
    }
}

#[test]
fn implode_applies_params() {
    // Local 12:00 at UTC+5:30 is 06:30 UTC.
    let t = ExplodedTime {
        hour: 12,
        mday: 1,
        year: 2000,
        params: TimeParameters { gmt_offset: 5 * 3600 + 30 * 60, dst_offset: 0 },
        ..Default::default()
    };
    let utc = ref_usec(2000, 0, 1, 6, 30, 0, 0, 0);
    assert_eq!(implode_time(&t), utc);

    // The DST contribution is subtracted too.
    let t = ExplodedTime {
        hour: 12,
        mday: 1,
        year: 2000,
        params: TimeParameters { gmt_offset: 3600, dst_offset: 3600 },
        ..Default::default()
    };
    assert_eq!(implode_time(&t), ref_usec(2000, 0, 1, 10, 0, 0, 0, 0));
}

#[test]
fn normalize_matches_reference_random() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..20_000 {
        let year = rng.range(1900, 2100);
        let month = rng.range(-20, 30);
        let mday = rng.range(-40, 80);
        let hour = rng.range(-50, 80);
        let min = rng.range(-100, 200);
        let sec = rng.range(-200, 200);
        let usec = rng.range(-2_000_000, 2_000_000);
        let mut t = ExplodedTime {
            usec: usec as i32,
            sec: sec as i32,
            min: min as i32,
            hour: hour as i32,
            mday: mday as i32,
            month: month as i32,
            year: year as i16,
            ..Default::default()
        };
        let expected = ref_usec(year, month, mday, hour, min, sec, usec, 0);
        normalize_time(&mut t, gmt_parameters);

        assert!((0..1_000_000).contains(&t.usec), "{t:?}");
        assert!((0..60).contains(&t.sec), "{t:?}");
        assert!((0..60).contains(&t.min), "{t:?}");
        assert!((0..24).contains(&t.hour), "{t:?}");
        assert!((0..12).contains(&t.month), "{t:?}");
        let (y, m, d) = (i64::from(t.year), i64::from(t.month), i64::from(t.mday));
        assert!(d >= 1 && d <= i64::from(N_DAYS[leap_index(t.year)][m as usize]), "{t:?}");
        let (wday, yday) = ref_wday_yday(y, m, d);
        assert_eq!(i64::from(t.wday), wday, "{t:?}");
        assert_eq!(i64::from(t.yday), yday, "{t:?}");
        assert_eq!(implode_time(&t), expected, "{t:?}");
    }
}

#[test]
fn normalize_with_offset_round_trips_random() {
    // Normalizing with a local offset moves the fields to local time, and
    // implode must still recover the same GMT instant. The offset stays
    // within 12 hours, which ApplySecOffset supports.
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    for _ in 0..20_000 {
        let year = rng.range(1900, 2100);
        let month = rng.range(-20, 30);
        let mday = rng.range(-40, 80);
        let hour = rng.range(-50, 80);
        let min = rng.range(-100, 200);
        let sec = rng.range(-200, 200);
        let offset = rng.range(-43_200, 43_200) as i32;
        let raw = ExplodedTime {
            usec: rng.range(0, 999_999) as i32,
            sec: sec as i32,
            min: min as i32,
            hour: hour as i32,
            mday: mday as i32,
            month: month as i32,
            year: year as i16,
            ..Default::default()
        };
        let gmt = implode_time(&raw);
        let mut local = raw;
        normalize_time(&mut local, |_| TimeParameters { gmt_offset: offset, dst_offset: 0 });
        assert_eq!(local.params.gmt_offset, offset);
        assert_eq!(implode_time(&local), gmt, "raw {raw:?} local {local:?}");

        let (wday, yday) =
            ref_wday_yday(i64::from(local.year), i64::from(local.month), i64::from(local.mday));
        assert_eq!(i64::from(local.wday), wday, "{local:?}");
        assert_eq!(i64::from(local.yday), yday, "{local:?}");
    }
}

#[test]
fn parse_formatted_dates_match_reference_random() {
    let mut rng = Rng(0x5151_5151_2323_2323);
    let months =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    for _ in 0..5_000 {
        let year = rng.range(1000, 9999);
        let month = rng.range(0, 11);
        let day = rng.range(1, 31);
        let hour = rng.range(0, 23);
        let min = rng.range(0, 59);
        let sec = rng.range(0, 59);
        let digits = rng.range(0, 7) as usize;
        let mut frac = String::new();
        let mut usec = 0i64;
        if digits > 0 {
            for k in 0..digits {
                let dgt = rng.range(0, 9);
                frac.push(char::from(b'0' + dgt as u8));
                if k < 6 {
                    usec = usec * 10 + dgt;
                }
            }
            for _ in digits..6 {
                usec *= 10;
            }
            frac.insert(0, '.');
        }
        // Zone: none, GMT, or a numeric offset. The reference applies the
        // offset to the local fields.
        let zone_kind = rng.range(0, 3);
        let (suffix, offset) = match zone_kind {
            0 => (String::new(), 0i64),
            1 => (" GMT".to_string(), 0),
            2 => {
                let mut minutes = rng.range(0, 14 * 60 - 1);
                let neg = rng.range(0, 1) == 1;
                // "-0001" hits the C sentinel quirk, tested separately.
                if neg && minutes == 1 {
                    minutes = 2;
                }
                let sign = if neg { '-' } else { '+' };
                let hh = minutes / 60;
                let mm = minutes % 60;
                (format!(" {sign}{hh:02}{mm:02}"), if neg { -minutes * 60 } else { minutes * 60 })
            }
            _ => (" Z".to_string(), 0),
        };
        let text = format!(
            "{day:02} {} {year} {hour:02}:{min:02}:{sec:02}{frac}{suffix}",
            months[month as usize]
        );
        // With no zone, the local zone is UTC.
        let expected = ref_usec(year, month, day, hour, min, sec, usec, offset);
        assert_eq!(parse_utc(&text), Some(expected), "input {text:?}");
        assert_eq!(
            parse_time_string(text.as_bytes(), true),
            Some(expected),
            "input {text:?} default_to_gmt"
        );

        // A constant fixed local zone gives the same result for every year,
        // including years before 1970, which use the 2 Jan 1970 offset.
        if zone_kind == 0 {
            let local = format!(
                "{day:02} {} {year} {hour:02}:{min:02}:{sec:02}{frac}",
                months[month as usize]
            );
            let fixed = |_: i64| -8 * 3600;
            let got = parse_time_string_with_local_offset(local.as_bytes(), false, &fixed);
            assert_eq!(
                got,
                Some(ref_usec(year, month, day, hour, min, sec, usec, -8 * 3600)),
                "input {local:?}"
            );
        }
    }
}

#[test]
fn parse_pre_1970_uses_offset_at_1970_01_02() {
    // The offset at 2 Jan 1970 00:00 UTC is -8 hours, and it applies to the
    // whole date. The result is 08:00 UTC.
    let fixed = |_: i64| -8 * 3600;
    let got = parse_time_string_with_local_offset(b"1 Jan 1960 00:00:00", false, &fixed);
    assert_eq!(got, Some(ref_usec(1960, 0, 1, 8, 0, 0, 0, 0)));
}

#[test]
fn parse_default_to_gmt_ignores_local_zone() {
    let fixed = |_: i64| -7 * 3600;
    let got = parse_time_string_with_local_offset(b"15 Oct 2007 12:45:00", true, &fixed);
    assert_eq!(got, Some(ref_usec(2007, 9, 15, 12, 45, 0, 0, 0)));
}

#[test]
fn named_zone_abbreviations() {
    // 12:00 in each zone, converted to UTC.
    // Offsets in seconds east of UTC. DST zones include the one hour.
    let cases: &[(&str, i64)] = &[
        ("PST", -8 * 3600),
        ("PDT", -7 * 3600),
        ("MST", -7 * 3600),
        ("MDT", -6 * 3600),
        ("CST", -6 * 3600),
        ("CDT", -5 * 3600),
        ("EST", -5 * 3600),
        ("EDT", -4 * 3600),
        ("AST", -4 * 3600),
        ("NST", -3 * 3600 - 30 * 60),
        ("GMT", 0),
        ("UT", 0),
        ("BST", 3600),
        ("MET", 3600),
        ("EET", 2 * 3600),
        ("JST", 9 * 3600),
    ];
    for (zone, offset) in cases {
        // A time of 12:00 in the named zone.
        let text = format!("15 Oct 2007 12:00 {zone}");
        let expected = ref_usec(2007, 9, 15, 12, 0, 0, 0, *offset);
        assert_eq!(parse_utc(&text), Some(expected), "{text}");
    }
}

#[test]
fn negative_minus_0001_offset_is_lost_like_the_c_sentinel() {
    // "-0001" gives zone_offset == -1 in the C++ code. That value is the
    // "no offset" sentinel, so the zone falls back to GMT with offset 0.
    assert_eq!(parse_utc("15 Oct 2007 12:00 -0001"), Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, 0)));
    // "+0001" is a real one-minute offset.
    assert_eq!(parse_utc("15 Oct 2007 12:00 +0001"), Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, 60)));
}

#[test]
fn numeric_offset_forms() {
    let expected = ref_usec(2007, 9, 15, 12, 0, 0, 0, 5 * 3600 + 30 * 60);
    assert_eq!(parse_utc("15 Oct 2007 12:00 +0530"), Some(expected));
    assert_eq!(parse_utc("15 Oct 2007 12:00 +0530 (note)"), Some(expected));
    // A single digit or a two-digit offset means hours.
    assert_eq!(
        parse_utc("15 Oct 2007 12:00 +5"),
        Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, 5 * 3600))
    );
    assert_eq!(
        parse_utc("15 Oct 2007 12:00 +05"),
        Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, 5 * 3600))
    );
    // Three digits are not an offset, so the zone stays unset and the local
    // zone (UTC) applies.
    assert_eq!(parse_utc("15 Oct 2007 12:00 +530"), Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, 0)));
    // A numeric offset after a named zone other than GMT is ignored.
    assert_eq!(
        parse_utc("15 Oct 2007 12:00 PST +0530"),
        Some(ref_usec(2007, 9, 15, 12, 0, 0, 0, -8 * 3600))
    );
}

#[test]
fn both_sides_of_ambiguous_numeric_dates() {
    // 06/21/95 is month first, because 21 cannot be a month.
    assert_eq!(parse_utc("06/21/95 04:24:34 PM"), Some(ref_usec(1995, 5, 21, 16, 24, 34, 0, 0)));
    // 20/06/95 is day first, because 20 cannot be a month.
    assert_eq!(parse_utc("20/06/95 21:07"), Some(ref_usec(1995, 5, 20, 21, 7, 0, 0, 0)));
    // 95-06-08 is year first.
    assert_eq!(
        parse_utc("95-06-08 19:32:48 EDT"),
        Some(ref_usec(1995, 5, 8, 19, 32, 48, 0, -4 * 3600))
    );
}

#[test]
fn ampm_rules() {
    assert_eq!(parse_utc("1 Jan 2000 12:00 am"), Some(ref_usec(2000, 0, 1, 0, 0, 0, 0, 0)));
    assert_eq!(parse_utc("1 Jan 2000 12:00 pm"), Some(ref_usec(2000, 0, 1, 12, 0, 0, 0, 0)));
    assert_eq!(parse_utc("1 Jan 2000 1:05pm"), Some(ref_usec(2000, 0, 1, 13, 5, 0, 0, 0)));
    assert_eq!(parse_utc("1 Jan 2000 12:05 AM"), Some(ref_usec(2000, 0, 1, 0, 5, 0, 0, 0)));
}

#[test]
fn fractional_seconds_are_truncated_to_microseconds() {
    assert_eq!(
        parse_utc("1 Jan 2000 00:00:00.1234567 GMT"),
        Some(ref_usec(2000, 0, 1, 0, 0, 0, 123_456, 0))
    );
    assert_eq!(
        parse_utc("1 Jan 2000 00:00:00.5 GMT"),
        Some(ref_usec(2000, 0, 1, 0, 0, 0, 500_000, 0))
    );
}

#[test]
fn nul_byte_ends_the_input() {
    // Bytes after a NUL are ignored, as with a C string.
    assert_eq!(
        parse_time_string(b"15 Oct 2007 19:45:00 GMT\0garbage", false),
        Some(COMPARISON_TIME_PDT)
    );
    // A NUL before the date leaves nothing to parse.
    assert_eq!(parse_time_string(b"\0 15 Oct 2007", false), None);
}

#[test]
fn rejects_strings_without_month_day_or_year() {
    assert_eq!(parse_utc(""), None);
    assert_eq!(parse_utc("12:00:00 GMT"), None);
    assert_eq!(parse_utc("15 2007 12:00"), None);
    assert_eq!(parse_utc("Oct 2007"), None);
    assert_eq!(parse_utc("Oct 15 12:00"), None);
}

#[test]
fn rejects_years_above_i16_max() {
    assert_eq!(parse_utc("15 Oct 32768 12:00 GMT"), None);
    assert!(parse_utc("15 Oct 32767 12:00 GMT").is_some());
}

#[test]
fn iteration_limit_is_1001_loop_iterations() {
    // Each "x-" costs one loop iteration. The 3 tokens of "Jan 1 2000" cost
    // three more, so 998 pairs reach exactly 1001 iterations.
    let ok = format!("{}Jan 1 2000", "x-".repeat(998));
    assert_eq!(parse_utc(&ok), Some(ref_usec(2000, 0, 1, 0, 0, 0, 0, 0)));
    let too_many = format!("{}Jan 1 2000", "x-".repeat(999));
    assert_eq!(parse_utc(&too_many), None);
}

#[test]
fn random_bytes_never_panic() {
    // The parser must return None or Some, never panic. The alphabet covers
    // every character the parser looks at, plus some high bytes.
    let alphabet: &[u8] =
        b"0123456789:/-+.,; \tTZzAPMapmJANGMTjanUTCESTJSTPDTMSTNSTBSTxyz()[]\x80\xff";
    let mut rng = Rng(0x1357_9bdf_2468_ace0);
    for _ in 0..50_000 {
        let len = rng.range(0, 40) as usize;
        let input: Vec<u8> =
            (0..len).map(|_| alphabet[rng.next() as usize % alphabet.len()]).collect();
        let _ = parse_time_string(&input, false);
        let _ = parse_time_string(&input, true);
    }
}

#[test]
fn accepted_formats_from_header_comment() {
    let cases = [
        "14 Apr 89 03:20:12",
        "14 Apr 89 03:20 GMT",
        "Fri, 17 Mar 89 4:01:33",
        "Fri, 17 Mar 89 4:01 GMT",
        "Mon Jan 16 16:12 PDT 1989",
        "Mon Jan 16 16:12 +0130 1989",
        "6 May 1992 16:41-JST (Wednesday)",
        "22-AUG-1993 10:59:12.82",
        "22-AUG-1993 10:59pm",
        "22-AUG-1993 12:59am",
        "22-AUG-1993 12:59 PM",
        "Friday, August 04, 1995 3:54 PM",
        "06/21/95 04:24:34 PM",
        "20/06/95 21:07",
        "95-06-08 19:32:48 EDT",
        "1995-06-17T23:11:25.342156Z",
    ];
    for text in cases {
        assert!(parse_utc(text).is_some(), "should accept {text:?}");
    }
}

#[test]
fn helper_round_trips_civil_dates() {
    for days in (-800_000..800_000).step_by(997) {
        let (y, m, d) = civil_from_days(days);
        assert_eq!(days_from_civil(y, m, d), days);
    }
}
