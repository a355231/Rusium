// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of `ui/gfx/geometry/rect_unittest.cc`.

use super::*;
use crate::insets::Insets;
use crate::point::Point;
use crate::rect_conversions::{to_enclosed_rect, to_enclosing_rect};
use crate::rect_f::RectF;
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
    // @@PORT_CONTINUE@@
}
