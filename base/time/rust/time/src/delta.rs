// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `TimeDelta`: a signed duration stored in microseconds. Port of
//! `base::TimeDelta` (base/time/time.h and time.cc).

use std::fmt;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
};

use crate::format::format_general;

pub const NANOSECONDS_PER_MICROSECOND: i64 = 1_000;
pub const NANOSECONDS_PER_MILLISECOND: i64 = 1_000_000;
pub const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;
pub const MICROSECONDS_PER_MILLISECOND: i64 = 1_000;
pub const MICROSECONDS_PER_SECOND: i64 = 1_000_000;
pub const MICROSECONDS_PER_MINUTE: i64 = 60 * MICROSECONDS_PER_SECOND;
pub const MICROSECONDS_PER_HOUR: i64 = 60 * MICROSECONDS_PER_MINUTE;
pub const MICROSECONDS_PER_DAY: i64 = 24 * MICROSECONDS_PER_HOUR;
pub const MICROSECONDS_PER_WEEK: i64 = 7 * MICROSECONDS_PER_DAY;
pub const MILLISECONDS_PER_SECOND: i64 = 1_000;
pub const MILLISECONDS_PER_DAY: i64 = 24 * 60 * 60 * MILLISECONDS_PER_SECOND;

mod sealed {
    pub trait Sealed {}
}

/// A numeric argument for the unit constructors ([`days`], [`seconds`], ...)
/// and for `TimeDelta`'s `*` and `/` operators. Implemented for `i32`, `i64`,
/// `f32` and `f64`.
///
/// Integer arithmetic saturates, as C++ `ClampedNumeric` does. Floating-point
/// arithmetic is done in the scalar's own precision, then converted to `i64`
/// by truncation toward zero. The conversion saturates at the ends of the
/// range, and NaN becomes 0.
pub trait Scalar: Copy + sealed::Sealed {
    /// Returns `self * k`.
    #[doc(hidden)]
    fn times(self, k: i64) -> i64;
    /// Returns `self / k`. A zero `k` saturates, as `ClampedNumeric` does.
    #[doc(hidden)]
    fn over(self, k: i64) -> i64;
    /// Returns `k / self`. A zero `self` saturates, as `ClampedNumeric` does.
    #[doc(hidden)]
    fn divided_into(self, k: i64) -> i64;
    /// Returns false only for zero. NaN counts as true, as in C++ `if (n)`.
    #[doc(hidden)]
    fn is_nonzero(self) -> bool;
}

/// `ClampedNumeric` integer division. Division by zero gives max or min,
/// with the sign taken from the numerator, or 0 for `0 / 0`.
fn clamped_div_int(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        return match numerator {
            0 => 0,
            n if n < 0 => i64::MIN,
            _ => i64::MAX,
        };
    }
    numerator.saturating_div(denominator)
}

macro_rules! impl_int_scalar {
    ($($t:ty),*) => {$(
        impl sealed::Sealed for $t {}
        impl Scalar for $t {
            fn times(self, k: i64) -> i64 {
                (self as i64).saturating_mul(k)
            }
            fn over(self, k: i64) -> i64 {
                clamped_div_int(self as i64, k)
            }
            fn divided_into(self, k: i64) -> i64 {
                clamped_div_int(k, self as i64)
            }
            fn is_nonzero(self) -> bool {
                self != 0
            }
        }
    )*};
}

macro_rules! impl_float_scalar {
    ($($t:ty),*) => {$(
        impl sealed::Sealed for $t {}
        impl Scalar for $t {
            fn times(self, k: i64) -> i64 {
                (self * (k as $t)) as i64
            }
            fn over(self, k: i64) -> i64 {
                (self / (k as $t)) as i64
            }
            fn divided_into(self, k: i64) -> i64 {
                ((k as $t) / self) as i64
            }
            fn is_nonzero(self) -> bool {
                self != 0.0
            }
        }
    )*};
}

impl_int_scalar!(i32, i64);
impl_float_scalar!(f32, f64);

/// Returns a delta of `n` days. Port of `base::Days`.
pub fn days<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(MICROSECONDS_PER_DAY))
}

/// Returns a delta of `n` hours. Port of `base::Hours`.
pub fn hours<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(MICROSECONDS_PER_HOUR))
}

/// Returns a delta of `n` minutes. Port of `base::Minutes`.
pub fn minutes<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(MICROSECONDS_PER_MINUTE))
}

/// Returns a delta of `n` seconds. Port of `base::Seconds`.
pub fn seconds<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(MICROSECONDS_PER_SECOND))
}

/// Returns a delta of `n` milliseconds. Port of `base::Milliseconds`.
pub fn milliseconds<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(MICROSECONDS_PER_MILLISECOND))
}

