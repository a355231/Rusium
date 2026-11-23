// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of `ui/gfx/geometry/rect_f_unittest.cc`.
//!
//! C++ double literals such as `9.999965732` are written as `9.999965732_f64 as
//! f32`, so they round through `f64` as the C++ narrowing does. The C++
//! `EXPECT_RECTF_EQ` compares each component with `FloatAlmostEqual`, which is
//! 4 ULP, and `assert_rectf_eq` does the same.

use super::*;
use crate::insets_f::InsetsF;
use crate::outsets_f::OutsetsF;
use crate::point::PointF;
use crate::rect::Rect;
use crate::size::{Size, SizeF};
use crate::test_util::float_eq;
use crate::vector::Vector2dF;

/// `EXPECT_RECTF_EQ`: each of x, y, width and height is within 4 ULP.
fn assert_rectf_eq(expected: RectF, actual: RectF) {
    assert!(
        float_eq(expected.x(), actual.x())
            && float_eq(expected.y(), actual.y())
            && float_eq(expected.width(), actual.width())
            && float_eq(expected.height(), actual.height()),
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn from_rect() {
    // Check that the explicit conversion from integer to float works.
    let a = Rect::new(10, 20, 30, 40);
    let b = RectF::new(10.0, 20.0, 30.0, 40.0);
    let c = RectF::from(a);
    assert_eq!(b, c);
}

#[test]
fn contains_point_f() {
    assert!(!RectF::default().contains_point(PointF::default()));
    let r = RectF::new(10.0, 20.0, 30.0, 40.0);
    assert!(!r.contains_point(PointF::new(0.0, 0.0)));
    assert!(!r.contains_point(PointF::new(9.9999, 20.0)));
    assert!(!r.contains_point(PointF::new(10.0, 19.9999)));
    assert!(r.contains_point(PointF::new(10.0, 20.0)));
    assert!(r.contains_point(PointF::new(39.9999, 20.0)));
    assert!(!r.contains_point(PointF::new(40.0, 20.0)));
    assert!(r.contains_point(PointF::new(10.0, 59.9999)));
    assert!(!r.contains_point(PointF::new(10.0, 60.0)));
    assert!(r.contains_point(PointF::new(39.9999, 59.9999)));
    assert!(!r.contains_point(PointF::new(40.0, 60.0)));
    assert!(!r.contains_point(PointF::new(100.0, 100.0)));
}

#[test]
fn contains_xy() {
    assert!(!RectF::default().contains_xy(0.0, 0.0));
    let r = RectF::new(10.0, 20.0, 30.0, 40.0);
    assert!(!r.contains_xy(0.0, 0.0));
    assert!(!r.contains_xy(9.9999, 20.0));
    assert!(!r.contains_xy(10.0, 19.9999));
    assert!(r.contains_xy(10.0, 20.0));
    assert!(r.contains_xy(39.9999, 20.0));
    assert!(!r.contains_xy(40.0, 20.0));
    assert!(r.contains_xy(10.0, 59.9999));
    assert!(!r.contains_xy(10.0, 60.0));
    assert!(r.contains_xy(39.9999, 59.9999));
    assert!(!r.contains_xy(40.0, 60.0));
    assert!(!r.contains_xy(100.0, 100.0));
}

#[test]
fn inclusive_contains_point_f() {
    assert!(RectF::default().inclusive_contains_point(PointF::default()));
    assert!(!RectF::default().inclusive_contains_point(PointF::new(0.0001, 0.0)));
    let r = RectF::new(10.0, 20.0, 30.0, 40.0);
    assert!(!r.inclusive_contains_point(PointF::new(0.0, 0.0)));
    assert!(!r.inclusive_contains_point(PointF::new(9.9999, 20.0)));
    assert!(!r.inclusive_contains_point(PointF::new(10.0, 19.9999)));
    assert!(r.inclusive_contains_point(PointF::new(10.0, 20.0)));
    assert!(r.inclusive_contains_point(PointF::new(40.0, 20.0)));
    assert!(!r.inclusive_contains_point(PointF::new(40.0001, 20.0)));
    assert!(r.inclusive_contains_point(PointF::new(10.0, 60.0)));
    assert!(!r.inclusive_contains_point(PointF::new(10.0, 60.0001)));
    assert!(r.inclusive_contains_point(PointF::new(40.0, 60.0)));
    assert!(!r.inclusive_contains_point(PointF::new(100.0, 100.0)));
}

#[test]
fn inclusive_contains_xy() {
    assert!(RectF::default().inclusive_contains_xy(0.0, 0.0));
    assert!(!RectF::default().inclusive_contains_xy(0.0001, 0.0));
    let r = RectF::new(10.0, 20.0, 30.0, 40.0);
    assert!(!r.inclusive_contains_xy(0.0, 0.0));
    assert!(!r.inclusive_contains_xy(9.9999, 20.0));
    assert!(!r.inclusive_contains_xy(10.0, 19.9999));
    assert!(r.inclusive_contains_xy(10.0, 20.0));
    assert!(r.inclusive_contains_xy(40.0, 20.0));
    assert!(!r.inclusive_contains_xy(40.0001, 20.0));
    assert!(r.inclusive_contains_xy(10.0, 60.0));
    assert!(!r.inclusive_contains_xy(10.0, 60.0001));
    assert!(r.inclusive_contains_xy(40.0, 60.0));
    assert!(!r.inclusive_contains_xy(100.0, 100.0));
}

#[test]
fn bounding_rect_test() {
    let p = |x: f32, y: f32| PointF::new(x, y);
    // If point B dominates A, then A should be the origin.
    assert_rectf_eq(RectF::new(4.2, 6.8, 0.0, 0.0), bounding_rect_f(p(4.2, 6.8), p(4.2, 6.8)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 4.3, 0.0), bounding_rect_f(p(4.2, 6.8), p(8.5, 6.8)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 0.0, 2.5), bounding_rect_f(p(4.2, 6.8), p(4.2, 9.3)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 4.3, 2.5), bounding_rect_f(p(4.2, 6.8), p(8.5, 9.3)));
    // If point A dominates B, then B should be the origin.
    assert_rectf_eq(RectF::new(4.2, 6.8, 0.0, 0.0), bounding_rect_f(p(4.2, 6.8), p(4.2, 6.8)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 4.3, 0.0), bounding_rect_f(p(8.5, 6.8), p(4.2, 6.8)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 0.0, 2.5), bounding_rect_f(p(4.2, 9.3), p(4.2, 6.8)));
    assert_rectf_eq(RectF::new(4.2, 6.8, 4.3, 2.5), bounding_rect_f(p(8.5, 9.3), p(4.2, 6.8)));
    // If neither point dominates, then the origin is a combination of the two.
    assert_rectf_eq(RectF::new(4.2, 4.2, 2.6, 2.6), bounding_rect_f(p(4.2, 6.8), p(6.8, 4.2)));
    assert_rectf_eq(RectF::new(-6.8, -6.8, 2.6, 2.6), bounding_rect_f(p(-4.2, -6.8), p(-6.8, -4.2)));
    assert_rectf_eq(RectF::new(-4.2, -4.2, 11.0, 11.0), bounding_rect_f(p(-4.2, 6.8), p(6.8, -4.2)));

    // If points A and B are so far apart that the exact A.x - B.x cannot be
    // represented by a float, the width or height grows so that both points
    // are included.
    let bounding_rect1 = bounding_rect_f(p(20.0, -1000000000.0), p(20.0, 10.0));
    assert!(bounding_rect1.inclusive_contains_point(p(20.0, -1000000000.0)));
    assert!(bounding_rect1.inclusive_contains_point(p(20.0, 10.0)));

    let bounding_rect2 = bounding_rect_f(p(20.0, 20.0), p(20.0, 1000000000.0));
    assert!(bounding_rect2.inclusive_contains_point(p(20.0, 20.0)));
    assert!(bounding_rect2.inclusive_contains_point(p(20.0, 1000000000.0)));

    let bounding_rect3 = bounding_rect_f(p(-1000000000.0, 20.0), p(20.0, 20.0));
    assert!(bounding_rect3.inclusive_contains_point(p(-1000000000.0, 20.0)));
    assert!(bounding_rect3.inclusive_contains_point(p(20.0, 20.0)));

    let bounding_rect4 = bounding_rect_f(p(20.0, 20.0), p(1000000000.0, 20.0));
    assert!(bounding_rect4.inclusive_contains_point(p(20.0, 20.0)));
    assert!(bounding_rect4.inclusive_contains_point(p(1000000000.0, 20.0)));
}

#[test]
fn union() {
    assert_rectf_eq(RectF::default(), union_rects_f(RectF::default(), RectF::default()));
    assert_rectf_eq(
        RectF::new(1.1, 2.2, 3.3, 4.4),
        union_rects_f(RectF::new(1.1, 2.2, 3.3, 4.4), RectF::new(1.1, 2.2, 3.3, 4.4)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 0.0, 8.8, 11.0),
        union_rects_f(RectF::new(0.0, 0.0, 3.3, 4.4), RectF::new(3.3, 4.4, 5.5, 6.6)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 0.0, 8.8, 11.0),
        union_rects_f(RectF::new(3.3, 4.4, 5.5, 6.6), RectF::new(0.0, 0.0, 3.3, 4.4)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 1.1, 3.3, 8.8),
        union_rects_f(RectF::new(0.0, 1.1, 3.3, 4.4), RectF::new(0.0, 5.5, 3.3, 4.4)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 1.1, 11.0, 12.1),
        union_rects_f(RectF::new(0.0, 1.1, 3.3, 4.4), RectF::new(4.4, 5.5, 6.6, 7.7)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 1.1, 11.0, 12.1),
        union_rects_f(RectF::new(4.4, 5.5, 6.6, 7.7), RectF::new(0.0, 1.1, 3.3, 4.4)),
    );
    assert_rectf_eq(
        RectF::new(2.2, 3.3, 4.4, 5.5),
        union_rects_f(RectF::new(8.8, 9.9, 0.0, 2.2), RectF::new(2.2, 3.3, 4.4, 5.5)),
    );
    assert_rectf_eq(
        RectF::new(2.2, 3.3, 4.4, 5.5),
        union_rects_f(RectF::new(2.2, 3.3, 4.4, 5.5), RectF::new(8.8, 9.9, 2.2, 0.0)),
    );
}

#[test]
fn union_even_if_empty() {
    assert_rectf_eq(
        RectF::default(),
        union_rects_even_if_empty_f(RectF::default(), RectF::default()),
    );
    assert_rectf_eq(
        RectF::new(0.0, 0.0, 3.3, 4.4),
        union_rects_even_if_empty_f(RectF::default(), RectF::new(3.3, 4.4, 0.0, 0.0)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 0.0, 8.8, 11.0),
        union_rects_even_if_empty_f(RectF::new(0.0, 0.0, 3.3, 4.4), RectF::new(3.3, 4.4, 5.5, 6.6)),
    );
    assert_rectf_eq(
        RectF::new(0.0, 0.0, 8.8, 11.0),
        union_rects_even_if_empty_f(RectF::new(3.3, 4.4, 5.5, 6.6), RectF::new(0.0, 0.0, 3.3, 4.4)),
    );
    assert_rectf_eq(
        RectF::new(2.2, 3.3, 6.6, 8.8),
        union_rects_even_if_empty_f(RectF::new(8.8, 9.9, 0.0, 2.2), RectF::new(2.2, 3.3, 4.4, 5.5)),
    );
    assert_rectf_eq(
        RectF::new(2.2, 3.3, 8.8, 6.6),
        union_rects_even_if_empty_f(RectF::new(2.2, 3.3, 4.4, 5.5), RectF::new(8.8, 9.9, 2.2, 0.0)),
    );
}

#[test]
fn union_ensures_contain_with_floating_error() {
    let mut f = 0.1f32;
    while f < 5.0 {
        let mut r1 = RectF::new(1.0, 2.0, 3.0, 4.0);
        r1.scale_xy(f, f + 0.05);
        let r2 = r1 + Vector2dF::new(10.0 + f, f - 10.0);
        let r3 = union_rects_f(r1, r2);
        assert!(r3.contains(r1));
        assert!(r3.contains(r2));
        f += 0.1;
    }
}

#[test]
fn union_if_empty_result_tiny_size() {
    let r1 = RectF::new(1e-15, 0.0, 0.0, 0.0);
    let r2 = RectF::new(0.0, 1e-15, 0.0, 0.0);
    let r3 = union_rects_even_if_empty_f(r1, r2);
    assert!(!r3.is_empty());
    assert!(r3.contains(r1));
    assert!(r3.contains(r2));
}

#[test]
fn union_max_rects() {
    let max_float = f32::MAX;
    let min_float = f32::MIN_POSITIVE; // std::numeric_limits<float>::min()
    let mut r1 = RectF::new(min_float, 0.0, max_float, max_float);
    let r2 = RectF::new(0.0, min_float, max_float, max_float);
    // This must not trigger the DCHECK failure in the C++ code.
    r1.union(r2);
}

#[test]
fn center_point() {
    // When origin is (0, 0).
    assert!(RectF::new(0.0, 0.0, 20.0, 20.0).center_point() == PointF::new(10.0, 10.0));
    // When origin is even.
    assert!(RectF::new(10.0, 10.0, 20.0, 20.0).center_point() == PointF::new(20.0, 20.0));
    // When origin is odd.
    assert!(RectF::new(11.0, 11.0, 20.0, 20.0).center_point() == PointF::new(21.0, 21.0));
    // When 0 width or height.
    assert!(RectF::new(10.0, 10.0, 0.0, 20.0).center_point() == PointF::new(10.0, 20.0));
    assert!(RectF::new(10.0, 10.0, 20.0, 0.0).center_point() == PointF::new(20.0, 10.0));
    // When an odd size.
    assert!(RectF::new(10.0, 10.0, 21.0, 21.0).center_point() == PointF::new(20.5, 20.5));
    // When an odd size and position.
    assert!(RectF::new(11.0, 11.0, 21.0, 21.0).center_point() == PointF::new(21.5, 21.5));
}

#[test]
fn scale_rect_test() {
    let input = RectF::new(3.0, 3.0, 3.0, 3.0);
    let size_f = SizeF::new(2.0, 3.0);
    let size = Size::new(3, 2);
    assert_rectf_eq(RectF::new(4.5, 4.5, 4.5, 4.5), scale_rect_uniform(input, 1.5));
    assert_rectf_eq(RectF::new(6.0, 9.0, 6.0, 9.0), scale_rect_by_size(input, size_f));
    assert_rectf_eq(RectF::new(9.0, 6.0, 9.0, 6.0), scale_rect_by_size(input, SizeF::from(size)));
    assert_rectf_eq(RectF::new(0.0, 0.0, 0.0, 0.0), scale_rect_uniform(input, 0.0));

    let max_float = f32::MAX;
    let max_int = i32::MAX;
    assert_rectf_eq(
        RectF::new(max_float, max_float, max_float, max_float),
        scale_rect_uniform(input, max_float),
    );
    let max_int_f = max_int as f32;
    assert_rectf_eq(
        RectF::new(
            input.x() * max_int_f,
            input.y() * max_int_f,
            input.width() * max_int_f,
            input.height() * max_int_f,
        ),
        scale_rect_by_size(input, SizeF::from(Size::new(max_int, max_int))),
    );

    let nan_rect = scale_rect_uniform(input, f32::NAN);
    assert!(nan_rect.x().is_nan());
    assert!(nan_rect.y().is_nan());
    // NaN is clamped to 0 by the SizeF constructor.
    assert_eq!(0.0, nan_rect.width());
    assert_eq!(0.0, nan_rect.height());
}

#[test]
fn is_expressible_as_rect() {
    assert!(RectF::default().is_expressible_as_rect());
    let min_int_f = i32::MIN as f32;
    let max_int_f = i32::MAX as f32;
    let infinity = f32::INFINITY;

    assert!(RectF::new(min_int_f + 200.0, min_int_f + 200.0, max_int_f - 200.0, max_int_f - 200.0)
        .is_expressible_as_rect());
    assert!(!RectF::new(min_int_f - 200.0, min_int_f + 200.0, max_int_f + 200.0, max_int_f + 200.0)
        .is_expressible_as_rect());
    assert!(!RectF::new(min_int_f + 200.0, min_int_f - 200.0, max_int_f + 200.0, max_int_f + 200.0)
        .is_expressible_as_rect());
    assert!(!RectF::new(min_int_f + 200.0, min_int_f + 200.0, max_int_f + 200.0, max_int_f - 200.0)
        .is_expressible_as_rect());
    assert!(!RectF::new(min_int_f + 200.0, min_int_f + 200.0, max_int_f - 200.0, max_int_f + 200.0)
        .is_expressible_as_rect());
    assert!(RectF::new(0.0, 0.0, max_int_f - 200.0, max_int_f - 200.0).is_expressible_as_rect());
    assert!(!RectF::new(200.0, 0.0, max_int_f + 200.0, max_int_f - 200.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, 200.0, max_int_f - 200.0, max_int_f + 200.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, 0.0, max_int_f + 200.0, max_int_f - 200.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, 0.0, max_int_f - 200.0, max_int_f + 200.0).is_expressible_as_rect());
    assert!(!RectF::new(infinity, 0.0, 1.0, 1.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, infinity, 1.0, 1.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, 0.0, infinity, 1.0).is_expressible_as_rect());
    assert!(!RectF::new(0.0, 0.0, 1.0, infinity).is_expressible_as_rect());
}

#[test]
fn offset() {
    let mut f = RectF::new(1.1, 2.2, 3.3, 4.4);
    assert_eq!(RectF::new(2.2, 1.1, 3.3, 4.4), f + Vector2dF::new(1.1, -1.1));
    assert_eq!(RectF::new(2.2, 1.1, 3.3, 4.4), Vector2dF::new(1.1, -1.1) + f);
    f += Vector2dF::new(1.1, -1.1);
    assert_eq!(RectF::new(2.2, 1.1, 3.3, 4.4), f);
    assert_eq!(RectF::new(1.1, 2.2, 3.3, 4.4), f - Vector2dF::new(1.1, -1.1));
    f -= Vector2dF::new(1.1, -1.1);
    assert_eq!(RectF::new(1.1, 2.2, 3.3, 4.4), f);
}

#[test]
fn corners() {
    let f = RectF::new(1.1, 2.1, 3.1, 4.1);
    assert_eq!(PointF::new(1.1, 2.1), f.origin());
    assert_eq!(PointF::new(4.2, 2.1), f.top_right());
    assert_eq!(PointF::new(1.1, 6.2), f.bottom_left());
    assert_eq!(PointF::new(4.2, 6.2), f.bottom_right());
}

#[test]
fn centers() {
    let f = RectF::new(10.1, 20.2, 30.3, 40.4);
    assert_eq!(PointF::new(10.1, 40.4), f.left_center());
    assert_eq!(PointF::new(25.25, 20.2), f.top_center());
    assert_eq!(PointF::new(40.4, 40.4), f.right_center());
    assert_eq!(25.25, f.bottom_center().x());
    assert!((60.6 - f.bottom_center().y()).abs() <= 0.001);
}

#[test]
fn transpose() {
    let mut f = RectF::new(10.1, 20.2, 30.3, 40.4);
    f.transpose();
    assert_eq!(RectF::new(20.2, 10.1, 40.4, 30.3), f);
}

#[test]
fn manhattan_distance_to_point() {
    let f = RectF::new(1.1, 2.1, 3.1, 4.1);
    assert!(float_eq(0.0, f.manhattan_distance_to_point(PointF::new(1.1, 2.1))));
    assert!(float_eq(0.0, f.manhattan_distance_to_point(PointF::new(4.2, 6.0))));
    assert!(float_eq(0.0, f.manhattan_distance_to_point(PointF::new(2.0, 4.0))));
    assert!(float_eq(3.2, f.manhattan_distance_to_point(PointF::new(0.0, 0.0))));
    assert!(float_eq(2.1, f.manhattan_distance_to_point(PointF::new(2.0, 0.0))));
    assert!(float_eq(2.9, f.manhattan_distance_to_point(PointF::new(5.0, 0.0))));
    assert!(float_eq(0.8, f.manhattan_distance_to_point(PointF::new(5.0, 4.0))));
    assert!(float_eq(2.6, f.manhattan_distance_to_point(PointF::new(5.0, 8.0))));
    assert!(float_eq(1.8, f.manhattan_distance_to_point(PointF::new(3.0, 8.0))));
    assert!(float_eq(1.9, f.manhattan_distance_to_point(PointF::new(0.0, 7.0))));
    assert!(float_eq(1.1, f.manhattan_distance_to_point(PointF::new(0.0, 3.0))));
}

#[test]
fn manhattan_internal_distance() {
    let f = RectF::new(0.0, 0.0, 400.0, 400.0);
    let epsilon = f32::EPSILON;
    assert!(float_eq(0.0, f.manhattan_internal_distance(RectF::new(-1.0, 0.0, 2.0, 1.0))));
    assert!(float_eq(epsilon, f.manhattan_internal_distance(RectF::new(400.0, 0.0, 1.0, 400.0))));
    assert!(float_eq(
        2.0 * epsilon,
        f.manhattan_internal_distance(RectF::new(-100.0, -100.0, 100.0, 100.0))
    ));
    assert!(float_eq(
        1.0 + epsilon,
        f.manhattan_internal_distance(RectF::new(-101.0, 100.0, 100.0, 100.0))
    ));
    assert!(float_eq(
        2.0 + 2.0 * epsilon,
        f.manhattan_internal_distance(RectF::new(-101.0, -101.0, 100.0, 100.0))
    ));
    assert!(float_eq(
        433.0 + 2.0 * epsilon,
        f.manhattan_internal_distance(RectF::new(630.0, 603.0, 100.0, 100.0))
    ));
    assert!(float_eq(0.0, f.manhattan_internal_distance(RectF::new(-1.0, 0.0, 1.1, 1.0))));
    assert!(float_eq(
        0.1 + epsilon,
        f.manhattan_internal_distance(RectF::new(-1.5, 0.0, 1.4, 1.0))
    ));
    assert!(float_eq(epsilon, f.manhattan_internal_distance(RectF::new(-1.5, 0.0, 1.5, 1.0))));
}

#[test]
fn inset() {
    let mut r = RectF::new(10.0, 20.0, 30.0, 40.0);
    r.inset_uniform(0.0);
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    r.inset_uniform(1.5);
    assert_rectf_eq(RectF::new(11.5, 21.5, 27.0, 37.0), r);
    r.inset_uniform(-1.5);
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    r.inset(InsetsF::vh(2.25, 1.5));
    assert_rectf_eq(RectF::new(11.5, 22.25, 27.0, 35.5), r);
    r.inset(InsetsF::vh(-2.25, -1.5));
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    // The parameters are top, left, bottom, right.
    r.inset(InsetsF::tlbr(2.25, 1.5, 4.0, 3.75));
    assert_rectf_eq(RectF::new(11.5, 22.25, 24.75, 33.75), r);
    r.inset(InsetsF::tlbr(-2.25, -1.5, -4.0, -3.75));
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    // The InsetsF parameters are top, right, bottom, left.
    r.inset(InsetsF::tlbr(1.5, 2.25, 3.75, 4.0));
    assert_rectf_eq(RectF::new(12.25, 21.5, 23.75, 34.75), r);
    r.inset(InsetsF::tlbr(-1.5, -2.25, -3.75, -4.0));
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
}

#[test]
fn outset() {
    let mut r = RectF::new(10.0, 20.0, 30.0, 40.0);
    r.outset_uniform(0.0);
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    r.outset_uniform(1.5);
    assert_rectf_eq(RectF::new(8.5, 18.5, 33.0, 43.0), r);
    r.outset_uniform(-1.5);
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    r.outset(OutsetsF::vh(2.25, 1.5));
    assert_rectf_eq(RectF::new(8.5, 17.75, 33.0, 44.5), r);
    r.outset(OutsetsF::vh(-2.25, -1.5));
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
    r.outset(OutsetsF::tlbr(2.25, 1.5, 4.0, 3.75));
    assert_rectf_eq(RectF::new(8.5, 17.75, 35.25, 46.25), r);
    r.outset(OutsetsF::tlbr(-2.25, -1.5, -4.0, -3.75));
    assert_rectf_eq(RectF::new(10.0, 20.0, 30.0, 40.0), r);
}

#[test]
fn inset_clamped() {
    let mut r = RectF::new(10.0, 20.0, 30.0, 40.0);
    r.inset_uniform(18.0);
    assert_rectf_eq(RectF::new(28.0, 38.0, 0.0, 4.0), r);
    r.inset_uniform(-18.0);
    assert_rectf_eq(RectF::new(10.0, 20.0, 36.0, 40.0), r);
    r.inset(InsetsF::vh(30.0, 15.0));
    assert_rectf_eq(RectF::new(25.0, 50.0, 6.0, 0.0), r);
    r.inset(InsetsF::vh(-30.0, -15.0));
    assert_rectf_eq(RectF::new(10.0, 20.0, 36.0, 60.0), r);
    r.inset(InsetsF::tlbr(30.0, 20.0, 50.0, 40.0));
    assert_rectf_eq(RectF::new(30.0, 50.0, 0.0, 0.0), r);
    r.inset(InsetsF::tlbr(-30.0, -20.0, -50.0, -40.0));
    assert_rectf_eq(RectF::new(10.0, 20.0, 60.0, 80.0), r);
}

#[test]
fn inclusive_intersect() {
    let mut rect = RectF::new(11.0, 12.0, 0.0, 0.0);
    assert!(rect.inclusive_intersect(RectF::new(11.0, 12.0, 13.0, 14.0)));
    assert_rectf_eq(RectF::new(11.0, 12.0, 0.0, 0.0), rect);

    rect = RectF::new(11.0, 12.0, 13.0, 14.0);
    assert!(rect.inclusive_intersect(RectF::new(24.0, 8.0, 0.0, 7.0)));
    assert_rectf_eq(RectF::new(24.0, 12.0, 0.0, 3.0), rect);

    rect = RectF::new(11.0, 12.0, 13.0, 14.0);
    assert!(rect.inclusive_intersect(RectF::new(9.0, 15.0, 4.0, 0.0)));
    assert_rectf_eq(RectF::new(11.0, 15.0, 2.0, 0.0), rect);

    rect = RectF::new(11.0, 12.0, 0.0, 14.0);
    assert!(!rect.inclusive_intersect(RectF::new(12.0, 13.0, 15.0, 16.0)));
    assert_rectf_eq(RectF::default(), rect);
}

#[test]
fn maximum_covered_rect_test() {
    // X aligned and intersect: unite.
    assert_eq!(
        RectF::new(10.0, 20.0, 30.0, 60.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(10.0, 30.0, 30.0, 50.0))
    );
    // X aligned and adjacent: unite.
    assert_eq!(
        RectF::new(10.0, 20.0, 30.0, 90.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(10.0, 60.0, 30.0, 50.0))
    );
    // X aligned and separate: choose the bigger one.
    assert_eq!(
        RectF::new(10.0, 61.0, 30.0, 50.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(10.0, 61.0, 30.0, 50.0))
    );
    // Y aligned and intersect: unite.
    assert_eq!(
        RectF::new(10.0, 20.0, 60.0, 40.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(30.0, 20.0, 40.0, 40.0))
    );
    // Y aligned and adjacent: unite.
    assert_eq!(
        RectF::new(10.0, 20.0, 70.0, 40.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(40.0, 20.0, 40.0, 40.0))
    );
    // Y aligned and separate: choose the bigger one.
    assert_eq!(
        RectF::new(41.0, 20.0, 40.0, 40.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(41.0, 20.0, 40.0, 40.0))
    );
    // Get the biggest expanded intersection.
    assert_eq!(
        RectF::new(0.0, 0.0, 9.0, 19.0),
        maximum_covered_rect_f(RectF::new(0.0, 0.0, 10.0, 10.0), RectF::new(0.0, 9.0, 9.0, 10.0))
    );
    assert_eq!(
        RectF::new(0.0, 0.0, 19.0, 9.0),
        maximum_covered_rect_f(RectF::new(0.0, 0.0, 10.0, 10.0), RectF::new(9.0, 0.0, 10.0, 9.0))
    );
    // Otherwise choose the bigger one.
    assert_eq!(
        RectF::new(20.0, 30.0, 40.0, 50.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 30.0, 40.0), RectF::new(20.0, 30.0, 40.0, 50.0))
    );
    assert_eq!(
        RectF::new(10.0, 20.0, 40.0, 50.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 40.0, 50.0), RectF::new(20.0, 30.0, 30.0, 40.0))
    );
    assert_eq!(
        RectF::new(10.0, 20.0, 40.0, 50.0),
        maximum_covered_rect_f(RectF::new(10.0, 20.0, 40.0, 50.0), RectF::new(20.0, 30.0, 40.0, 50.0))
    );
}

