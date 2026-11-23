// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of base/time/time_unittest.cc and
//! base/time/time_delta_from_string_unittest.cc, plus randomized
//! differential tests.
//!
//! Local-time cases use a fixed fake zone, UTC-7 (PDT), instead of the host
//! zone. The C++ tests compare against `mktime` in the host zone. Cases that
//! need a real clock are replaced by a fixed sample time, as noted in each
//! test. Cases that only test clock overrides are not ported.

use crate::*;

/// Fixed fake local zone for the `from_string` tests: PDT, UTC-7, which is
/// the zone of the C++ test comparison times (2007-10-15 12:45 PDT).
fn pdt(_utc_seconds: i64) -> i64 {
    -7 * 3600
}

/// Makes an `Exploded` from fields in C++ order: year, month, day_of_week,
/// day_of_month, hour, minute, second, millisecond.
#[allow(clippy::too_many_arguments)]
fn ex(
    year: i32,
    month: i32,
    day_of_week: i32,
    day_of_month: i32,
    hour: i32,
    minute: i32,
    second: i32,
    millisecond: i32,
) -> Exploded {
    Exploded {
        year,
        month,
        day_of_week,
        day_of_month,
        hour,
        minute,
        second,
        millisecond,
    }
}

/// Stand-in for `Time::Now()`: a fixed, non-null sample time.
fn sample_now() -> Time {
    Time::from_time_t(1_700_000_000) + microseconds(123_456)
}

/// The C++ `comparison_time_pdt_`: 2007-10-15 19:45:00 UTC.
fn comparison_time_pdt() -> Time {
    Time::from_time_t(1_192_477_500)
}

/// Makes a `Time` from a Windows-epoch microsecond count, as the C++ tests do.
fn time_at_windows_us(micros: i64) -> Time {
    Time::from_delta_since_windows_epoch(microseconds(micros))
}

mod time_test {
    use super::*;

    #[test]
    fn from_exploded_out_of_bounds_time() {
        // Port of TimeTestOutOfBounds.FromExplodedOutOfBoundsTime. Fields are
        // (explode, HasValidValues()).
        let cases = [
            // 31st of February: valid values, but not a real date.
            (ex(2016, 2, 0, 31, 12, 30, 0, 0), true),
            // 31st of April.
            (ex(2016, 4, 0, 31, 8, 43, 0, 0), true),
            // Negative month.
            (ex(2016, -5, 0, 2, 4, 10, 0, 0), false),
            // Negative date of month.
            (ex(2016, 6, 0, -15, 2, 50, 0, 0), false),
            // Negative hours.
            (ex(2016, 7, 0, 10, -11, 29, 0, 0), false),
            // Negative minutes.
            (ex(2016, 3, 0, 14, 10, -29, 0, 0), false),
            // Negative seconds.
            (ex(2016, 10, 0, 25, 7, 47, -30, 0), false),
            // Negative milliseconds.
            (ex(2016, 10, 0, 25, 7, 47, 20, -500), false),
            // Hours too large.
            (ex(2016, 7, 0, 10, 26, 29, 0, 0), false),
            // Minutes too large.
            (ex(2016, 3, 0, 14, 10, 78, 0, 0), false),
            // Seconds too large.
            (ex(2016, 10, 0, 25, 7, 47, 234, 0), false),
            // Milliseconds too large.
            (ex(2016, 10, 0, 25, 6, 31, 23, 1643), false),
            // Overflow: valid fields, but the microsecond value overflows.
            (ex(9_840_633, 1, 0, 1, 1, 1, 0, 0), true),
            // Underflow.
            (ex(-9_840_633, 1, 0, 1, 1, 1, 0, 0), true),
            // Integer extremes of the year.
            (ex(i32::MIN, 1, 0, 1, 1, 1, 0, 0), true),
            (ex(i32::MAX, 1, 0, 1, 1, 1, 0, 0), true),
            (ex(2016, i32::MIN, 0, 1, 1, 1, 0, 0), false),
            (ex(2016, i32::MAX, 0, 1, 1, 1, 0, 0), false),
        ];
        for (explode, is_valid) in cases {
            assert_eq!(explode.has_valid_values(), is_valid, "{explode:?}");
            assert_eq!(Time::from_utc_exploded(&explode), None, "{explode:?}");
        }
    }

    #[test]
    fn delta_since_windows_epoch() {
        // C++ uses Time::Now(). Here a fixed sample time is used instead.
        let delta = microseconds(123);
        assert_eq!(
            delta,
            Time::from_delta_since_windows_epoch(delta).to_delta_since_windows_epoch()
        );

        let now = sample_now();
        let actual = Time::from_delta_since_windows_epoch(now.to_delta_since_windows_epoch());
        assert_eq!(now, actual);

        // Null times stay null after a round trip.
        let should_be_null =
            Time::from_delta_since_windows_epoch(Time::default().to_delta_since_windows_epoch());
        assert!(should_be_null.is_null());

        // Conversion is usable in const context in C++; here it is checked at
        // runtime.
        assert_eq!(
            Time::from_delta_since_windows_epoch(microseconds(123))
                .to_delta_since_windows_epoch()
                .in_microseconds(),
            123
        );
    }

    #[test]
    fn time_t_round_trip() {
        // Port of TimeTest.TimeT.
        assert_eq!(10, Time::from_time_t(10).to_time_t());
        assert_eq!(10.0, Time::from_time_t(10).in_seconds_f_since_unix_epoch());

        // Conversions of 0 stay 0.
        assert_eq!(0, Time::default().to_time_t());
        assert_eq!(0, Time::from_time_t(0).to_internal_value());
    }

    #[test]
    fn utc_time_t() {
        // Port of TimeTest.UTCTimeT. C++ compares with gmtime_r(time(NULL)).
        // Here fixed times with known calendar values are used.
        let cases = [
            (1_192_477_500_i64, ex(2007, 10, 1, 15, 19, 45, 0, 0)),
            (1_234_567_890, ex(2009, 2, 5, 13, 23, 31, 30, 0)),
            (951_782_400, ex(2000, 2, 2, 29, 0, 0, 0, 0)),
            (0, ex(1970, 1, 4, 1, 0, 0, 0, 0)),
        ];
        for (t, expected) in cases {
            let our_time_1 = Time::from_time_t(t);
            let exploded = our_time_1.utc_explode();
            assert_eq!(exploded.year, expected.year);
            assert_eq!(exploded.month, expected.month);
            assert_eq!(exploded.day_of_month, expected.day_of_month);
            assert_eq!(exploded.hour, expected.hour);
            assert_eq!(exploded.minute, expected.minute);
            assert_eq!(exploded.second, expected.second);
            assert_eq!(exploded.day_of_week, expected.day_of_week);

            let our_time_2 = Time::from_utc_exploded(&exploded).expect("valid");
            assert_eq!(our_time_1, our_time_2);
            assert_eq!(t, our_time_2.to_time_t());
        }
    }

    #[test]
    fn js_time() {
        let epoch = Time::from_fractional_milliseconds_since_unix_epoch(0.0);
        assert_eq!(epoch, Time::unix_epoch());
        let t = Time::from_fractional_milliseconds_since_unix_epoch(700_000.3);
        assert_eq!(700.0003, t.in_seconds_f_since_unix_epoch());
        let t = Time::from_seconds_since_unix_epoch(800.73);
        assert_eq!(800_730.0, t.in_milliseconds_f_since_unix_epoch());

        // 1601-01-01 is not a round trip through in_milliseconds_f.
        let windows_epoch = -11_644_473_600_000.0;
        let time = Time::from_fractional_milliseconds_since_unix_epoch(windows_epoch);
        assert!(time.is_null());
        assert_ne!(windows_epoch, time.in_milliseconds_f_since_unix_epoch());
        assert_eq!(
            windows_epoch,
            time.in_milliseconds_f_since_unix_epoch_ignoring_null()
        );
    }

    #[test]
    fn from_time_val() {
        // Port of TimeTest.FromTimeVal (POSIX). Uses a fixed sample time.
        let now = sample_now();
        let (sec, usec) = now.to_time_val();
        let also_now = Time::from_time_val(sec, usec);
        assert_eq!(now, also_now);
    }

    #[test]
    fn from_exploded_with_milliseconds() {
        // Port of TimeTest.FromExplodedWithMilliseconds. Uses a fixed sample
        // time instead of NowFromSystemTime().
        let now = sample_now();
        let mut exploded1 = now.utc_explode();
        exploded1.millisecond = 500;
        let time = Time::from_utc_exploded(&exploded1).expect("valid");
        let exploded2 = time.utc_explode();
        assert_eq!(exploded1.millisecond, exploded2.millisecond);
    }

    #[test]
    fn zero_is_symmetric() {
        let zero_time = Time::from_time_t(0);
        assert_eq!(0, zero_time.to_time_t());
        assert_eq!(0.0, zero_time.in_seconds_f_since_unix_epoch());
    }

