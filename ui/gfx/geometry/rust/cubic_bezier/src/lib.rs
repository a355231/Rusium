// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `gfx::CubicBezier`
//! (ui/gfx/geometry/cubic_bezier.{h,cc}).
//!
//! A CSS-style cubic bezier timing curve with implicit end points (0, 0) and
//! (1, 1) and two control points (p1x, p1y), (p2x, p2y). `solve` maps an x in
//! [0, 1] to the curve's y value by finding the curve parameter t with
//! `x(t) = x`, using a few Newton steps and then bisection, as the C++ does.
//! Outside [0, 1] the end gradients are used to extrapolate linearly.
//!
//! Differences from the C++ version:
//! - The constructor returns a new value (`CubicBezier::new`). The type is
//!   `Copy`, so the C++ copy constructor and assignment are not needed.
//! - `GetDefaultEpsilon()` is `CubicBezier::default_epsilon()`.
//! - The `DCHECK`s on the x range of `SolveCurveX` and on monotonicity are
//!   `debug_assert!`s. Like the C++ DCHECKs they are active in debug builds
//!   only. In release builds, x outside [0, 1] is not checked, as in C++.
//! - `SolveCurveX` in C++ reads uninitialized `t0`/`t1` when x is greater than
//!   the last spline sample. That can happen only through rounding, for
//!   example when x = 1 and `SampleCurveX(1)` is just below 1. Here `t0`/`t1`
//!   default to the last spline segment, [0.9, 1].
//! - `std::min`, `std::max` and `std::clamp` are reproduced with the same
//!   comparison order, so NaN inputs behave as in C++.
//! - The `CUBIC_BEZIER_SPLINE_SAMPLES` macro is the constant
//!   `SPLINE_SAMPLES`.
//! - Every public function of the C++ class is ported. Nothing is omitted.

#![forbid(unsafe_code)]

/// Number of precomputed samples of x(t). Same as the C++
/// `CUBIC_BEZIER_SPLINE_SAMPLES`.
const SPLINE_SAMPLES: usize = 11;

/// Newton iterations before falling back to bisection.
const MAX_NEWTON_ITERATIONS: usize = 4;

/// Default solver tolerance. Same as the C++ `kBezierEpsilon`.
const BEZIER_EPSILON: f64 = 1e-7;

/// Replaces an infinite value with the largest or smallest finite `f64`. NaN
/// is returned unchanged. Same as the C++ `CubicBezier::ToFinite`.
fn to_finite(value: f64) -> f64 {
    if value.is_infinite() {
        if value > 0.0 {
            f64::MAX
        } else {
            f64::MIN
        }
    } else {
        value
    }
}

/// `std::min(a, b)`: returns `b` only when `b < a`.
fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `std::max(a, b)`: returns `b` only when `a < b`.
fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

/// `std::clamp(v, lo, hi)`.
fn cpp_clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if hi < v {
        hi
    } else {
        v
    }
}

/// A cubic bezier timing curve with end points (0, 0) and (1, 1).
#[derive(Debug, Clone, Copy)]
pub struct CubicBezier {
    // Polynomial coefficients: x(t) = ax t^3 + bx t^2 + cx t, likewise y.
    ax: f64,
    bx: f64,
    cx: f64,

    ay: f64,
    by: f64,
    cy: f64,

    start_gradient: f64,
    end_gradient: f64,

    range_min: f64,
    range_max: f64,

    spline_samples: [f64; SPLINE_SAMPLES],

    // Set when both internal x control points lie in [0, 1], so x(t) is
    // monotone and `solve_curve_x` has a unique answer.
    monotonically_increasing: bool,
}

impl CubicBezier {
    /// Builds the curve with control points (`p1x`, `p1y`) and (`p2x`,
    /// `p2y`). Same as the C++ constructor.
    pub fn new(p1x: f64, p1y: f64, p2x: f64, p2y: f64) -> Self {
        let mut curve = Self {
            ax: 0.0,
            bx: 0.0,
            cx: 0.0,
            ay: 0.0,
            by: 0.0,
            cy: 0.0,
            start_gradient: 0.0,
            end_gradient: 0.0,
            range_min: 0.0,
            range_max: 1.0,
            spline_samples: [0.0; SPLINE_SAMPLES],
            monotonically_increasing: false,
        };
        curve.init_coefficients(p1x, p1y, p2x, p2y);
        curve.init_gradients(p1x, p1y, p2x, p2y);
        curve.init_range(p1y, p2y);
        curve.init_spline();
        curve
    }