#[test]
fn closest_point() {
    //         r.x()=50   r.right()=350
    //            |          |
    //        1   |    2     |  3
    //      ------+----------+--------r.y()=100
    //        4   |    5(in) |  6
    //      ------+----------+--------r.bottom()=250
    //        7   |    8     |  9
    let r = RectF::new(50.0, 100.0, 300.0, 150.0);
    // 1
    assert_eq!(PointF::new(50.0, 100.0), r.closest_point(PointF::new(10.0, 20.0)));
    // 2
    assert_eq!(PointF::new(110.0, 100.0), r.closest_point(PointF::new(110.0, 80.0)));
    // 3
    assert_eq!(PointF::new(350.0, 100.0), r.closest_point(PointF::new(400.0, 80.0)));
    // 4
    assert_eq!(PointF::new(50.0, 110.0), r.closest_point(PointF::new(10.0, 110.0)));
    // 5
    assert_eq!(PointF::new(50.0, 100.0), r.closest_point(PointF::new(50.0, 100.0)));
    assert_eq!(PointF::new(150.0, 100.0), r.closest_point(PointF::new(150.0, 100.0)));
    assert_eq!(PointF::new(350.0, 100.0), r.closest_point(PointF::new(350.0, 100.0)));
    assert_eq!(PointF::new(350.0, 150.0), r.closest_point(PointF::new(350.0, 150.0)));
    assert_eq!(PointF::new(350.0, 250.0), r.closest_point(PointF::new(350.0, 250.0)));
    assert_eq!(PointF::new(150.0, 250.0), r.closest_point(PointF::new(150.0, 250.0)));
    assert_eq!(PointF::new(50.0, 250.0), r.closest_point(PointF::new(50.0, 250.0)));
    assert_eq!(PointF::new(50.0, 150.0), r.closest_point(PointF::new(50.0, 150.0)));
    assert_eq!(PointF::new(150.0, 150.0), r.closest_point(PointF::new(150.0, 150.0)));
    // 6
    assert_eq!(PointF::new(350.0, 150.0), r.closest_point(PointF::new(380.0, 150.0)));
    // 7
    assert_eq!(PointF::new(50.0, 250.0), r.closest_point(PointF::new(10.0, 280.0)));
    // 8
    assert_eq!(PointF::new(180.0, 250.0), r.closest_point(PointF::new(180.0, 300.0)));
    // 9
    assert_eq!(PointF::new(350.0, 250.0), r.closest_point(PointF::new(450.0, 450.0)));
}