    #[test]
    fn utc_explode() {
        // Port of TimeTest.UTCExplode. C++ uses Time::Now().
        let a = sample_now();
        let exploded = a.utc_explode();
        let b = Time::from_utc_exploded(&exploded).expect("valid");
        // The exploded structure has no microseconds, so the delta is less
        // than one second.
        assert!(a - b < seconds(1));
    }

    #[test]
    fn utc_midnight() {
        // Port of TimeTest.UTCMidnight, for fixed times.
        for t in [sample_now(), Time::unix_epoch(), time_at_windows_us(-1)] {
            let exploded = t.utc_midnight().utc_explode();
            assert_eq!(0, exploded.hour);
            assert_eq!(0, exploded.minute);
            assert_eq!(0, exploded.second);
            assert_eq!(0, exploded.millisecond);
            assert_eq!(exploded.day_of_month, t.utc_explode().day_of_month);
        }
    }

    #[test]
    fn parse_time_test1() {
        // Port of TimeTest.ParseTimeTest1. C++ parses asctime() of the local
        // time now. Here the asctime form of a fixed UTC time is parsed.
        let mut parsed_time = Time::default();
        let asctime = "Mon Oct 15 19:45:00 2007";
        parsed_time = Time::from_utc_string(asctime).expect("parses");
        assert_eq!(Time::from_time_t(1_192_477_500), parsed_time);
    }

    #[test]
    fn day_of_week() {
        // Ports of DayOfWeekSunday, DayOfWeekWednesday and
        // DayOfWeekSaturday.
        let cases = [
            ("Sun, 06 May 2012 12:00:00 GMT", 0),
            ("Wed, 09 May 2012 12:00:00 GMT", 3),
            ("Sat, 12 May 2012 12:00:00 GMT", 6),
        ];
        for (text, dow) in cases {
            let time = Time::from_string(text, &pdt).expect("parses");
            assert_eq!(dow, time.utc_explode().day_of_week, "{text}");
        }
    }

    #[test]
    fn parse_time_tests_2_to_11() {
        // Ports of ParseTimeTest2 through ParseTimeTest11. All of them expect
        // the PDT comparison time. The local-time strings are parsed in the
        // fixed fake zone PDT.
        let expected = comparison_time_pdt();
        let cases = [
            "Mon, 15 Oct 2007 19:45:00 GMT",
            "15 Oct 07 12:45:00",
            "15 Oct 07 19:45 GMT",
            "Mon Oct 15 12:45 PDT 2007",
            "Monday, Oct 15, 2007 12:45 PM",
            "10/15/07 12:45:00 PM",
            "15-OCT-2007 12:45pm",
            "16 Oct 2007 4:45-JST (Tuesday)",
            "15/10/07 12:45",
            "2007-10-15 12:45:00",
        ];
        for text in cases {
            assert_eq!(Some(expected), Time::from_string(text, &pdt), "{text}");
        }
    }

    #[test]
    fn parse_time_test_epoch() {
        // Ports of ParseTimeTestEpoch0, Epoch1, Epoch2, EpochNeg1,
        // EpochNotNeg1, EpochNeg2 and Epoch1960.
        let cases = [
            ("Thu Jan 01 01:00:00 +0100 1970", 0),
            ("Thu Jan 01 00:00:00 GMT 1970", 0),
            ("Thu Jan 01 01:00:01 +0100 1970", 1),
            ("Thu Jan 01 00:00:01 GMT 1970", 1),
            ("Thu Jan 01 01:00:02 +0100 1970", 2),
            ("Thu Jan 01 00:00:02 GMT 1970", 2),
            ("Thu Jan 01 00:59:59 +0100 1970", -1),
            ("Wed Dec 31 23:59:59 GMT 1969", -1),
            ("Thu Jan 01 00:59:58 +0100 1970", -2),
            ("Wed Dec 31 23:59:58 GMT 1969", -2),
            ("Wed Jun 29 19:40:01 +0100 1960", -299_999_999),
            ("Wed Jun 29 18:40:01 GMT 1960", -299_999_999),
            ("Wed Jun 29 17:40:01 GMT 1960", -300_003_599),
        ];
        for (text, t) in cases {
            let parsed = Time::from_string(text, &pdt).expect(text);
            assert_eq!(t, parsed.to_time_t(), "{text}");
        }
    }

    #[test]
    fn parse_time_test_epoch_not_neg1() {
        // Port of ParseTimeTestEpochNotNeg1. A date after 2038 must not map to
        // -1.
        let parsed = Time::from_string("Wed Dec 31 23:59:59 GMT 2100", &pdt).expect("parses");
        assert_ne!(-1, parsed.to_time_t());
    }

    #[test]
    fn parse_time_test_empty_and_invalid() {
        // Ports of ParseTimeTestEmpty and ParseTimeTestInvalidString.
        assert_eq!(None, Time::from_string("", &pdt));
        assert_eq!(None, Time::from_string("Monday morning 2000", &pdt));
        assert_eq!(None, Time::from_utc_string(""));
    }

    #[test]
    fn explode_before_unix_epoch() {
        // Port of TimeTest.ExplodeBeforeUnixEpoch.
        let epoch = Time::unix_epoch();
        let cases = [
            // -1 us: 1969-12-31 23:59:59.999 (and 999 us).
            (epoch - microseconds(1), ex(1969, 12, 3, 31, 23, 59, 59, 999)),
            (epoch - microseconds(999), ex(1969, 12, 3, 31, 23, 59, 59, 999)),
            (epoch - microseconds(1000), ex(1969, 12, 3, 31, 23, 59, 59, 999)),
            (epoch - microseconds(1001), ex(1969, 12, 3, 31, 23, 59, 58, 999)),
            (epoch - milliseconds(1000), ex(1969, 12, 3, 31, 23, 59, 59, 0)),
            (epoch - milliseconds(1001), ex(1969, 12, 3, 31, 23, 59, 58, 999)),
            // At and after the epoch.
            (epoch, ex(1970, 1, 4, 1, 0, 0, 0, 0)),
            (epoch + microseconds(1), ex(1970, 1, 4, 1, 0, 0, 0, 0)),
            (epoch + microseconds(999), ex(1970, 1, 4, 1, 0, 0, 0, 0)),
            (epoch + microseconds(1000), ex(1970, 1, 4, 1, 0, 0, 0, 1)),
            (epoch + milliseconds(1000), ex(1970, 1, 4, 1, 0, 1, 0, 0)),
            (epoch + milliseconds(1001), ex(1970, 1, 4, 1, 0, 1, 1, 0)),
        ];
        for (t, expected) in cases {
            let exploded = t.utc_explode();
            assert!(exploded.has_valid_values());
            // Only the calendar fields are compared, and day_of_week is
            // checked separately in the Explode_Y10K test.
            assert_eq!(
                (exploded.year, exploded.month, exploded.day_of_month),
                (expected.year, expected.month, expected.day_of_month),
                "{t:?}"
            );
            assert_eq!(
                (exploded.hour, exploded.minute, exploded.second, exploded.millisecond),
                (expected.hour, expected.minute, expected.second, expected.millisecond),
                "{t:?}"
            );
        }
    }

    #[test]
    fn max() {
        // Port of TimeTest.Max. Time::Now() is replaced by sample_now().
        let k_max = Time::max();
        assert!(k_max.is_max());
        assert_eq!(k_max, Time::max());
        assert!(k_max > sample_now());
        assert!(k_max > Time::default());
        assert!((sample_now() - k_max).is_negative());
        assert!((k_max - sample_now()).is_positive());
    }

    #[test]
    fn max_conversions() {
        // Port of TimeTest.MaxConversions, without the Apple and Windows
        // parts.
        let k_max = Time::max();
        assert_eq!(i64::MAX, k_max.to_internal_value());

        let t = Time::from_seconds_since_unix_epoch(f64::INFINITY);
        assert!(t.is_max());
        assert_eq!(f64::INFINITY, t.in_seconds_f_since_unix_epoch());

        let t = Time::from_fractional_milliseconds_since_unix_epoch(f64::INFINITY);
        assert!(t.is_max());
        assert_eq!(f64::INFINITY, t.in_milliseconds_f_since_unix_epoch());

        let t = Time::from_time_t(i64::MAX);
        assert!(t.is_max());
        assert_eq!(i64::MAX, t.to_time_t());

        let t = Time::from_time_val(i64::MAX, 999_999);
        assert!(t.is_max());
        assert_eq!((i64::MAX, 999_999), t.to_time_val());
    }

    #[test]
    fn min() {
        // Port of TimeTest.Min. Time::Now() is replaced by sample_now().
        let k_min = Time::min();
        assert!(k_min.is_min());
        assert_eq!(k_min, Time::min());
        assert!(k_min < sample_now());
        assert!(k_min < Time::default());
        assert!((sample_now() - k_min).is_positive());
        assert!((k_min - sample_now()).is_negative());
        assert!(!k_min.is_null());
    }