    fn init_coefficients(&mut self, p1x: f64, p1y: f64, p2x: f64, p2y: f64) {
        // The implicit first and last control points are (0,0) and (1,1).
        self.cx = 3.0 * p1x;
        self.bx = 3.0 * (p2x - p1x) - self.cx;
        self.ax = 1.0 - self.cx - self.bx;

        self.cy = to_finite(3.0 * p1y);
        self.by = to_finite(3.0 * (p2y - p1y) - self.cy);
        self.ay = to_finite(1.0 - self.cy - self.by);

        self.monotonically_increasing = (0.0..=1.0).contains(&p1x) && (0.0..=1.0).contains(&p2x);
    }

    fn init_gradients(&mut self, p1x: f64, p1y: f64, p2x: f64, p2y: f64) {
        // End-point gradients are used outside the range [0, 1]. See the C++
        // comments in cubic_bezier.cc for the four cases handled here.
        self.start_gradient = if p1x > 0.0 {
            p1y / p1x
        } else if p1y == 0.0 && p2x > 0.0 {
            p2y / p2x
        } else if p1y == 0.0 && p2y == 0.0 {
            1.0
        } else {
            0.0
        };

        self.end_gradient = if p2x < 1.0 {
            (p2y - 1.0) / (p2x - 1.0)
        } else if p2y == 1.0 && p1x < 1.0 {
            (p1y - 1.0) / (p1x - 1.0)
        } else if p2y == 1.0 && p1y == 1.0 {
            1.0
        } else {
            0.0
        };
    }

    // Finds the minimum and maximum of y over t in [0, 1]. The extremes are
    // at the end points or at the zeros of y'(t) inside (0, 1).
    fn init_range(&mut self, p1y: f64, p2y: f64) {
        self.range_min = 0.0;
        self.range_max = 1.0;
        if (0.0..1.0).contains(&p1y) && (0.0..=1.0).contains(&p2y) {
            return;
        }

        let epsilon = BEZIER_EPSILON;

        // The derivative in the form a t^2 + b t + c, as in
        // `sample_curve_derivative_y`.
        let a = 3.0 * self.ay;
        let b = 2.0 * self.by;
        let c = self.cy;

        // The derivative is constant.
        if a.abs() < epsilon && b.abs() < epsilon {
            return;
        }

        let (t1, t2) = if a.abs() < epsilon {
            // The derivative is linear.
            (-c / b, 0.0)
        } else {
            // The derivative is a quadratic.
            let discriminant = b * b - 4.0 * a * c;
            if discriminant < 0.0 {
                return;
            }
            let discriminant_sqrt = discriminant.sqrt();
            ((-b + discriminant_sqrt) / (2.0 * a), (-b - discriminant_sqrt) / (2.0 * a))
        };

        let mut sol1 = 0.0;
        let mut sol2 = 0.0;
        if 0.0 < t1 && t1 < 1.0 {
            sol1 = self.sample_curve_y(t1);
        }
        if 0.0 < t2 && t2 < 1.0 {
            sol2 = self.sample_curve_y(t2);
        }

        self.range_min = cpp_min(cpp_min(self.range_min, sol1), sol2);
        self.range_max = cpp_max(cpp_max(self.range_max, sol1), sol2);
    }

    fn init_spline(&mut self) {
        let delta_t = 1.0 / (SPLINE_SAMPLES - 1) as f64;
        for i in 0..SPLINE_SAMPLES {
            self.spline_samples[i] = self.sample_curve_x(i as f64 * delta_t);
        }
    }

    /// The default solver tolerance, `1e-7`.
    pub fn default_epsilon() -> f64 {
        BEZIER_EPSILON
    }

    /// Evaluates x(t). Uses Horner's rule. The control points for x are in
    /// [0, 1] when the curve is monotone, so no clamping is needed.
    pub fn sample_curve_x(&self, t: f64) -> f64 {
        ((self.ax * t + self.bx) * t + self.cx) * t
    }

    /// Evaluates y(t), clamped to finite values.
    pub fn sample_curve_y(&self, t: f64) -> f64 {
        to_finite(((self.ay * t + self.by) * t + self.cy) * t)
    }

    /// Evaluates x'(t).
    pub fn sample_curve_derivative_x(&self, t: f64) -> f64 {
        (3.0 * self.ax * t + 2.0 * self.bx) * t + self.cx
    }

    /// Evaluates y'(t), clamped to finite values at each step.
    pub fn sample_curve_derivative_y(&self, t: f64) -> f64 {
        to_finite(to_finite(to_finite(3.0 * self.ay) * t + to_finite(2.0 * self.by)) * t + self.cy)
    }

