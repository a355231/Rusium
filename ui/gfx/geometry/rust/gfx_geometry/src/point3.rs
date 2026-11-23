// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `gfx::Point3F` from `point3_f.{h,cc}`.

use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::format::format_g;
use crate::point::PointF;
use crate::vector3::Vector3dF;

/// A point with float x, y and z coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point3F {
    x: f32,
    y: f32,
    z: f32,
}

impl Point3F {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Point3F { x, y, z }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    pub fn set_x(&mut self, x: f32) {
        self.x = x;
    }

    pub fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    pub fn set_z(&mut self, z: f32) {
        self.z = z;
    }

    pub fn set_point(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Scales all three coordinates by `scale`.
    pub fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    pub fn scale_xyz(&mut self, x_scale: f32, y_scale: f32, z_scale: f32) {
        self.set_point(self.x * x_scale, self.y * y_scale, self.z * z_scale);
    }

    pub fn is_origin(self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }

    /// Returns the squared Euclidean distance to `other`, computed in float.
    pub fn squared_distance_to(self, other: Point3F) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the `(x, y)` components.
    pub fn as_point_f(self) -> PointF {
        PointF::new(self.x, self.y)
    }

    pub const fn offset_from_origin(self) -> Vector3dF {
        Vector3dF::new(self.x, self.y, self.z)
    }
}

impl fmt::Display for Point3F {
    /// `"%g,%g,%g"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{}",
            format_g(self.x as f64),
            format_g(self.y as f64),
            format_g(self.z as f64)
        )
    }
}

impl From<PointF> for Point3F {
    /// The explicit `Point3F(const PointF&)` conversion, with z = 0.
    fn from(p: PointF) -> Point3F {
        Point3F::new(p.x(), p.y(), 0.0)
    }
}

impl AddAssign<Vector3dF> for Point3F {
    fn add_assign(&mut self, v: Vector3dF) {
        self.x += v.x();
        self.y += v.y();
        self.z += v.z();
    }
}

impl SubAssign<Vector3dF> for Point3F {
    fn sub_assign(&mut self, v: Vector3dF) {
        self.x -= v.x();
        self.y -= v.y();
        self.z -= v.z();
    }
}

impl Add<Vector3dF> for Point3F {
    type Output = Point3F;
    fn add(self, rhs: Vector3dF) -> Point3F {
        Point3F::new(self.x + rhs.x(), self.y + rhs.y(), self.z + rhs.z())
    }
}

impl Sub<Vector3dF> for Point3F {
    type Output = Point3F;
    fn sub(self, rhs: Vector3dF) -> Point3F {
        Point3F::new(self.x - rhs.x(), self.y - rhs.y(), self.z - rhs.z())
    }
}