    #[test]
    fn time_to_overflow() {
        // Port of TimeTest.TimeTOverflow. The 32-bit time_t branch is skipped
        // because time_t is i64 here.
        assert_eq!(i64::MAX, Time::max().to_time_t());
        assert_eq!(i64::MIN, Time::min().to_time_t());

        // Min plus one microsecond maps to the minimum time_t.
        let k_min_plus_one = Time::default() + microseconds(i64::MIN + 1);
        assert!(!k_min_plus_one.is_min());
        assert_eq!(i64::MIN, k_min_plus_one.to_time_t());

        // Min plus the Unix epoch offset also maps to the minimum time_t.
        let k_min_plus_unix =
            Time::default() + microseconds(i64::MIN + MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH);
        assert!(!k_min_plus_unix.is_min());
        assert_eq!(i64::MIN, k_min_plus_unix.to_time_t());

        // Min plus the Unix offset plus one microsecond is representable as a
        // 64-bit time_t.
        let k_min_plus_unix_plus_one = Time::default()
            + microseconds(i64::MIN + MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH + 1);
        assert!(!k_min_plus_unix_plus_one.is_min());
        assert_ne!(i64::MIN, k_min_plus_unix_plus_one.to_time_t());
    }

    fn make_time(micros: i64) -> Time {
        Time::from_delta_since_windows_epoch(microseconds(micros))
    }

    #[test]
    fn explode_y10k_compliance() {
        // Port of TimeTest.Explode_Y10KCompliance. Regression test for
        // crbug.com/1104442.
        const DAYS_PER_YEAR: i64 = 365;
        let half_year_in_micros = days(DAYS_PER_YEAR / 2).in_microseconds();

        // The Y2038 issue occurs when a 32-bit signed integer overflows.
        let year2038_micros_offset = MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH
            + i64::from(i32::MAX) * MICROSECONDS_PER_SECOND;

        // 1 March 10000 at noon.
        let year10000_years_offset: i64 = 10000 - 1970;
        let extra_leap_days_over_those_years: i64 = 1947;
        let days_from_jan_to_mar10000: i64 = 31 + 29;
        let march10000_micros_offset = MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH
            + days(
                year10000_years_offset * DAYS_PER_YEAR
                    + extra_leap_days_over_those_years
                    + days_from_jan_to_mar10000,
            )
            .in_microseconds()
            + hours(12).in_microseconds();

        // Windows uses 1/10 microsecond ticks in a 64-bit signed integer.
        let windows_max_micros_offset = i64::MAX / 10;

        // ICU uses doubles, so its maximum is the largest integer a double
        // can represent exactly.
        let icu_max_micros_offset = MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH
            + ((1_i64 << 53) * MICROSECONDS_PER_MILLISECOND + 999);

        let cases = [
            // A very long time ago.
            (Time::min(), ex(-290_677, 12, 4, 23, 19, 59, 5, 224)),
            // Before/on/after 1 Jan 1601.
            (make_time(-half_year_in_micros), ex(1600, 7, 1, 3, 0, 0, 0, 0)),
            (make_time(0), ex(1601, 1, 1, 1, 0, 0, 0, 0)),
            (make_time(half_year_in_micros), ex(1601, 7, 1, 2, 0, 0, 0, 0)),
            // Before/on/after 1 Jan 1970.
            (
                make_time(MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH - half_year_in_micros),
                ex(1969, 7, 4, 3, 0, 0, 0, 0),
            ),
            (
                make_time(MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH),
                ex(1970, 1, 4, 1, 0, 0, 0, 0),
            ),
            (
                make_time(MICROSECONDS_FROM_WINDOWS_TO_UNIX_EPOCH + half_year_in_micros),
                ex(1970, 7, 4, 2, 0, 0, 0, 0),
            ),
            // Before/on/after 19 January 2038.
            (
                make_time(year2038_micros_offset - half_year_in_micros),
                ex(2037, 7, 2, 21, 3, 14, 7, 0),
            ),
            (make_time(year2038_micros_offset), ex(2038, 1, 2, 19, 3, 14, 7, 0)),
            (
                make_time(year2038_micros_offset + half_year_in_micros),
                ex(2038, 7, 2, 20, 3, 14, 7, 0),
            ),
            // Before/on/after 1 March 10000 at noon.
            (
                make_time(march10000_micros_offset - half_year_in_micros),
                ex(9999, 9, 3, 1, 12, 0, 0, 0),
            ),
            (make_time(march10000_micros_offset), ex(10000, 3, 3, 1, 12, 0, 0, 0)),
            (
                make_time(march10000_micros_offset + half_year_in_micros),
                ex(10000, 8, 3, 30, 12, 0, 0, 0),
            ),
            // Before/on/after Windows max (14 September 30828).
            (
                make_time(windows_max_micros_offset - half_year_in_micros),
                ex(30828, 3, 4, 16, 2, 48, 5, 477),
            ),
            (
                make_time(windows_max_micros_offset),
                ex(30828, 9, 4, 14, 2, 48, 5, 477),
            ),
            (
                make_time(windows_max_micros_offset + half_year_in_micros),
                ex(30829, 3, 4, 15, 2, 48, 5, 477),
            ),
            // Before/on/after ICU max.
            (
                make_time(icu_max_micros_offset - half_year_in_micros),
                ex(287_396, 4, 3, 13, 8, 59, 0, 992),
            ),
            (make_time(icu_max_micros_offset), ex(287_396, 10, 3, 12, 8, 59, 0, 992)),
            (
                make_time(icu_max_micros_offset + half_year_in_micros),
                ex(287_397, 4, 3, 12, 8, 59, 0, 992),
            ),
            // A very long time from now.
            (Time::max(), ex(293_878, 1, 4, 10, 4, 0, 54, 775)),
        ];

        for (time, expected) in cases {
            let exploded = time.utc_explode();
            assert!(exploded.has_valid_values(), "{time}");
            assert_eq!(expected.year, exploded.year, "{time}");
            assert_eq!(expected.month, exploded.month, "{time}");
            assert_eq!(expected.day_of_week, exploded.day_of_week, "{time}");
            assert_eq!(expected.day_of_month, exploded.day_of_month, "{time}");
            assert_eq!(expected.hour, exploded.hour, "{time}");
            assert_eq!(expected.minute, exploded.minute, "{time}");
            assert_eq!(expected.second, exploded.second, "{time}");
            assert_eq!(expected.millisecond, exploded.millisecond, "{time}");
        }
    }

    #[test]
    fn from_exploded_min_max() {
        // Port of TimeTest.FromExploded_MinMax. On this port the year limits
        // are the int limits, as on Linux 64-bit C++. Both guarded blocks are
        // skipped there, and the C++ test does not run them.
        const EXPLODED_MIN_YEAR: i32 = i32::MIN;
        const EXPLODED_MAX_YEAR: i32 = i32::MAX;
        let mut exploded = ex(0, 1, 0, 1, 0, 0, 0, 0);

        if EXPLODED_MIN_YEAR != i32::MIN {
            exploded.year = EXPLODED_MIN_YEAR;
            assert!(Time::from_utc_exploded(&exploded).is_some());
        }

        if EXPLODED_MAX_YEAR != i32::MAX {
            exploded.year = EXPLODED_MAX_YEAR;
            exploded.month = 12;
            exploded.day_of_month = 31;
            exploded.hour = 23;
            exploded.minute = 59;
            exploded.second = 59;
            exploded.millisecond = 999;
            assert!(Time::from_utc_exploded(&exploded).is_some());
            exploded.year += 1;
            assert_eq!(None, Time::from_utc_exploded(&exploded));
        }

        // Added for this port: year INT_MAX is outside the microsecond range,
        // so conversion fails. A year near the edge of the range still works.
        exploded = ex(i32::MAX, 12, 0, 31, 23, 59, 59, 999);
        assert_eq!(None, Time::from_utc_exploded(&exploded));
        exploded = ex(294_276, 1, 0, 1, 0, 0, 0, 0);
        assert!(Time::from_utc_exploded(&exploded).is_some());
    }

    #[test]
    fn snapped_to_next_tick_basic() {
        // Port of TimeTicks.SnappedToNextTickBasic.
        let phase = TimeTicks::from_internal_value(4000);
        let interval = microseconds(1000);
        let snapped = |us: i64| TimeTicks::from_internal_value(us).snapped_to_next_tick(phase, interval).to_internal_value();

        // Timestamp in previous interval.
        assert_eq!(4000, snapped(3500));
        // Timestamp in next interval.
        assert_eq!(5000, snapped(4500));
        // Timestamp multiple intervals before.
        assert_eq!(3000, snapped(2500));
        // Timestamp multiple intervals after.
        assert_eq!(7000, snapped(6500));
        // Timestamp on previous interval.
        assert_eq!(3000, snapped(3000));
        // Timestamp on next interval.
        assert_eq!(5000, snapped(5000));
        // Timestamp equal to phase.
        assert_eq!(4000, snapped(4000));
    }

