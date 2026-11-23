// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of `ui/gfx/geometry/rect_conversions_unittest.cc`.

use super::*;
use crate::rect::Rect;
use crate::rect_f::RectF;

const MAX_INT: i32 = i32::MAX;
const MAX_FLOAT: f32 = f32::MAX;
const EPSILON_FLOAT: f32 = f32::EPSILON;
const MAX_INT_F: f32 = i32::MAX as f32;
const MIN_INT_F: f32 = i32::MIN as f32;

#[test]
fn to_enclosed_rect_test() {
    assert_eq!(Rect::default(), to_enclosed_rect(RectF::default()));
    assert_eq!(Rect::new(-1, -1, 2, 2), to_enclosed_rect(RectF::new(-1.5, -1.5, 3.0, 3.0)));
    assert_eq!(Rect::new(-1, -1, 3, 3), to_enclosed_rect(RectF::new(-1.5, -1.5, 3.5, 3.5)));
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        to_enclosed_rect(RectF::new(MAX_FLOAT, MAX_FLOAT, 2.0, 2.0))
    );
    assert_eq!(
        Rect::new(0, 0, MAX_INT, MAX_INT),
        to_enclosed_rect(RectF::new(0.0, 0.0, MAX_FLOAT, MAX_FLOAT))
    );
    assert_eq!(
        Rect::new(20001, 20001, 0, 0),
        to_enclosed_rect(RectF::new(20000.5, 20000.5, 0.5, 0.5))
    );
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        to_enclosed_rect(RectF::new(MAX_INT_F, MAX_INT_F, MAX_INT_F, MAX_INT_F))
    );
    assert_eq!(Rect::new(2, 3, 5, 5), to_enclosed_rect(RectF::new(1.9999, 2.0002, 5.9998, 6.0001)));
    assert_eq!(Rect::new(2, 3, 6, 4), to_enclosed_rect(RectF::new(1.9999, 2.0001, 6.0002, 5.9998)));
    assert_eq!(Rect::new(2, 3, 5, 5), to_enclosed_rect(RectF::new(1.9998, 2.0002, 6.0001, 5.9999)));
}

#[test]
fn to_enclosed_rect_huge_rect_f() {
    let source = RectF::new(MIN_INT_F, MIN_INT_F, MAX_INT_F * 3.0, MAX_INT_F * 3.0);
    let enclosed = to_enclosed_rect(source);
    // That rect can't be represented, but it should be big.
    assert_eq!(MAX_INT, enclosed.width());
    assert_eq!(MAX_INT, enclosed.height());
    // It should include some axis near the global origin.
    assert!(1 > enclosed.x());
    assert!(1 > enclosed.y());
    // And it should not cause computation issues for itself.
    assert!(0 < enclosed.right());
    assert!(0 < enclosed.bottom());
}

#[test]
fn to_enclosing_rect_test() {
    assert_eq!(Rect::default(), to_enclosing_rect(RectF::default()));
    assert_eq!(Rect::new(5, 5, 0, 0), to_enclosing_rect(RectF::new(5.5, 5.5, 0.0, 0.0)));
    assert_eq!(Rect::new(3, 2, 0, 0), to_enclosing_rect(RectF::new(3.5, 2.5, EPSILON_FLOAT, -0.0)));
    assert_eq!(Rect::new(3, 2, 0, 1), to_enclosing_rect(RectF::new(3.5, 2.5, 0.0, 0.001)));
    assert_eq!(Rect::new(-2, -2, 4, 4), to_enclosing_rect(RectF::new(-1.5, -1.5, 3.0, 3.0)));
    assert_eq!(Rect::new(-2, -2, 4, 4), to_enclosing_rect(RectF::new(-1.5, -1.5, 3.5, 3.5)));
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        to_enclosing_rect(RectF::new(MAX_FLOAT, MAX_FLOAT, 2.0, 2.0))
    );
    assert_eq!(
        Rect::new(0, 0, MAX_INT, MAX_INT),
        to_enclosing_rect(RectF::new(0.0, 0.0, MAX_FLOAT, MAX_FLOAT))
    );
    assert_eq!(Rect::new(20000, 20000, 1, 1), to_enclosing_rect(RectF::new(20000.5, 20000.5, 0.5, 0.5)));
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        to_enclosing_rect(RectF::new(MAX_INT_F, MAX_INT_F, MAX_INT_F, MAX_INT_F))
    );
    assert_eq!(
        Rect::new(-1, -1, 22777713, 2),
        to_enclosing_rect(RectF::new(-0.5, -0.5, 22777712.0, 1.0))
    );
    assert_eq!(Rect::new(1, 2, 7, 7), to_enclosing_rect(RectF::new(1.9999, 2.0002, 5.9998, 6.0001)));
    assert_eq!(Rect::new(1, 2, 8, 6), to_enclosing_rect(RectF::new(1.9999, 2.0001, 6.0002, 5.9998)));
    assert_eq!(Rect::new(1, 2, 7, 7), to_enclosing_rect(RectF::new(1.9998, 2.0002, 6.0001, 5.9999)));
}

