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

    #[test]
    fn test_rgba_from_hex() {
        let color = Rgba::from_hex(0xFF0000);
        assert_eq!(color, Rgba::RED);

        let color = Rgba::from_hex(0x00FF00);
        assert_eq!(color, Rgba::GREEN);
    }

    #[test]
    fn test_hsla_to_rgba() {
        // Red
        let hsla = Hsla::hsl(0.0, 1.0, 0.5);
        let rgba = hsla.to_rgba();
        assert!((rgba.r - 1.0).abs() < 0.01);
        assert!(rgba.g.abs() < 0.01);
        assert!(rgba.b.abs() < 0.01);
    }

    #[test]
    fn test_color_lerp() {
        let black = Rgba::BLACK;
        let white = Rgba::WHITE;
        let gray = black.lerp(white, 0.5);

        assert!((gray.r - 0.5).abs() < 0.01);
        assert!((gray.g - 0.5).abs() < 0.01);
        assert!((gray.b - 0.5).abs() < 0.01);
    }
}
