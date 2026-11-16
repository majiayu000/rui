use super::*;

pub(super) const EPSILON: f32 = 0.001;

pub(super) fn assert_rgba_eq(a: Rgba, b: Rgba) {
    assert!(
        (a.r - b.r).abs() < EPSILON
            && (a.g - b.g).abs() < EPSILON
            && (a.b - b.b).abs() < EPSILON
            && (a.a - b.a).abs() < EPSILON,
        "Colors not equal: {:?} vs {:?}",
        a,
        b
    );
}

// ==================== Rgba Tests ====================

mod rgba_new {
    use super::*;

    #[test]
    fn test_new_creates_color_with_values() {
        let cases = [
            ((0.0, 0.0, 0.0, 0.0), [0.0, 0.0, 0.0, 0.0]),
            ((1.0, 1.0, 1.0, 1.0), [1.0, 1.0, 1.0, 1.0]),
            ((0.5, 0.25, 0.75, 0.5), [0.5, 0.25, 0.75, 0.5]),
            ((0.1, 0.2, 0.3, 0.4), [0.1, 0.2, 0.3, 0.4]),
        ];

        for ((r, g, b, a), expected) in cases {
            let color = Rgba::new(r, g, b, a);
            assert_eq!(color.r, expected[0]);
            assert_eq!(color.g, expected[1]);
            assert_eq!(color.b, expected[2]);
            assert_eq!(color.a, expected[3]);
        }
    }

    #[test]
    fn test_rgb_sets_alpha_to_one() {
        let cases = [
            ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0)),
            ((1.0, 0.5, 0.25), (1.0, 0.5, 0.25, 1.0)),
            ((0.33, 0.66, 0.99), (0.33, 0.66, 0.99, 1.0)),
        ];

        for ((r, g, b), (er, eg, eb, ea)) in cases {
            let color = Rgba::rgb(r, g, b);
            assert_eq!(color.r, er);
            assert_eq!(color.g, eg);
            assert_eq!(color.b, eb);
            assert_eq!(color.a, ea);
        }
    }
}

mod rgba_constants {
    use super::*;