/// Returns a delta of `n` microseconds. Port of `base::Microseconds`.
/// Floating-point input is truncated toward zero.
pub fn microseconds<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.times(1))
}

/// Returns a delta of `n` nanoseconds, truncated to whole microseconds. Port
/// of `base::Nanoseconds`.
pub fn nanoseconds<S: Scalar>(n: S) -> TimeDelta {
    TimeDelta::from_internal_value(n.over(NANOSECONDS_PER_MICROSECOND))
}

/// Returns the period of a frequency of `n` Hz. Port of `base::Hertz`. Zero
/// gives [`TimeDelta::max()`].
pub fn hertz<S: Scalar>(n: S) -> TimeDelta {
    if n.is_nonzero() {
        TimeDelta::from_internal_value(n.divided_into(MICROSECONDS_PER_SECOND))
    } else {
        TimeDelta::max()
    }
}

/// A signed duration in microseconds. Port of `base::TimeDelta`.
///
/// Arithmetic saturates at the ends of the `i64` range. [`TimeDelta::max()`]
/// and [`TimeDelta::min()`] act as infinities: converted to `f64` they become
/// `±inf`, and adding a finite delta leaves them unchanged. Adding opposite
/// infinities and subtracting equal ones panic, as the C++ `CHECK`s do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeDelta {
    us: i64,
}

impl TimeDelta {
    /// Creates a delta from a microsecond count. Port of `FromInternalValue`.
    pub const fn from_internal_value(us: i64) -> Self {
        Self { us }
    }

    /// Returns the raw microsecond count. Port of `ToInternalValue`.
    pub const fn to_internal_value(self) -> i64 {
        self.us
    }

    /// The largest delta, treated as `+inf`.
    pub const fn max() -> Self {
        Self { us: i64::MAX }
    }

    /// The smallest delta, treated as `-inf`.
    pub const fn min() -> Self {
        Self { us: i64::MIN }
    }

    /// The largest delta that is not `+inf`.
    pub const fn finite_max() -> Self {
        Self { us: i64::MAX - 1 }
    }

    /// The smallest delta that is not `-inf`.
    pub const fn finite_min() -> Self {
        Self { us: i64::MIN + 1 }
    }

    /// Returns the absolute value. `min()` gives `max()`.
    pub fn magnitude(self) -> Self {
        Self {
            us: self.us.saturating_abs(),
        }
    }

    /// Returns true for a zero delta.
    pub const fn is_zero(self) -> bool {
        self.us == 0
    }

    /// Returns true if the delta is greater than zero.
    pub const fn is_positive(self) -> bool {
        self.us > 0
    }