#[test]
fn map_rect_test() {
    assert_rectf_eq(RectF::default(), map_rect(RectF::default(), RectF::default(), RectF::default()));
    assert_rectf_eq(
        RectF::default(),
        map_rect(RectF::new(1.0, 2.0, 3.0, 4.0), RectF::default(), RectF::new(5.0, 6.0, 7.0, 8.0)),
    );
    assert_rectf_eq(
        RectF::new(1.0, 2.0, 3.0, 4.0),
        map_rect(
            RectF::new(1.0, 2.0, 3.0, 4.0),
            RectF::new(5.0, 6.0, 7.0, 8.0),
            RectF::new(5.0, 6.0, 7.0, 8.0),
        ),
    );
    assert_rectf_eq(
        RectF::new(5.0, 6.0, 7.0, 8.0),
        map_rect(
            RectF::new(1.0, 2.0, 3.0, 4.0),
            RectF::new(1.0, 2.0, 3.0, 4.0),
            RectF::new(5.0, 6.0, 7.0, 8.0),
        ),
    );
    assert_rectf_eq(
        RectF::new(200.0, 300.0, 300.0, 400.0),
        map_rect(
            RectF::new(1.0, 2.0, 3.0, 4.0),
            RectF::new(0.0, 1.0, 6.0, 8.0),
            RectF::new(100.0, 200.0, 600.0, 800.0),
        ),
    );
    assert_rectf_eq(
        RectF::new(1.0, 2.0, 3.0, 4.0),
        map_rect(
            RectF::new(200.0, 300.0, 300.0, 400.0),
            RectF::new(100.0, 200.0, 600.0, 800.0),
            RectF::new(0.0, 1.0, 6.0, 8.0),
        ),
    );
}

