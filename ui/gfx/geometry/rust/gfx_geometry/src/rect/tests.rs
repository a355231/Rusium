// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of `ui/gfx/geometry/rect_unittest.cc`.

use super::*;
use crate::insets::Insets;
use crate::outsets::Outsets;
use crate::point::Point;
use crate::size::Size;
use crate::vector::Vector2d;

const MAX_INT: i32 = i32::MAX;
const MIN_INT: i32 = i32::MIN;

#[test]
fn contains() {
    // (rect_x, rect_y, rect_width, rect_height, point_x, point_y, contained)
    let contains_cases: [(i32, i32, i32, i32, i32, i32, bool); 8] = [
        (0, 0, 10, 10, 0, 0, true),
        (0, 0, 10, 10, 5, 5, true),
        (0, 0, 10, 10, 9, 9, true),
        (0, 0, 10, 10, 5, 10, false),
        (0, 0, 10, 10, 10, 5, false),
        (0, 0, 10, 10, -1, -1, false),
        (0, 0, 10, 10, 50, 50, false),
        // The C++ test also has `{0, 0, -10, -10, 0, 0, false}` under NDEBUG.
        // A negative size becomes zero here, so the result is the same.
        (0, 0, -10, -10, 0, 0, false),
    ];
    for &(rx, ry, rw, rh, px, py, contained) in &contains_cases {
        let rect = Rect::new(rx, ry, rw, rh);
        assert_eq!(contained, rect.contains_xy(px, py));
    }
}

#[test]
fn intersects() {
    // (x1, y1, w1, h1, x2, y2, w2, h2, intersects)
    let tests: [(i32, i32, i32, i32, i32, i32, i32, i32, bool); 10] = [
        (0, 0, 0, 0, 0, 0, 0, 0, false),
        (0, 0, 0, 0, -10, -10, 20, 20, false),
        (-10, 0, 0, 20, 0, -10, 20, 0, false),
        (0, 0, 10, 10, 0, 0, 10, 10, true),
        (0, 0, 10, 10, 10, 10, 10, 10, false),
        (10, 10, 10, 10, 0, 0, 10, 10, false),
        (10, 10, 10, 10, 5, 5, 10, 10, true),
        (10, 10, 10, 10, 15, 15, 10, 10, true),
        (10, 10, 10, 10, 20, 15, 10, 10, false),
        (10, 10, 10, 10, 21, 15, 10, 10, false),
    ];
    for &(x1, y1, w1, h1, x2, y2, w2, h2, intersects_expected) in &tests {
        let r1 = Rect::new(x1, y1, w1, h1);
        let r2 = Rect::new(x2, y2, w2, h2);
        assert_eq!(intersects_expected, r1.intersects(r2));
        assert_eq!(intersects_expected, r2.intersects(r1));
    }
}

#[test]
fn intersect() {
    // (r1 x,y,w,h), (r2 x,y,w,h), (r3 = intersection x,y,w,h)
    let tests: [[i32; 12]; 6] = [
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // zeros
        [0, 0, 4, 4, 0, 0, 4, 4, 0, 0, 4, 4], // equal
        [0, 0, 4, 4, 4, 4, 4, 4, 0, 0, 0, 0], // neighboring
        [0, 0, 4, 4, 2, 2, 4, 4, 2, 2, 2, 2], // overlapping corners
        [0, 0, 4, 4, 3, 1, 4, 2, 3, 1, 1, 2], // T junction
        [3, 0, 2, 2, 0, 0, 2, 2, 0, 0, 0, 0], // gap
    ];
    for t in &tests {
        let r1 = Rect::new(t[0], t[1], t[2], t[3]);
        let r2 = Rect::new(t[4], t[5], t[6], t[7]);
        let r3 = Rect::new(t[8], t[9], t[10], t[11]);
        assert_eq!(r3, intersect_rects(r1, r2));
    }
}

#[test]
fn inclusive_intersect() {
    let mut rect = Rect::new(11, 12, 0, 0);
    assert!(rect.inclusive_intersect(Rect::new(11, 12, 13, 14)));
    assert_eq!(Rect::new(11, 12, 0, 0), rect);

    rect = Rect::new(11, 12, 13, 14);
    assert!(rect.inclusive_intersect(Rect::new(24, 8, 0, 7)));
    assert_eq!(Rect::new(24, 12, 0, 3), rect);

    rect = Rect::new(11, 12, 13, 14);
    assert!(rect.inclusive_intersect(Rect::new(9, 15, 4, 0)));
    assert_eq!(Rect::new(11, 15, 2, 0), rect);

    rect = Rect::new(11, 12, 0, 14);
    assert!(!rect.inclusive_intersect(Rect::new(12, 13, 15, 16)));
    assert_eq!(Rect::default(), rect);
}

