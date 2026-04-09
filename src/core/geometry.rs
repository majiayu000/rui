//! Geometry primitives for layout and rendering

use bytemuck::{Pod, Zeroable};
use std::ops::{Add, Mul, Sub};

/// A 2D point
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl From<(f32, f32)> for Point {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

/// A 2D size
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

impl Mul<f32> for Size {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

impl From<(f32, f32)> for Size {
    fn from((width, height): (f32, f32)) -> Self {
        Self::new(width, height)
    }
}

/// A rectangle defined by origin and size
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(Point::new(x, y), Size::new(width, height))
    }

    pub fn x(&self) -> f32 {
        self.origin.x
    }

    pub fn y(&self) -> f32 {
        self.origin.y
    }

    pub fn width(&self) -> f32 {
        self.size.width
    }

    pub fn height(&self) -> f32 {
        self.size.height
    }

    pub fn min_x(&self) -> f32 {
        self.origin.x
    }

    pub fn min_y(&self) -> f32 {
        self.origin.y
    }

    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2.0,
            self.origin.y + self.size.height / 2.0,
        )
    }

    pub fn contains(&self, point: Point) -> bool {
        let x1 = self.origin.x;
        let y1 = self.origin.y;
        let x2 = x1 + self.size.width;
        let y2 = y1 + self.size.height;

        point.x >= x1 && point.x <= x2 && point.y >= y1 && point.y <= y2
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        let ax1 = self.origin.x;
        let ay1 = self.origin.y;
        let ax2 = ax1 + self.size.width;
        let ay2 = ay1 + self.size.height;

        let bx1 = other.origin.x;
        let by1 = other.origin.y;
        let bx2 = bx1 + other.size.width;
        let by2 = by1 + other.size.height;

        ax1 < bx2 && ax2 > bx1 && ay1 < by2 && ay2 > by1
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let ax1 = self.origin.x;
        let ay1 = self.origin.y;
        let ax2 = ax1 + self.size.width;
        let ay2 = ay1 + self.size.height;

        let bx1 = other.origin.x;
        let by1 = other.origin.y;
        let bx2 = bx1 + other.size.width;
        let by2 = by1 + other.size.height;

        let x1 = ax1.max(bx1);
        let y1 = ay1.max(by1);
        let x2 = ax2.min(bx2);
        let y2 = ay2.min(by2);

        if x1 < x2 && y1 < y2 {
            Some(Rect::from_xywh(x1, y1, x2 - x1, y2 - y1))
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }
}

/// Bounds alias for Rect (commonly used in layout)
pub type Bounds = Rect;

/// Edge values (top, right, bottom, left)
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    pub const fn horizontal(value: f32) -> Self {
        Self::new(0.0, value, 0.0, value)
    }

    pub const fn vertical(value: f32) -> Self {
        Self::new(value, 0.0, value, 0.0)
    }

    pub fn horizontal_sum(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical_sum(&self) -> f32 {
        self.top + self.bottom
    }
}

impl From<f32> for Edges {
    fn from(value: f32) -> Self {
        Self::all(value)
    }
}

impl From<(f32, f32)> for Edges {
    fn from((vertical, horizontal): (f32, f32)) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}

