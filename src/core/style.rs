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

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // Corners Tests
    // ========================================

    mod corners_tests {
        use super::*;

        #[test]
        fn test_corners_zero_constant() {
            let corners = Corners::ZERO;
            assert_eq!(corners.top_left, 0.0);
            assert_eq!(corners.top_right, 0.0);
            assert_eq!(corners.bottom_right, 0.0);
            assert_eq!(corners.bottom_left, 0.0);
        }

        #[test]
        fn test_corners_new() {
            let corners = Corners::new(1.0, 2.0, 3.0, 4.0);
            assert_eq!(corners.top_left, 1.0);
            assert_eq!(corners.top_right, 2.0);
            assert_eq!(corners.bottom_right, 3.0);
            assert_eq!(corners.bottom_left, 4.0);
        }

        #[test]
        fn test_corners_all() {
            let corners = Corners::all(5.0);
            assert_eq!(corners.top_left, 5.0);
            assert_eq!(corners.top_right, 5.0);
            assert_eq!(corners.bottom_right, 5.0);
            assert_eq!(corners.bottom_left, 5.0);
        }

        #[test]
        fn test_corners_top() {
            let corners = Corners::top(10.0);
            assert_eq!(corners.top_left, 10.0);
            assert_eq!(corners.top_right, 10.0);
            assert_eq!(corners.bottom_right, 0.0);
            assert_eq!(corners.bottom_left, 0.0);
        }

        #[test]
        fn test_corners_bottom() {
            let corners = Corners::bottom(10.0);
            assert_eq!(corners.top_left, 0.0);
            assert_eq!(corners.top_right, 0.0);
            assert_eq!(corners.bottom_right, 10.0);
            assert_eq!(corners.bottom_left, 10.0);
        }

        #[test]
        fn test_corners_left() {
            let corners = Corners::left(10.0);
            assert_eq!(corners.top_left, 10.0);
            assert_eq!(corners.top_right, 0.0);
            assert_eq!(corners.bottom_right, 0.0);
            assert_eq!(corners.bottom_left, 10.0);
        }

        #[test]
        fn test_corners_right() {
            let corners = Corners::right(10.0);
            assert_eq!(corners.top_left, 0.0);
            assert_eq!(corners.top_right, 10.0);
            assert_eq!(corners.bottom_right, 10.0);
            assert_eq!(corners.bottom_left, 0.0);
        }

        #[test]
        fn test_corners_max() {
            let test_cases = [
                (Corners::new(1.0, 2.0, 3.0, 4.0), 4.0),
                (Corners::new(10.0, 2.0, 3.0, 4.0), 10.0),
                (Corners::new(1.0, 20.0, 3.0, 4.0), 20.0),
                (Corners::new(1.0, 2.0, 30.0, 4.0), 30.0),
                (Corners::ZERO, 0.0),
                (Corners::all(5.0), 5.0),
            ];

            for (corners, expected_max) in test_cases {
                assert_eq!(corners.max(), expected_max);
            }
        }

        #[test]
        fn test_corners_is_zero() {
            let test_cases = [
                (Corners::ZERO, true),
                (Corners::new(0.0, 0.0, 0.0, 0.0), true),
                (Corners::new(1.0, 0.0, 0.0, 0.0), false),
                (Corners::new(0.0, 1.0, 0.0, 0.0), false),
                (Corners::new(0.0, 0.0, 1.0, 0.0), false),
                (Corners::new(0.0, 0.0, 0.0, 1.0), false),
                (Corners::all(5.0), false),
            ];

            for (corners, expected) in test_cases {
                assert_eq!(corners.is_zero(), expected);
            }
        }

        #[test]
        fn test_corners_from_f32() {
            let corners: Corners = 8.0.into();
            assert_eq!(corners, Corners::all(8.0));
        }

        #[test]
        fn test_corners_default() {
            let corners = Corners::default();
            assert_eq!(corners, Corners::ZERO);
        }

        #[test]
        fn test_corners_clone_and_copy() {
            let corners = Corners::new(1.0, 2.0, 3.0, 4.0);
            let cloned = corners.clone();
            let copied = corners;
            assert_eq!(corners, cloned);
            assert_eq!(corners, copied);
        }

        #[test]
        fn test_corners_partial_eq() {
            let c1 = Corners::new(1.0, 2.0, 3.0, 4.0);
            let c2 = Corners::new(1.0, 2.0, 3.0, 4.0);
            let c3 = Corners::new(1.0, 2.0, 3.0, 5.0);
            assert_eq!(c1, c2);
            assert_ne!(c1, c3);
        }
    }

    // ========================================
    // BorderStyle Tests
    // ========================================

    mod border_style_tests {
        use super::*;

        #[test]
        fn test_border_style_none_constant() {
            let border = BorderStyle::NONE;
            assert_eq!(border.width, Edges::ZERO);
            assert_eq!(border.color, Color::TRANSPARENT);
            assert_eq!(border.radius, Corners::ZERO);
        }

        #[test]
        fn test_border_style_new() {
            let border = BorderStyle::new(2.0, Color::RED);
            assert_eq!(border.width, Edges::all(2.0));
            assert_eq!(border.color, Color::RED);
            assert_eq!(border.radius, Corners::ZERO);
        }

        #[test]
        fn test_border_style_with_radius_f32() {
            let border = BorderStyle::new(1.0, Color::BLACK).with_radius(5.0);
            assert_eq!(border.radius, Corners::all(5.0));
        }

        #[test]
        fn test_border_style_with_radius_corners() {
            let corners = Corners::new(1.0, 2.0, 3.0, 4.0);
            let border = BorderStyle::new(1.0, Color::BLACK).with_radius(corners);
            assert_eq!(border.radius, corners);
        }

        #[test]
        fn test_border_style_default() {
            let border = BorderStyle::default();
            assert_eq!(border, BorderStyle::NONE);
        }

        #[test]
        fn test_border_style_clone_and_copy() {
            let border = BorderStyle::new(2.0, Color::RED).with_radius(5.0);
            let cloned = border.clone();
            let copied = border;
            assert_eq!(border, cloned);
            assert_eq!(border, copied);
        }
    }

    // ========================================
    // Background Tests
    // ========================================

    mod background_tests {
        use super::*;

        #[test]
        fn test_background_none_constant() {
            let bg = Background::NONE;
            assert_eq!(bg, Background::None);
        }

        #[test]
        fn test_background_solid() {
            let bg = Background::solid(Color::RED);
            assert_eq!(bg, Background::Solid(Color::RED));
        }

        #[test]
        fn test_background_solid_with_hex() {
            let bg = Background::solid(Color::hex(0xFF0000));
            match bg {
                Background::Solid(color) => {
                    let rgba = color.to_rgba();
                    assert!((rgba.r - 1.0).abs() < 0.01);
                    assert!(rgba.g.abs() < 0.01);
                    assert!(rgba.b.abs() < 0.01);
                }
                _ => panic!("Expected Solid background"),
            }
        }

        #[test]
        fn test_background_linear_gradient() {
            let bg = Background::linear_gradient(Color::RED, Color::BLUE, 45.0);
            match bg {
                Background::LinearGradient { start, end, angle } => {
                    assert_eq!(start, Color::RED);
                    assert_eq!(end, Color::BLUE);
                    assert_eq!(angle, 45.0);
                }
                _ => panic!("Expected LinearGradient background"),
            }
        }

        #[test]
        fn test_background_radial_gradient() {
            let bg = Background::radial_gradient(Color::WHITE, Color::BLACK);
            match bg {
                Background::RadialGradient { inner, outer } => {
                    assert_eq!(inner, Color::WHITE);
                    assert_eq!(outer, Color::BLACK);
                }
                _ => panic!("Expected RadialGradient background"),
            }
        }

        #[test]
        fn test_background_default() {
            let bg = Background::default();
            assert_eq!(bg, Background::None);
        }

        #[test]
        fn test_background_from_color() {
            let bg: Background = Color::GREEN.into();
            assert_eq!(bg, Background::Solid(Color::GREEN));
        }

        #[test]
        fn test_background_clone_and_copy() {
            let bg = Background::solid(Color::RED);
            let cloned = bg.clone();
            let copied = bg;
            assert_eq!(bg, cloned);
            assert_eq!(bg, copied);
        }

        #[test]
        fn test_background_linear_gradient_various_angles() {
            let test_cases = [0.0, 45.0, 90.0, 180.0, 270.0, 360.0, -45.0];
            for angle in test_cases {
                let bg = Background::linear_gradient(Color::RED, Color::BLUE, angle);
                match bg {
                    Background::LinearGradient { angle: a, .. } => assert_eq!(a, angle),
                    _ => panic!("Expected LinearGradient"),
                }
            }
        }
    }

    // ========================================
    // Shadow Tests
    // ========================================

    mod shadow_tests {
        use super::*;

        #[test]
        fn test_shadow_new() {
            let shadow = Shadow::new(2.0, 4.0, 8.0, Color::BLACK);
            assert_eq!(shadow.offset_x, 2.0);
            assert_eq!(shadow.offset_y, 4.0);
            assert_eq!(shadow.blur_radius, 8.0);
            assert_eq!(shadow.spread_radius, 0.0);
            assert_eq!(shadow.color, Color::BLACK);
        }

        #[test]
        fn test_shadow_with_spread() {
            let shadow = Shadow::new(0.0, 0.0, 10.0, Color::BLACK).with_spread(5.0);
            assert_eq!(shadow.spread_radius, 5.0);
        }

        #[test]
        fn test_shadow_negative_offsets() {
            let shadow = Shadow::new(-5.0, -10.0, 8.0, Color::BLACK);
            assert_eq!(shadow.offset_x, -5.0);
            assert_eq!(shadow.offset_y, -10.0);
        }

        #[test]
        fn test_shadow_clone_and_copy() {
            let shadow = Shadow::new(1.0, 2.0, 3.0, Color::RED);
            let cloned = shadow.clone();
            let copied = shadow;
            assert_eq!(shadow, cloned);
            assert_eq!(shadow, copied);
        }

        #[test]
        fn test_shadow_with_various_colors() {
            let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::BLACK, Color::WHITE];
            for color in colors {
                let shadow = Shadow::new(0.0, 0.0, 5.0, color);
                assert_eq!(shadow.color, color);
            }
        }
    }

    // ========================================
    // Display Tests
    // ========================================

    mod display_tests {
        use super::*;

        #[test]
        fn test_display_default() {
            let display = Display::default();
            assert_eq!(display, Display::Flex);
        }

        #[test]
        fn test_display_variants() {
            let test_cases = [
                (Display::Flex, Display::Flex),
                (Display::Block, Display::Block),
                (Display::None, Display::None),
            ];

            for (display, expected) in test_cases {
                assert_eq!(display, expected);
            }
        }

        #[test]
        fn test_display_clone_and_copy() {
            let display = Display::Block;
            let cloned = display.clone();
            let copied = display;
            assert_eq!(display, cloned);
            assert_eq!(display, copied);
        }

        #[test]
        fn test_display_partial_eq() {
            assert_eq!(Display::Flex, Display::Flex);
            assert_ne!(Display::Flex, Display::Block);
            assert_ne!(Display::Flex, Display::None);
            assert_ne!(Display::Block, Display::None);
        }
    }

    // ========================================
    // FlexDirection Tests
    // ========================================

    mod flex_direction_tests {
        use super::*;

        #[test]
        fn test_flex_direction_default() {
            let direction = FlexDirection::default();
            assert_eq!(direction, FlexDirection::Row);
        }

        #[test]
        fn test_flex_direction_variants() {
            let test_cases = [
                FlexDirection::Row,
                FlexDirection::Column,
                FlexDirection::RowReverse,
                FlexDirection::ColumnReverse,
            ];

            for direction in test_cases {
                let cloned = direction.clone();
                assert_eq!(direction, cloned);
            }
        }

        #[test]
        fn test_flex_direction_partial_eq() {
            assert_eq!(FlexDirection::Row, FlexDirection::Row);
            assert_ne!(FlexDirection::Row, FlexDirection::Column);
            assert_ne!(FlexDirection::Row, FlexDirection::RowReverse);
            assert_ne!(FlexDirection::Column, FlexDirection::ColumnReverse);
        }
    }

    // ========================================
    // JustifyContent Tests
    // ========================================

    mod justify_content_tests {
        use super::*;

        #[test]
        fn test_justify_content_default() {
            let justify = JustifyContent::default();
            assert_eq!(justify, JustifyContent::FlexStart);
        }

        #[test]
        fn test_justify_content_variants() {
            let test_cases = [
                JustifyContent::FlexStart,
                JustifyContent::FlexEnd,
                JustifyContent::Center,
                JustifyContent::SpaceBetween,
                JustifyContent::SpaceAround,
                JustifyContent::SpaceEvenly,
            ];

            for justify in test_cases {
                let cloned = justify.clone();
                assert_eq!(justify, cloned);
            }
        }

        #[test]
        fn test_justify_content_partial_eq() {
            assert_eq!(JustifyContent::Center, JustifyContent::Center);
            assert_ne!(JustifyContent::Center, JustifyContent::FlexStart);
            assert_ne!(JustifyContent::SpaceBetween, JustifyContent::SpaceAround);
        }
    }

    // ========================================
    // AlignItems Tests
    // ========================================

    mod align_items_tests {
        use super::*;

        #[test]
        fn test_align_items_default() {
            let align = AlignItems::default();
            assert_eq!(align, AlignItems::Stretch);
        }

        #[test]
        fn test_align_items_variants() {
            let test_cases = [
                AlignItems::FlexStart,
                AlignItems::FlexEnd,
                AlignItems::Center,
                AlignItems::Stretch,
                AlignItems::Baseline,
            ];

            for align in test_cases {
                let cloned = align.clone();
                assert_eq!(align, cloned);
            }
        }

        #[test]
        fn test_align_items_partial_eq() {
            assert_eq!(AlignItems::Center, AlignItems::Center);
            assert_ne!(AlignItems::Center, AlignItems::Stretch);
            assert_ne!(AlignItems::Baseline, AlignItems::FlexEnd);
        }
    }

    // ========================================
    // Position Tests
    // ========================================

    mod position_tests {
        use super::*;

        #[test]
        fn test_position_default() {
            let position = Position::default();
            assert_eq!(position, Position::Relative);
        }

        #[test]
        fn test_position_variants() {
            let test_cases = [Position::Relative, Position::Absolute];

            for position in test_cases {
                let cloned = position.clone();
                assert_eq!(position, cloned);
            }
        }

        #[test]
        fn test_position_partial_eq() {
            assert_eq!(Position::Relative, Position::Relative);
            assert_eq!(Position::Absolute, Position::Absolute);
            assert_ne!(Position::Relative, Position::Absolute);
        }
    }

    // ========================================
    // Overflow Tests
    // ========================================

    mod overflow_tests {
        use super::*;

        #[test]
        fn test_overflow_default() {
            let overflow = Overflow::default();
            assert_eq!(overflow, Overflow::Visible);
        }

        #[test]
        fn test_overflow_variants() {
            let test_cases = [Overflow::Visible, Overflow::Hidden, Overflow::Scroll];

            for overflow in test_cases {
                let cloned = overflow.clone();
                assert_eq!(overflow, cloned);
            }
        }

        #[test]
        fn test_overflow_partial_eq() {
            assert_eq!(Overflow::Visible, Overflow::Visible);
            assert_ne!(Overflow::Visible, Overflow::Hidden);
            assert_ne!(Overflow::Hidden, Overflow::Scroll);
        }
    }

    // ========================================
    // Style Tests
    // ========================================

    mod style_tests {
        use super::*;

        #[test]
        fn test_style_new() {
            let style = Style::new();
            assert_eq!(style.opacity, 1.0);
            assert_eq!(style.flex_shrink, 1.0);
        }

        #[test]
        fn test_style_default() {
            let style = Style::default();
            assert_eq!(style.display, Display::Flex);
            assert_eq!(style.position, Position::Relative);
            assert_eq!(style.flex_direction, FlexDirection::Row);
            assert_eq!(style.justify_content, JustifyContent::FlexStart);
            assert_eq!(style.align_items, AlignItems::Stretch);
            assert_eq!(style.flex_grow, 0.0);
            assert_eq!(style.flex_shrink, 0.0);
            assert_eq!(style.gap, 0.0);
            assert_eq!(style.width, None);
            assert_eq!(style.height, None);
            assert_eq!(style.min_width, None);
            assert_eq!(style.min_height, None);
            assert_eq!(style.max_width, None);
            assert_eq!(style.max_height, None);
            assert_eq!(style.margin, Edges::ZERO);
            assert_eq!(style.padding, Edges::ZERO);
            assert_eq!(style.background, Background::None);
            assert_eq!(style.border, BorderStyle::NONE);
            assert_eq!(style.shadow, None);
            assert_eq!(style.opacity, 0.0);
            assert_eq!(style.overflow_x, Overflow::Visible);
            assert_eq!(style.overflow_y, Overflow::Visible);
        }

        #[test]
        fn test_style_new_vs_default() {
            let style_new = Style::new();
            let style_default = Style::default();

            // Style::new() sets opacity to 1.0 and flex_shrink to 1.0
            // Style::default() uses Default trait (opacity = 0.0, flex_shrink = 0.0)
            assert_ne!(style_new.opacity, style_default.opacity);
            assert_eq!(style_new.opacity, 1.0);
            assert_eq!(style_default.opacity, 0.0);

            assert_ne!(style_new.flex_shrink, style_default.flex_shrink);
            assert_eq!(style_new.flex_shrink, 1.0);
            assert_eq!(style_default.flex_shrink, 0.0);
        }

        #[test]
        fn test_style_layout_properties() {
            let mut style = Style::new();
            style.display = Display::Block;
            style.position = Position::Absolute;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = JustifyContent::Center;
            style.align_items = AlignItems::Center;
            style.flex_grow = 1.0;
            style.flex_shrink = 0.5;
            style.gap = 10.0;

            assert_eq!(style.display, Display::Block);
            assert_eq!(style.position, Position::Absolute);
            assert_eq!(style.flex_direction, FlexDirection::Column);
            assert_eq!(style.justify_content, JustifyContent::Center);
            assert_eq!(style.align_items, AlignItems::Center);
            assert_eq!(style.flex_grow, 1.0);
            assert_eq!(style.flex_shrink, 0.5);
            assert_eq!(style.gap, 10.0);
        }

        #[test]
        fn test_style_sizing_properties() {
            let mut style = Style::new();
            style.width = Some(100.0);
            style.height = Some(200.0);
            style.min_width = Some(50.0);
            style.min_height = Some(75.0);
            style.max_width = Some(300.0);
            style.max_height = Some(400.0);

            assert_eq!(style.width, Some(100.0));
            assert_eq!(style.height, Some(200.0));
            assert_eq!(style.min_width, Some(50.0));
            assert_eq!(style.min_height, Some(75.0));
            assert_eq!(style.max_width, Some(300.0));
            assert_eq!(style.max_height, Some(400.0));
        }

        #[test]
        fn test_style_spacing_properties() {
            let mut style = Style::new();
            style.margin = Edges::all(10.0);
            style.padding = Edges::new(5.0, 10.0, 15.0, 20.0);

            assert_eq!(style.margin, Edges::all(10.0));
            assert_eq!(style.padding, Edges::new(5.0, 10.0, 15.0, 20.0));
        }

        #[test]
        fn test_style_appearance_properties() {
            let mut style = Style::new();
            style.background = Background::solid(Color::RED);
            style.border = BorderStyle::new(2.0, Color::BLACK).with_radius(5.0);
            style.shadow = Some(Shadow::new(0.0, 4.0, 8.0, Color::BLACK));
            style.opacity = 0.8;

            assert_eq!(style.background, Background::Solid(Color::RED));
            assert_eq!(style.border.width, Edges::all(2.0));
            assert_eq!(style.border.color, Color::BLACK);
            assert_eq!(style.border.radius, Corners::all(5.0));
            assert!(style.shadow.is_some());
            assert_eq!(style.opacity, 0.8);
        }

        #[test]
        fn test_style_overflow_properties() {
            let mut style = Style::new();
            style.overflow_x = Overflow::Hidden;
            style.overflow_y = Overflow::Scroll;

            assert_eq!(style.overflow_x, Overflow::Hidden);
            assert_eq!(style.overflow_y, Overflow::Scroll);
        }

        #[test]
        fn test_style_clone() {
            let mut style = Style::new();
            style.width = Some(100.0);
            style.background = Background::solid(Color::BLUE);

            let cloned = style.clone();
            assert_eq!(style, cloned);
        }

        #[test]
        fn test_style_partial_eq() {
            let style1 = Style::new();
            let style2 = Style::new();
            let mut style3 = Style::new();
            style3.opacity = 0.5;

            assert_eq!(style1, style2);
            assert_ne!(style1, style3);
        }

        #[test]
        fn test_style_with_all_flex_directions() {
            let directions = [
                FlexDirection::Row,
                FlexDirection::Column,
                FlexDirection::RowReverse,
                FlexDirection::ColumnReverse,
            ];

            for direction in directions {
                let mut style = Style::new();
                style.flex_direction = direction;
                assert_eq!(style.flex_direction, direction);
            }
        }

        #[test]
        fn test_style_with_all_justify_content() {
            let justifications = [
                JustifyContent::FlexStart,
                JustifyContent::FlexEnd,
                JustifyContent::Center,
                JustifyContent::SpaceBetween,
                JustifyContent::SpaceAround,
                JustifyContent::SpaceEvenly,
            ];

            for justify in justifications {
                let mut style = Style::new();
                style.justify_content = justify;
                assert_eq!(style.justify_content, justify);
            }
        }

        #[test]
        fn test_style_with_all_align_items() {
            let alignments = [
                AlignItems::FlexStart,
                AlignItems::FlexEnd,
                AlignItems::Center,
                AlignItems::Stretch,
                AlignItems::Baseline,
            ];

            for align in alignments {
                let mut style = Style::new();
                style.align_items = align;
                assert_eq!(style.align_items, align);
            }
        }

        #[test]
        fn test_style_display_none_visibility() {
            let mut style = Style::new();
            style.display = Display::None;
            assert_eq!(style.display, Display::None);
        }

        #[test]
        fn test_style_opacity_boundary_values() {
            let test_cases = [0.0, 0.5, 1.0];

            for opacity in test_cases {
                let mut style = Style::new();
                style.opacity = opacity;
                assert_eq!(style.opacity, opacity);
            }
        }

        #[test]
        fn test_style_negative_gap() {
            // Gap can technically be negative, test it doesn't panic
            let mut style = Style::new();
            style.gap = -10.0;
            assert_eq!(style.gap, -10.0);
        }

        #[test]
        fn test_style_zero_sizing() {
            let mut style = Style::new();
            style.width = Some(0.0);
            style.height = Some(0.0);
            assert_eq!(style.width, Some(0.0));
            assert_eq!(style.height, Some(0.0));
        }
    }

    // ========================================
    // Integration Tests
    // ========================================

    mod integration_tests {
        use super::*;

        #[test]
        fn test_complete_style_configuration() {
            let mut style = Style::new();

            // Layout
            style.display = Display::Flex;
            style.position = Position::Relative;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = JustifyContent::SpaceBetween;
            style.align_items = AlignItems::Center;
            style.flex_grow = 1.0;
            style.flex_shrink = 0.0;
            style.gap = 16.0;

            // Sizing
            style.width = Some(200.0);
            style.height = Some(300.0);
            style.min_width = Some(100.0);
            style.min_height = Some(150.0);
            style.max_width = Some(400.0);
            style.max_height = Some(600.0);

            // Spacing
            style.margin = Edges::new(10.0, 20.0, 10.0, 20.0);
            style.padding = Edges::all(16.0);

            // Appearance
            style.background = Background::linear_gradient(
                Color::hex(0x4A90D9),
                Color::hex(0x357ABD),
                180.0,
            );
            style.border = BorderStyle::new(1.0, Color::hex(0xCCCCCC)).with_radius(8.0);
            style.shadow = Some(Shadow::new(0.0, 2.0, 4.0, Color::rgba(0.0, 0.0, 0.0, 0.1)));
            style.opacity = 1.0;

            // Overflow
            style.overflow_x = Overflow::Hidden;
            style.overflow_y = Overflow::Scroll;

            // Verify all properties
            assert_eq!(style.display, Display::Flex);
            assert_eq!(style.flex_direction, FlexDirection::Column);
            assert_eq!(style.width, Some(200.0));
            assert!(style.shadow.is_some());
        }

        #[test]
        fn test_card_style_pattern() {
            let mut card_style = Style::new();
            card_style.background = Background::solid(Color::WHITE);
            card_style.border = BorderStyle::new(1.0, Color::hex(0xE0E0E0)).with_radius(12.0);
            card_style.shadow = Some(Shadow::new(0.0, 2.0, 8.0, Color::rgba(0.0, 0.0, 0.0, 0.1)));
            card_style.padding = Edges::all(16.0);
            card_style.margin = Edges::all(8.0);

            assert_eq!(card_style.background, Background::Solid(Color::WHITE));
            assert_eq!(card_style.border.radius, Corners::all(12.0));
        }

        #[test]
        fn test_button_style_pattern() {
            let mut button_style = Style::new();
            button_style.display = Display::Flex;
            button_style.justify_content = JustifyContent::Center;
            button_style.align_items = AlignItems::Center;
            button_style.padding = Edges::new(8.0, 16.0, 8.0, 16.0);
            button_style.background = Background::solid(Color::hex(0x007AFF));
            button_style.border = BorderStyle::new(0.0, Color::TRANSPARENT).with_radius(6.0);

            assert_eq!(button_style.justify_content, JustifyContent::Center);
            assert_eq!(button_style.align_items, AlignItems::Center);
        }

        #[test]
        fn test_scrollable_container_pattern() {
            let mut container_style = Style::new();
            container_style.display = Display::Flex;
            container_style.flex_direction = FlexDirection::Column;
            container_style.overflow_x = Overflow::Hidden;
            container_style.overflow_y = Overflow::Scroll;
            container_style.height = Some(400.0);
            container_style.max_height = Some(600.0);

            assert_eq!(container_style.overflow_y, Overflow::Scroll);
            assert_eq!(container_style.max_height, Some(600.0));
        }

        #[test]
        fn test_absolute_positioned_overlay() {
            let mut overlay_style = Style::new();
            overlay_style.position = Position::Absolute;
            overlay_style.width = Some(100.0);
            overlay_style.height = Some(100.0);
            overlay_style.background = Background::solid(Color::rgba(0.0, 0.0, 0.0, 0.5));

            assert_eq!(overlay_style.position, Position::Absolute);
        }

        #[test]
        fn test_flex_container_with_gap() {
            let mut flex_container = Style::new();
            flex_container.display = Display::Flex;
            flex_container.flex_direction = FlexDirection::Row;
            flex_container.gap = 12.0;
            flex_container.justify_content = JustifyContent::SpaceEvenly;

            assert_eq!(flex_container.gap, 12.0);
            assert_eq!(flex_container.justify_content, JustifyContent::SpaceEvenly);
        }

        #[test]
        fn test_gradient_background_variations() {
            // Linear gradient
            let linear_bg = Background::linear_gradient(Color::RED, Color::BLUE, 45.0);
            match linear_bg {
                Background::LinearGradient { angle, .. } => assert_eq!(angle, 45.0),
                _ => panic!("Expected LinearGradient"),
            }

            // Radial gradient
            let radial_bg = Background::radial_gradient(Color::WHITE, Color::BLACK);
            match radial_bg {
                Background::RadialGradient { inner, outer } => {
                    assert_eq!(inner, Color::WHITE);
                    assert_eq!(outer, Color::BLACK);
                }
                _ => panic!("Expected RadialGradient"),
            }
        }

        #[test]
        fn test_shadow_with_spread_pattern() {
            let shadow = Shadow::new(0.0, 4.0, 16.0, Color::rgba(0.0, 0.0, 0.0, 0.2))
                .with_spread(2.0);

            assert_eq!(shadow.offset_x, 0.0);
            assert_eq!(shadow.offset_y, 4.0);
            assert_eq!(shadow.blur_radius, 16.0);
            assert_eq!(shadow.spread_radius, 2.0);
        }
    }

    // ========================================
    // Edge Cases and Boundary Tests
    // ========================================

    mod edge_case_tests {
        use super::*;

        #[test]
        fn test_corners_with_large_values() {
            let corners = Corners::all(1000000.0);
            assert_eq!(corners.max(), 1000000.0);
        }

        #[test]
        fn test_corners_with_negative_values() {
            // Negative radii are technically invalid but shouldn't panic
            let corners = Corners::new(-1.0, -2.0, -3.0, -4.0);
            assert_eq!(corners.top_left, -1.0);
        }

        #[test]
        fn test_shadow_with_zero_blur() {
            let shadow = Shadow::new(0.0, 0.0, 0.0, Color::BLACK);
            assert_eq!(shadow.blur_radius, 0.0);
        }

        #[test]
        fn test_background_gradient_with_same_colors() {
            let bg = Background::linear_gradient(Color::RED, Color::RED, 0.0);
            match bg {
                Background::LinearGradient { start, end, .. } => {
                    assert_eq!(start, end);
                }
                _ => panic!("Expected LinearGradient"),
            }
        }

        #[test]
        fn test_style_with_none_dimensions() {
            let style = Style::new();
            assert!(style.width.is_none());
            assert!(style.height.is_none());
            assert!(style.min_width.is_none());
            assert!(style.min_height.is_none());
            assert!(style.max_width.is_none());
            assert!(style.max_height.is_none());
        }

        #[test]
        fn test_edges_in_style() {
            let mut style = Style::new();
            style.margin = Edges::horizontal(20.0);
            style.padding = Edges::vertical(10.0);

            assert_eq!(style.margin.left, 20.0);
            assert_eq!(style.margin.right, 20.0);
            assert_eq!(style.margin.top, 0.0);
            assert_eq!(style.margin.bottom, 0.0);

            assert_eq!(style.padding.top, 10.0);
            assert_eq!(style.padding.bottom, 10.0);
            assert_eq!(style.padding.left, 0.0);
            assert_eq!(style.padding.right, 0.0);
        }

        #[test]
        fn test_opacity_extreme_values() {
            let mut style = Style::new();

            // Opacity below 0 (invalid but shouldn't panic)
            style.opacity = -0.5;
            assert_eq!(style.opacity, -0.5);

            // Opacity above 1 (invalid but shouldn't panic)
            style.opacity = 1.5;
            assert_eq!(style.opacity, 1.5);
        }

        #[test]
        fn test_flex_grow_shrink_combinations() {
            let test_cases = [
                (0.0, 0.0),
                (1.0, 1.0),
                (2.0, 0.5),
                (0.0, 1.0),
                (1.0, 0.0),
            ];

            for (grow, shrink) in test_cases {
                let mut style = Style::new();
                style.flex_grow = grow;
                style.flex_shrink = shrink;
                assert_eq!(style.flex_grow, grow);
                assert_eq!(style.flex_shrink, shrink);
            }
        }

        #[test]
        fn test_min_max_sizing_constraints() {
            let mut style = Style::new();
            style.width = Some(100.0);
            style.min_width = Some(50.0);
            style.max_width = Some(200.0);

            // Verify constraints are set (actual constraint logic would be in layout)
            assert!(style.min_width.unwrap() <= style.width.unwrap());
            assert!(style.width.unwrap() <= style.max_width.unwrap());
        }
    }
}