    /// Returns true if the delta is less than zero.
    pub const fn is_negative(self) -> bool {
        self.us < 0
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

    /// Returns the frequency in Hz whose period is `self`. Port of `ToHz`.
    pub fn to_hz(self) -> f64 {
        seconds(1) / self
    }

    /// Returns the delta in days, rounded toward zero. Infinities give
    /// `i32::MIN` or `i32::MAX`.
    pub fn in_days(self) -> i32 {
        if self.is_inf() {
            return if self.us < 0 { i32::MIN } else { i32::MAX };
        }
        saturate_i32(self.us / MICROSECONDS_PER_DAY)
    }

    /// Returns the delta in days, rounded toward negative infinity.
    pub fn in_days_floored(self) -> i32 {
        if self.is_inf() {
            return if self.us < 0 { i32::MIN } else { i32::MAX };
        }
        let result = self.us / MICROSECONDS_PER_DAY;
        let floored = if result * MICROSECONDS_PER_DAY > self.us {
            result - 1
        } else {
            result
        };
        saturate_i32(floored)
    }

    /// Returns the delta in hours, rounded toward zero and saturated to `i32`.
    pub fn in_hours(self) -> i32 {
        saturate_i32(self.us.saturating_div(MICROSECONDS_PER_HOUR))
    }

    /// Returns the delta in minutes, rounded toward zero and saturated to
    /// `i32`.
    pub fn in_minutes(self) -> i32 {
        saturate_i32(self.us.saturating_div(MICROSECONDS_PER_MINUTE))
    }

    /// Returns the delta in seconds as `f64`. Infinities give `±inf`.
    pub fn in_seconds_f(self) -> f64 {
        self.to_double() / MICROSECONDS_PER_SECOND as f64
    }

    /// Returns the delta in seconds, rounded toward zero. Infinities give the
    /// raw `i64` extremes.
    pub fn in_seconds(self) -> i64 {
        if self.is_inf() {
            self.us
        } else {
            self.us / MICROSECONDS_PER_SECOND
        }
    }

    /// Returns the delta in seconds, rounded toward negative infinity.
    pub fn in_seconds_floored(self) -> i64 {
        if self.is_inf() {
            return self.us;
        }
        let result = self.us / MICROSECONDS_PER_SECOND;
        if result * MICROSECONDS_PER_SECOND > self.us {
            result - 1
        } else {
            result
        }
    }

    /// Returns the delta in milliseconds as `f64`. Infinities give `±inf`.
    pub fn in_milliseconds_f(self) -> f64 {
        self.to_double() / MICROSECONDS_PER_MILLISECOND as f64
    }

    /// Returns the delta in milliseconds, rounded toward zero. Infinities
    /// give `i64::MIN` or `i64::MAX`.
    pub fn in_milliseconds(self) -> i64 {
        if self.is_inf() {
            return if self.us < 0 { i64::MIN } else { i64::MAX };
        }
        self.us / MICROSECONDS_PER_MILLISECOND
    }

    /// Returns the delta in milliseconds, rounded toward positive infinity.
    pub fn in_milliseconds_rounded_up(self) -> i64 {
        if self.is_inf() {
            return self.us;
        }
        let result = self.us / MICROSECONDS_PER_MILLISECOND;
        if self.us > result * MICROSECONDS_PER_MILLISECOND {
            result + 1
        } else {
            result
        }
    }

    /// Returns the raw microsecond count. Port of `InMicroseconds`.
    pub fn in_microseconds(self) -> i64 {
        self.us
    }

    /// Returns the delta in microseconds as `f64`. Infinities give `±inf`.
    pub fn in_microseconds_f(self) -> f64 {
        self.to_double()
    }

    /// Returns the delta in nanoseconds, saturated to the `i64` range.
    pub fn in_nanoseconds(self) -> i64 {
        self.us.saturating_mul(NANOSECONDS_PER_MICROSECOND)
    }

    /// Returns the integer quotient of `self / a`, rounded toward zero and
    /// saturated to `i64`. Port of `IntDiv`.
    ///
    /// Infinities and zero divisors follow the C++ rules: when the quotient
    /// is not representable, the result is `i64::MAX` or `i64::MIN`, with the
    /// sign of the quotient. `0 / 0` and `inf / inf` panic.
    pub fn int_div(self, a: TimeDelta) -> i64 {
        if !self.is_inf() && !a.is_zero() {
            return self.us.saturating_div(a.us);
        }
        assert!(
            !self.is_zero() || !a.is_zero(),
            "TimeDelta::IntDiv: 0 / 0 is invalid"
        );
        assert!(
            !self.is_inf() || !a.is_inf(),
            "TimeDelta::IntDiv: inf / inf is invalid"
        );
        if (self.us < 0) == (a.us < 0) {
            i64::MAX
        } else {
            i64::MIN
        }
    }

    /// Returns `self` ceiled to the nearest multiple of `interval`. Ceiling is
    /// toward positive infinity.
    pub fn ceil_to_multiple(self, interval: TimeDelta) -> TimeDelta {
        if self.is_inf() || interval.is_zero() {
            return self;
        }
        let remainder = self % interval;
        if self.us < 0 {
            return self - remainder;
        }
        if remainder.is_zero() {
            self
        } else {
            self - remainder + interval.magnitude()
        }
    }

    /// Returns `self` floored to the nearest multiple of `interval`. Flooring
    /// is toward negative infinity.
    pub fn floor_to_multiple(self, interval: TimeDelta) -> TimeDelta {
        if self.is_inf() || interval.is_zero() {
            return self;
        }
        let remainder = self % interval;
        if self.us < 0 {
            if remainder.is_zero() {
                self
            } else {
                self - remainder - interval.magnitude()
            }
        } else {
            self - remainder
        }
    }

    /// Returns `self` rounded to the nearest multiple of `interval`, with
    /// halves rounded away from zero.
    pub fn round_to_multiple(self, interval: TimeDelta) -> TimeDelta {
        if self.is_inf() || interval.is_zero() {
            return self;
        }
        if interval.is_inf() {
            return TimeDelta::default();
        }
        let half = interval.magnitude() / 2;
        if self.us < 0 {
            (self - half).ceil_to_multiple(interval)
        } else {
            (self + half).floor_to_multiple(interval)
        }
    }

    /// Returns the value as `f64`, with the infinities mapped to `±inf`.
    fn to_double(self) -> f64 {
        if self.is_inf() {
            if self.us < 0 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        } else {
            self.us as f64
        }
    }
}

/// Saturating conversion from `i64` to `i32`, as `saturated_cast<int>`.
fn saturate_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Add for TimeDelta {
    type Output = TimeDelta;

    fn add(self, other: TimeDelta) -> TimeDelta {
        if !other.is_inf() {
            return TimeDelta {
                us: self.us.saturating_add(other.us),
            };
        }
        // Adding two infinities is only valid if their signs match.
        assert!(
            !self.is_inf() || self.us == other.us,
            "TimeDelta::Add: adding infinities of opposite sign is invalid"
        );
        other
    }
}

impl Sub for TimeDelta {
    type Output = TimeDelta;