#[test]
fn union() {
    assert_eq!(Rect::default(), union_rects(Rect::default(), Rect::default()));
    assert_eq!(
        Rect::new(1, 2, 3, 4),
        union_rects(Rect::new(1, 2, 3, 4), Rect::new(1, 2, 3, 4))
    );
    assert_eq!(
        Rect::new(0, 0, 8, 10),
        union_rects(Rect::new(0, 0, 3, 4), Rect::new(3, 4, 5, 6))
    );
    assert_eq!(
        Rect::new(0, 0, 8, 10),
        union_rects(Rect::new(3, 4, 5, 6), Rect::new(0, 0, 3, 4))
    );
    assert_eq!(
        Rect::new(0, 1, 3, 8),
        union_rects(Rect::new(0, 1, 3, 4), Rect::new(0, 5, 3, 4))
    );
    assert_eq!(
        Rect::new(0, 1, 10, 11),
        union_rects(Rect::new(0, 1, 3, 4), Rect::new(4, 5, 6, 7))
    );
    assert_eq!(
        Rect::new(0, 1, 10, 11),
        union_rects(Rect::new(4, 5, 6, 7), Rect::new(0, 1, 3, 4))
    );
    assert_eq!(
        Rect::new(2, 3, 4, 5),
        union_rects(Rect::new(8, 9, 0, 2), Rect::new(2, 3, 4, 5))
    );
    assert_eq!(
        Rect::new(2, 3, 4, 5),
        union_rects(Rect::new(2, 3, 4, 5), Rect::new(8, 9, 2, 0))
    );
}

#[test]
fn union_even_if_empty() {
    assert_eq!(
        Rect::default(),
        union_rects_even_if_empty(Rect::default(), Rect::default())
    );
    assert_eq!(
        Rect::new(0, 0, 3, 4),
        union_rects_even_if_empty(Rect::default(), Rect::new(3, 4, 0, 0))
    );
    assert_eq!(
        Rect::new(0, 0, 8, 10),
        union_rects_even_if_empty(Rect::new(0, 0, 3, 4), Rect::new(3, 4, 5, 6))
    );
    assert_eq!(
        Rect::new(0, 0, 8, 10),
        union_rects_even_if_empty(Rect::new(3, 4, 5, 6), Rect::new(0, 0, 3, 4))
    );
    assert_eq!(
        Rect::new(2, 3, 6, 8),
        union_rects_even_if_empty(Rect::new(8, 9, 0, 2), Rect::new(2, 3, 4, 5))
    );
    assert_eq!(
        Rect::new(2, 3, 8, 6),
        union_rects_even_if_empty(Rect::new(2, 3, 4, 5), Rect::new(8, 9, 2, 0))
    );
}

#[test]
fn equals() {
    assert!(Rect::new(0, 0, 0, 0) == Rect::new(0, 0, 0, 0));
    assert!(Rect::new(1, 2, 3, 4) == Rect::new(1, 2, 3, 4));
    assert!(Rect::new(0, 0, 0, 0) != Rect::new(0, 0, 0, 1));
    assert!(Rect::new(0, 0, 0, 0) != Rect::new(0, 0, 1, 0));
    assert!(Rect::new(0, 0, 0, 0) != Rect::new(0, 1, 0, 0));
    assert!(Rect::new(0, 0, 0, 0) != Rect::new(1, 0, 0, 0));
}

#[test]
fn adjust_to_fit() {
    // (src x,y,w,h), (target x,y,w,h), (expected x,y,w,h)
    let tests: [[i32; 12]; 5] = [
        [0, 0, 2, 2, 0, 0, 2, 2, 0, 0, 2, 2],
        [2, 2, 3, 3, 0, 0, 4, 4, 1, 1, 3, 3],
        [-1, -1, 5, 5, 0, 0, 4, 4, 0, 0, 4, 4],
        [2, 2, 4, 4, 0, 0, 3, 3, 0, 0, 3, 3],
        [2, 2, 1, 1, 0, 0, 3, 3, 2, 2, 1, 1],
    ];
    for t in &tests {
        let r1 = Rect::new(t[0], t[1], t[2], t[3]);
        let r2 = Rect::new(t[4], t[5], t[6], t[7]);
        let r3 = Rect::new(t[8], t[9], t[10], t[11]);
        let mut u = r1;
        u.adjust_to_fit(r2);
        assert_eq!(r3, u);
    }
}

#[test]
fn subtract() {
    let mut result;

    // Matching.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(10, 10, 20, 20));
    assert_eq!(Rect::new(0, 0, 0, 0), result);

    // Contains.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(5, 5, 30, 30));
    assert_eq!(Rect::new(0, 0, 0, 0), result);

    // No intersection.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(30, 30, 30, 30));
    assert_eq!(Rect::new(10, 10, 20, 20), result);

    // Not a complete intersection in either direction.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(15, 15, 20, 20));
    assert_eq!(Rect::new(10, 10, 20, 20), result);

    // Complete intersection in the x-direction, top edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(10, 15, 20, 20));
    assert_eq!(Rect::new(10, 10, 20, 5), result);

    // Complete intersection in the x-direction, top edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(5, 15, 30, 20));
    assert_eq!(Rect::new(10, 10, 20, 5), result);

    // Complete intersection in the x-direction, bottom edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(5, 5, 30, 20));
    assert_eq!(Rect::new(10, 25, 20, 5), result);

    // Complete intersection in the x-direction, none of the edges is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(5, 15, 30, 1));
    assert_eq!(Rect::new(10, 10, 20, 20), result);

    // Complete intersection in the y-direction, left edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(10, 10, 10, 30));
    assert_eq!(Rect::new(20, 10, 10, 20), result);

    // Complete intersection in the y-direction, left edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(5, 5, 20, 30));
    assert_eq!(Rect::new(25, 10, 5, 20), result);

    // Complete intersection in the y-direction, right edge is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(20, 5, 20, 30));
    assert_eq!(Rect::new(10, 10, 10, 20), result);

    // Complete intersection in the y-direction, none of the edges is fully covered.
    result = Rect::new(10, 10, 20, 20);
    result.subtract(Rect::new(15, 5, 1, 30));
    assert_eq!(Rect::new(10, 10, 20, 20), result);
}

