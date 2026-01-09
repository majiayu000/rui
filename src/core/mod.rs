//! Core types and abstractions

pub mod color;
pub mod geometry;
pub mod style;
pub mod app;
pub mod window;
pub mod entity;
pub mod view;
pub mod event;
pub mod animation;

pub use app::{App, AppContext};
pub use color::{Color, Hsla, Rgba};
pub use entity::EntityId;
pub use geometry::{Bounds, Edges, Point, Rect, Size};
pub use style::{Background, BorderStyle, Corners, Style};
pub use view::{View, ViewContext};
pub use window::{Window, WindowOptions};

use std::sync::atomic::{AtomicU64, Ordering};

/// Generate unique element IDs
static ELEMENT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for elements in the UI tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);

impl ElementId {
    pub fn new() -> Self {
        Self(ELEMENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ElementId {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixels unit type for type-safe pixel measurements
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Pixels(pub f32);

impl Pixels {
    pub const ZERO: Self = Self(0.0);

    pub fn new(value: f32) -> Self {
        Self(value)
    }
}

impl From<f32> for Pixels {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Pixels> for f32 {
    fn from(value: Pixels) -> Self {
        value.0
    }
}

impl std::ops::Add for Pixels {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Pixels {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<f32> for Pixels {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl std::ops::Div<f32> for Pixels {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}