#[test]
fn approximately_equal() {
    let rect1 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    let rect2 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    assert!(rect1.approximately_equal(rect2, 0.0, 0.0));
    assert!(rect1.approximately_equal(rect2, 0.001, 0.001));
    assert!(rect1.approximately_equal(rect2, 0.001, 0.00001));
    assert!(rect1.approximately_equal(rect2, 0.00001, 0.001));
    assert!(rect1.approximately_equal(rect2, 0.00001, 0.00001));

    let rect3 = RectF::new(9.999965732_f64 as f32, 20.0, 1920.0, 1080.0);
    let rect4 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    assert!(!rect3.approximately_equal(rect4, 0.0, 0.0));
    assert!(rect3.approximately_equal(rect4, 0.001, 0.001));
    assert!(rect3.approximately_equal(rect4, 0.001, 0.000001));
    assert!(!rect3.approximately_equal(rect4, 0.000001, 0.001));
    assert!(!rect3.approximately_equal(rect4, 0.000001, 0.000001));

    let rect5 = RectF::new(10.0, 20.000001_f64 as f32, 1920.0, 1080.0);
    let rect6 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    assert!(!rect5.approximately_equal(rect6, 0.0, 0.0));
    assert!(rect5.approximately_equal(rect6, 0.001, 0.001));
    assert!(!rect5.approximately_equal(rect6, 0.001, 0.0000001));
    assert!(rect5.approximately_equal(rect6, 0.0000001, 0.001));
    assert!(!rect5.approximately_equal(rect6, 0.0000001, 0.0000001));

    let rect7 = RectF::new(10.0, 20.0, 1919.99987792969_f64 as f32, 1080.0);
    let rect8 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    assert!(!rect7.approximately_equal(rect8, 0.0, 0.0));
    assert!(rect7.approximately_equal(rect8, 0.001, 0.001));
    assert!(rect7.approximately_equal(rect8, 0.001, 0.00001));
    assert!(!rect7.approximately_equal(rect8, 0.00001, 0.001));
    assert!(!rect7.approximately_equal(rect8, 0.00001, 0.00001));

    let rect9 = RectF::new(10.0, 20.0, 1920.0, 1080.0001_f64 as f32);
    let rect10 = RectF::new(10.0, 20.0, 1920.0, 1080.0);
    assert!(!rect9.approximately_equal(rect10, 0.0, 0.0));
    assert!(rect9.approximately_equal(rect10, 0.001, 0.001));
    assert!(!rect9.approximately_equal(rect10, 0.001, 0.000001));
    assert!(rect9.approximately_equal(rect10, 0.000001, 0.001));
    assert!(!rect9.approximately_equal(rect10, 0.000001, 0.000001));
}
