//! Color types and conversions

use bytemuck::{Pod, Zeroable};

/// RGBA color (linear space, 0.0-1.0 range)
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Create from hex value (0xRRGGBB or 0xRRGGBBAA)
    pub fn from_hex(hex: u32) -> Self {
        if hex > 0xFFFFFF {
            // Has alpha
            Self::new(
                ((hex >> 24) & 0xFF) as f32 / 255.0,
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
            )
        } else {
            Self::new(
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
                1.0,
            )
        }
    }

    /// Create from u8 values (0-255)
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        Self::new(self.r, self.g, self.b, alpha)
    }

    /// Convert to array [r, g, b, a]
    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Blend this color over another (alpha compositing)
    pub fn over(&self, below: Rgba) -> Rgba {
        let a = self.a + below.a * (1.0 - self.a);
        if a == 0.0 {
            return Rgba::TRANSPARENT;
        }
        Rgba::new(
            (self.r * self.a + below.r * below.a * (1.0 - self.a)) / a,
            (self.g * self.a + below.g * below.a * (1.0 - self.a)) / a,
            (self.b * self.a + below.b * below.a * (1.0 - self.a)) / a,
            a,
        )
    }

    /// Linear interpolation between colors
    pub fn lerp(&self, other: Rgba, t: f32) -> Rgba {
        Rgba::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }
}

/// HSLA color (hue 0-360, saturation/lightness/alpha 0-1)
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
#[repr(C)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl Hsla {
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }

    pub const fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::new(h, s, l, 1.0)
    }

    pub fn to_rgba(&self) -> Rgba {
        let h = self.h / 360.0;
        let s = self.s;
        let l = self.l;

        if s == 0.0 {
            return Rgba::new(l, l, l, self.a);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;

        fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                return p + (q - p) * 6.0 * t;
            }
            if t < 1.0 / 2.0 {
                return q;
            }
            if t < 2.0 / 3.0 {
                return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
            }
            p
        }

        Rgba::new(
            hue_to_rgb(p, q, h + 1.0 / 3.0),
            hue_to_rgb(p, q, h),
            hue_to_rgb(p, q, h - 1.0 / 3.0),
            self.a,
        )
    }
}

impl From<Hsla> for Rgba {
    fn from(hsla: Hsla) -> Self {
        hsla.to_rgba()
    }
}

impl From<[f32; 4]> for Rgba {
    fn from(arr: [f32; 4]) -> Self {
        Self::new(arr[0], arr[1], arr[2], arr[3])
    }
}

/// High-level Color type that can be either RGBA or HSLA
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Rgba(Rgba),
    Hsla(Hsla),
}

impl Color {
    pub const TRANSPARENT: Self = Self::Rgba(Rgba::TRANSPARENT);
    pub const BLACK: Self = Self::Rgba(Rgba::BLACK);
    pub const WHITE: Self = Self::Rgba(Rgba::WHITE);
    pub const RED: Self = Self::Rgba(Rgba::RED);
    pub const GREEN: Self = Self::Rgba(Rgba::GREEN);
    pub const BLUE: Self = Self::Rgba(Rgba::BLUE);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::Rgba(Rgba::rgb(r, g, b))
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::Rgba(Rgba::new(r, g, b, a))
    }

    pub const fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::Hsla(Hsla::hsl(h, s, l))
    }

    pub const fn hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self::Hsla(Hsla::new(h, s, l, a))
    }

    pub fn hex(value: u32) -> Self {
        Self::Rgba(Rgba::from_hex(value))
    }

    pub fn to_rgba(&self) -> Rgba {
        match self {
            Color::Rgba(rgba) => *rgba,
            Color::Hsla(hsla) => hsla.to_rgba(),
        }
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        match self {
            Color::Rgba(rgba) => Color::Rgba(rgba.with_alpha(alpha)),
            Color::Hsla(hsla) => Color::Hsla(Hsla::new(hsla.h, hsla.s, hsla.l, alpha)),
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl From<Rgba> for Color {
    fn from(rgba: Rgba) -> Self {
        Color::Rgba(rgba)
    }
}

impl From<Hsla> for Color {
    fn from(hsla: Hsla) -> Self {
        Color::Hsla(hsla)
    }
}

impl From<u32> for Color {
    fn from(hex: u32) -> Self {
        Color::hex(hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn assert_rgba_eq(a: Rgba, b: Rgba) {
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
                (0xFF0000FF, Rgba::new(1.0, 0.0, 0.0, 1.0)),   // Red with full alpha
                (0xFFFF00FF, Rgba::new(1.0, 1.0, 0.0, 1.0)),   // Yellow with full alpha
                (0xFF000080, Rgba::new(1.0, 0.0, 0.0, 128.0 / 255.0)), // Red with half alpha
                (0xFFFFFFFF, Rgba::new(1.0, 1.0, 1.0, 1.0)),   // White with full alpha
                (0x01000000, Rgba::new(1.0 / 255.0, 0.0, 0.0, 0.0)), // Minimal red, no alpha
                (0x80808080, Rgba::new(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0)), // Gray with half alpha
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
            let cloned = original.clone();
            assert_eq!(original, copied);
            assert_eq!(original, cloned);
        }

        #[test]
        fn test_hsla_copy_and_clone() {
            let original = Hsla::new(180.0, 0.5, 0.6, 0.7);
            let copied = original;
            let cloned = original.clone();
            assert_eq!(original, copied);
            assert_eq!(original, cloned);
        }

        #[test]
        fn test_color_copy_and_clone() {
            let original = Color::rgba(0.1, 0.2, 0.3, 0.4);
            let copied = original;
            let cloned = original.clone();
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
}