    /// Given x in [0, 1], finds a parameter t with x(t) approximately x. The
    /// result is within `epsilon` of x when the solver converges. Requires a
    /// monotone curve (both internal x control points in [0, 1]).
    /// Same as the C++ `SolveCurveX`.
    pub fn solve_curve_x(&self, x: f64, epsilon: f64) -> f64 {
        debug_assert!(x >= 0.0, "x must be >= 0, got {x}");
        debug_assert!(x <= 1.0, "x must be <= 1, got {x}");
        debug_assert!(self.monotonically_increasing);

        let delta_t = 1.0 / (SPLINE_SAMPLES - 1) as f64;

        // Initial guess from linear interpolation of the spline samples.
        let mut t2 = x;
        // Default segment; see the crate docs for why it is needed.
        let mut t0 = 1.0 - delta_t;
        let mut t1 = 1.0;
        for i in 1..SPLINE_SAMPLES {
            if x <= self.spline_samples[i] {
                t1 = delta_t * i as f64;
                t0 = t1 - delta_t;
                t2 = t0
                    + (t1 - t0) * (x - self.spline_samples[i - 1])
                        / (self.spline_samples[i] - self.spline_samples[i - 1]);
                break;
            }
        }

        // A few Newton iterations, which usually converge quickly.
        let newton_epsilon = cpp_min(BEZIER_EPSILON, epsilon);
        let mut x2 = 0.0;
        for _ in 0..MAX_NEWTON_ITERATIONS {
            x2 = self.sample_curve_x(t2) - x;
            if x2.abs() < newton_epsilon {
                return t2;
            }
            let d2 = self.sample_curve_derivative_x(t2);
            if d2.abs() < BEZIER_EPSILON {
                break;
            }
            t2 -= x2 / d2;
        }
        if x2.abs() < epsilon {
            return t2;
        }

        // Bisection is slower but reliable.
        while t0 < t1 {
            let x_t2 = self.sample_curve_x(t2);
            if (x_t2 - x).abs() < epsilon {
                return t2;
            }
            if x > x_t2 {
                t0 = t2;
            } else {
                t1 = t2;
            }
            t2 = (t1 + t0) * 0.5;
        }

        // The solver did not converge. Return the last estimate, as C++ does.
        t2
    }

    /// Returns y for the given x, using the default epsilon.
    pub fn solve(&self, x: f64) -> f64 {
        self.solve_with_epsilon(x, BEZIER_EPSILON)
    }

    /// Returns y for the given x. `epsilon` is a hint for the accuracy and is
    /// not guaranteed. For x outside [0, 1] the end gradient is used to
    /// extrapolate linearly.
    pub fn solve_with_epsilon(&self, x: f64, epsilon: f64) -> f64 {
        if x < 0.0 {
            return to_finite(0.0 + self.start_gradient * x);
        }
        if x > 1.0 {
            return to_finite(1.0 + self.end_gradient * (x - 1.0));
        }
        self.sample_curve_y(self.solve_curve_x(x, epsilon))
    }

    /// Returns dy/dx at x, using the default epsilon.
    pub fn slope(&self, x: f64) -> f64 {
        self.slope_with_epsilon(x, BEZIER_EPSILON)
    }

    /// Returns dy/dx at x. x is clamped to [0, 1].
    pub fn slope_with_epsilon(&self, x: f64, epsilon: f64) -> f64 {
        let x = cpp_clamp(x, 0.0, 1.0);
        let t = self.solve_curve_x(x, epsilon);
        let dx = self.sample_curve_derivative_x(t);
        let dy = self.sample_curve_derivative_y(t);
        // The C++ tests `!dx`, which is true only for zero (not NaN).
        if dx == 0.0 && dy == 0.0 {
            return 0.0;
        }
        to_finite(dy / dx)
    }

    /// Returns the first control point's x, recovered from the coefficients.
    pub fn get_x1(&self) -> f64 {
        self.cx / 3.0
    }

    /// Returns the first control point's y, recovered from the coefficients.
    pub fn get_y1(&self) -> f64 {
        self.cy / 3.0
    }

    /// Returns the second control point's x, recovered from the coefficients.
    pub fn get_x2(&self) -> f64 {
        (self.bx + self.cx) / 3.0 + self.get_x1()
    }

    /// Returns the second control point's y, recovered from the coefficients.
    pub fn get_y2(&self) -> f64 {
        (self.by + self.cy) / 3.0 + self.get_y1()
    }