impl From<(f32, f32, f32, f32)> for Edges {
    fn from((top, right, bottom, left): (f32, f32, f32, f32)) -> Self {
        Self::new(top, right, bottom, left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Point Tests ====================

    mod point_tests {
        use super::*;

        #[test]
        fn test_point_new() {
            let cases = [
                ((0.0, 0.0), Point { x: 0.0, y: 0.0 }),
                ((1.0, 2.0), Point { x: 1.0, y: 2.0 }),
                ((-5.0, 10.0), Point { x: -5.0, y: 10.0 }),
                (
                    (f32::MAX, f32::MIN),
                    Point {
                        x: f32::MAX,
                        y: f32::MIN,
                    },
                ),
            ];

            for ((x, y), expected) in cases {
                assert_eq!(Point::new(x, y), expected);
            }
        }

        #[test]
        fn test_point_zero_constant() {
            assert_eq!(Point::ZERO, Point::new(0.0, 0.0));
            assert_eq!(Point::ZERO.x, 0.0);
            assert_eq!(Point::ZERO.y, 0.0);
        }

        #[test]
        fn test_point_default() {
            let default_point: Point = Default::default();
            assert_eq!(default_point, Point::ZERO);
        }

        #[test]
        fn test_point_distance() {
            let cases = [
                // (p1, p2, expected_distance)
                (Point::new(0.0, 0.0), Point::new(3.0, 4.0), 5.0),
                (Point::new(0.0, 0.0), Point::new(0.0, 0.0), 0.0),
                (Point::new(1.0, 1.0), Point::new(1.0, 1.0), 0.0),
                (Point::new(-3.0, 0.0), Point::new(0.0, 4.0), 5.0),
                (Point::new(0.0, 0.0), Point::new(1.0, 0.0), 1.0),
                (Point::new(0.0, 0.0), Point::new(0.0, 1.0), 1.0),
            ];

            for (p1, p2, expected) in cases {
                let distance = p1.distance(p2);
                assert!(
                    (distance - expected).abs() < f32::EPSILON,
                    "distance({:?}, {:?}) = {}, expected {}",
                    p1,
                    p2,
                    distance,
                    expected
                );
            }
        }

        #[test]
        fn test_point_distance_symmetric() {
            let p1 = Point::new(3.0, 7.0);
            let p2 = Point::new(-2.0, 11.0);
            assert_eq!(p1.distance(p2), p2.distance(p1));
        }

        #[test]
        fn test_point_add() {
            let cases = [
                (
                    Point::new(1.0, 2.0),
                    Point::new(3.0, 4.0),
                    Point::new(4.0, 6.0),
                ),
                (Point::ZERO, Point::new(5.0, 5.0), Point::new(5.0, 5.0)),
                (Point::new(-1.0, -2.0), Point::new(1.0, 2.0), Point::ZERO),
                (
                    Point::new(-5.0, -10.0),
                    Point::new(-3.0, -4.0),
                    Point::new(-8.0, -14.0),
                ),
            ];

            for (p1, p2, expected) in cases {
                assert_eq!(p1 + p2, expected);
            }
        }

        #[test]
        fn test_point_sub() {
            let cases = [
                (
                    Point::new(3.0, 4.0),
                    Point::new(1.0, 2.0),
                    Point::new(2.0, 2.0),
                ),
                (Point::new(5.0, 5.0), Point::ZERO, Point::new(5.0, 5.0)),
                (Point::new(1.0, 2.0), Point::new(1.0, 2.0), Point::ZERO),
                (
                    Point::new(-1.0, -2.0),
                    Point::new(-3.0, -4.0),
                    Point::new(2.0, 2.0),
                ),
            ];

            for (p1, p2, expected) in cases {
                assert_eq!(p1 - p2, expected);
            }
        }

        #[test]
        fn test_point_from_tuple() {
            let cases = [
                ((0.0, 0.0), Point::ZERO),
                ((1.0, 2.0), Point::new(1.0, 2.0)),
                ((-5.5, 10.5), Point::new(-5.5, 10.5)),
            ];

            for (tuple, expected) in cases {
                let point: Point = tuple.into();
                assert_eq!(point, expected);
            }
        }

        #[test]
        fn test_point_clone_and_copy() {
            let p1 = Point::new(1.0, 2.0);
            let p2 = p1;
            let p3 = p1.clone();
            assert_eq!(p1, p2);
            assert_eq!(p1, p3);
        }

        #[test]
        fn test_point_debug() {
            let p = Point::new(1.0, 2.0);
            let debug_str = format!("{:?}", p);
            assert!(debug_str.contains("Point"));
            assert!(debug_str.contains("1.0") || debug_str.contains("1"));
            assert!(debug_str.contains("2.0") || debug_str.contains("2"));
        }
    }

    // ==================== Size Tests ====================

    mod size_tests {
        use super::*;

        #[test]
        fn test_size_new() {
            let cases = [
                (
                    (0.0, 0.0),
                    Size {
                        width: 0.0,
                        height: 0.0,
                    },
                ),
                (
                    (100.0, 200.0),
                    Size {
                        width: 100.0,
                        height: 200.0,
                    },
                ),
                (
                    (1.5, 2.5),
                    Size {
                        width: 1.5,
                        height: 2.5,
                    },
                ),
            ];

            for ((w, h), expected) in cases {
                assert_eq!(Size::new(w, h), expected);
            }
        }

        #[test]
        fn test_size_zero_constant() {
            assert_eq!(Size::ZERO, Size::new(0.0, 0.0));
            assert_eq!(Size::ZERO.width, 0.0);
            assert_eq!(Size::ZERO.height, 0.0);
        }

        #[test]
        fn test_size_default() {
            let default_size: Size = Default::default();
            assert_eq!(default_size, Size::ZERO);
        }

        #[test]
        fn test_size_area() {
            let cases = [
                (Size::new(10.0, 20.0), 200.0),
                (Size::ZERO, 0.0),
                (Size::new(5.0, 5.0), 25.0),
                (Size::new(1.0, 100.0), 100.0),
                (Size::new(0.5, 0.5), 0.25),
            ];

            for (size, expected) in cases {
                assert!(
                    (size.area() - expected).abs() < f32::EPSILON,
                    "area({:?}) = {}, expected {}",
                    size,
                    size.area(),
                    expected
                );
            }
        }

        #[test]
        fn test_size_is_empty() {
            let cases = [
                (Size::ZERO, true),
                (Size::new(0.0, 10.0), true),
                (Size::new(10.0, 0.0), true),
                (Size::new(-1.0, 10.0), true),
                (Size::new(10.0, -1.0), true),
                (Size::new(-1.0, -1.0), true),
                (Size::new(1.0, 1.0), false),
                (Size::new(0.001, 0.001), false),
            ];

            for (size, expected) in cases {
                assert_eq!(
                    size.is_empty(),
                    expected,
                    "is_empty({:?}) = {}, expected {}",
                    size,
                    size.is_empty(),
                    expected
                );
            }
        }

        #[test]
        fn test_size_mul_scalar() {
            let cases = [
                (Size::new(10.0, 20.0), 2.0, Size::new(20.0, 40.0)),
                (Size::new(5.0, 5.0), 0.0, Size::ZERO),
                (Size::new(10.0, 10.0), 0.5, Size::new(5.0, 5.0)),
                (Size::new(3.0, 4.0), -1.0, Size::new(-3.0, -4.0)),
                (Size::ZERO, 100.0, Size::ZERO),
            ];

            for (size, scalar, expected) in cases {
                assert_eq!(size * scalar, expected);
            }
        }

        #[test]
        fn test_size_from_tuple() {
            let cases = [
                ((0.0, 0.0), Size::ZERO),
                ((100.0, 200.0), Size::new(100.0, 200.0)),
                ((1.5, 2.5), Size::new(1.5, 2.5)),
            ];

            for (tuple, expected) in cases {
                let size: Size = tuple.into();
                assert_eq!(size, expected);
            }
        }

        #[test]
        fn test_size_clone_and_copy() {
            let s1 = Size::new(10.0, 20.0);
            let s2 = s1;
            let s3 = s1.clone();
            assert_eq!(s1, s2);
            assert_eq!(s1, s3);
        }
    }

    // ==================== Rect Tests ====================

    mod rect_tests {
        use super::*;

        #[test]
        fn test_rect_new() {
            let origin = Point::new(10.0, 20.0);
            let size = Size::new(100.0, 200.0);
            let rect = Rect::new(origin, size);
            assert_eq!(rect.origin, origin);
            assert_eq!(rect.size, size);
        }

        #[test]
        fn test_rect_from_xywh() {
            let rect = Rect::from_xywh(10.0, 20.0, 100.0, 200.0);
            assert_eq!(rect.origin, Point::new(10.0, 20.0));
            assert_eq!(rect.size, Size::new(100.0, 200.0));
        }

        #[test]
        fn test_rect_zero_constant() {
            assert_eq!(Rect::ZERO.origin, Point::ZERO);
            assert_eq!(Rect::ZERO.size, Size::ZERO);
        }

        #[test]
        fn test_rect_default() {
            let default_rect: Rect = Default::default();
            assert_eq!(default_rect, Rect::ZERO);
        }

        #[test]
        fn test_rect_accessors() {
            let rect = Rect::from_xywh(10.0, 20.0, 100.0, 200.0);

            assert_eq!(rect.x(), 10.0);
            assert_eq!(rect.y(), 20.0);
            assert_eq!(rect.width(), 100.0);
            assert_eq!(rect.height(), 200.0);
        }

        #[test]
        fn test_rect_min_max() {
            let cases = [
                // (rect, min_x, min_y, max_x, max_y)
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    0.0,
                    0.0,
                    100.0,
                    100.0,
                ),
                (
                    Rect::from_xywh(10.0, 20.0, 30.0, 40.0),
                    10.0,
                    20.0,
                    40.0,
                    60.0,
                ),
                (
                    Rect::from_xywh(-50.0, -50.0, 100.0, 100.0),
                    -50.0,
                    -50.0,
                    50.0,
                    50.0,
                ),
                (Rect::ZERO, 0.0, 0.0, 0.0, 0.0),
            ];

            for (rect, min_x, min_y, max_x, max_y) in cases {
                assert_eq!(rect.min_x(), min_x, "min_x for {:?}", rect);
                assert_eq!(rect.min_y(), min_y, "min_y for {:?}", rect);
                assert_eq!(rect.max_x(), max_x, "max_x for {:?}", rect);
                assert_eq!(rect.max_y(), max_y, "max_y for {:?}", rect);
            }
        }

        #[test]
        fn test_rect_center() {
            let cases = [
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Point::new(50.0, 50.0),
                ),
                (
                    Rect::from_xywh(10.0, 20.0, 30.0, 40.0),
                    Point::new(25.0, 40.0),
                ),
                (
                    Rect::from_xywh(-50.0, -50.0, 100.0, 100.0),
                    Point::new(0.0, 0.0),
                ),
                (Rect::ZERO, Point::ZERO),
            ];

            for (rect, expected) in cases {
                assert_eq!(rect.center(), expected, "center for {:?}", rect);
            }
        }

        #[test]
        fn test_rect_contains() {
            let rect = Rect::from_xywh(10.0, 10.0, 100.0, 100.0);

            let inside_cases = [
                Point::new(50.0, 50.0),   // center
                Point::new(10.0, 10.0),   // top-left corner (on boundary)
                Point::new(110.0, 10.0),  // top-right corner
                Point::new(10.0, 110.0),  // bottom-left corner
                Point::new(110.0, 110.0), // bottom-right corner
                Point::new(60.0, 10.0),   // on top edge
                Point::new(60.0, 110.0),  // on bottom edge
                Point::new(10.0, 60.0),   // on left edge
                Point::new(110.0, 60.0),  // on right edge
            ];

            for point in inside_cases {
                assert!(rect.contains(point), "should contain {:?}", point);
            }

            let outside_cases = [
                Point::new(5.0, 50.0),    // left of rect
                Point::new(115.0, 50.0),  // right of rect
                Point::new(50.0, 5.0),    // above rect
                Point::new(50.0, 115.0),  // below rect
                Point::new(0.0, 0.0),     // origin (outside)
                Point::new(-10.0, -10.0), // negative coords
            ];

            for point in outside_cases {
                assert!(!rect.contains(point), "should not contain {:?}", point);
            }
        }

        #[test]
        fn test_rect_contains_zero_size() {
            let rect = Rect::ZERO;
            assert!(rect.contains(Point::ZERO));
            assert!(!rect.contains(Point::new(1.0, 0.0)));
        }

        #[test]
        fn test_rect_intersects() {
            let r1 = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);

            let intersecting_cases = [
                Rect::from_xywh(50.0, 50.0, 100.0, 100.0),   // overlapping
                Rect::from_xywh(-50.0, -50.0, 100.0, 100.0), // overlapping from negative
                Rect::from_xywh(25.0, 25.0, 50.0, 50.0),     // fully inside
                Rect::from_xywh(-50.0, 25.0, 200.0, 50.0),   // horizontal overlap
                Rect::from_xywh(25.0, -50.0, 50.0, 200.0),   // vertical overlap
            ];

            for r2 in intersecting_cases {
                assert!(r1.intersects(&r2), "{:?} should intersect {:?}", r1, r2);
                assert!(
                    r2.intersects(&r1),
                    "{:?} should intersect {:?} (symmetric)",
                    r2,
                    r1
                );
            }

            let non_intersecting_cases = [
                Rect::from_xywh(100.0, 0.0, 100.0, 100.0), // touching right edge (not overlapping)
                Rect::from_xywh(0.0, 100.0, 100.0, 100.0), // touching bottom edge
                Rect::from_xywh(-100.0, 0.0, 100.0, 100.0), // touching left edge
                Rect::from_xywh(0.0, -100.0, 100.0, 100.0), // touching top edge
                Rect::from_xywh(200.0, 200.0, 100.0, 100.0), // far away
            ];

            for r2 in non_intersecting_cases {
                assert!(
                    !r1.intersects(&r2),
                    "{:?} should not intersect {:?}",
                    r1,
                    r2
                );
            }
        }