#[test]
fn is_empty() {
    assert!(Rect::new(0, 0, 0, 0).is_empty());
    assert!(Rect::new(0, 0, 0, 0).size().is_empty());
    assert!(Rect::new(0, 0, 10, 0).is_empty());
    assert!(Rect::new(0, 0, 10, 0).size().is_empty());
    assert!(Rect::new(0, 0, 0, 10).is_empty());
    assert!(Rect::new(0, 0, 0, 10).size().is_empty());
    assert!(!Rect::new(0, 0, 10, 10).is_empty());
    assert!(!Rect::new(0, 0, 10, 10).size().is_empty());
}

#[test]
fn split_vertically() {
    // Splitting when origin is (0, 0).
    let (left_half, right_half) = Rect::new(0, 0, 20, 20).split_vertically();
    assert!(left_half == Rect::new(0, 0, 10, 20));
    assert!(right_half == Rect::new(10, 0, 10, 20));

    // Splitting when origin is arbitrary.
    let (left_half, right_half) = Rect::new(10, 10, 20, 10).split_vertically();
    assert!(left_half == Rect::new(10, 10, 10, 10));
    assert!(right_half == Rect::new(20, 10, 10, 10));

    // Splitting a rectangle of zero width.
    let (left_half, right_half) = Rect::new(10, 10, 0, 10).split_vertically();
    assert!(left_half == Rect::new(10, 10, 0, 10));
    assert!(right_half == Rect::new(10, 10, 0, 10));

    // Splitting a rectangle of odd width.
    let (left_half, right_half) = Rect::new(10, 10, 5, 10).split_vertically();
    assert!(left_half == Rect::new(10, 10, 2, 10));
    assert!(right_half == Rect::new(12, 10, 3, 10));
}

#[test]
fn split_horizontally() {
    // Splitting when origin is (0, 0).
    let (top_half, bottom_half) = Rect::new(0, 0, 10, 20).split_horizontally();
    assert_eq!(Rect::new(0, 0, 10, 10), top_half);
    assert_eq!(Rect::new(0, 10, 10, 10), bottom_half);

    // Splitting when origin is arbitrary.
    let (top_half, bottom_half) = Rect::new(10, 10, 10, 20).split_horizontally();
    assert_eq!(Rect::new(10, 10, 10, 10), top_half);
    assert_eq!(Rect::new(10, 20, 10, 10), bottom_half);

    // Splitting a rectangle of zero height.
    let (top_half, bottom_half) = Rect::new(10, 10, 10, 0).split_horizontally();
    assert_eq!(Rect::new(10, 10, 10, 0), top_half);
    assert_eq!(Rect::new(10, 10, 10, 0), bottom_half);

    // Splitting a rectangle of odd height.
    let (top_half, bottom_half) = Rect::new(10, 10, 10, 5).split_horizontally();
    assert_eq!(Rect::new(10, 10, 10, 2), top_half);
    assert_eq!(Rect::new(10, 12, 10, 3), bottom_half);
}

#[test]
fn center_point() {
    // When origin is (0, 0).
    assert!(Rect::new(0, 0, 20, 20).center_point() == Point::new(10, 10));
    // When origin is even.
    assert!(Rect::new(10, 10, 20, 20).center_point() == Point::new(20, 20));
    // When origin is odd.
    assert!(Rect::new(11, 11, 20, 20).center_point() == Point::new(21, 21));
    // When 0 width or height.
    assert!(Rect::new(10, 10, 0, 20).center_point() == Point::new(10, 20));
    assert!(Rect::new(10, 10, 20, 0).center_point() == Point::new(20, 10));
    // When an odd size.
    assert!(Rect::new(10, 10, 21, 21).center_point() == Point::new(20, 20));
    // When an odd size and position.
    assert!(Rect::new(11, 11, 21, 21).center_point() == Point::new(21, 21));
}

#[test]
fn shares_edge_with() {
    let r = Rect::new(2, 3, 4, 5);

    // Must be non-overlapping.
    assert!(!r.shares_edge_with(r));

    let just_above = Rect::new(2, 1, 4, 2);
    let just_below = Rect::new(2, 8, 4, 2);
    let just_left = Rect::new(0, 3, 2, 5);
    let just_right = Rect::new(6, 3, 2, 5);
    assert!(r.shares_edge_with(just_above));
    assert!(r.shares_edge_with(just_below));
    assert!(r.shares_edge_with(just_left));
    assert!(r.shares_edge_with(just_right));

    // Wrong placement.
    let same_height_no_edge = Rect::new(0, 0, 1, 5);
    let same_width_no_edge = Rect::new(0, 0, 4, 1);
    assert!(!r.shares_edge_with(same_height_no_edge));
    assert!(!r.shares_edge_with(same_width_no_edge));

    let just_above_no_edge = Rect::new(2, 1, 5, 2); // too wide
    let just_below_no_edge = Rect::new(2, 8, 3, 2); // too narrow
    let just_left_no_edge = Rect::new(0, 3, 2, 6); // too tall
    let just_right_no_edge = Rect::new(6, 3, 2, 4); // too short
    assert!(!r.shares_edge_with(just_above_no_edge));
    assert!(!r.shares_edge_with(just_below_no_edge));
    assert!(!r.shares_edge_with(just_left_no_edge));
    assert!(!r.shares_edge_with(just_right_no_edge));
}