impl Sub<Point3F> for Point3F {
    type Output = Vector3dF;
    fn sub(self, rhs: Point3F) -> Vector3dF {
        Vector3dF::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

pub const fn point3_at_offset_from_origin(offset: Vector3dF) -> Point3F {
    Point3F::new(offset.x(), offset.y(), offset.z())
}

/// Returns `p` with each coordinate multiplied by the matching scale.
pub fn scale_point3(p: Point3F, x_scale: f32, y_scale: f32, z_scale: f32) -> Point3F {
    Point3F::new(p.x * x_scale, p.y * y_scale, p.z * z_scale)
}

/// Returns `p` with each coordinate multiplied by the matching component of
/// `v`.
pub fn scale_point3_by(p: Point3F, v: Vector3dF) -> Point3F {
    Point3F::new(p.x * v.x(), p.y * v.y(), p.z * v.z())
}

pub fn scale_point3_uniform(p: Point3F, scale: f32) -> Point3F {
    scale_point3(p, scale, scale, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- point3_f_unittest.cc ---------------------------------------------

    #[test]
    fn point3_f_test_vector_arithmetic() {
        let a = Point3F::new(1.6, 5.1, 3.2);
        let v1 = Vector3dF::new(3.1, -3.2, 9.3);
        let v2 = Vector3dF::new(-8.1, 1.2, 3.3);

        let tests = [
            (Point3F::new(4.7, 1.9, 12.5), a + v1),
            (Point3F::new(-1.5, 8.3, -6.1), a - v1),
            (a, a - v1 + v1),
            (a, a + v1 - v1),
            (a, a + Vector3dF::default()),
            (Point3F::new(12.8, 0.7, 9.2), a + v1 - v2),
            (Point3F::new(-9.6, 9.5, -2.8), a - v1 + v2),
        ];
        for (expected, actual) in tests {
            assert_eq!(expected.to_string(), actual.to_string());
        }

        let mut a = a;
        a += v1;
        assert_eq!(Point3F::new(4.7, 1.9, 12.5).to_string(), a.to_string());
        a -= v2;
        assert_eq!(Point3F::new(12.8, 0.7, 9.2).to_string(), a.to_string());
    }

    #[test]
    fn point3_f_test_vector_from_points() {
        let a = Point3F::new(1.6, 5.2, 3.2);
        let v1 = Vector3dF::new(3.1, -3.2, 9.3);
        let b = a + v1;
        assert_eq!((b - a).to_string(), v1.to_string());
    }

    #[test]
    fn point3_f_test_scale() {
        assert_eq!(Point3F::default().to_string(), scale_point3_uniform(Point3F::default(), 2.0).to_string());
        assert_eq!(
            Point3F::default().to_string(),
            scale_point3(Point3F::default(), 2.0, 2.0, 2.0).to_string()
        );
        assert_eq!(
            Point3F::new(2.0, -2.0, 4.0).to_string(),
            scale_point3_uniform(Point3F::new(1.0, -1.0, 2.0), 2.0).to_string()
        );
        assert_eq!(
            Point3F::new(2.0, -3.0, 8.0).to_string(),
            scale_point3(Point3F::new(1.0, -1.0, 2.0), 2.0, 3.0, 4.0).to_string()
        );

        let mut zero = Point3F::default();
        zero.scale(2.0);
        zero.scale_xyz(6.0, 3.0, 1.5);
        assert_eq!(Point3F::default().to_string(), zero.to_string());

        let mut point = Point3F::new(1.0, -1.0, 2.0);
        point.scale(2.0);
        point.scale_xyz(6.0, 3.0, 1.5);
        assert_eq!(Point3F::new(12.0, -6.0, 6.0).to_string(), point.to_string());
    }

    #[test]
    fn point3_f_test_is_origin() {
        assert!(Point3F::default().is_origin());
        assert!(!Point3F::new(0.0, 0.0, 0.1).is_origin());
        assert!(!Point3F::new(0.0, 0.1, 0.0).is_origin());
        assert!(!Point3F::new(0.1, 0.0, 0.0).is_origin());
        assert!(!Point3F::new(0.0, 0.0, -0.1).is_origin());
        assert!(!Point3F::new(0.0, -0.1, 0.0).is_origin());
        assert!(!Point3F::new(-0.1, 0.0, 0.0).is_origin());
    }

    #[test]
    fn point3_f_test_offset_from_origin() {
        assert_eq!(
            Vector3dF::new(1.25, 2.5, -3.75),
            Point3F::new(1.25, 2.5, -3.75).offset_from_origin()
        );
    }

    #[test]
    fn point3_f_test_to_string() {
        assert_eq!("1.03125,2.5,-3", Point3F::new(1.03125, 2.5, -3.0).to_string());
    }

    // ---- Extra tests -------------------------------------------------------

    #[test]
    fn point3_f_test_squared_distance_and_projection() {
        let a = Point3F::new(1.0, 2.0, 2.0);
        assert_eq!(9.0, a.squared_distance_to(Point3F::default()));
        assert_eq!(PointF::new(1.0, 2.0), a.as_point_f());
        assert_eq!(Point3F::new(1.0, 2.0, 0.0), Point3F::from(PointF::new(1.0, 2.0)));
        assert_eq!(
            Point3F::new(-1.0, -2.0, -2.0),
            scale_point3_by(a, Vector3dF::new(-1.0, -1.0, -1.0))
        );
    }
}
