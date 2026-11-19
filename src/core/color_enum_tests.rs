use super::tests::assert_rgba_eq;
use super::*;

// ==================== Color Enum Tests ====================

mod color_constants {
    use super::*;

    #[test]
    fn test_transparent() {
        assert_eq!(Color::TRANSPARENT, Color::Rgba(Rgba::TRANSPARENT));
    }

    #[test]
    fn test_black() {
        assert_eq!(Color::BLACK, Color::Rgba(Rgba::BLACK));
    }

    #[test]
    fn test_white() {
        assert_eq!(Color::WHITE, Color::Rgba(Rgba::WHITE));
    }

    #[test]
    fn test_red() {
        assert_eq!(Color::RED, Color::Rgba(Rgba::RED));
    }

    #[test]
    fn test_green() {
        assert_eq!(Color::GREEN, Color::Rgba(Rgba::GREEN));
    }

    #[test]
    fn test_blue() {
        assert_eq!(Color::BLUE, Color::Rgba(Rgba::BLUE));
    }
}

mod color_constructors {
    use super::*;

    #[test]
    fn test_rgb() {
        let color = Color::rgb(0.5, 0.6, 0.7);
        match color {
            Color::Rgba(rgba) => {
                assert_eq!(rgba.r, 0.5);
                assert_eq!(rgba.g, 0.6);
                assert_eq!(rgba.b, 0.7);
                assert_eq!(rgba.a, 1.0);
            }
            _ => panic!("Expected Rgba variant"),
        }
    }

    #[test]
    fn test_rgba() {
        let color = Color::rgba(0.1, 0.2, 0.3, 0.4);
        match color {
            Color::Rgba(rgba) => {
                assert_eq!(rgba.r, 0.1);
                assert_eq!(rgba.g, 0.2);
                assert_eq!(rgba.b, 0.3);
                assert_eq!(rgba.a, 0.4);
            }
            _ => panic!("Expected Rgba variant"),
        }
    }

    #[test]
    fn test_hsl() {
        let color = Color::hsl(120.0, 0.5, 0.6);
        match color {
            Color::Hsla(hsla) => {
                assert_eq!(hsla.h, 120.0);
                assert_eq!(hsla.s, 0.5);
                assert_eq!(hsla.l, 0.6);
                assert_eq!(hsla.a, 1.0);
            }
            _ => panic!("Expected Hsla variant"),
        }
    }

    #[test]
    fn test_hsla() {
        let color = Color::hsla(180.0, 0.3, 0.4, 0.5);
        match color {
            Color::Hsla(hsla) => {
                assert_eq!(hsla.h, 180.0);
                assert_eq!(hsla.s, 0.3);
                assert_eq!(hsla.l, 0.4);
                assert_eq!(hsla.a, 0.5);
            }
            _ => panic!("Expected Hsla variant"),
        }
    }

    #[test]
    fn test_hex() {
        let cases = [
            (0xFF0000, Rgba::new(1.0, 0.0, 0.0, 1.0)),
            (0x00FF00, Rgba::new(0.0, 1.0, 0.0, 1.0)),
            (0x0000FF, Rgba::new(0.0, 0.0, 1.0, 1.0)),
        ];

        for (hex, expected) in cases {
            let color = Color::hex(hex);
            match color {
                Color::Rgba(rgba) => assert_rgba_eq(rgba, expected),
                _ => panic!("Expected Rgba variant"),
            }
        }
    }
}

mod color_to_rgba {
    use super::*;

    #[test]
    fn test_rgba_variant_returns_self() {
        let original = Rgba::new(0.1, 0.2, 0.3, 0.4);
        let color = Color::Rgba(original);
        let result = color.to_rgba();
        assert_eq!(result, original);
    }

    #[test]
    fn test_hsla_variant_converts() {
        let hsla = Hsla::hsl(0.0, 1.0, 0.5);
        let color = Color::Hsla(hsla);
        let result = color.to_rgba();
        assert_rgba_eq(result, Rgba::new(1.0, 0.0, 0.0, 1.0));
    }
}

mod color_with_alpha {
    use super::*;

    #[test]
    fn test_rgba_variant() {
        let color = Color::rgba(1.0, 0.0, 0.0, 1.0);
        let result = color.with_alpha(0.5);
        match result {
            Color::Rgba(rgba) => {
                assert_eq!(rgba.r, 1.0);
                assert_eq!(rgba.g, 0.0);
                assert_eq!(rgba.b, 0.0);
                assert_eq!(rgba.a, 0.5);
            }
            _ => panic!("Expected Rgba variant"),
        }
    }