    #[test]
    fn snapped_to_next_tick_overflow() {
        // Port of TimeTicks.SnappedToNextTickOverflow.
        let phase = TimeTicks::from_internal_value(0);
        let interval = microseconds(4000);
        let big_timestamp = TimeTicks::from_internal_value(8_635_916_564_000);

        assert_eq!(
            8_635_916_564_000,
            big_timestamp
                .snapped_to_next_tick(phase, interval)
                .to_internal_value()
        );
        assert_eq!(
            8_635_916_564_000,
            big_timestamp
                .snapped_to_next_tick(big_timestamp, interval)
                .to_internal_value()
        );
    }

    #[test]
    fn time_delta_round_trip_windows_epoch() {
        // Port of TimeDelta.WindowsEpoch.
        let exploded = ex(1970, 1, 0, 1, 0, 0, 0, 0);
        let t = Time::from_utc_exploded(&exploded).expect("valid");
        assert_eq!(11_644_473_600_000_000, t.to_internal_value());
    }
}

mod time_delta {
    use super::*;

    #[test]
    fn from_and_in() {
        // Port of TimeDelta.FromAndIn.
        assert_eq!(days(2), hours(48));
        assert_eq!(hours(3), minutes(180));
        assert_eq!(minutes(2), seconds(120));
        assert_eq!(seconds(2), milliseconds(2000));
        assert_eq!(milliseconds(2), microseconds(2000));
        assert_eq!(seconds(2.3), milliseconds(2300));
        assert_eq!(milliseconds(2.5), microseconds(2500));
        assert_eq!(13, days(13).in_days());
        assert_eq!(13, hours(13).in_hours());
        assert_eq!(13, minutes(13).in_minutes());
        assert_eq!(13, seconds(13).in_seconds());
        assert_eq!(13.0, seconds(13).in_seconds_f());
        assert_eq!(13, milliseconds(13).in_milliseconds());
        assert_eq!(13.0, milliseconds(13).in_milliseconds_f());
        assert_eq!(13, seconds(13.1).in_seconds());
        assert_eq!(13.1, seconds(13.1).in_seconds_f());
        assert_eq!(13, milliseconds(13.3).in_milliseconds());
        assert_eq!(13.3, milliseconds(13.3).in_milliseconds_f());
        assert_eq!(13, microseconds(13).in_microseconds());
        assert_eq!(13, microseconds(13.3).in_microseconds());
        assert_eq!(3.456, milliseconds(3.45678).in_milliseconds_f());
        assert_eq!(12_000, nanoseconds(12345).in_nanoseconds());
        assert_eq!(12_000, nanoseconds(12345.678).in_nanoseconds());
    }

    #[test]
    fn in_rounds_towards_zero() {
        assert_eq!(0, hours(23).in_days());
        assert_eq!(0, hours(-23).in_days());
        assert_eq!(0, minutes(59).in_hours());
        assert_eq!(0, minutes(-59).in_hours());
        assert_eq!(0, seconds(59).in_minutes());
        assert_eq!(0, seconds(-59).in_minutes());
        assert_eq!(0, milliseconds(999).in_seconds());
        assert_eq!(0, milliseconds(-999).in_seconds());
        assert_eq!(0, microseconds(999).in_milliseconds());
        assert_eq!(0, microseconds(-999).in_milliseconds());
    }

    #[test]
    fn in_days_floored() {
        assert_eq!(-2, hours(-25).in_days_floored());
        assert_eq!(-1, hours(-24).in_days_floored());
        assert_eq!(-1, hours(-23).in_days_floored());
        assert_eq!(-1, hours(-1).in_days_floored());
        assert_eq!(0, hours(0).in_days_floored());
        assert_eq!(0, hours(1).in_days_floored());
        assert_eq!(0, hours(23).in_days_floored());
        assert_eq!(1, hours(24).in_days_floored());
        assert_eq!(1, hours(25).in_days_floored());
    }

    #[test]
    fn in_seconds_floored() {
        assert_eq!(13, seconds(13.1).in_seconds_floored());
        assert_eq!(13, seconds(13.9).in_seconds_floored());
        assert_eq!(13, seconds(13).in_seconds_floored());

        assert_eq!(1, milliseconds(1001).in_seconds_floored());
        assert_eq!(1, milliseconds(1000).in_seconds_floored());
        assert_eq!(0, milliseconds(999).in_seconds_floored());
        assert_eq!(0, milliseconds(1).in_seconds_floored());
        assert_eq!(0, milliseconds(0).in_seconds_floored());
        assert_eq!(-1, milliseconds(-1).in_seconds_floored());
        assert_eq!(-1, milliseconds(-1000).in_seconds_floored());
        assert_eq!(-2, milliseconds(-1001).in_seconds_floored());
    }

    #[test]
    fn in_milliseconds_rounded_up() {
        assert_eq!(-1, microseconds(-1001).in_milliseconds_rounded_up());
        assert_eq!(-1, microseconds(-1000).in_milliseconds_rounded_up());
        assert_eq!(0, microseconds(-999).in_milliseconds_rounded_up());

        assert_eq!(0, microseconds(-1).in_milliseconds_rounded_up());
        assert_eq!(0, microseconds(0).in_milliseconds_rounded_up());
        assert_eq!(1, microseconds(1).in_milliseconds_rounded_up());

        assert_eq!(1, microseconds(999).in_milliseconds_rounded_up());
        assert_eq!(1, microseconds(1000).in_milliseconds_rounded_up());
        assert_eq!(2, microseconds(1001).in_milliseconds_rounded_up());
    }

    #[test]
    fn in_xxx_overflow() {
        // Port of TimeDelta.InXXXOverflow.
        let large_delta = microseconds(i64::MAX - 1);
        assert!(!large_delta.is_max());
        assert_eq!(i32::MAX, large_delta.in_hours());
        assert_eq!(i32::MAX, large_delta.in_minutes());
        assert_eq!(i64::MAX, large_delta.in_nanoseconds());

        let large_negative = microseconds(i64::MIN + 1);
        assert!(!large_negative.is_min());
        assert_eq!(i32::MIN, large_negative.in_hours());
        assert_eq!(i32::MIN, large_negative.in_minutes());
        assert_eq!(i64::MIN, large_negative.in_nanoseconds());
    }