    #[test]
    fn test_transparent() {
        assert_eq!(Rgba::TRANSPARENT, Rgba::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_black() {
        assert_eq!(Rgba::BLACK, Rgba::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_white() {
        assert_eq!(Rgba::WHITE, Rgba::new(1.0, 1.0, 1.0, 1.0));
    }

    #[test]
    fn test_red() {
        assert_eq!(Rgba::RED, Rgba::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_green() {
        assert_eq!(Rgba::GREEN, Rgba::new(0.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn test_blue() {
        assert_eq!(Rgba::BLUE, Rgba::new(0.0, 0.0, 1.0, 1.0));
    }
}

mod rgba_from_hex {
    use super::*;

    #[test]
    fn test_from_hex_rgb_format() {
        let cases = [
            (0xFF0000, Rgba::new(1.0, 0.0, 0.0, 1.0)),
            (0x00FF00, Rgba::new(0.0, 1.0, 0.0, 1.0)),
            (0x0000FF, Rgba::new(0.0, 0.0, 1.0, 1.0)),
            (0xFFFFFF, Rgba::new(1.0, 1.0, 1.0, 1.0)),
            (0x000000, Rgba::new(0.0, 0.0, 0.0, 1.0)),
            (
                0x808080,
                Rgba::new(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0),
            ),
        ];

        for (hex, expected) in cases {
            let color = Rgba::from_hex(hex);
            assert_rgba_eq(color, expected);
        }
    }

    #[test]
    fn test_from_hex_rgba_format() {
        // Note: Only hex values > 0xFFFFFF are treated as RGBA format
        // Values <= 0xFFFFFF are treated as RGB (alpha = 1.0)
        let cases = [
            (0xFF0000FF, Rgba::new(1.0, 0.0, 0.0, 1.0)), // Red with full alpha
            (0xFFFF00FF, Rgba::new(1.0, 1.0, 0.0, 1.0)), // Yellow with full alpha
            (0xFF000080, Rgba::new(1.0, 0.0, 0.0, 128.0 / 255.0)), // Red with half alpha
            (0xFFFFFFFF, Rgba::new(1.0, 1.0, 1.0, 1.0)), // White with full alpha
            (0x01000000, Rgba::new(1.0 / 255.0, 0.0, 0.0, 0.0)), // Minimal red, no alpha
            (
                0x80808080,
                Rgba::new(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0),
            ), // Gray with half alpha
        ];

        for (hex, expected) in cases {
            let color = Rgba::from_hex(hex);
            assert_rgba_eq(color, expected);
        }
    }

    #[test]
    fn test_from_hex_boundary() {
        // 0xFFFFFF is the max RGB value without alpha
        let at_boundary = Rgba::from_hex(0xFFFFFF);
        assert_rgba_eq(at_boundary, Rgba::new(1.0, 1.0, 1.0, 1.0));

        // 0x1000000 is treated as RGBA
        let just_over = Rgba::from_hex(0x1000000);
        assert_rgba_eq(just_over, Rgba::new(1.0 / 255.0, 0.0, 0.0, 0.0));
    }
}

mod rgba_from_u8 {
    use super::*;

    #[test]
    fn test_from_u8_converts_correctly() {
        let cases = [
            ((0, 0, 0, 0), Rgba::new(0.0, 0.0, 0.0, 0.0)),
            ((255, 255, 255, 255), Rgba::new(1.0, 1.0, 1.0, 1.0)),
            ((255, 0, 0, 255), Rgba::new(1.0, 0.0, 0.0, 1.0)),
            ((0, 255, 0, 255), Rgba::new(0.0, 1.0, 0.0, 1.0)),
            ((0, 0, 255, 255), Rgba::new(0.0, 0.0, 1.0, 1.0)),
            (
                (128, 128, 128, 128),
                Rgba::new(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0),
            ),
        ];

        for ((r, g, b, a), expected) in cases {
            let color = Rgba::from_u8(r, g, b, a);
            assert_rgba_eq(color, expected);
        }
    }
}

mod rgba_with_alpha {
    use super::*;

    #[test]
    fn test_with_alpha_changes_alpha() {
        let cases = [
            (Rgba::RED, 0.5, Rgba::new(1.0, 0.0, 0.0, 0.5)),
            (Rgba::WHITE, 0.0, Rgba::new(1.0, 1.0, 1.0, 0.0)),
            (Rgba::TRANSPARENT, 1.0, Rgba::new(0.0, 0.0, 0.0, 1.0)),
            (
                Rgba::new(0.1, 0.2, 0.3, 0.4),
                0.9,
                Rgba::new(0.1, 0.2, 0.3, 0.9),
            ),
        ];

        for (original, new_alpha, expected) in cases {
            let result = original.with_alpha(new_alpha);
            assert_rgba_eq(result, expected);
        }
    }
}

mod rgba_to_array {
    use super::*;

    #[test]
    fn test_to_array_returns_components() {
        let cases = [
            (Rgba::new(0.1, 0.2, 0.3, 0.4), [0.1, 0.2, 0.3, 0.4]),
            (Rgba::RED, [1.0, 0.0, 0.0, 1.0]),
            (Rgba::TRANSPARENT, [0.0, 0.0, 0.0, 0.0]),
        ];

        for (color, expected) in cases {
            let arr = color.to_array();
            assert_eq!(arr, expected);
        }
    }
}

mod rgba_over {
    use super::*;

    #[test]
    fn test_over_opaque_over_opaque() {
        let top = Rgba::RED;
        let bottom = Rgba::BLUE;
        let result = top.over(bottom);
        assert_rgba_eq(result, Rgba::RED);
    }

    #[test]
    fn test_over_transparent_over_opaque() {
        let top = Rgba::TRANSPARENT;
        let bottom = Rgba::BLUE;
        let result = top.over(bottom);
        assert_rgba_eq(result, Rgba::BLUE);
    }

    #[test]
    fn test_over_opaque_over_transparent() {
        let top = Rgba::RED;
        let bottom = Rgba::TRANSPARENT;
        let result = top.over(bottom);
        assert_rgba_eq(result, Rgba::RED);
    }

    #[test]
    fn test_over_transparent_over_transparent() {
        let top = Rgba::TRANSPARENT;
        let bottom = Rgba::TRANSPARENT;
        let result = top.over(bottom);
        assert_rgba_eq(result, Rgba::TRANSPARENT);
    }

    #[test]
    fn test_over_semitransparent() {
        let top = Rgba::new(1.0, 0.0, 0.0, 0.5);
        let bottom = Rgba::new(0.0, 0.0, 1.0, 1.0);
        let result = top.over(bottom);

        // Alpha: 0.5 + 1.0 * (1.0 - 0.5) = 1.0
        assert!((result.a - 1.0).abs() < EPSILON);
        // R: (1.0 * 0.5 + 0.0 * 1.0 * 0.5) / 1.0 = 0.5
        assert!((result.r - 0.5).abs() < EPSILON);
        // B: (0.0 * 0.5 + 1.0 * 1.0 * 0.5) / 1.0 = 0.5
        assert!((result.b - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_over_partial_alpha_blending() {
        let top = Rgba::new(1.0, 1.0, 1.0, 0.25);
        let bottom = Rgba::new(0.0, 0.0, 0.0, 0.5);
        let result = top.over(bottom);

        // Alpha: 0.25 + 0.5 * 0.75 = 0.625
        assert!((result.a - 0.625).abs() < EPSILON);
    }
}

mod rgba_lerp {
    use super::*;

    #[test]
    fn test_lerp_at_zero() {
        let start = Rgba::RED;
        let end = Rgba::BLUE;
        let result = start.lerp(end, 0.0);
        assert_rgba_eq(result, start);
    }

    #[test]
    fn test_lerp_at_one() {
        let start = Rgba::RED;
        let end = Rgba::BLUE;
        let result = start.lerp(end, 1.0);
        assert_rgba_eq(result, end);
    }

    #[test]
    fn test_lerp_at_half() {
        let cases = [
            (Rgba::BLACK, Rgba::WHITE, Rgba::new(0.5, 0.5, 0.5, 1.0)),
            (Rgba::RED, Rgba::BLUE, Rgba::new(0.5, 0.0, 0.5, 1.0)),
            (
                Rgba::TRANSPARENT,
                Rgba::WHITE,
                Rgba::new(0.5, 0.5, 0.5, 0.5),
            ),
        ];

        for (start, end, expected) in cases {
            let result = start.lerp(end, 0.5);
            assert_rgba_eq(result, expected);
        }
    }

    #[test]
    fn test_lerp_custom_values() {
        let start = Rgba::new(0.0, 0.0, 0.0, 0.0);
        let end = Rgba::new(1.0, 1.0, 1.0, 1.0);

        let cases = [
            (0.25, Rgba::new(0.25, 0.25, 0.25, 0.25)),
            (0.75, Rgba::new(0.75, 0.75, 0.75, 0.75)),
            (0.1, Rgba::new(0.1, 0.1, 0.1, 0.1)),
        ];

        for (t, expected) in cases {
            let result = start.lerp(end, t);
            assert_rgba_eq(result, expected);
        }
    }

    #[test]
    fn test_lerp_extrapolation() {
        // t outside 0-1 range should extrapolate
        let start = Rgba::new(0.5, 0.5, 0.5, 1.0);
        let end = Rgba::new(1.0, 1.0, 1.0, 1.0);

        let result = start.lerp(end, 2.0);
        assert_rgba_eq(result, Rgba::new(1.5, 1.5, 1.5, 1.0));
    }
}

mod rgba_from_traits {
    use super::*;

    #[test]
    fn test_from_array() {
        let arr = [0.1, 0.2, 0.3, 0.4];
        let color: Rgba = arr.into();
        assert_eq!(color, Rgba::new(0.1, 0.2, 0.3, 0.4));
    }

    #[test]
    fn test_default() {
        let color = Rgba::default();
        assert_eq!(color, Rgba::new(0.0, 0.0, 0.0, 0.0));
    }
}

// ==================== Hsla Tests ====================

mod hsla_new {
    use super::*;

    #[test]
    fn test_new_creates_color() {
        let cases = [
            ((0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0)),
            ((360.0, 1.0, 1.0, 1.0), (360.0, 1.0, 1.0, 1.0)),
            ((180.0, 0.5, 0.5, 0.5), (180.0, 0.5, 0.5, 0.5)),
        ];

        for ((h, s, l, a), (eh, es, el, ea)) in cases {
            let color = Hsla::new(h, s, l, a);
            assert_eq!(color.h, eh);
            assert_eq!(color.s, es);
            assert_eq!(color.l, el);
            assert_eq!(color.a, ea);
        }
    }

    #[test]
    fn test_hsl_sets_alpha_to_one() {
        let color = Hsla::hsl(120.0, 0.5, 0.5);
        assert_eq!(color.h, 120.0);
        assert_eq!(color.s, 0.5);
        assert_eq!(color.l, 0.5);
        assert_eq!(color.a, 1.0);
    }
}

mod hsla_to_rgba {
    use super::*;

    #[test]
    fn test_red() {
        let hsla = Hsla::hsl(0.0, 1.0, 0.5);
        let rgba = hsla.to_rgba();
        assert_rgba_eq(rgba, Rgba::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_green() {
        let hsla = Hsla::hsl(120.0, 1.0, 0.5);
        let rgba = hsla.to_rgba();
        assert_rgba_eq(rgba, Rgba::new(0.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn test_blue() {
        let hsla = Hsla::hsl(240.0, 1.0, 0.5);
        let rgba = hsla.to_rgba();
        assert_rgba_eq(rgba, Rgba::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn test_white() {
        let hsla = Hsla::hsl(0.0, 0.0, 1.0);
        let rgba = hsla.to_rgba();
        assert_rgba_eq(rgba, Rgba::WHITE);
    }

    #[test]
    fn test_black() {
        let hsla = Hsla::hsl(0.0, 0.0, 0.0);
        let rgba = hsla.to_rgba();
        assert_rgba_eq(rgba, Rgba::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_gray_zero_saturation() {
        // When saturation is 0, result should be grayscale
        let cases = [
            (Hsla::hsl(0.0, 0.0, 0.5), Rgba::new(0.5, 0.5, 0.5, 1.0)),
            (
                Hsla::hsl(180.0, 0.0, 0.25),
                Rgba::new(0.25, 0.25, 0.25, 1.0),
            ),
            (Hsla::hsl(90.0, 0.0, 0.75), Rgba::new(0.75, 0.75, 0.75, 1.0)),
        ];

        for (hsla, expected) in cases {
            let rgba = hsla.to_rgba();
            assert_rgba_eq(rgba, expected);
        }
    }

    #[test]
    fn test_lightness_less_than_half() {
        // Test the q = l * (1 + s) branch
        let hsla = Hsla::hsl(0.0, 1.0, 0.25);
        let rgba = hsla.to_rgba();
        assert!((rgba.r - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_lightness_greater_than_half() {
        // Test the q = l + s - l * s branch
        let hsla = Hsla::hsl(0.0, 1.0, 0.75);
        let rgba = hsla.to_rgba();
        assert!((rgba.r - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_alpha_preserved() {
        let hsla = Hsla::new(0.0, 1.0, 0.5, 0.5);
        let rgba = hsla.to_rgba();
        assert!((rgba.a - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_various_hues() {
        // Yellow (H=60)
        let yellow = Hsla::hsl(60.0, 1.0, 0.5).to_rgba();
        assert_rgba_eq(yellow, Rgba::new(1.0, 1.0, 0.0, 1.0));

        // Cyan (H=180)
        let cyan = Hsla::hsl(180.0, 1.0, 0.5).to_rgba();
        assert_rgba_eq(cyan, Rgba::new(0.0, 1.0, 1.0, 1.0));

        // Magenta (H=300)
        let magenta = Hsla::hsl(300.0, 1.0, 0.5).to_rgba();
        assert_rgba_eq(magenta, Rgba::new(1.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn test_hue_to_rgb_edge_cases() {
        // Test hue values that hit different branches of hue_to_rgb
        // t < 1/6
        let h30 = Hsla::hsl(30.0, 1.0, 0.5).to_rgba();
        assert!(h30.r > 0.9);

        // 1/6 <= t < 1/2
        let h90 = Hsla::hsl(90.0, 1.0, 0.5).to_rgba();
        assert!((h90.r - 0.5).abs() < 0.01);

        // 1/2 <= t < 2/3
        let h150 = Hsla::hsl(150.0, 1.0, 0.5).to_rgba();
        assert!(h150.r < 0.1);

        // t >= 2/3
        let h270 = Hsla::hsl(270.0, 1.0, 0.5).to_rgba();
        assert!((h270.r - 0.5).abs() < 0.1);
    }
}

mod hsla_from_trait {
    use super::*;

    #[test]
    fn test_hsla_to_rgba_via_from() {
        let hsla = Hsla::hsl(0.0, 1.0, 0.5);
        let rgba: Rgba = hsla.into();
        assert_rgba_eq(rgba, Rgba::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_default() {
        let color = Hsla::default();
        assert_eq!(color, Hsla::new(0.0, 0.0, 0.0, 0.0));
    }
}