    /// Minimum y value over t in [0, 1].
    pub fn range_min(&self) -> f64 {
        self.range_min
    }

    /// Maximum y value over t in [0, 1].
    pub fn range_max(&self) -> f64 {
        self.range_max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port of the C++ `XorShift` style PRNG used for the randomized tests.
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

        /// Uniform in [0, 1).
        fn next_unit(&mut self) -> f64 {
            (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
        }

        /// Uniform in [lo, hi).
        fn next_range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next_unit()
        }
    }

    /// Mirrors `EXPECT_NEAR`: passes when |actual - expected| <= epsilon.
    fn expect_near(actual: f64, expected: f64, epsilon: f64) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "expected {expected} +/- {epsilon}, got {actual}"
        );
    }

    fn validate_solver(cubic_bezier: &CubicBezier) {
        let epsilon = 1e-7;
        let precision = 1e-5;
        let mut t = 0.0;
        while t <= 1.0 {
            let x = cubic_bezier.sample_curve_x(t);
            let root = cubic_bezier.solve_curve_x(x, epsilon);
            expect_near(root, t, precision);
            t += 0.05;
        }
    }

    #[test]
    fn basic() {
        let function = CubicBezier::new(0.25, 0.0, 0.75, 1.0);

        let epsilon = 0.00015;

        expect_near(function.solve(0.0), 0.0, epsilon);
        expect_near(function.solve(0.05), 0.01136, epsilon);
        expect_near(function.solve(0.1), 0.03978, epsilon);
        expect_near(function.solve(0.15), 0.079780, epsilon);
        expect_near(function.solve(0.2), 0.12803, epsilon);
        expect_near(function.solve(0.25), 0.18235, epsilon);
        expect_near(function.solve(0.3), 0.24115, epsilon);
        expect_near(function.solve(0.35), 0.30323, epsilon);
        expect_near(function.solve(0.4), 0.36761, epsilon);
        expect_near(function.solve(0.45), 0.43345, epsilon);
        expect_near(function.solve(0.5), 0.5, epsilon);
        expect_near(function.solve(0.6), 0.63238, epsilon);
        expect_near(function.solve(0.65), 0.69676, epsilon);
        expect_near(function.solve(0.7), 0.75884, epsilon);
        expect_near(function.solve(0.75), 0.81764, epsilon);
        expect_near(function.solve(0.8), 0.87196, epsilon);
        expect_near(function.solve(0.85), 0.92021, epsilon);
        expect_near(function.solve(0.9), 0.96021, epsilon);
        expect_near(function.solve(0.95), 0.98863, epsilon);
        expect_near(function.solve(1.0), 1.0, epsilon);

        let basic_use = CubicBezier::new(0.5, 1.0, 0.5, 1.0);
        assert_eq!(0.875, basic_use.solve(0.5));

        let overshoot = CubicBezier::new(0.5, 2.0, 0.5, 2.0);
        assert_eq!(1.625, overshoot.solve(0.5));

        let undershoot = CubicBezier::new(0.5, -1.0, 0.5, -1.0);
        assert_eq!(-0.625, undershoot.solve(0.5));
    }

    // Tests that solving the bezier works with knots with y not in (0, 1).
    #[test]
    fn unclamped_y_values() {
        let function = CubicBezier::new(0.5, -1.0, 0.5, 2.0);

        let epsilon = 0.00015;

        expect_near(function.solve(0.0), 0.0, epsilon);
        expect_near(function.solve(0.05), -0.08954, epsilon);
        expect_near(function.solve(0.1), -0.15613, epsilon);
        expect_near(function.solve(0.15), -0.19641, epsilon);
        expect_near(function.solve(0.2), -0.20651, epsilon);
        expect_near(function.solve(0.25), -0.18232, epsilon);
        expect_near(function.solve(0.3), -0.11992, epsilon);
        expect_near(function.solve(0.35), -0.01672, epsilon);
        expect_near(function.solve(0.4), 0.12660, epsilon);
        expect_near(function.solve(0.45), 0.30349, epsilon);
        expect_near(function.solve(0.5), 0.50000, epsilon);
        expect_near(function.solve(0.55), 0.69651, epsilon);
        expect_near(function.solve(0.6), 0.87340, epsilon);
        expect_near(function.solve(0.65), 1.01672, epsilon);
        expect_near(function.solve(0.7), 1.11992, epsilon);
        expect_near(function.solve(0.75), 1.18232, epsilon);
        expect_near(function.solve(0.8), 1.20651, epsilon);
        expect_near(function.solve(0.85), 1.19641, epsilon);
        expect_near(function.solve(0.9), 1.15613, epsilon);
        expect_near(function.solve(0.95), 1.08954, epsilon);
        expect_near(function.solve(1.0), 1.0, epsilon);
    }

    fn test_bezier_finite_range(function: &CubicBezier) {
        let mut i = 0.0;
        while i <= 1.01 {
            assert!(function.solve(i).is_finite());
            assert!(function.slope(i).is_finite());
            assert!(function.get_x2().is_finite());
            assert!(function.get_y2().is_finite());
            assert!(function.sample_curve_x(i).is_finite());
            assert!(function.sample_curve_y(i).is_finite());
            assert!(function.sample_curve_derivative_x(i).is_finite());
            assert!(function.sample_curve_derivative_y(i).is_finite());
            i += 0.05;
        }
    }

    // Tests that solving the bezier works with huge value infinity evaluation
    #[test]
    fn clamp_infinity_evaluation() {
        let max = f64::MAX;
        let lowest = f64::MIN;
        let test_cases = [
            CubicBezier::new(0.5, max, 0.5, max),
            CubicBezier::new(0.5, lowest, 0.5, max),
            CubicBezier::new(0.5, max, 0.5, lowest),
            CubicBezier::new(0.5, lowest, 0.5, lowest),
            CubicBezier::new(0.0, max, 0.0, max),
            CubicBezier::new(0.0, lowest, 0.0, max),
            CubicBezier::new(0.0, max, 0.0, lowest),
            CubicBezier::new(0.0, lowest, 0.0, lowest),
            CubicBezier::new(1.0, max, 1.0, max),
            CubicBezier::new(1.0, lowest, 1.0, max),
            CubicBezier::new(1.0, max, 1.0, lowest),
            CubicBezier::new(1.0, lowest, 1.0, lowest),
            CubicBezier::new(0.0, 0.0, 0.0, max),
            CubicBezier::new(0.0, lowest, 0.0, 0.0),
            CubicBezier::new(1.0, 0.0, 0.0, lowest),
            CubicBezier::new(0.0, lowest, 1.0, 1.0),
        ];

        for tc in &test_cases {
            test_bezier_finite_range(tc);
        }
    }

    #[test]
    fn range() {
        let epsilon = 0.00015;

        // Derivative is a constant.
        let mut function = CubicBezier::new(0.25, 1.0 / 3.0, 0.75, 2.0 / 3.0);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());

        // Derivative is linear.
        function = CubicBezier::new(0.25, -0.5, 0.75, -1.0 / 6.0);
        expect_near(function.range_min(), -0.225, epsilon);
        assert_eq!(1.0, function.range_max());

        // Derivative has no real roots.
        function = CubicBezier::new(0.25, 0.25, 0.75, 0.5);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());

        // Derivative has exactly one real root.
        function = CubicBezier::new(0.0, 1.0, 1.0, 0.0);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());

        // Derivative has one root < 0 and one root > 1.
        function = CubicBezier::new(0.25, 0.1, 0.75, 0.9);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());

        // Derivative has two roots in [0,1].
        function = CubicBezier::new(0.25, 2.5, 0.75, 0.5);
        assert_eq!(0.0, function.range_min());
        expect_near(function.range_max(), 1.28818, epsilon);
        function = CubicBezier::new(0.25, 0.5, 0.75, -1.5);
        expect_near(function.range_min(), -0.28818, epsilon);
        assert_eq!(1.0, function.range_max());

        // Derivative has one root < 0 and one root in [0,1].
        function = CubicBezier::new(0.25, 0.1, 0.75, 1.5);
        assert_eq!(0.0, function.range_min());
        expect_near(function.range_max(), 1.10755, epsilon);

        // Derivative has one root in [0,1] and one root > 1.
        function = CubicBezier::new(0.25, -0.5, 0.75, 0.9);
        expect_near(function.range_min(), -0.10755, epsilon);
        assert_eq!(1.0, function.range_max());

        // Derivative has two roots < 0.
        function = CubicBezier::new(0.25, 0.3, 0.75, 0.633);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());

        // Derivative has two roots > 1.
        function = CubicBezier::new(0.25, 0.367, 0.75, 0.7);
        assert_eq!(0.0, function.range_min());
        assert_eq!(1.0, function.range_max());
    }

    #[test]
    fn slope() {
        let function = CubicBezier::new(0.25, 0.0, 0.75, 1.0);

        let epsilon = 0.00015;

        expect_near(function.slope(-0.1), 0.0, epsilon);
        expect_near(function.slope(0.0), 0.0, epsilon);
        expect_near(function.slope(0.05), 0.42170, epsilon);
        expect_near(function.slope(0.1), 0.69778, epsilon);
        expect_near(function.slope(0.15), 0.89121, epsilon);
        expect_near(function.slope(0.2), 1.03184, epsilon);
        expect_near(function.slope(0.25), 1.13576, epsilon);
        expect_near(function.slope(0.3), 1.21239, epsilon);
        expect_near(function.slope(0.35), 1.26751, epsilon);
        expect_near(function.slope(0.4), 1.30474, epsilon);
        expect_near(function.slope(0.45), 1.32628, epsilon);
        expect_near(function.slope(0.5), 1.33333, epsilon);
        expect_near(function.slope(0.55), 1.32628, epsilon);
        expect_near(function.slope(0.6), 1.30474, epsilon);
        expect_near(function.slope(0.65), 1.26751, epsilon);
        expect_near(function.slope(0.7), 1.21239, epsilon);
        expect_near(function.slope(0.75), 1.13576, epsilon);
        expect_near(function.slope(0.8), 1.03184, epsilon);
        expect_near(function.slope(0.85), 0.89121, epsilon);
        expect_near(function.slope(0.9), 0.69778, epsilon);
        expect_near(function.slope(0.95), 0.42170, epsilon);
        expect_near(function.slope(1.0), 0.0, epsilon);
        expect_near(function.slope(1.1), 0.0, epsilon);
    }

    #[test]
    fn input_out_of_range() {
        let simple = CubicBezier::new(0.5, 1.0, 0.5, 1.0);
        assert_eq!(-2.0, simple.solve(-1.0));
        assert_eq!(1.0, simple.solve(2.0));

        let at_edge_of_range = CubicBezier::new(0.5, 1.0, 0.5, 1.0);
        assert_eq!(0.0, at_edge_of_range.solve(0.0));
        assert_eq!(1.0, at_edge_of_range.solve(1.0));

        let large_epsilon = CubicBezier::new(0.5, 1.0, 0.5, 1.0);
        assert_eq!(-2.0, large_epsilon.solve_with_epsilon(-1.0, 1.0));
        assert_eq!(1.0, large_epsilon.solve_with_epsilon(2.0, 1.0));

        let coincident_endpoints = CubicBezier::new(0.0, 0.0, 1.0, 1.0);
        assert_eq!(-1.0, coincident_endpoints.solve(-1.0));
        assert_eq!(2.0, coincident_endpoints.solve(2.0));

        let vertical_gradient = CubicBezier::new(0.0, 1.0, 1.0, 0.0);
        assert_eq!(0.0, vertical_gradient.solve(-1.0));
        assert_eq!(1.0, vertical_gradient.solve(2.0));

        let vertical_trailing_gradient = CubicBezier::new(0.5, 0.0, 1.0, 0.5);
        assert_eq!(0.0, vertical_trailing_gradient.solve(-1.0));
        assert_eq!(1.0, vertical_trailing_gradient.solve(2.0));

        let distinct_endpoints = CubicBezier::new(0.1, 0.2, 0.8, 0.8);
        assert_eq!(-2.0, distinct_endpoints.solve(-1.0));
        assert_eq!(2.0, distinct_endpoints.solve(2.0));

        let coincident_leading_endpoint = CubicBezier::new(0.0, 0.0, 0.5, 1.0);
        assert_eq!(-2.0, coincident_leading_endpoint.solve(-1.0));
        assert_eq!(1.0, coincident_leading_endpoint.solve(2.0));

        let coincident_trailing_endpoint = CubicBezier::new(1.0, 0.5, 1.0, 1.0);
        assert_eq!(-0.5, coincident_trailing_endpoint.solve(-1.0));
        assert_eq!(1.0, coincident_trailing_endpoint.solve(2.0));

        // Two special cases with three coincident points. Both are equivalent
        // to linear.
        let all_zeros = CubicBezier::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(-1.0, all_zeros.solve(-1.0));
        assert_eq!(2.0, all_zeros.solve(2.0));

        let all_ones = CubicBezier::new(1.0, 1.0, 1.0, 1.0);
        assert_eq!(-1.0, all_ones.solve(-1.0));
        assert_eq!(2.0, all_ones.solve(2.0));
    }

    #[test]
    fn get_points() {
        let epsilon = 0.00015;

        let cubic1 = CubicBezier::new(0.1, 0.2, 0.8, 0.9);
        expect_near(0.1, cubic1.get_x1(), epsilon);
        expect_near(0.2, cubic1.get_y1(), epsilon);
        expect_near(0.8, cubic1.get_x2(), epsilon);
        expect_near(0.9, cubic1.get_y2(), epsilon);

        let cubic_zero = CubicBezier::new(0.0, 0.0, 0.0, 0.0);
        expect_near(0.0, cubic_zero.get_x1(), epsilon);
        expect_near(0.0, cubic_zero.get_y1(), epsilon);
        expect_near(0.0, cubic_zero.get_x2(), epsilon);
        expect_near(0.0, cubic_zero.get_y2(), epsilon);

        let cubic_one = CubicBezier::new(1.0, 1.0, 1.0, 1.0);
        expect_near(1.0, cubic_one.get_x1(), epsilon);
        expect_near(1.0, cubic_one.get_y1(), epsilon);
        expect_near(1.0, cubic_one.get_x2(), epsilon);
        expect_near(1.0, cubic_one.get_y2(), epsilon);

        let cubic_oor = CubicBezier::new(-0.5, -1.5, 1.5, -1.6);
        expect_near(-0.5, cubic_oor.get_x1(), epsilon);
        expect_near(-1.5, cubic_oor.get_y1(), epsilon);
        expect_near(1.5, cubic_oor.get_x2(), epsilon);
        expect_near(-1.6, cubic_oor.get_y2(), epsilon);
    }

    #[test]
    fn common_easing_functions() {
        validate_solver(&CubicBezier::new(0.25, 0.1, 0.25, 1.0)); // ease
        validate_solver(&CubicBezier::new(0.42, 0.0, 1.0, 1.0)); // ease-in
        validate_solver(&CubicBezier::new(0.0, 0.0, 0.58, 1.0)); // ease-out
        validate_solver(&CubicBezier::new(0.42, 0.0, 0.58, 1.0)); // ease-in-out
    }

    #[test]
    fn linear_equivalent_beziers() {
        validate_solver(&CubicBezier::new(0.0, 0.0, 0.0, 0.0));
        validate_solver(&CubicBezier::new(1.0, 1.0, 1.0, 1.0));
    }

    #[test]
    fn control_points_outside_unit_square() {
        validate_solver(&CubicBezier::new(0.3, 1.5, 0.8, 1.5));
        validate_solver(&CubicBezier::new(0.4, -0.8, 0.7, 1.7));
        validate_solver(&CubicBezier::new(0.7, -2.0, 1.0, -1.5));
        validate_solver(&CubicBezier::new(0.0, 4.0, 1.0, -3.0));
    }

    // Extra tests (not in the C++ suite) below.

    #[test]
    fn default_epsilon_matches_cpp() {
        assert_eq!(CubicBezier::default_epsilon(), 1e-7);
    }

    #[test]
    fn to_finite_clamps_only_infinities() {
        assert_eq!(to_finite(f64::INFINITY), f64::MAX);
        assert_eq!(to_finite(f64::NEG_INFINITY), f64::MIN);
        assert_eq!(to_finite(1.5), 1.5);
        assert!(to_finite(f64::NAN).is_nan());
    }

    #[test]
    fn cpp_helpers_match_std_nan_behavior() {
        // std::min(a, b) keeps a when b is NaN, and std::max does the same.
        assert_eq!(cpp_min(1.0, f64::NAN), 1.0);
        assert_eq!(cpp_max(1.0, f64::NAN), 1.0);
        // std::min({a, b, c}) with a NaN first keeps the NaN.
        assert!(cpp_min(f64::NAN, 1.0).is_nan());
        assert_eq!(cpp_clamp(-1.0, 0.0, 1.0), 0.0);
        assert_eq!(cpp_clamp(2.0, 0.0, 1.0), 1.0);
        assert!(cpp_clamp(f64::NAN, 0.0, 1.0).is_nan());
    }

    #[test]
    fn solve_at_endpoints_is_exact_enough_for_random_curves() {
        let mut rng = XorShift64(0x9e37_79b9_7f4a_7c15);
        for _ in 0..2000 {
            let c = CubicBezier::new(
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
            );
            // x = 0 and x = 1 must be solvable even when SampleCurveX(1) is
            // a little below 1 because of rounding.
            expect_near(c.solve(0.0), 0.0, 1e-6);
            expect_near(c.solve(1.0), 1.0, 1e-6);
        }
    }

    // Differential test: the solver's t must satisfy x(t) = x, up to epsilon.
    // Monotone curves (internal x control points in [0, 1]) only.
    #[test]
    fn solve_curve_x_residual_random() {
        let mut rng = XorShift64(0x0123_4567_89ab_cdef);
        for _ in 0..2000 {
            let c = CubicBezier::new(
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
            );
            for _ in 0..20 {
                let x = rng.next_unit();
                let t = c.solve_curve_x(x, 1e-7);
                assert!(t.is_finite());
                expect_near(c.sample_curve_x(t), x, 1e-6);
            }
        }
    }

    // Differential test against a plain bisection reference that uses only
    // the monotone x(t) polynomial. Compares y values.
    #[test]
    fn solve_matches_bisection_reference_random() {
        fn reference_y(c: &CubicBezier, x: f64) -> f64 {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if c.sample_curve_x(mid) < x {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            c.sample_curve_y(0.5 * (lo + hi))
        }

        let mut rng = XorShift64(0xdead_beef_cafe_f00d);
        for _ in 0..500 {
            // Keep the x-derivative away from zero so y(x) is well conditioned.
            let c = CubicBezier::new(
                rng.next_range(0.05, 0.95),
                rng.next_range(-2.0, 3.0),
                rng.next_range(0.05, 0.95),
                rng.next_range(-2.0, 3.0),
            );
            for _ in 0..20 {
                let x = rng.next_unit();
                expect_near(c.solve(x), reference_y(&c, x), 1e-5);
            }
        }
    }

    // The range must contain y(t) for all t in [0, 1], and must be tight: a
    // dense grid search must not find a smaller min or larger max than the
    // computed range (up to grid resolution).
    #[test]
    fn range_is_tight_random() {
        let mut rng = XorShift64(0x5151_5151_0a0a_0a0a);
        for _ in 0..300 {
            let c = CubicBezier::new(
                rng.next_range(0.0, 1.0),
                rng.next_range(-3.0, 4.0),
                rng.next_range(0.0, 1.0),
                rng.next_range(-3.0, 4.0),
            );
            let n = 10_000;
            let mut grid_min = f64::INFINITY;
            let mut grid_max = f64::NEG_INFINITY;
            for i in 0..=n {
                let y = c.sample_curve_y(i as f64 / n as f64);
                grid_min = grid_min.min(y);
                grid_max = grid_max.max(y);
            }
            assert!(c.range_min() <= grid_min + 1e-9, "min too large");
            assert!(c.range_max() >= grid_max - 1e-9, "max too small");
            expect_near(c.range_min(), grid_min, 1e-4);
            expect_near(c.range_max(), grid_max, 1e-4);
        }
    }

    // Slope must agree with a central finite difference of solve().
    #[test]
    fn slope_matches_finite_difference_random() {
        let mut rng = XorShift64(0x7777_1111_3333_9999);
        for _ in 0..300 {
            let c = CubicBezier::new(
                rng.next_range(0.1, 0.9),
                rng.next_range(-1.0, 2.0),
                rng.next_range(0.1, 0.9),
                rng.next_range(-1.0, 2.0),
            );
            for _ in 0..10 {
                let x = rng.next_range(0.1, 0.9);
                // The step must be large relative to the solver's 1e-7
                // tolerance, or the reference itself is noisy.
                let h = 1e-3;
                let fd = (c.solve(x + h) - c.solve(x - h)) / (2.0 * h);
                let tol = 2e-3 * fd.abs().max(1.0);
                expect_near(c.slope(x), fd, tol);
            }
        }
    }

    // Out-of-range x uses the end gradients, so it is linear in x.
    #[test]
    fn extrapolation_is_linear_random() {
        let mut rng = XorShift64(0xabcdef12_34567890);
        for _ in 0..200 {
            let c = CubicBezier::new(
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
                rng.next_unit(),
                rng.next_range(-2.0, 3.0),
            );
            let d = rng.next_range(0.0, 3.0);
            // Linear: solve(-d) == start_gradient * (-d) (+ 0.0). Check
            // linearity in d directly.
            let y1 = c.solve(-d);
            let y2 = c.solve(-2.0 * d);
            expect_near(y2, 2.0 * y1, 1e-9 * (1.0 + y1.abs()));
            let z1 = c.solve(1.0 + d);
            let z2 = c.solve(1.0 + 2.0 * d);
            expect_near(z2 - 1.0, 2.0 * (z1 - 1.0), 1e-9 * (1.0 + z1.abs()));
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "x must be <= 1")]
    fn solve_curve_x_checks_range_in_debug() {
        // Mirrors the C++ DCHECK_LE(x, 1.0).
        CubicBezier::new(0.25, 0.1, 0.25, 1.0).solve_curve_x(1.5, 1e-7);
    }
}
