//! Style definitions for UI elements

use crate::core::color::Color;
use crate::core::geometry::Edges;
use bytemuck::{Pod, Zeroable};

/// Corner radii for rounded rectangles
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const ZERO: Self = Self {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn all(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }

    pub const fn top(radius: f32) -> Self {
        Self::new(radius, radius, 0.0, 0.0)
    }

    pub const fn bottom(radius: f32) -> Self {
        Self::new(0.0, 0.0, radius, radius)
    }

    pub const fn left(radius: f32) -> Self {
        Self::new(radius, 0.0, 0.0, radius)
    }

    pub const fn right(radius: f32) -> Self {
        Self::new(0.0, radius, radius, 0.0)
    }

    pub fn max(&self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }
}

impl From<f32> for Corners {
    fn from(radius: f32) -> Self {
        Self::all(radius)
    }
}

/// Border style for elements
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BorderStyle {
    pub width: Edges,
    pub color: Color,
    pub radius: Corners,
}

impl BorderStyle {
    pub const NONE: Self = Self {
        width: Edges::ZERO,
        color: Color::TRANSPARENT,
        radius: Corners::ZERO,
    };

    pub fn new(width: f32, color: Color) -> Self {
        Self {
            width: Edges::all(width),
            color,
            radius: Corners::ZERO,
        }
    }

    pub fn with_radius(mut self, radius: impl Into<Corners>) -> Self {
        self.radius = radius.into();
        self
    }
}

/// Background fill for elements
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Background {
    None,
    Solid(Color),
    LinearGradient {
        start: Color,
        end: Color,
        angle: f32, // degrees
    },
    RadialGradient {
        inner: Color,
        outer: Color,
    },
}

impl Background {
    pub const NONE: Self = Self::None;

    pub fn solid(color: impl Into<Color>) -> Self {
        Self::Solid(color.into())
    }

    pub fn linear_gradient(start: impl Into<Color>, end: impl Into<Color>, angle: f32) -> Self {
        Self::LinearGradient {
            start: start.into(),
            end: end.into(),
            angle,
        }
    }

    pub fn radial_gradient(inner: impl Into<Color>, outer: impl Into<Color>) -> Self {
        Self::RadialGradient {
            inner: inner.into(),
            outer: outer.into(),
        }
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::None
    }
}

impl<C: Into<Color>> From<C> for Background {
    fn from(color: C) -> Self {
        Background::Solid(color.into())
    }
}

/// Shadow style for elements
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
}

impl Shadow {
    pub fn new(offset_x: f32, offset_y: f32, blur_radius: f32, color: impl Into<Color>) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius: 0.0,
            color: color.into(),
        }
    }

    pub fn with_spread(mut self, spread: f32) -> Self {
        self.spread_radius = spread;
        self
    }
}

/// Display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Flex,
    Block,
    None,
}

/// Flex direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Justify content (main axis alignment)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Align items (cross axis alignment)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    #[default]
    Stretch,
    Baseline,
}

/// Position type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// Complete style for an element
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    // Layout
    pub display: Display,
    pub position: Position,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub gap: f32,

    // Sizing
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,

    // Spacing
    pub margin: Edges,
    pub padding: Edges,

    // Appearance
    pub background: Background,
    pub border: BorderStyle,
    pub shadow: Option<Shadow>,
    pub opacity: f32,

    // Overflow
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
}

/// Overflow behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
}

impl Style {
    pub fn new() -> Self {
        Self {
            opacity: 1.0,
            flex_shrink: 1.0,
            ..Default::default()
        }
    }
}