        #[test]
        fn test_rect_intersects_symmetric() {
            let r1 = Rect::from_xywh(10.0, 10.0, 50.0, 50.0);
            let r2 = Rect::from_xywh(30.0, 30.0, 50.0, 50.0);
            assert_eq!(r1.intersects(&r2), r2.intersects(&r1));
        }

        #[test]
        fn test_rect_intersection() {
            let cases = [
                // (r1, r2, expected intersection)
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Rect::from_xywh(50.0, 50.0, 100.0, 100.0),
                    Some(Rect::from_xywh(50.0, 50.0, 50.0, 50.0)),
                ),
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Rect::from_xywh(25.0, 25.0, 50.0, 50.0),
                    Some(Rect::from_xywh(25.0, 25.0, 50.0, 50.0)), // r2 fully inside r1
                ),
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Rect::from_xywh(100.0, 0.0, 100.0, 100.0),
                    None, // touching edges, no overlap
                ),
                (
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Rect::from_xywh(200.0, 200.0, 100.0, 100.0),
                    None, // no intersection
                ),
                (
                    Rect::from_xywh(-50.0, -50.0, 100.0, 100.0),
                    Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
                    Some(Rect::from_xywh(0.0, 0.0, 50.0, 50.0)),
                ),
            ];

            for (r1, r2, expected) in cases {
                let result = r1.intersection(&r2);
                assert_eq!(result, expected, "intersection({:?}, {:?})", r1, r2);

                // Test symmetry
                let result_symmetric = r2.intersection(&r1);
                assert_eq!(
                    result_symmetric, expected,
                    "intersection({:?}, {:?}) symmetric",
                    r2, r1
                );
            }
        }

        #[test]
        fn test_rect_is_empty() {
            let cases = [
                (Rect::ZERO, true),
                (Rect::from_xywh(0.0, 0.0, 0.0, 100.0), true),
                (Rect::from_xywh(0.0, 0.0, 100.0, 0.0), true),
                (Rect::from_xywh(0.0, 0.0, -10.0, 100.0), true),
                (Rect::from_xywh(0.0, 0.0, 100.0, -10.0), true),
                (Rect::from_xywh(0.0, 0.0, 1.0, 1.0), false),
                (Rect::from_xywh(10.0, 10.0, 100.0, 100.0), false),
            ];

            for (rect, expected) in cases {
                assert_eq!(rect.is_empty(), expected, "is_empty({:?})", rect);
            }
        }

        #[test]
        fn test_rect_clone_and_copy() {
            let r1 = Rect::from_xywh(10.0, 20.0, 30.0, 40.0);
            let r2 = r1;
            let r3 = r1.clone();
            assert_eq!(r1, r2);
            assert_eq!(r1, r3);
        }

        #[test]
        fn test_bounds_alias() {
            let bounds: Bounds = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
            assert_eq!(bounds.width(), 100.0);
            assert_eq!(bounds.height(), 100.0);
        }
    }

    // ==================== Edges Tests ====================

    mod edges_tests {
        use super::*;

        #[test]
        fn test_edges_new() {
            let edges = Edges::new(1.0, 2.0, 3.0, 4.0);
            assert_eq!(edges.top, 1.0);
            assert_eq!(edges.right, 2.0);
            assert_eq!(edges.bottom, 3.0);
            assert_eq!(edges.left, 4.0);
        }

        #[test]
        fn test_edges_zero_constant() {
            assert_eq!(Edges::ZERO.top, 0.0);
            assert_eq!(Edges::ZERO.right, 0.0);
            assert_eq!(Edges::ZERO.bottom, 0.0);
            assert_eq!(Edges::ZERO.left, 0.0);
        }

        #[test]
        fn test_edges_default() {
            let default_edges: Edges = Default::default();
            assert_eq!(default_edges, Edges::ZERO);
        }

        #[test]
        fn test_edges_all() {
            let cases = [
                (0.0, Edges::new(0.0, 0.0, 0.0, 0.0)),
                (10.0, Edges::new(10.0, 10.0, 10.0, 10.0)),
                (-5.0, Edges::new(-5.0, -5.0, -5.0, -5.0)),
            ];

            for (value, expected) in cases {
                assert_eq!(Edges::all(value), expected);
            }
        }

        #[test]
        fn test_edges_horizontal() {
            let cases = [
                (0.0, Edges::new(0.0, 0.0, 0.0, 0.0)),
                (10.0, Edges::new(0.0, 10.0, 0.0, 10.0)),
                (5.5, Edges::new(0.0, 5.5, 0.0, 5.5)),
            ];

            for (value, expected) in cases {
                assert_eq!(Edges::horizontal(value), expected);
            }
        }

        #[test]
        fn test_edges_vertical() {
            let cases = [
                (0.0, Edges::new(0.0, 0.0, 0.0, 0.0)),
                (10.0, Edges::new(10.0, 0.0, 10.0, 0.0)),
                (5.5, Edges::new(5.5, 0.0, 5.5, 0.0)),
            ];

            for (value, expected) in cases {
                assert_eq!(Edges::vertical(value), expected);
            }
        }

        #[test]
        fn test_edges_horizontal_sum() {
            let cases = [
                (Edges::ZERO, 0.0),
                (Edges::new(0.0, 10.0, 0.0, 20.0), 30.0),
                (Edges::all(5.0), 10.0),
                (Edges::horizontal(15.0), 30.0),
                (Edges::vertical(15.0), 0.0),
            ];

            for (edges, expected) in cases {
                assert!(
                    (edges.horizontal_sum() - expected).abs() < f32::EPSILON,
                    "horizontal_sum({:?}) = {}, expected {}",
                    edges,
                    edges.horizontal_sum(),
                    expected
                );
            }
        }

        #[test]
        fn test_edges_vertical_sum() {
            let cases = [
                (Edges::ZERO, 0.0),
                (Edges::new(10.0, 0.0, 20.0, 0.0), 30.0),
                (Edges::all(5.0), 10.0),
                (Edges::horizontal(15.0), 0.0),
                (Edges::vertical(15.0), 30.0),
            ];

            for (edges, expected) in cases {
                assert!(
                    (edges.vertical_sum() - expected).abs() < f32::EPSILON,
                    "vertical_sum({:?}) = {}, expected {}",
                    edges,
                    edges.vertical_sum(),
                    expected
                );
            }
        }

        #[test]
        fn test_edges_from_f32() {
            let cases = [
                (0.0, Edges::all(0.0)),
                (10.0, Edges::all(10.0)),
                (-5.0, Edges::all(-5.0)),
            ];

            for (value, expected) in cases {
                let edges: Edges = value.into();
                assert_eq!(edges, expected);
            }
        }

        #[test]
        fn test_edges_from_tuple_2() {
            let cases = [
                ((0.0, 0.0), Edges::new(0.0, 0.0, 0.0, 0.0)),
                ((10.0, 20.0), Edges::new(10.0, 20.0, 10.0, 20.0)), // (vertical, horizontal)
                ((5.0, 15.0), Edges::new(5.0, 15.0, 5.0, 15.0)),
            ];

            for (tuple, expected) in cases {
                let edges: Edges = tuple.into();
                assert_eq!(edges, expected);
            }
        }

        #[test]
        fn test_edges_from_tuple_4() {
            let cases = [
                ((0.0, 0.0, 0.0, 0.0), Edges::ZERO),
                ((1.0, 2.0, 3.0, 4.0), Edges::new(1.0, 2.0, 3.0, 4.0)),
                ((-1.0, -2.0, -3.0, -4.0), Edges::new(-1.0, -2.0, -3.0, -4.0)),
            ];

            for (tuple, expected) in cases {
                let edges: Edges = tuple.into();
                assert_eq!(edges, expected);
            }
        }

        #[test]
        fn test_edges_clone_and_copy() {
            let e1 = Edges::new(1.0, 2.0, 3.0, 4.0);
            let e2 = e1;
            let e3 = e1.clone();
            assert_eq!(e1, e2);
            assert_eq!(e1, e3);
        }
    }

    // ==================== Edge Cases and Special Values ====================

    mod edge_cases {
        use super::*;

        #[test]
        fn test_point_with_infinity() {
            let p1 = Point::new(f32::INFINITY, f32::NEG_INFINITY);
            let p2 = Point::new(0.0, 0.0);

            // Distance to infinity should be infinity
            assert!(p1.distance(p2).is_infinite());
        }

        #[test]
        fn test_point_with_nan() {
            let p1 = Point::new(f32::NAN, 0.0);
            let p2 = Point::new(0.0, 0.0);

            // Distance with NaN should be NaN
            assert!(p1.distance(p2).is_nan());
        }

        #[test]
        fn test_size_with_negative_values() {
            let size = Size::new(-10.0, -20.0);
            assert!(size.is_empty());
            assert_eq!(size.area(), 200.0); // area is still positive (negative * negative)
        }

        #[test]
        fn test_rect_with_negative_origin() {
            let rect = Rect::from_xywh(-100.0, -100.0, 200.0, 200.0);
            assert!(rect.contains(Point::ZERO));
            assert_eq!(rect.center(), Point::ZERO);
        }

        #[test]
        fn test_very_small_values() {
            let tiny = 1e-10_f32;
            let size = Size::new(tiny, tiny);
            assert!(!size.is_empty());
            assert!(size.area() > 0.0);
        }

        #[test]
        fn test_very_large_values() {
            let large = 1e30_f32;
            let size = Size::new(large, large);
            assert!(!size.is_empty());
            // Area might overflow to infinity
            assert!(size.area().is_infinite() || size.area() > 0.0);
        }

        #[test]
        fn test_rect_self_intersection() {
            let rect = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
            assert!(rect.intersects(&rect));
            assert_eq!(rect.intersection(&rect), Some(rect));
        }

        #[test]
        fn test_rect_contains_own_center() {
            let rect = Rect::from_xywh(10.0, 20.0, 100.0, 200.0);
            assert!(rect.contains(rect.center()));
        }
    }
}