    fn sub(self, other: TimeDelta) -> TimeDelta {
        if !other.is_inf() {
            return TimeDelta {
                us: self.us.saturating_sub(other.us),
            };
        }
        // Subtracting two infinities is only valid if their signs differ.
        assert_ne!(
            self.us, other.us,
            "TimeDelta::Sub: subtracting infinities of the same sign is invalid"
        );
        if other.us < 0 {
            TimeDelta::max()
        } else {
            TimeDelta::min()
        }
    }
}

impl Neg for TimeDelta {
    type Output = TimeDelta;

    fn neg(self) -> TimeDelta {
        if !self.is_inf() {
            return TimeDelta {
                us: self.us.saturating_neg(),
            };
        }
        if self.us < 0 {
            TimeDelta::max()
        } else {
            TimeDelta::min()
        }
    }
}

impl<S: Scalar> Mul<S> for TimeDelta {
    type Output = TimeDelta;

    fn mul(self, a: S) -> TimeDelta {
        TimeDelta {
            us: a.times(self.us),
        }
    }
}

impl<S: Scalar> Div<S> for TimeDelta {
    type Output = TimeDelta;

    fn div(self, a: S) -> TimeDelta {
        TimeDelta {
            us: a.divided_into(self.us),
        }
    }
}

impl Div<TimeDelta> for TimeDelta {
    type Output = f64;

    /// Floating-point division. Port of `operator/(TimeDelta)`.
    fn div(self, a: TimeDelta) -> f64 {
        // 0/0 and inf/inf are invalid. They give NaN, which becomes 0 when
        // clamped to an integer, so they are rejected here.
        assert!(
            !self.is_zero() || !a.is_zero(),
            "TimeDelta::Div: 0 / 0 is invalid"
        );
        assert!(
            !self.is_inf() || !a.is_inf(),
            "TimeDelta::Div: inf / inf is invalid"
        );
        self.to_double() / a.to_double()
    }
}

impl Rem for TimeDelta {
    type Output = TimeDelta;

    /// Returns `self` modulo `a`. If either operand is infinite or `a` is
    /// zero, returns `self` unchanged.
    fn rem(self, a: TimeDelta) -> TimeDelta {
        if self.is_inf() || a.is_zero() || a.is_inf() {
            return self;
        }
        TimeDelta {
            us: self.us.wrapping_rem(a.us),
        }
    }
}

impl AddAssign for TimeDelta {
    fn add_assign(&mut self, other: TimeDelta) {
        *self = *self + other;
    }
}

impl SubAssign for TimeDelta {
    fn sub_assign(&mut self, other: TimeDelta) {
        *self = *self - other;
    }
}

impl<S: Scalar> MulAssign<S> for TimeDelta {
    fn mul_assign(&mut self, a: S) {
        *self = *self * a;
    }
}

impl<S: Scalar> DivAssign<S> for TimeDelta {
    fn div_assign(&mut self, a: S) {
        *self = *self / a;
    }
}

impl RemAssign for TimeDelta {
    fn rem_assign(&mut self, other: TimeDelta) {
        *self = *self % other;
    }
}

macro_rules! impl_scalar_times_delta {
    ($($t:ty),*) => {$(
        impl Mul<TimeDelta> for $t {
            type Output = TimeDelta;

            fn mul(self, td: TimeDelta) -> TimeDelta {
                td * self
            }
        }
    )*};
}

impl_scalar_times_delta!(i32, i64, f32, f64);

impl fmt::Display for TimeDelta {
    /// Matches C++ `operator<<`: seconds with `%g` formatting, then " s".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} s", format_general(self.in_seconds_f()))
    }
}

impl TimeDelta {
    /// Converts a `timespec`-style value to a delta, clamping negative results
    /// to zero. Port of `TimeDelta::FromTimeSpec` on the Linux 64-bit layout.
    pub fn from_time_spec(tv_sec: i64, tv_nsec: i64) -> TimeDelta {
        let delta = seconds(tv_sec) + nanoseconds(tv_nsec);
        if delta.is_positive() {
            delta
        } else {
            TimeDelta::default()
        }
    }

    /// Converts to a `timespec`-style `(tv_sec, tv_nsec)` pair. Negative
    /// deltas give `(0, 0)`. Port of `ToTimeSpec`.
    pub fn to_time_spec(self) -> (i64, i64) {
        if self.is_negative() {
            return (0, 0);
        }
        let extra_microseconds = self.in_microseconds() % MICROSECONDS_PER_SECOND;
        (
            self.in_seconds(),
            extra_microseconds * NANOSECONDS_PER_MICROSECOND,
        )
    }
}