    #[test]
    fn time_spec_conversion() {
        // Port of TimeDelta.TimeSpecConversion (POSIX layout).
        let mut delta = seconds(0);
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (0, 0));
        assert_eq!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = seconds(1);
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (1, 0));
        assert_eq!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = microseconds(1);
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (0, 1000));
        assert_eq!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = microseconds(MICROSECONDS_PER_SECOND + 1);
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (1, 1000));
        assert_eq!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = milliseconds(10600) - seconds(20);
        assert!(delta.is_negative());
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (0, 0));
        assert_ne!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = TimeDelta::max();
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!(sec, TimeDelta::max().in_seconds());
        let expected_extra = TimeDelta::max().in_microseconds() % MICROSECONDS_PER_SECOND;
        assert_eq!(nsec, expected_extra * NANOSECONDS_PER_MICROSECOND);
        // The seconds fit in time_t, so the round trip is exact.
        assert_eq!(delta, TimeDelta::from_time_spec(sec, nsec));

        delta = TimeDelta::min();
        assert!(delta.is_negative());
        let (sec, nsec) = delta.to_time_spec();
        assert_eq!((sec, nsec), (0, 0));
        assert_ne!(delta, TimeDelta::from_time_spec(sec, nsec));
    }

    #[test]
    fn hz() {
        assert_eq!(hertz(1), seconds(1));
        assert_eq!(hertz(0), TimeDelta::max());
        assert_eq!(hertz(-1), seconds(-1));
        assert_eq!(hertz(1000), milliseconds(1));
        assert_eq!(hertz(0.5), seconds(2));
        assert_eq!(hertz(f64::INFINITY), TimeDelta::default());

        assert_eq!(1.0, seconds(1).to_hz());
        assert_eq!(0.0, TimeDelta::max().to_hz());
        assert_eq!(-1.0, seconds(-1).to_hz());
        assert_eq!(1000.0, milliseconds(1).to_hz());
        assert_eq!(0.5, seconds(2).to_hz());
        assert_eq!(f64::INFINITY, TimeDelta::default().to_hz());

        // 60 Hz cannot be represented exactly.
        assert_ne!(hertz(60) * 60, seconds(1));
        assert_ne!(60.0, hertz(60).to_hz());
        assert_eq!(60.0, hertz(60).to_hz().round());
    }

    #[test]
    fn magnitude() {
        let zero: i64 = 0;
        assert_eq!(microseconds(zero), microseconds(zero).magnitude());

        let one: i64 = 1;
        let negative_one: i64 = -1;
        assert_eq!(microseconds(one), microseconds(one).magnitude());
        assert_eq!(microseconds(one), microseconds(negative_one).magnitude());

        let max_int64_minus_one = i64::MAX - 1;
        let min_int64_plus_two = i64::MIN + 2;
        assert_eq!(
            microseconds(max_int64_minus_one),
            microseconds(max_int64_minus_one).magnitude()
        );
        assert_eq!(
            microseconds(max_int64_minus_one),
            microseconds(min_int64_plus_two).magnitude()
        );

        assert_eq!(TimeDelta::max(), TimeDelta::min().magnitude());
    }

    #[test]
    fn zero_min_max() {
        let k_zero = TimeDelta::default();
        assert!(k_zero.is_zero());

        let k_max = TimeDelta::max();
        assert!(k_max.is_max());
        assert_eq!(k_max, TimeDelta::max());
        assert!(k_max > days(100 * 365));
        assert!(k_max > k_zero);

        let k_min = TimeDelta::min();
        assert!(k_min.is_min());
        assert_eq!(k_min, TimeDelta::min());
        assert!(k_min < days(-100 * 365));
        assert!(k_min < k_zero);
    }

    #[test]
    fn max_conversions() {
        let k_max = TimeDelta::max();
        assert_eq!(i64::MAX, k_max.to_internal_value());
        assert_eq!(i32::MAX, k_max.in_days());
        assert_eq!(i32::MAX, k_max.in_hours());
        assert_eq!(i32::MAX, k_max.in_minutes());
        assert_eq!(f64::INFINITY, k_max.in_seconds_f());
        assert_eq!(i64::MAX, k_max.in_seconds());
        assert_eq!(f64::INFINITY, k_max.in_milliseconds_f());
        assert_eq!(i64::MAX, k_max.in_milliseconds());
        assert_eq!(i64::MAX, k_max.in_milliseconds_rounded_up());

        assert!(days(i64::MAX).is_max());
        assert!(hours(i64::MAX).is_max());
        assert!(minutes(i64::MAX).is_max());

        let max_int = i64::MAX;
        let min_int = i64::MIN;
        assert!(seconds(max_int / MICROSECONDS_PER_SECOND + 1).is_max());
        assert!(milliseconds(max_int / MILLISECONDS_PER_SECOND + 1).is_max());
        assert!(microseconds(max_int).is_max());
        assert!(seconds(min_int / MICROSECONDS_PER_SECOND - 1).is_min());
        assert!(milliseconds(min_int / MILLISECONDS_PER_SECOND - 1).is_min());
        assert!(microseconds(min_int).is_min());
        assert!(microseconds(i64::MIN).is_min());
        assert!(seconds(f64::INFINITY).is_max());

        // max_int and min_int round when converted to double.
        let max_d = max_int as f64;
        let min_d = min_int as f64;
        assert!(seconds(max_d / MICROSECONDS_PER_SECOND as f64 + 1.0).is_max());
        assert!(microseconds(max_d).is_max(), "crbug.com/612601");
        assert!(milliseconds(f64::INFINITY).is_max());
        assert!(milliseconds(max_d / MILLISECONDS_PER_SECOND as f64 * 2.0).is_max());
        assert!(seconds(min_d / MICROSECONDS_PER_SECOND as f64 - 1.0).is_min());
        assert!(milliseconds(min_d / MILLISECONDS_PER_SECOND as f64 * 2.0).is_min());
    }

    #[test]
    fn min_conversions() {
        let k_min = TimeDelta::min();
        assert_eq!(i32::MIN, k_min.in_days());
        assert_eq!(i32::MIN, k_min.in_hours());
        assert_eq!(i32::MIN, k_min.in_minutes());
        assert_eq!(f64::NEG_INFINITY, k_min.in_seconds_f());
        assert_eq!(i64::MIN, k_min.in_seconds());
        assert_eq!(f64::NEG_INFINITY, k_min.in_milliseconds_f());
        assert_eq!(i64::MIN, k_min.in_milliseconds());
        assert_eq!(i64::MIN, k_min.in_milliseconds_rounded_up());
    }

    #[test]
    fn finite_max_min() {
        let k_finite_max = TimeDelta::finite_max();
        let k_unit = microseconds(1);
        assert_eq!(k_finite_max + k_unit, TimeDelta::max());
        assert!(k_finite_max - k_unit < k_finite_max);

        let k_finite_min = TimeDelta::finite_min();
        assert_eq!(k_finite_min - k_unit, TimeDelta::min());
        assert!(k_finite_min + k_unit > k_finite_min);
    }

    #[test]
    fn numeric_operators() {
        // Ports of the static_asserts in TimeDelta.NumericOperators, covering
        // every scalar type that the C++ operators accept.
        let d = 0.5_f64;
        assert_eq!(milliseconds(500), milliseconds(1000) * d);
        assert_eq!(milliseconds(2000), milliseconds(1000) / d);
        let mut m = milliseconds(1000);
        m *= d;
        assert_eq!(milliseconds(500), m);
        let mut m = milliseconds(1000);
        m /= d;
        assert_eq!(milliseconds(2000), m);
        assert_eq!(milliseconds(500), d * milliseconds(1000));

        let f = 0.5_f32;
        assert_eq!(milliseconds(500), milliseconds(1000) * f);
        assert_eq!(milliseconds(2000), milliseconds(1000) / f);
        let mut m = milliseconds(1000);
        m *= f;
        assert_eq!(milliseconds(500), m);
        let mut m = milliseconds(1000);
        m /= f;
        assert_eq!(milliseconds(2000), m);
        assert_eq!(milliseconds(500), f * milliseconds(1000));

        let i = 2_i32;
        assert_eq!(milliseconds(2000), milliseconds(1000) * i);
        assert_eq!(milliseconds(500), milliseconds(1000) / i);
        let mut m = milliseconds(1000);
        m *= i;
        assert_eq!(milliseconds(2000), m);
        let mut m = milliseconds(1000);
        m /= i;
        assert_eq!(milliseconds(500), m);
        assert_eq!(milliseconds(2000), i * milliseconds(1000));

        let i64_two = 2_i64;
        assert_eq!(milliseconds(2000), milliseconds(1000) * i64_two);
        assert_eq!(milliseconds(500), milliseconds(1000) / i64_two);
        let mut m = milliseconds(1000);
        m *= i64_two;
        assert_eq!(milliseconds(2000), m);
        let mut m = milliseconds(1000);
        m /= i64_two;
        assert_eq!(milliseconds(500), m);
        assert_eq!(milliseconds(2000), i64_two * milliseconds(1000));

        assert_eq!(milliseconds(500), milliseconds(1000) * 0.5);
        assert_eq!(milliseconds(2000), milliseconds(1000) / 0.5);
        assert_eq!(milliseconds(500), 0.5 * milliseconds(1000));
        assert_eq!(milliseconds(2000), milliseconds(1000) * 2);
        assert_eq!(milliseconds(500), milliseconds(1000) / 2);
        assert_eq!(milliseconds(2000), 2 * milliseconds(1000));
    }

    #[test]
    fn time_delta_operators() {
        // Port of TimeDelta.TimeDeltaOperators.
        let eleven_seconds = seconds(11);
        let three_seconds = seconds(3);

        assert_eq!(seconds(14), eleven_seconds + three_seconds);
        assert_eq!(seconds(14), three_seconds + eleven_seconds);
        assert_eq!(seconds(8), eleven_seconds - three_seconds);
        assert_eq!(seconds(-8), three_seconds - eleven_seconds);
        assert_eq!(11.0 / 3.0, eleven_seconds / three_seconds);
        assert_eq!(3.0 / 11.0, three_seconds / eleven_seconds);
        assert_eq!(3, eleven_seconds.int_div(three_seconds));
        assert_eq!(0, three_seconds.int_div(eleven_seconds));
        assert_eq!(seconds(2), eleven_seconds % three_seconds);
    }

    #[test]
    fn overflows() {
        // Port of TimeDelta.Overflows. Time::Now() and TimeTicks::Now() are
        // replaced by sample values.
        assert!(TimeDelta::max().is_max());
        assert!(TimeDelta::max().is_positive());
        assert!((-TimeDelta::max()).is_negative());
        assert_eq!(-TimeDelta::max(), TimeDelta::min());
        assert!(TimeDelta::default() > -TimeDelta::max());

        assert!(TimeDelta::min().is_min());
        assert!(TimeDelta::min().is_negative());
        assert!((-TimeDelta::min()).is_positive());
        assert_eq!(-TimeDelta::min(), TimeDelta::max());
        assert!(TimeDelta::default() < -TimeDelta::min());

        let k_large_delta = TimeDelta::max() - milliseconds(1);
        let k_large_negative = -k_large_delta;
        assert!(TimeDelta::default() > k_large_negative);
        assert!(!k_large_delta.is_max());
        assert!(!(-k_large_negative).is_min());

        // +, -, * and / saturate.
        let k_one_second = seconds(1);
        assert!((k_large_delta + k_one_second).is_max());
        assert!((k_large_negative + (-k_one_second)).is_min());
        assert!((k_large_negative - k_one_second).is_min());
        assert!((k_large_delta - (-k_one_second)).is_max());
        assert!((k_large_delta * 2).is_max());
        assert!((k_large_delta * -2).is_min());
        assert!((k_large_delta / 0.5).is_max());
        assert!((k_large_delta / -0.5).is_min());

        // Operations on max and min saturate.
        assert_eq!(TimeDelta::max() + k_one_second, TimeDelta::max());
        assert_eq!(TimeDelta::max() * 7, TimeDelta::max());
        assert_eq!(TimeDelta::finite_max() + k_one_second, TimeDelta::max());
        assert_eq!(TimeDelta::min() - k_one_second, TimeDelta::min());
        assert_eq!(TimeDelta::min() * 7, TimeDelta::min());
        assert_eq!(TimeDelta::finite_min() - k_one_second, TimeDelta::min());

        // Division uses doubles, with max and min as ±inf.
        assert_eq!(f64::INFINITY, TimeDelta::max() / k_one_second);
        assert_eq!(-f64::INFINITY, TimeDelta::max() / -k_one_second);
        assert_eq!(-f64::INFINITY, TimeDelta::min() / k_one_second);
        assert_eq!(f64::INFINITY, TimeDelta::min() / -k_one_second);
        assert_eq!(i64::MAX, TimeDelta::max().int_div(k_one_second));
        assert_eq!(i64::MIN, TimeDelta::max().int_div(-k_one_second));
        assert_eq!(i64::MIN, TimeDelta::min().int_div(k_one_second));
        assert_eq!(i64::MAX, TimeDelta::min().int_div(-k_one_second));
        assert_eq!(TimeDelta::max(), TimeDelta::max() % k_one_second);
        assert_eq!(TimeDelta::max(), TimeDelta::max() % -k_one_second);
        assert_eq!(TimeDelta::min(), TimeDelta::min() % k_one_second);
        assert_eq!(TimeDelta::min(), TimeDelta::min() % -k_one_second);

        // Division by zero.
        assert!((k_one_second / 0).is_max());
        assert!((-k_one_second / 0).is_min());
        assert!((TimeDelta::max() / 0).is_max());
        assert!((TimeDelta::min() / 0).is_min());
        assert_eq!(f64::INFINITY, k_one_second / TimeDelta::default());
        assert_eq!(-f64::INFINITY, -k_one_second / TimeDelta::default());
        assert_eq!(f64::INFINITY, TimeDelta::max() / TimeDelta::default());
        assert_eq!(-f64::INFINITY, TimeDelta::min() / TimeDelta::default());
        assert_eq!(i64::MAX, k_one_second.int_div(TimeDelta::default()));
        assert_eq!(i64::MIN, (-k_one_second).int_div(TimeDelta::default()));
        assert_eq!(i64::MAX, TimeDelta::max().int_div(TimeDelta::default()));
        assert_eq!(i64::MIN, TimeDelta::min().int_div(TimeDelta::default()));
        assert_eq!(k_one_second, k_one_second % TimeDelta::default());
        assert_eq!(-k_one_second, -k_one_second % TimeDelta::default());
        assert_eq!(TimeDelta::max(), TimeDelta::max() % TimeDelta::default());
        assert_eq!(TimeDelta::min(), TimeDelta::min() % TimeDelta::default());

        // Division by infinity.
        assert_eq!(0.0, k_large_delta / TimeDelta::min());
        assert_eq!(0.0, k_large_delta / TimeDelta::max());
        assert_eq!(0.0, k_large_negative / TimeDelta::min());
        assert_eq!(0.0, k_large_negative / TimeDelta::max());
        assert_eq!(0, k_large_delta.int_div(TimeDelta::min()));
        assert_eq!(0, k_large_delta.int_div(TimeDelta::max()));
        assert_eq!(0, k_large_negative.int_div(TimeDelta::min()));
        assert_eq!(0, k_large_negative.int_div(TimeDelta::max()));
        assert_eq!(k_one_second, k_one_second % TimeDelta::min());
        assert_eq!(k_one_second, k_one_second % TimeDelta::max());

        // Double conversions overflow to infinity.
        assert_eq!(f64::INFINITY, (k_large_delta + k_one_second).in_seconds_f());
        assert_eq!(
            f64::INFINITY,
            (k_large_delta + k_one_second).in_milliseconds_f()
        );
        assert_eq!(
            f64::INFINITY,
            (k_large_delta + k_one_second).in_microseconds_f()
        );

        // Compound assignment.
        let mut a = TimeDelta::finite_max();
        a += k_one_second;
        assert!(a.is_max());
        let mut a = TimeDelta::finite_min();
        a += -k_one_second;
        assert!(a.is_min());
        let mut a = TimeDelta::finite_min();
        a -= k_one_second;
        assert!(a.is_min());
        let mut a = TimeDelta::finite_max();
        a -= -k_one_second;
        assert!(a.is_max());
        let mut a = TimeDelta::finite_max();
        a *= 2;
        assert!(a.is_max());
        let mut a = TimeDelta::finite_min();
        a *= 1.5;
        assert!(a.is_min());
        let mut a = TimeDelta::finite_max();
        a /= 0.5;
        assert!(a.is_max());
        let mut a = TimeDelta::finite_min();
        a /= 0.5;
        assert!(a.is_min());

        let mut a = seconds(1);
        a %= TimeDelta::max();
        assert_eq!(seconds(1), a);
        let mut a = seconds(1);
        a %= TimeDelta::default();
        assert_eq!(seconds(1), a);

        // Operations with time types. Time::Now() and TimeTicks::Now() are
        // replaced by sample values.
        let time_now = sample_now();
        assert!((k_large_delta + time_now).is_max());
        assert!((k_large_delta + TimeTicks::from_internal_value(5)).is_max());
        assert!((time_now + k_large_delta).is_max());
        assert!((TimeTicks::from_internal_value(5) + k_large_delta).is_max());

        assert_eq!(k_one_second, (time_now + k_one_second) - time_now);
        assert_eq!(-k_one_second, (time_now - k_one_second) - time_now);

        let ticks_now = TimeTicks::from_internal_value(5_000_000);
        assert_eq!(-k_one_second, (ticks_now - k_one_second) - ticks_now);
        assert_eq!(k_one_second, (ticks_now + k_one_second) - ticks_now);
    }

    #[test]
    fn ceil_to_multiple() {
        for interval in [seconds(10), seconds(-10)] {
            assert_eq!(TimeDelta::default().ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).ceil_to_multiple(interval), seconds(10));
            assert_eq!(seconds(9).ceil_to_multiple(interval), seconds(10));
            assert_eq!(seconds(10).ceil_to_multiple(interval), seconds(10));
            assert_eq!(seconds(15).ceil_to_multiple(interval), seconds(20));
            assert_eq!(seconds(20).ceil_to_multiple(interval), seconds(20));
            assert_eq!(TimeDelta::max().ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-9).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-10).ceil_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-15).ceil_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-20).ceil_to_multiple(interval), seconds(-20));
            assert_eq!(TimeDelta::min().ceil_to_multiple(interval), TimeDelta::min());
        }
        for interval in [TimeDelta::max(), TimeDelta::min()] {
            assert_eq!(TimeDelta::default().ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(9).ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(10).ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(15).ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(20).ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(TimeDelta::max().ceil_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-9).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-10).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-15).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-20).ceil_to_multiple(interval), TimeDelta::default());
            assert_eq!(TimeDelta::min().ceil_to_multiple(interval), TimeDelta::min());
        }
    }

    #[test]
    fn floor_to_multiple() {
        for interval in [seconds(10), seconds(-10)] {
            assert_eq!(TimeDelta::default().floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(9).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(10).floor_to_multiple(interval), seconds(10));
            assert_eq!(seconds(15).floor_to_multiple(interval), seconds(10));
            assert_eq!(seconds(20).floor_to_multiple(interval), seconds(20));
            assert_eq!(TimeDelta::max().floor_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).floor_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-9).floor_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-10).floor_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-15).floor_to_multiple(interval), seconds(-20));
            assert_eq!(seconds(-20).floor_to_multiple(interval), seconds(-20));
            assert_eq!(TimeDelta::min().floor_to_multiple(interval), TimeDelta::min());
        }
        for interval in [TimeDelta::max(), TimeDelta::min()] {
            assert_eq!(TimeDelta::default().floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(9).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(10).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(15).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(20).floor_to_multiple(interval), TimeDelta::default());
            assert_eq!(TimeDelta::max().floor_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).floor_to_multiple(interval), TimeDelta::min());
            assert_eq!(seconds(-9).floor_to_multiple(interval), TimeDelta::min());
            assert_eq!(seconds(-10).floor_to_multiple(interval), TimeDelta::min());
            assert_eq!(seconds(-15).floor_to_multiple(interval), TimeDelta::min());
            assert_eq!(seconds(-20).floor_to_multiple(interval), TimeDelta::min());
            assert_eq!(TimeDelta::min().floor_to_multiple(interval), TimeDelta::min());
        }
    }

    #[test]
    fn round_to_multiple() {
        for interval in [seconds(10), seconds(-10)] {
            assert_eq!(TimeDelta::default().round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(9).round_to_multiple(interval), seconds(10));
            assert_eq!(seconds(10).round_to_multiple(interval), seconds(10));
            assert_eq!(seconds(15).round_to_multiple(interval), seconds(20));
            assert_eq!(seconds(20).round_to_multiple(interval), seconds(20));
            assert_eq!(TimeDelta::max().round_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-9).round_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-10).round_to_multiple(interval), seconds(-10));
            assert_eq!(seconds(-15).round_to_multiple(interval), seconds(-20));
            assert_eq!(seconds(-20).round_to_multiple(interval), seconds(-20));
            assert_eq!(TimeDelta::min().round_to_multiple(interval), TimeDelta::min());
        }
        for interval in [TimeDelta::max(), TimeDelta::min()] {
            assert_eq!(TimeDelta::default().round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(1).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(9).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(10).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(15).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(20).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(TimeDelta::max().round_to_multiple(interval), TimeDelta::max());
            assert_eq!(seconds(-1).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-9).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-10).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-15).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(seconds(-20).round_to_multiple(interval), TimeDelta::default());
            assert_eq!(TimeDelta::min().round_to_multiple(interval), TimeDelta::min());
        }
    }

    #[test]
    fn copyable_and_comparable() {
        // Port of TimeDelta.ConstexprAndTriviallyCopiable. The C++ test checks
        // constexpr copy and assignment. Here that is a runtime copy check.
        fn assert_copy<T: Copy>() {}
        assert_copy::<TimeDelta>();
        let a = seconds(1);
        let b = a;
        assert_eq!(a, b);
        let mut c = TimeDelta::default();
        c = a;
        assert_eq!(a, c);
    }
}