/// Shared overflow cases for the three `ScaleTo*Rect` functions
/// (`TestScaleRectOverflowClamp`).
fn test_scale_rect_overflow_clamp(function: fn(Rect, f32, f32) -> Rect) {
    let min_int_f = MIN_INT as f32;
    let max_int_f = MAX_INT as f32;

    // The whole rect is scaled out of kMinInt.
    let xy_underflow1 = Rect::new(-100000, -123456, 10, 20);
    assert_eq!(
        Rect::new(MIN_INT, MIN_INT, 0, 0),
        function(xy_underflow1, 100000.0, 100000.0)
    );

    // This rect's right/bottom is 0. The origin overflows, and is clamped to
    // -kMaxInt (instead of kMinInt) to keep width/height from overflowing.
    let xy_underflow2 = Rect::new(-100000, -123456, 100000, 123456);
    assert_eq!(
        Rect::new(-MAX_INT, -MAX_INT, MAX_INT, MAX_INT),
        function(xy_underflow2, 100000.0, 100000.0)
    );

    // A location overflow means that width/right and bottom/top also
    // overflow so need to be clamped.
    let xy_overflow = Rect::new(100000, 123456, 10, 20);
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        function(xy_overflow, 100000.0, 100000.0)
    );

    // In practice all rects are clamped to 0 width / 0 height so negative
    // sizes don't matter, but try this for the sake of testing.
    let size_underflow = Rect::new(-1, -2, 100000, 100000);
    assert_eq!(
        Rect::new(100000, 200000, 0, 0),
        function(size_underflow, -100000.0, -100000.0)
    );

    let size_overflow = Rect::new(-1, -2, 123456, 234567);
    assert_eq!(
        Rect::new(-100000, -200000, MAX_INT, MAX_INT),
        function(size_overflow, 100000.0, 100000.0)
    );

    // Verify width/right gets clamped properly too if x/y positive.
    let size_overflow2 = Rect::new(1, 2, 123456, 234567);
    assert_eq!(
        Rect::new(100000, 200000, MAX_INT - 100000, MAX_INT - 200000),
        function(size_overflow2, 100000.0, 100000.0)
    );

    let max_origin_rect = Rect::new(MAX_INT, MAX_INT, MAX_INT, MAX_INT);
    // width/height of max_origin_rect has already been clamped to 0.
    assert_eq!(Rect::new(MAX_INT, MAX_INT, 0, 0), max_origin_rect);
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        function(max_origin_rect, max_int_f, max_int_f)
    );

    let max_size_rect1 = Rect::new(0, 0, MAX_INT, MAX_INT);
    // A max-sized rect can't be scaled up any further in any dimension.
    assert_eq!(max_size_rect1, function(max_size_rect1, 2.0, 3.5));
    assert_eq!(max_size_rect1, function(max_size_rect1, max_int_f, max_int_f));
    // A max-sized rect scaled by a negative scale is an empty rect.
    assert_eq!(Rect::default(), function(max_size_rect1, min_int_f, min_int_f));

    let max_size_rect2 = Rect::new(-MAX_INT, -MAX_INT, MAX_INT, MAX_INT);
    assert_eq!(max_size_rect2, function(max_size_rect2, 2.0, 3.5));
    assert_eq!(max_size_rect2, function(max_size_rect2, max_int_f, max_int_f));
    assert_eq!(
        Rect::new(MAX_INT, MAX_INT, 0, 0),
        function(max_size_rect2, min_int_f, min_int_f)
    );
}

