use rui::core::color::Color;
use rui::core::geometry::Edges;
use rui::core::style::*;

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
        style.background =
            Background::linear_gradient(Color::hex(0x4A90D9), Color::hex(0x357ABD), 180.0);
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
        let shadow = Shadow::new(0.0, 4.0, 16.0, Color::rgba(0.0, 0.0, 0.0, 0.2)).with_spread(2.0);

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
        let test_cases = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.5), (0.0, 1.0), (1.0, 0.0)];

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