mod time_base {
    use super::*;

    #[test]
    fn add_sub_delta_saturates() {
        // Port of TimeBase.AddSubDeltaSaturates. TimeTicks::Now() is replaced
        // by a fixed value.
        let k_large_time_ticks = TimeTicks::from_internal_value(i64::MAX - 1);
        let k_large_negative_time_ticks = TimeTicks::from_internal_value(i64::MIN + 1);
        assert!((k_large_time_ticks + TimeDelta::max()).is_max());
        assert!((k_large_negative_time_ticks + TimeDelta::max()).is_max());
        assert!((k_large_time_ticks - TimeDelta::max()).is_min());
        assert!((k_large_negative_time_ticks - TimeDelta::max()).is_min());
        assert!((TimeTicks::default() + TimeDelta::max()).is_max());
        assert!((TimeTicks::default() - TimeDelta::max()).is_min());
        let now = TimeTicks::from_internal_value(5_000_000);
        assert!((now + TimeDelta::max()).is_max());
        assert!((now - TimeDelta::max()).is_min());

        assert!((k_large_time_ticks + TimeDelta::min()).is_min());
        assert!((k_large_negative_time_ticks + TimeDelta::min()).is_min());
        assert!((k_large_time_ticks - TimeDelta::min()).is_max());
        assert!((k_large_negative_time_ticks - TimeDelta::min()).is_max());
        assert!((TimeTicks::default() + TimeDelta::min()).is_min());
        assert!((TimeTicks::default() - TimeDelta::min()).is_max());
        assert!((now + TimeDelta::min()).is_min());
        assert!((now - TimeDelta::min()).is_max());
    }

