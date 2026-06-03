use rui::core::color::Color;
use rui::core::geometry::Edges;
use rui::core::style::*;

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
        let colors = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::BLACK,
            Color::WHITE,
        ];
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
