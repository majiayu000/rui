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

    #[inline]
    pub fn y(&self) -> f32 {
        self.origin.y
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.size.width
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.size.height
    }

    #[inline]
    pub fn min_x(&self) -> f32 {
        self.origin.x
    }

    #[inline]
    pub fn min_y(&self) -> f32 {
        self.origin.y
    }

    #[inline]
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    #[inline]
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2.0,
            self.origin.y + self.size.height / 2.0,
        )
    }

    #[inline]
    pub fn contains(&self, point: Point) -> bool {
        let x1 = self.origin.x;
        let y1 = self.origin.y;
        let x2 = x1 + self.size.width;
        let y2 = y1 + self.size.height;

        point.x >= x1 && point.x <= x2 && point.y >= y1 && point.y <= y2
    }

    #[inline]
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

    #[inline]
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
#[path = "geometry_tests.rs"]
mod tests;