    #[test]
    fn test_hsla_variant() {
        let color = Color::hsla(120.0, 0.5, 0.6, 1.0);
        let result = color.with_alpha(0.25);
        match result {
            Color::Hsla(hsla) => {
                assert_eq!(hsla.h, 120.0);
                assert_eq!(hsla.s, 0.5);
                assert_eq!(hsla.l, 0.6);
                assert_eq!(hsla.a, 0.25);
            }
            _ => panic!("Expected Hsla variant"),
        }
    }
}

mod color_default {
    use super::*;

    #[test]
    fn test_default_is_transparent() {
        let color = Color::default();
        assert_eq!(color, Color::TRANSPARENT);
    }
}

mod color_from_traits {
    use super::*;

    #[test]
    fn test_from_rgba() {
        let rgba = Rgba::new(0.1, 0.2, 0.3, 0.4);
        let color: Color = rgba.into();
        assert_eq!(color, Color::Rgba(rgba));
    }

    #[test]
    fn test_from_hsla() {
        let hsla = Hsla::new(100.0, 0.5, 0.6, 0.7);
        let color: Color = hsla.into();
        assert_eq!(color, Color::Hsla(hsla));
    }

    #[test]
    fn test_from_u32() {
        let cases = [
            (0xFF0000_u32, Rgba::new(1.0, 0.0, 0.0, 1.0)),
            (0x00FF00_u32, Rgba::new(0.0, 1.0, 0.0, 1.0)),
            (0x0000FF_u32, Rgba::new(0.0, 0.0, 1.0, 1.0)),
        ];

        for (hex, expected_rgba) in cases {
            let color: Color = hex.into();
            match color {
                Color::Rgba(rgba) => assert_rgba_eq(rgba, expected_rgba),
                _ => panic!("Expected Rgba variant"),
            }
        }
    }
}

// ==================== Edge Cases and Boundary Tests ====================

mod edge_cases {
    use super::*;

    #[test]
    fn test_rgba_values_outside_range() {
        // Values outside 0-1 should be allowed (no clamping)
        let color = Rgba::new(2.0, -1.0, 1.5, 0.5);
        assert_eq!(color.r, 2.0);
        assert_eq!(color.g, -1.0);
        assert_eq!(color.b, 1.5);
        assert_eq!(color.a, 0.5);
    }

    #[test]
    fn test_lerp_with_negative_t() {
        let start = Rgba::new(0.5, 0.5, 0.5, 1.0);
        let end = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let result = start.lerp(end, -1.0);
        assert_rgba_eq(result, Rgba::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_hsla_extreme_hue_values() {
        // 360 should wrap to same as 0
        let h360 = Hsla::hsl(360.0, 1.0, 0.5).to_rgba();
        let h0 = Hsla::hsl(0.0, 1.0, 0.5).to_rgba();
        assert_rgba_eq(h360, h0);
    }

    #[test]
    fn test_rgba_copy_and_clone() {
        let original = Rgba::new(0.1, 0.2, 0.3, 0.4);
        let copied = original;
        let cloned = original;
        assert_eq!(original, copied);
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_hsla_copy_and_clone() {
        let original = Hsla::new(180.0, 0.5, 0.6, 0.7);
        let copied = original;
        let cloned = original;
        assert_eq!(original, copied);
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_color_copy_and_clone() {
        let original = Color::rgba(0.1, 0.2, 0.3, 0.4);
        let copied = original;
        let cloned = original;
        assert_eq!(original, copied);
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_rgba_debug() {
        let color = Rgba::new(0.1, 0.2, 0.3, 0.4);
        let debug = format!("{:?}", color);
        assert!(debug.contains("Rgba"));
        assert!(debug.contains("0.1"));
    }

    #[test]
    fn test_hsla_debug() {
        let color = Hsla::new(180.0, 0.5, 0.6, 0.7);
        let debug = format!("{:?}", color);
        assert!(debug.contains("Hsla"));
        assert!(debug.contains("180"));
    }

    #[test]
    fn test_color_debug() {
        let color = Color::rgba(0.1, 0.2, 0.3, 0.4);
        let debug = format!("{:?}", color);
        assert!(debug.contains("Rgba"));
    }
}

mod partial_eq {
    use super::*;

    #[test]
    fn test_rgba_partial_eq() {
        assert_eq!(Rgba::RED, Rgba::RED);
        assert_ne!(Rgba::RED, Rgba::BLUE);
    }

    #[test]
    fn test_hsla_partial_eq() {
        let a = Hsla::hsl(120.0, 0.5, 0.6);
        let b = Hsla::hsl(120.0, 0.5, 0.6);
        let c = Hsla::hsl(180.0, 0.5, 0.6);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_color_partial_eq() {
        assert_eq!(Color::RED, Color::RED);
        assert_ne!(Color::RED, Color::BLUE);
        assert_ne!(Color::rgb(1.0, 0.0, 0.0), Color::hsl(0.0, 1.0, 0.5));
    }
}