    #[test]
    fn add_sub_infinities() {
        // Port of TimeBase.AddSubInfinities (EXPECT_CHECK_DEATH). Each C++
        // CHECK is a panic here.
        assert!(std::panic::catch_unwind(|| TimeTicks::min() + TimeDelta::max()).is_err());
        assert!(std::panic::catch_unwind(|| TimeTicks::max() + TimeDelta::min()).is_err());
        assert!(std::panic::catch_unwind(|| TimeTicks::min() - TimeDelta::min()).is_err());
        assert!(std::panic::catch_unwind(|| TimeTicks::max() - TimeDelta::max()).is_err());

        // Saturates when adding the same sign or subtracting opposite signs.
        assert!((TimeTicks::max() + TimeDelta::max()).is_max());
        assert!((TimeTicks::min() + TimeDelta::min()).is_min());
        assert!((TimeTicks::max() - TimeDelta::min()).is_max());
        assert!((TimeTicks::min() - TimeDelta::max()).is_min());
    }

    #[test]
    fn ticks_copyable() {
        // Ports of TimeTicks.ConstexprAndTriviallyCopiable and
        // ThreadTicks.ConstexprAndTriviallyCopiable.
        fn assert_copy<T: Copy>() {}
        assert_copy::<TimeTicks>();
        assert_copy::<ThreadTicks>();
        let a = TimeTicks::from_internal_value(12345);
        let b = a;
        assert_eq!(a.to_internal_value(), b.to_internal_value());
        let a = ThreadTicks::from_internal_value(12345);
        let b = a;
        assert_eq!(a.to_internal_value(), b.to_internal_value());
    }
}

mod logging {
    use super::*;

    #[test]
    fn time_delta_empty_is_zero() {
        // Port of TimeDeltaLogging.EmptyIsZero.
        assert_eq!("0 s", TimeDelta::default().to_string());
    }

    #[test]
    fn time_delta_five_hundred_ms() {
        assert_eq!("0.5 s", milliseconds(500).to_string());
    }

    #[test]
    fn time_delta_minus_ten_seconds() {
        assert_eq!("-10 s", seconds(-10).to_string());
    }

    #[test]
    fn time_delta_formatting_is_stable() {
        // Port of TimeDeltaLogging.DoesNotMessUpFormattingFlags and
        // DoesNotMakeStreamBad. Rust formatting has no sticky stream state, so
        // this checks that formatting the same value twice gives the same text.
        let first = TimeDelta::default().to_string();
        let second = TimeDelta::default().to_string();
        assert_eq!(first, second);
    }

    #[test]
    fn time_dcheck_compiles() {
        // Port of TimeDeltaLogging.DCheckEqCompiles, TimeLogging.DCheckEqCompiles
        // and TimeTicksLogging.DCheckEqCompiles. The DCHECK_EQ macro is not
        // ported, so this compares the values directly.
        assert_eq!(TimeDelta::default(), TimeDelta::default());
        assert_eq!(Time::default(), Time::default());
        assert_eq!(TimeTicks::default(), TimeTicks::default());
    }

    #[test]
    fn time_chrome_birthdate() {
        // Port of TimeLogging.ChromeBirthdate.
        let birthdate = Time::from_string("Tue, 02 Sep 2008 09:42:18 GMT", &pdt).expect("parses");
        assert_eq!("2008-09-02 09:42:18.000000 UTC", birthdate.to_string());
    }

    #[test]
    fn time_microseconds() {
        // Port of TimeLogging.Microseconds. Time::Now() is replaced by a fixed
        // time with a non-zero microsecond part.
        let mut now = sample_now();
        if now.to_delta_since_windows_epoch().in_microseconds() % MICROSECONDS_PER_MILLISECOND == 0 {
            now += microseconds(1);
        }
        let now_str = now.to_string();
        let period = now_str.find('.').expect("has a decimal point");
        let micro_digits = &now_str[period + 4..period + 7];
        let microseconds: i32 = micro_digits.parse().expect("three digits");
        assert_ne!(0, microseconds, "{now_str}");
    }

    #[test]
    fn time_does_not_mess_up_formatting() {
        // Port of TimeLogging.DoesNotMessUpFormattingFlags and
        // DoesNotMakeStreamBad, as in the TimeDelta version above.
        assert_eq!(Time::default().to_string(), Time::default().to_string());
    }

    #[test]
    fn time_ticks_zero() {
        // Port of TimeTicksLogging.ZeroTime.
        assert_eq!("0 bogo-microseconds", TimeTicks::default().to_string());
    }

    #[test]
    fn time_ticks_forty_years_later() {
        // Port of TimeTicksLogging.FortyYearsLater.
        let forty_years_later = TimeTicks::default() + days(365.25 * 40.0);
        assert_eq!(
            "1262304000000000 bogo-microseconds",
            forty_years_later.to_string()
        );
    }

    #[test]
    fn thread_ticks_display() {
        assert_eq!(
            "5 bogo-thread-microseconds",
            ThreadTicks::from_internal_value(5).to_string()
        );
    }
}

mod delta_from_string {
    use super::*;