#[test]
fn to_enclosing_rect_huge_rect_f() {
    let source = RectF::new(MIN_INT_F, MIN_INT_F, MAX_INT_F * 3.0, MAX_INT_F * 3.0);
    let enclosing = to_enclosing_rect(source);
    // That rect can't be represented, but it should be big.
    assert_eq!(MAX_INT, enclosing.width());
    assert_eq!(MAX_INT, enclosing.height());
    // It should include some axis near the global origin.
    assert!(1 > enclosing.x());
    assert!(1 > enclosing.y());
    // And it should cause computation issues for itself.
    assert!(0 < enclosing.right());
    assert!(0 < enclosing.bottom());
}

#[test]
fn to_enclosing_rect_ignoring_error_test() {
    const ERROR: f32 = 0.001;
    let e = |r: RectF| to_enclosing_rect_ignoring_error(r, ERROR);
    assert_eq!(Rect::default(), e(RectF::default()));
    assert_eq!(Rect::new(5, 5, 0, 0), e(RectF::new(5.5, 5.5, 0.0, 0.0)));
    assert_eq!(Rect::new(3, 2, 0, 0), e(RectF::new(3.5, 2.5, EPSILON_FLOAT, -0.0)));
    assert_eq!(Rect::new(3, 2, 0, 1), e(RectF::new(3.5, 2.5, 0.0, 0.001)));
    assert_eq!(Rect::new(-2, -2, 4, 4), e(RectF::new(-1.5, -1.5, 3.0, 3.0)));
    assert_eq!(Rect::new(-2, -2, 4, 4), e(RectF::new(-1.5, -1.5, 3.5, 3.5)));
    assert_eq!(Rect::new(MAX_INT, MAX_INT, 0, 0), e(RectF::new(MAX_FLOAT, MAX_FLOAT, 2.0, 2.0)));
    assert_eq!(Rect::new(0, 0, MAX_INT, MAX_INT), e(RectF::new(0.0, 0.0, MAX_FLOAT, MAX_FLOAT)));
    assert_eq!(Rect::new(20000, 20000, 1, 1), e(RectF::new(20000.5, 20000.5, 0.5, 0.5)));
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        e(RectF::new(MAX_INT_F, MAX_INT_F, MAX_INT_F, MAX_INT_F))
    );
    assert_eq!(Rect::new(-1, -1, 22777713, 2), e(RectF::new(-0.5, -0.5, 22777712.0, 1.0)));
    assert_eq!(Rect::new(2, 2, 6, 6), e(RectF::new(1.9999, 2.0002, 5.9998, 6.0001)));
    assert_eq!(Rect::new(2, 2, 6, 6), e(RectF::new(1.9999, 2.0001, 6.0002, 5.9998)));
    assert_eq!(Rect::new(2, 2, 6, 6), e(RectF::new(1.9998, 2.0002, 6.0001, 5.9999)));
}

#[test]
fn to_nearest_rect_test() {
    let rect = Rect::default();
    assert_eq!(rect, to_nearest_rect(RectF::from(rect)));
    let rect = Rect::new(-1, -1, 3, 3);
    assert_eq!(rect, to_nearest_rect(RectF::from(rect)));
    let rectf = RectF::new(-1.00001, -0.999999, 3.0000001, 2.999999);
    assert_eq!(rect, to_nearest_rect(rectf));
}

#[test]
fn to_floored_rect() {
    assert_eq!(Rect::default(), to_floored_rect_deprecated(RectF::default()));
    assert_eq!(Rect::new(-2, -2, 3, 3), to_floored_rect_deprecated(RectF::new(-1.5, -1.5, 3.0, 3.0)));
    assert_eq!(Rect::new(-2, -2, 3, 3), to_floored_rect_deprecated(RectF::new(-1.5, -1.5, 3.5, 3.5)));
    assert_eq!(
        Rect::new(20000, 20000, 0, 0),
        to_floored_rect_deprecated(RectF::new(20000.5, 20000.5, 0.5, 0.5))
    );
}

#[test]
fn is_nearest_rect_within_distance_test() {
    // Not in the C++ test file. Covers the IsNearestRectWithinDistance
    // function, which the C++ code also defines.
    assert!(is_nearest_rect_within_distance(RectF::new(-1.0, -1.0, 3.0, 3.0), 0.01));
    assert!(!is_nearest_rect_within_distance(RectF::new(-1.5, -1.0, 3.0, 3.0), 0.01));
}