#[test]
fn scale_to_enclosed_rect_test() {
    assert_eq!(Rect::default(), scale_to_enclosed_rect_uniform(Rect::default(), 5.0));
    assert_eq!(
        Rect::new(5, 5, 5, 5),
        scale_to_enclosed_rect_uniform(Rect::new(1, 1, 1, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, -5, 0, 0),
        scale_to_enclosed_rect_uniform(Rect::new(-1, -1, 0, 0), 5.0)
    );
    assert_eq!(
        Rect::new(5, -5, 0, 5),
        scale_to_enclosed_rect_uniform(Rect::new(1, -1, 0, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, 5, 5, 0),
        scale_to_enclosed_rect_uniform(Rect::new(-1, 1, 1, 0), 5.0)
    );
    assert_eq!(
        Rect::new(2, 3, 4, 6),
        scale_to_enclosed_rect_uniform(Rect::new(1, 2, 3, 4), 1.5)
    );
    assert_eq!(
        Rect::new(-1, -3, 0, 0),
        scale_to_enclosed_rect_uniform(Rect::new(-1, -2, 0, 0), 1.5)
    );
    assert_eq!(
        Rect::new(1, 2, 2, 1),
        scale_to_enclosed_rect_uniform(Rect::new(2, 4, 9, 8), 0.3)
    );
    test_scale_rect_overflow_clamp(|r, x, y| scale_to_enclosed_rect(r, x, y));
}

#[test]
fn scale_to_enclosing_rect_test() {
    assert_eq!(Rect::default(), scale_to_enclosing_rect_uniform(Rect::default(), 5.0));
    assert_eq!(
        Rect::new(5, 5, 5, 5),
        scale_to_enclosing_rect_uniform(Rect::new(1, 1, 1, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, -5, 0, 0),
        scale_to_enclosing_rect_uniform(Rect::new(-1, -1, 0, 0), 5.0)
    );
    assert_eq!(
        Rect::new(5, -5, 0, 5),
        scale_to_enclosing_rect_uniform(Rect::new(1, -1, 0, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, 5, 5, 0),
        scale_to_enclosing_rect_uniform(Rect::new(-1, 1, 1, 0), 5.0)
    );
    assert_eq!(
        Rect::new(1, 3, 5, 6),
        scale_to_enclosing_rect_uniform(Rect::new(1, 2, 3, 4), 1.5)
    );
    assert_eq!(
        Rect::new(-2, -3, 0, 0),
        scale_to_enclosing_rect_uniform(Rect::new(-1, -2, 0, 0), 1.5)
    );
    assert_eq!(
        Rect::new(0, 1, 4, 3),
        scale_to_enclosing_rect_uniform(Rect::new(2, 4, 9, 8), 0.3)
    );
    test_scale_rect_overflow_clamp(|r, x, y| scale_to_enclosing_rect(r, x, y));
}

#[test]
fn scale_to_rounded_rect_test() {
    assert_eq!(Rect::default(), scale_to_rounded_rect_uniform(Rect::default(), 5.0));
    assert_eq!(
        Rect::new(5, 5, 5, 5),
        scale_to_rounded_rect_uniform(Rect::new(1, 1, 1, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, -5, 0, 0),
        scale_to_rounded_rect_uniform(Rect::new(-1, -1, 0, 0), 5.0)
    );
    assert_eq!(
        Rect::new(5, -5, 0, 5),
        scale_to_rounded_rect_uniform(Rect::new(1, -1, 0, 1), 5.0)
    );
    assert_eq!(
        Rect::new(-5, 5, 5, 0),
        scale_to_rounded_rect_uniform(Rect::new(-1, 1, 1, 0), 5.0)
    );
    assert_eq!(
        Rect::new(2, 3, 4, 6),
        scale_to_rounded_rect_uniform(Rect::new(1, 2, 3, 4), 1.5)
    );
    assert_eq!(
        Rect::new(-2, -3, 0, 0),
        scale_to_rounded_rect_uniform(Rect::new(-1, -2, 0, 0), 1.5)
    );
    assert_eq!(
        Rect::new(1, 1, 2, 3),
        scale_to_rounded_rect_uniform(Rect::new(2, 4, 9, 8), 0.3)
    );
    test_scale_rect_overflow_clamp(|r, x, y| scale_to_rounded_rect(r, x, y));
}

#[test]
fn bounding_rect_test() {
    // (a, b, expected)
    let int_tests = [
        // If point B dominates A, then A should be the origin.
        (Point::new(4, 6), Point::new(4, 6), Rect::new(4, 6, 0, 0)),
        (Point::new(4, 6), Point::new(8, 6), Rect::new(4, 6, 4, 0)),
        (Point::new(4, 6), Point::new(4, 9), Rect::new(4, 6, 0, 3)),
        (Point::new(4, 6), Point::new(8, 9), Rect::new(4, 6, 4, 3)),
        // If point A dominates B, then B should be the origin.
        (Point::new(4, 6), Point::new(4, 6), Rect::new(4, 6, 0, 0)),
        (Point::new(8, 6), Point::new(4, 6), Rect::new(4, 6, 4, 0)),
        (Point::new(4, 9), Point::new(4, 6), Rect::new(4, 6, 0, 3)),
        (Point::new(8, 9), Point::new(4, 6), Rect::new(4, 6, 4, 3)),
        // If neither point dominates, then the origin is a combination of the two.
        (Point::new(4, 6), Point::new(6, 4), Rect::new(4, 4, 2, 2)),
        (Point::new(-4, -6), Point::new(-6, -4), Rect::new(-6, -6, 2, 2)),
        (Point::new(-4, 6), Point::new(6, -4), Rect::new(-4, -4, 10, 10)),
    ];
    for (a, b, expected) in int_tests {
        assert_eq!(expected, bounding_rect(a, b));
    }
}

#[test]
fn offset() {
    let mut i = Rect::new(1, 2, 3, 4);
    assert_eq!(Rect::new(2, 1, 3, 4), i + Vector2d::new(1, -1));
    assert_eq!(Rect::new(2, 1, 3, 4), Vector2d::new(1, -1) + i);
    i += Vector2d::new(1, -1);
    assert_eq!(Rect::new(2, 1, 3, 4), i);
    assert_eq!(Rect::new(1, 2, 3, 4), i - Vector2d::new(1, -1));
    i -= Vector2d::new(1, -1);
    assert_eq!(Rect::new(1, 2, 3, 4), i);
    i.offset_xy(2, -2);
    assert_eq!(Rect::new(3, 0, 3, 4), i);

    assert_eq!(
        Rect::new(MAX_INT - 2, MAX_INT - 2, 2, 2),
        Rect::new(0, 0, MAX_INT - 2, MAX_INT - 2) + Vector2d::new(MAX_INT - 2, MAX_INT - 2)
    );
    assert_eq!(
        Rect::new(MAX_INT - 2, MAX_INT - 2, 2, 2),
        Rect::new(0, 0, MAX_INT - 2, MAX_INT - 2) - Vector2d::new(2 - MAX_INT, 2 - MAX_INT)
    );
}

#[test]
fn corners() {
    let i = Rect::new(1, 2, 3, 4);
    assert_eq!(Point::new(1, 2), i.origin());
    assert_eq!(Point::new(4, 2), i.top_right());
    assert_eq!(Point::new(1, 6), i.bottom_left());
    assert_eq!(Point::new(4, 6), i.bottom_right());
}

#[test]
fn centers() {
    let i = Rect::new(10, 20, 30, 40);
    assert_eq!(Point::new(10, 40), i.left_center());
    assert_eq!(Point::new(25, 20), i.top_center());
    assert_eq!(Point::new(40, 40), i.right_center());
    assert_eq!(Point::new(25, 60), i.bottom_center());
}

#[test]
fn transpose() {
    let mut i = Rect::new(10, 20, 30, 40);
    i.transpose();
    assert_eq!(Rect::new(20, 10, 40, 30), i);
}

#[test]
fn manhattan_distance_to_point() {
    let i = Rect::new(1, 2, 3, 4);
    assert_eq!(0, i.manhattan_distance_to_point(Point::new(1, 2)));
    assert_eq!(0, i.manhattan_distance_to_point(Point::new(4, 6)));
    assert_eq!(0, i.manhattan_distance_to_point(Point::new(2, 4)));
    assert_eq!(3, i.manhattan_distance_to_point(Point::new(0, 0)));
    assert_eq!(2, i.manhattan_distance_to_point(Point::new(2, 0)));
    assert_eq!(3, i.manhattan_distance_to_point(Point::new(5, 0)));
    assert_eq!(1, i.manhattan_distance_to_point(Point::new(5, 4)));
    assert_eq!(3, i.manhattan_distance_to_point(Point::new(5, 8)));
    assert_eq!(2, i.manhattan_distance_to_point(Point::new(3, 8)));
    assert_eq!(2, i.manhattan_distance_to_point(Point::new(0, 7)));
    assert_eq!(1, i.manhattan_distance_to_point(Point::new(0, 3)));
}

#[test]
fn manhattan_internal_distance() {
    let i = Rect::new(0, 0, 400, 400);
    assert_eq!(0, i.manhattan_internal_distance(Rect::new(-1, 0, 2, 1)));
    assert_eq!(1, i.manhattan_internal_distance(Rect::new(400, 0, 1, 400)));
    assert_eq!(2, i.manhattan_internal_distance(Rect::new(-100, -100, 100, 100)));
    assert_eq!(2, i.manhattan_internal_distance(Rect::new(-101, 100, 100, 100)));
    assert_eq!(4, i.manhattan_internal_distance(Rect::new(-101, -101, 100, 100)));
    assert_eq!(435, i.manhattan_internal_distance(Rect::new(630, 603, 100, 100)));
}

#[test]
fn integer_overflow() {
    let limit = MAX_INT;
    let min_limit = MIN_INT;
    let expected_thickness = 10;
    let large_number = limit - expected_thickness;

    let height_overflow = Rect::new(0, large_number, 100, 100);
    assert_eq!(large_number, height_overflow.y());
    assert_eq!(expected_thickness, height_overflow.height());

    let width_overflow = Rect::new(large_number, 0, 100, 100);
    assert_eq!(large_number, width_overflow.x());
    assert_eq!(expected_thickness, width_overflow.width());

    let size_height_overflow =
        Rect::from_origin_size(Point::new(0, large_number), Size::new(100, 100));
    assert_eq!(large_number, size_height_overflow.y());
    assert_eq!(expected_thickness, size_height_overflow.height());

    let size_width_overflow =
        Rect::from_origin_size(Point::new(large_number, 0), Size::new(100, 100));
    assert_eq!(large_number, size_width_overflow.x());
    assert_eq!(expected_thickness, size_width_overflow.width());

    let mut set_height_overflow = Rect::new(0, large_number, 100, 5);
    assert_eq!(5, set_height_overflow.height());
    set_height_overflow.set_height(100);
    assert_eq!(expected_thickness, set_height_overflow.height());

    let mut set_y_overflow = Rect::new(100, 100, 100, 100);
    assert_eq!(100, set_y_overflow.height());
    set_y_overflow.set_y(large_number);
    assert_eq!(expected_thickness, set_y_overflow.height());

    let mut set_width_overflow = Rect::new(large_number, 0, 5, 100);
    assert_eq!(5, set_width_overflow.width());
    set_width_overflow.set_width(100);
    assert_eq!(expected_thickness, set_width_overflow.width());

    let mut set_x_overflow = Rect::new(100, 100, 100, 100);
    assert_eq!(100, set_x_overflow.width());
    set_x_overflow.set_x(large_number);
    assert_eq!(expected_thickness, set_x_overflow.width());

    let large_offset = Point::new(large_number, large_number);
    let size = Size::new(100, 100);
    let expected_size = Size::new(10, 10);

    let mut set_origin_overflow = Rect::new(100, 100, 100, 100);
    assert_eq!(size, set_origin_overflow.size());
    set_origin_overflow.set_origin(large_offset);
    assert_eq!(large_offset, set_origin_overflow.origin());
    assert_eq!(expected_size, set_origin_overflow.size());

    let mut set_size_overflow = Rect::new(large_number, large_number, 5, 5);
    assert_eq!(Size::new(5, 5), set_size_overflow.size());
    set_size_overflow.set_size(size);
    assert_eq!(large_offset, set_size_overflow.origin());
    assert_eq!(expected_size, set_size_overflow.size());

    let mut set_rect_overflow = Rect::default();
    set_rect_overflow.set_rect(large_number, large_number, 100, 100);
    assert_eq!(large_offset, set_rect_overflow.origin());
    assert_eq!(expected_size, set_rect_overflow.size());

    // Insetting an empty rect, but the total inset (left + right) could overflow.
    let mut inset_overflow = Rect::default();
    inset_overflow.inset(Insets::tlbr(large_number, large_number, 100, 100));
    assert_eq!(large_offset, inset_overflow.origin());
    assert_eq!(Size::default(), inset_overflow.size());

    // Insetting by the min limit in all directions cannot represent width()
    // without overflow, so the width clamps.
    let mut inset_overflow2 = Rect::default();
    inset_overflow2.inset_uniform(min_limit);
    assert_eq!(inset_overflow2, Rect::new(min_limit, min_limit, limit, limit));

    // Insetting where the width shouldn't change. The insets must be clamped
    // in the proper order: width - (left + right), not (width - left) - right.
    let mut inset_overflow3 = Rect::new(0, 0, limit, limit);
    inset_overflow3.inset(Insets::tlbr(-100, -100, 100, 100));
    assert_eq!(inset_overflow3, Rect::new(-100, -100, limit, limit));

    let mut inset_overflow4 = Rect::new(-1000, -1000, limit, limit);
    inset_overflow4.inset(Insets::tlbr(100, 100, -100, -100));
    assert_eq!(inset_overflow4, Rect::new(-900, -900, limit, limit));

    let mut offset_overflow = Rect::new(0, 0, 100, 100);
    offset_overflow.offset_xy(large_number, large_number);
    assert_eq!(large_offset, offset_overflow.origin());
    assert_eq!(expected_size, offset_overflow.size());

    let mut operator_overflow = Rect::new(0, 0, 100, 100);
    operator_overflow += Vector2d::new(large_number, large_number);
    assert_eq!(large_offset, operator_overflow.origin());
    assert_eq!(expected_size, operator_overflow.size());

    let origin_maxint = Rect::new(limit, limit, limit, limit);
    assert_eq!(origin_maxint, Rect::from_origin_size(Point::new(limit, limit), Size::default()));

    // A rect at the origin and a rect whose right/bottom is maxint create a
    // rect that extends from 0..maxint in both extents.
    {
        let origin_small = Rect::new(0, 0, 100, 100);
        let big_clamped = Rect::new(50, 50, limit, limit);
        assert_eq!(big_clamped.right(), limit);
        let unioned = union_rects(origin_small, big_clamped);
        let rect_limit = Rect::new(0, 0, limit, limit);
        assert_eq!(unioned, rect_limit);
    }

    // A rect that would overflow width (but not right) is clamped, and has
    // maxint extents after unioning.
    {
        let small = Rect::new(-500, -400, 100, 100);
        let big = Rect::new(-400, -500, limit, limit);
        // Technically this should be limit + 100 width, but it clamps to maxint.
        assert_eq!(union_rects(small, big), Rect::new(-500, -500, limit, limit));
    }

    // A rect that would overflow right *and* width is clamped.
    {
        let clamped = Rect::new(500, 500, limit, limit);
        let positive_origin = Rect::new(100, 100, 500, 500);
        // Ideally this would be (100, 100, limit + 400, limit + 400). Width
        // overflows and clamps to limit, and right overflows too, so it clamps
        // to limit - 100.
        let expected_rect = Rect::new(100, 100, limit - 100, limit - 100);
        assert_eq!(union_rects(clamped, positive_origin), expected_rect);
    }

    // Unioning a left=minint rect with a right=maxint rect. Both ends of the
    // spectrum cannot fit in the same rect, so keep the most useful area.
    {
        let part_limit = min_limit / 3;
        let left_minint = Rect::new(min_limit, min_limit, 1, 1);
        let right_maxint = Rect::new(limit - 1, limit - 1, limit, limit);
        let expected_rect = Rect::new(part_limit, part_limit, 2 * part_limit, 2 * part_limit);
        let result = union_rects(left_minint, right_maxint);
        // The result should be maximally big.
        assert_eq!(limit, result.height());
        assert_eq!(limit, result.width());
        // The result should include the area near the origin.
        assert!(-part_limit > result.x());
        assert!(part_limit < result.right());
        assert!(-part_limit > result.y());
        assert!(part_limit < result.bottom());
        // More succinctly, but harder to read in the results.
        assert!(union_rects(left_minint, right_maxint).contains(expected_rect));
    }
}

#[test]
fn inset() {
    let mut r = Rect::new(10, 20, 30, 40);
    r.inset_uniform(0);
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.inset_uniform(1);
    assert_eq!(Rect::new(11, 21, 28, 38), r);
    r.inset_uniform(-1);
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.inset(Insets::vh(2, 1));
    assert_eq!(Rect::new(11, 22, 28, 36), r);
    r.inset(Insets::vh(-2, -1));
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    // The parameters are top, left, bottom, right.
    r.inset(Insets::tlbr(2, 1, 4, 3));
    assert_eq!(Rect::new(11, 22, 26, 34), r);
    r.inset(Insets::tlbr(-2, -1, -4, -3));
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.inset(Insets::tlbr(1, 2, 3, 4));
    assert_eq!(Rect::new(12, 21, 24, 36), r);
    r.inset(Insets::tlbr(-1, -2, -3, -4));
    assert_eq!(Rect::new(10, 20, 30, 40), r);
}

#[test]
fn outset() {
    let mut r = Rect::new(10, 20, 30, 40);
    r.outset_uniform(0);
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.outset_uniform(1);
    assert_eq!(Rect::new(9, 19, 32, 42), r);
    r.outset_uniform(-1);
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.outset(Outsets::vh(2, 1));
    assert_eq!(Rect::new(9, 18, 32, 44), r);
    r.outset(Outsets::vh(-2, -1));
    assert_eq!(Rect::new(10, 20, 30, 40), r);
    r.outset(Outsets::tlbr(2, 1, 4, 3));
    assert_eq!(Rect::new(9, 18, 34, 46), r);
    r.outset(Outsets::tlbr(-2, -1, -4, -3));
    assert_eq!(Rect::new(10, 20, 30, 40), r);
}

#[test]
fn inset_outset_clamped() {
    let mut r = Rect::new(10, 20, 30, 40);
    r.inset_uniform(18);
    assert_eq!(Rect::new(28, 38, 0, 4), r);
    r.inset_uniform(-18);
    assert_eq!(Rect::new(10, 20, 36, 40), r);
    r.inset(Insets::vh(30, 15));
    assert_eq!(Rect::new(25, 50, 6, 0), r);
    r.inset(Insets::vh(-30, -15));
    assert_eq!(Rect::new(10, 20, 36, 60), r);
    r.inset(Insets::tlbr(30, 20, 50, 40));
    assert_eq!(Rect::new(30, 50, 0, 0), r);
    r.inset(Insets::tlbr(-30, -20, -50, -40));
    assert_eq!(Rect::new(10, 20, 60, 80), r);
    r.outset_uniform(MAX_INT);
    assert_eq!(Rect::new(10 - MAX_INT, 20 - MAX_INT, MAX_INT, MAX_INT), r);

    let mut o = Outsets::default();
    o.set_top_bottom(MAX_INT, MAX_INT);
    r.outset(o);
    assert_eq!(Rect::new(10 - MAX_INT, MIN_INT, MAX_INT, MAX_INT), r);

    let mut o = Outsets::default();
    o.set_right(MAX_INT).set_top(MAX_INT);
    r.outset(o);
    assert_eq!(Rect::new(10 - MAX_INT, MIN_INT, MAX_INT, MAX_INT), r);

    let mut o = Outsets::default();
    o.set_left_right(MAX_INT, MAX_INT);
    r.outset(o);
    assert_eq!(Rect::new(MIN_INT, MIN_INT, MAX_INT, MAX_INT), r);
}

#[test]
fn set_by_bounds() {
    let mut r = Rect::default();
    r.set_by_bounds(1, 2, 30, 40);
    assert_eq!(Rect::new(1, 2, 29, 38), r);
    r.set_by_bounds(30, 40, 1, 2);
    assert_eq!(Rect::new(30, 40, 0, 0), r);
    r.set_by_bounds(0, 0, MAX_INT, MAX_INT);
    assert_eq!(Rect::new(0, 0, MAX_INT, MAX_INT), r);
    r.set_by_bounds(-1, -1, MAX_INT, MAX_INT);
    assert_eq!(Rect::new(-1, -1, MAX_INT, MAX_INT), r);
    r.set_by_bounds(1, 1, MAX_INT, MAX_INT);
    assert_eq!(Rect::new(1, 1, MAX_INT - 1, MAX_INT - 1), r);
    r.set_by_bounds(MIN_INT, MIN_INT, 0, 0);
    assert_eq!(Rect::new(MIN_INT + 1, MIN_INT + 1, MAX_INT, MAX_INT), r);
    r.set_by_bounds(MIN_INT, MIN_INT, 1, 1);
    assert_eq!(Rect::new(MIN_INT + 2, MIN_INT + 2, MAX_INT, MAX_INT), r);
    r.set_by_bounds(MIN_INT, MIN_INT, -1, -1);
    assert_eq!(Rect::new(MIN_INT, MIN_INT, MAX_INT, MAX_INT), r);
    r.set_by_bounds(MIN_INT, MIN_INT, MAX_INT, MAX_INT);
    assert_eq!(Rect::new(MIN_INT / 2 - 1, MIN_INT / 2 - 1, MAX_INT, MAX_INT), r);
}

#[test]
fn maximum_covered_rect_test() {
    // X aligned and intersect: unite.
    assert_eq!(
        Rect::new(10, 20, 30, 60),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(10, 30, 30, 50))
    );
    // X aligned and adjacent: unite.
    assert_eq!(
        Rect::new(10, 20, 30, 90),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(10, 60, 30, 50))
    );
    // X aligned and separate: choose the bigger one.
    assert_eq!(
        Rect::new(10, 61, 30, 50),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(10, 61, 30, 50))
    );
    // Y aligned and intersect: unite.
    assert_eq!(
        Rect::new(10, 20, 60, 40),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(30, 20, 40, 40))
    );
    // Y aligned and adjacent: unite.
    assert_eq!(
        Rect::new(10, 20, 70, 40),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(40, 20, 40, 40))
    );
    // Y aligned and separate: choose the bigger one.
    assert_eq!(
        Rect::new(41, 20, 40, 40),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(41, 20, 40, 40))
    );
    // Get the biggest expanded intersection.
    assert_eq!(
        Rect::new(0, 0, 9, 19),
        maximum_covered_rect(Rect::new(0, 0, 10, 10), Rect::new(0, 9, 9, 10))
    );
    assert_eq!(
        Rect::new(0, 0, 19, 9),
        maximum_covered_rect(Rect::new(0, 0, 10, 10), Rect::new(9, 0, 10, 9))
    );
    // Otherwise choose the bigger one.
    assert_eq!(
        Rect::new(20, 30, 40, 50),
        maximum_covered_rect(Rect::new(10, 20, 30, 40), Rect::new(20, 30, 40, 50))
    );
    assert_eq!(
        Rect::new(10, 20, 40, 50),
        maximum_covered_rect(Rect::new(10, 20, 40, 50), Rect::new(20, 30, 30, 40))
    );
    assert_eq!(
        Rect::new(10, 20, 40, 50),
        maximum_covered_rect(Rect::new(10, 20, 40, 50), Rect::new(20, 30, 40, 50))
    );
}