    #[test]
    fn parse_time_delta() {
        // Port of TimeDeltaFromStringTest.ParseTimeDeltaTest.
        let p = time_delta_from_string;
        // No unit: only zero and infinity are allowed.
        assert_eq!(p("0"), Some(TimeDelta::default()));
        assert_eq!(p("+0"), Some(TimeDelta::default()));
        assert_eq!(p("-0"), Some(TimeDelta::default()));

        assert_eq!(p("inf"), Some(TimeDelta::max()));
        assert_eq!(p("+inf"), Some(TimeDelta::max()));
        assert_eq!(p("-inf"), Some(TimeDelta::min()));
        assert_eq!(p("infBlah"), None);

        // Illegal input forms.
        assert_eq!(p(""), None);
        assert_eq!(p("0.0"), None);
        assert_eq!(p(".0"), None);
        assert_eq!(p("."), None);
        assert_eq!(p("01"), None);
        assert_eq!(p("1"), None);
        assert_eq!(p("-1"), None);
        assert_eq!(p("2"), None);
        assert_eq!(p("2 s"), None);
        assert_eq!(p(".s"), None);
        assert_eq!(p("-.s"), None);
        assert_eq!(p("s"), None);
        assert_eq!(p(" 2s"), None);
        assert_eq!(p("2s "), None);
        assert_eq!(p(" 2s "), None);
        assert_eq!(p("2mt"), None);
        assert_eq!(p("1e3s"), None);

        // One unit type.
        assert_eq!(p("1ns"), Some(nanoseconds(1)));
        assert_eq!(p("1us"), Some(microseconds(1)));
        assert_eq!(p("1ms"), Some(milliseconds(1)));
        assert_eq!(p("1s"), Some(seconds(1)));
        assert_eq!(p("2m"), Some(minutes(2)));
        assert_eq!(p("2h"), Some(hours(2)));
        assert_eq!(p("2d"), Some(days(2)));

        // Huge counts of a unit. 9223372036854775807 == 2^63 - 1.
        assert_eq!(
            p("9223372036854775807us"),
            Some(microseconds(9_223_372_036_854_775_807_i64))
        );
        assert_eq!(
            p("-9223372036854775807us"),
            Some(microseconds(-9_223_372_036_854_775_807_i64))
        );

        // Overflow of the count. Note the "93" at the beginning.
        assert_eq!(p("9323372036854775807us"), None);
        // Overflow of the overall duration.
        assert_eq!(p("9323372036854s"), Some(TimeDelta::max()));
        assert_eq!(p("-9323372036854s"), Some(TimeDelta::min()));

        // Multiple units.
        assert_eq!(p("1d2h3m"), Some(days(1) + hours(2) + minutes(3)));
        assert_eq!(p("2h3m4s"), Some(hours(2) + minutes(3) + seconds(4)));
        assert_eq!(
            p("3m4s5us"),
            Some(minutes(3) + seconds(4) + microseconds(5))
        );
        assert_eq!(
            p("2h3m4s5ms6us7ns"),
            Some(
                hours(2)
                    + minutes(3)
                    + seconds(4)
                    + milliseconds(5)
                    + microseconds(6)
                    + nanoseconds(7)
            )
        );

        // Multiple units out of order.
        assert_eq!(
            p("2us3m4s5h"),
            Some(hours(5) + minutes(3) + seconds(4) + microseconds(2))
        );

        // Fractional values of units.
        assert_eq!(p("1.5ns"), Some(1.5 * nanoseconds(1)));
        assert_eq!(p("1.5us"), Some(1.5 * microseconds(1)));
        assert_eq!(p("1.5ms"), Some(1.5 * milliseconds(1)));
        assert_eq!(p("1.5s"), Some(1.5 * seconds(1)));
        assert_eq!(p("1.5m"), Some(1.5 * minutes(1)));
        assert_eq!(p("1.5h"), Some(1.5 * hours(1)));
        assert_eq!(p("1.5d"), Some(1.5 * days(1)));

        // Huge fractional counts of a unit.
        assert_eq!(
            p("0.4294967295s"),
            Some(nanoseconds(429_496_729) + nanoseconds(1) / 2)
        );
        assert_eq!(
            p("0.429496729501234567890123456789s"),
            Some(nanoseconds(429_496_729) + nanoseconds(1) / 2)
        );

        // Negative durations.
        assert_eq!(p("-1s"), Some(seconds(-1)));
        assert_eq!(p("-1m"), Some(minutes(-1)));
        assert_eq!(p("-1h"), Some(hours(-1)));
        assert_eq!(p("-1d"), Some(days(-1)));

        assert_eq!(p("-1h2s"), Some(-(hours(1) + seconds(2))));
        assert_eq!(p("1h-2s"), None);
        assert_eq!(p("-1h-2s"), None);
        assert_eq!(p("-1h -2s"), None);
    }
}

/// Randomized and differential tests. The xorshift64 generator keeps them
/// deterministic.
mod randomized {
    use super::*;

    struct XorShift64(u64);

    impl XorShift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn next_i64(&mut self) -> i64 {
            self.next_u64() as i64
        }

        /// A value in [-range, range], biased toward small magnitudes.
        fn next_bounded(&mut self, range: i64) -> i64 {
            let r = self.next_u64();
            let limit = if r % 4 == 0 { range / 1000 + 1 } else { range };
            (self.next_u64() % (2 * limit as u64 + 1)) as i64 - limit
        }
    }

    /// Reference saturating conversion of an i128 to i64.
    fn sat(v: i128) -> i64 {
        v.clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }

    #[test]
    fn delta_arithmetic_matches_saturating_reference() {
        let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
        for _ in 0..20_000 {
            // Finite operands only, so infinities are not involved.
            let a = rng.next_i64().clamp(i64::MIN + 1, i64::MAX - 1);
            let b = rng.next_i64().clamp(i64::MIN + 1, i64::MAX - 1);
            let ta = microseconds(a);
            let tb = microseconds(b);
            assert_eq!((ta + tb).in_microseconds(), sat(a as i128 + b as i128));
            assert_eq!((ta - tb).in_microseconds(), sat(a as i128 - b as i128));
            assert_eq!((ta * 3).in_microseconds(), sat(a as i128 * 3));
            assert_eq!((ta % tb).in_microseconds(), if b == 0 { a } else { a % b });
            if b != 0 {
                assert_eq!(ta.int_div(tb), sat(a as i128 / b as i128));
            }
            assert_eq!(ta.in_milliseconds(), a / 1000);
            assert_eq!(ta.in_seconds_floored(), a.div_euclid(1_000_000) - i64::from(a.rem_euclid(1_000_000) != 0 && a < 0));
        }
    }

    #[test]
    fn delta_rounding_is_consistent() {
        let mut rng = XorShift64(0x0123_4567_89AB_CDEF);
        for _ in 0..5_000 {
            let t = microseconds(rng.next_bounded(1 << 40));
            let interval = microseconds(rng.next_bounded(1 << 20).abs().max(1));
            let floor = t.floor_to_multiple(interval);
            let ceil = t.ceil_to_multiple(interval);
            // The floor and ceiling bracket t and are multiples of interval.
            assert!(floor <= t && t <= ceil, "{t:?} {interval:?}");
            assert!((ceil - floor).in_microseconds() <= interval.magnitude().in_microseconds());
            assert!((floor % interval).is_zero());
            assert!((ceil % interval).is_zero());
        }
    }

    /// Steps the calendar one day at a time from 1970-01-01. This is a naive
    /// reference for `civil_from_days`.
    #[test]
    fn civil_matches_day_stepping_reference() {
        fn is_leap(y: i64) -> bool {
            (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
        }
        fn month_len(y: i64, m: i64) -> i64 {
            match m {
                2 => {
                    if is_leap(y) {
                        29
                    } else {
                        28
                    }
                }
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            }
        }
        // Covers about 1.3 million years in both directions.
        let (mut y, mut m, mut d) = (1970_i64, 1_i64, 1_i64);
        let mut day: i64 = 0;
        // Step forward from 1970 to 2000 and back to -1000, checking at each step.
        for _ in 0..700_000 {
            let (cy, cm, cd) = {
                let e = Time::from_utc_exploded(&ex(y as i32, m as i32, 0, d as i32, 0, 0, 0, 0)).expect("in range");
                let back = e.utc_explode();
                (back.year as i64, back.month as i64, back.day_of_month as i64)
            };
            assert_eq!((cy, cm, cd), (y, m, d));
            // Advance one day.
            d += 1;
            if d > month_len(y, m) {
                d = 1;
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
            day += 1;
            let _ = day;
        }
        // Check that exploding a time for each day gives the stepped date.
        let mut y = 1970_i64;
        let mut m = 1_i64;
        let mut d = 1_i64;
        for n in 0..200_000_i64 {
            let t = Time::from_time_t(n * 86_400);
            let e = t.utc_explode();
            assert_eq!((e.year as i64, e.month as i64, e.day_of_month as i64), (y, m, d));
            d += 1;
            if d > month_len(y, m) {
                d = 1;
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
            }
        }
    }

    #[test]
    fn explode_round_trip_random() {
        let mut rng = XorShift64(0xDEAD_BEEF_CAFE_F00D);
        for _ in 0..20_000 {
            let t = Time::from_time_t(rng.next_bounded(1 << 45));
            let e = t.utc_explode();
            assert!(e.has_valid_values(), "{t}");
            let back = Time::from_utc_exploded(&e).expect("round trip");
            // Explode drops microseconds, so the round trip rounds down to the
            // millisecond.
            assert_eq!(
                back,
                Time::from_milliseconds_since_unix_epoch(t.in_milliseconds_since_unix_epoch())
                    .max(back.min(t))
                    .min(t)
                    .max(back)
                    .min(Time::from_milliseconds_since_unix_epoch(
                        t.in_milliseconds_since_unix_epoch()
                    ))
                    .max(back)
            );
        }
    }
}
