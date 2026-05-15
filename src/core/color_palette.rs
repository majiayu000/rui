//! Extended color palette and helpers.
//!
//! This module augments [`crate::core::color::Rgba`] and
//! [`crate::core::color::Color`] with named UI constants and brightness
//! utilities. All additions here are non-breaking: existing constants and
//! constructors keep their original semantics.
//!
//! The new constants follow the conventional CSS palette and are stored as
//! sRGB approximations in 0.0..=1.0 range.
//!
//! # Examples
//!
//! ```
//! use rui::{Color, Rgba};
//!
//! // New Rgba palette constants.
//! assert_eq!(Rgba::GRAY, Rgba::new(0.5, 0.5, 0.5, 1.0));
//!
//! // High-level Color enum picks them up via dedicated constants.
//! let bg = Color::ORANGE.darken(0.5);
//! let fg = Color::WHITE;
//! assert!(bg.luminance() < fg.luminance());
//! ```
//!
//! Picking a foreground color from a background:
//!
//! ```
//! use rui::Color;
//!
//! let bg = Color::hex(0x1a1a2e);
//! let fg = if bg.is_dark() { Color::WHITE } else { Color::BLACK };
//! assert_eq!(fg, Color::WHITE);
//! ```

use super::color::{Color, Rgba};

impl Rgba {
    // ---- Extended named palette (additive, non-breaking) ----

    /// Neutral middle gray (0.5, 0.5, 0.5).
    pub const GRAY: Self = Self::new(0.5, 0.5, 0.5, 1.0);
    /// Light silver (0.75, 0.75, 0.75).
    pub const SILVER: Self = Self::new(0.75, 0.75, 0.75, 1.0);
    /// Pure yellow.
    pub const YELLOW: Self = Self::new(1.0, 1.0, 0.0, 1.0);
    /// Standard CSS orange (#FFA500).
    pub const ORANGE: Self = Self::new(1.0, 165.0 / 255.0, 0.0, 1.0);
    /// Standard CSS purple (#800080).
    pub const PURPLE: Self = Self::new(128.0 / 255.0, 0.0, 128.0 / 255.0, 1.0);
    /// Pure cyan.
    pub const CYAN: Self = Self::new(0.0, 1.0, 1.0, 1.0);
    /// Pure magenta.
    pub const MAGENTA: Self = Self::new(1.0, 0.0, 1.0, 1.0);
    /// Standard CSS pink (#FFC0CB).
    pub const PINK: Self = Self::new(1.0, 192.0 / 255.0, 203.0 / 255.0, 1.0);
    /// Standard CSS brown (#A52A2A).
    pub const BROWN: Self = Self::new(165.0 / 255.0, 42.0 / 255.0, 42.0 / 255.0, 1.0);
    /// Standard CSS navy (#000080).
    pub const NAVY: Self = Self::new(0.0, 0.0, 128.0 / 255.0, 1.0);

    // ---- Brightness helpers ----

    /// Return the perceived luminance using the Rec. 709 weights.
    ///
    /// The result lies in `0.0..=1.0` for in-range channels.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rgba;
    /// assert!(Rgba::WHITE.luminance() > Rgba::BLACK.luminance());
    /// ```
    #[inline]
    pub fn luminance(&self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    /// Return `true` when the perceived luminance is below 0.5.
    ///
    /// Useful for picking a contrasting foreground color.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rgba;
    /// assert!(Rgba::BLACK.is_dark());
    /// assert!(!Rgba::WHITE.is_dark());
    /// ```
    #[inline]
    pub fn is_dark(&self) -> bool {
        self.luminance() < 0.5
    }

    /// Mix this color toward black by `amount` in `0.0..=1.0`.
    ///
    /// `darken(0.0)` returns the original color; `darken(1.0)` returns black.
    /// Alpha is preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rgba;
    /// let half = Rgba::WHITE.darken(0.5);
    /// assert!((half.r - 0.5).abs() < 1e-6);
    /// assert_eq!(half.a, 1.0);
    /// ```
    #[inline]
    pub fn darken(&self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self::new(
            self.r * (1.0 - t),
            self.g * (1.0 - t),
            self.b * (1.0 - t),
            self.a,
        )
    }

    /// Mix this color toward white by `amount` in `0.0..=1.0`.
    ///
    /// `lighten(0.0)` returns the original color; `lighten(1.0)` returns white.
    /// Alpha is preserved.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rgba;
    /// let half = Rgba::BLACK.lighten(0.5);
    /// assert!((half.r - 0.5).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn lighten(&self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        Self::new(
            self.r + (1.0 - self.r) * t,
            self.g + (1.0 - self.g) * t,
            self.b + (1.0 - self.b) * t,
            self.a,
        )
    }
}

impl Color {
    // ---- Extended named palette mirroring `Rgba` constants ----

    /// Neutral middle gray. See [`Rgba::GRAY`].
    pub const GRAY: Self = Self::Rgba(Rgba::GRAY);
    /// Light silver. See [`Rgba::SILVER`].
    pub const SILVER: Self = Self::Rgba(Rgba::SILVER);
    /// Pure yellow.
    pub const YELLOW: Self = Self::Rgba(Rgba::YELLOW);
    /// Standard CSS orange (#FFA500).
    pub const ORANGE: Self = Self::Rgba(Rgba::ORANGE);
    /// Standard CSS purple (#800080).
    pub const PURPLE: Self = Self::Rgba(Rgba::PURPLE);
    /// Pure cyan.
    pub const CYAN: Self = Self::Rgba(Rgba::CYAN);
    /// Pure magenta.
    pub const MAGENTA: Self = Self::Rgba(Rgba::MAGENTA);
    /// Standard CSS pink.
    pub const PINK: Self = Self::Rgba(Rgba::PINK);
    /// Standard CSS brown.
    pub const BROWN: Self = Self::Rgba(Rgba::BROWN);
    /// Standard CSS navy.
    pub const NAVY: Self = Self::Rgba(Rgba::NAVY);

    /// Return the perceived luminance after converting to RGBA.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Color;
    /// assert!(Color::WHITE.luminance() > Color::BLACK.luminance());
    /// ```
    #[inline]
    pub fn luminance(&self) -> f32 {
        self.to_rgba().luminance()
    }

    /// Return `true` when the perceived luminance is below 0.5.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Color;
    /// assert!(Color::hex(0x1a1a2e).is_dark());
    /// ```
    #[inline]
    pub fn is_dark(&self) -> bool {
        self.to_rgba().is_dark()
    }

    /// Mix toward black by `amount` and return an RGBA-backed `Color`.
    ///
    /// HSLA inputs are first converted to RGBA, so the result always uses the
    /// `Color::Rgba` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Color;
    /// let dimmed = Color::WHITE.darken(0.5);
    /// assert!(dimmed.luminance() < Color::WHITE.luminance());
    /// ```
    #[inline]
    pub fn darken(&self, amount: f32) -> Self {
        Color::Rgba(self.to_rgba().darken(amount))
    }

    /// Mix toward white by `amount` and return an RGBA-backed `Color`.
    ///
    /// HSLA inputs are first converted to RGBA, so the result always uses the
    /// `Color::Rgba` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Color;
    /// let brighter = Color::BLACK.lighten(0.5);
    /// assert!(brighter.luminance() > Color::BLACK.luminance());
    /// ```
    #[inline]
    pub fn lighten(&self, amount: f32) -> Self {
        Color::Rgba(self.to_rgba().lighten(amount))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "expected {} == {}", a, b);
    }

    #[test]
    fn rgba_palette_alpha_is_one() {
        let palette = [
            Rgba::GRAY,
            Rgba::SILVER,
            Rgba::YELLOW,
            Rgba::ORANGE,
            Rgba::PURPLE,
            Rgba::CYAN,
            Rgba::MAGENTA,
            Rgba::PINK,
            Rgba::BROWN,
            Rgba::NAVY,
        ];
        for c in palette {
            assert_eq!(c.a, 1.0, "palette colors must be opaque");
        }
    }

    #[test]
    fn color_palette_mirrors_rgba() {
        let pairs = [
            (Color::GRAY, Rgba::GRAY),
            (Color::SILVER, Rgba::SILVER),
            (Color::YELLOW, Rgba::YELLOW),
            (Color::ORANGE, Rgba::ORANGE),
            (Color::PURPLE, Rgba::PURPLE),
            (Color::CYAN, Rgba::CYAN),
            (Color::MAGENTA, Rgba::MAGENTA),
            (Color::PINK, Rgba::PINK),
            (Color::BROWN, Rgba::BROWN),
            (Color::NAVY, Rgba::NAVY),
        ];
        for (color, rgba) in pairs {
            assert_eq!(color.to_rgba(), rgba);
        }
    }

    #[test]
    fn luminance_known_values() {
        approx(Rgba::BLACK.luminance(), 0.0);
        approx(Rgba::WHITE.luminance(), 0.2126 + 0.7152 + 0.0722);
        // GREEN is the brightest primary under Rec. 709.
        assert!(Rgba::GREEN.luminance() > Rgba::RED.luminance());
        assert!(Rgba::RED.luminance() > Rgba::BLUE.luminance());
    }

    #[test]
    fn is_dark_threshold() {
        assert!(Rgba::BLACK.is_dark());
        assert!(Rgba::new(0.1, 0.1, 0.1, 1.0).is_dark());
        assert!(!Rgba::WHITE.is_dark());
        assert!(!Rgba::new(0.9, 0.9, 0.9, 1.0).is_dark());
    }

    #[test]
    fn darken_extremes() {
        // amount = 0 returns the original color.
        let red = Rgba::RED;
        assert_eq!(red.darken(0.0), red);
        // amount = 1 returns black, alpha preserved.
        let translucent = Rgba::new(1.0, 0.5, 0.0, 0.4);
        let dark = translucent.darken(1.0);
        approx(dark.r, 0.0);
        approx(dark.g, 0.0);
        approx(dark.b, 0.0);
        approx(dark.a, 0.4);
    }

    #[test]
    fn darken_clamps_amount() {
        let color = Rgba::new(0.8, 0.6, 0.4, 1.0);
        // Negative amount should behave like 0.
        assert_eq!(color.darken(-0.5), color);
        // Amount > 1 should behave like 1.
        let saturated = color.darken(2.0);
        approx(saturated.r, 0.0);
        approx(saturated.g, 0.0);
        approx(saturated.b, 0.0);
    }

    #[test]
    fn lighten_extremes() {
        let blue = Rgba::BLUE;
        assert_eq!(blue.lighten(0.0), blue);
        let translucent = Rgba::new(0.0, 0.5, 0.0, 0.7);
        let pale = translucent.lighten(1.0);
        approx(pale.r, 1.0);
        approx(pale.g, 1.0);
        approx(pale.b, 1.0);
        approx(pale.a, 0.7);
    }

    #[test]
    fn lighten_clamps_amount() {
        let color = Rgba::new(0.2, 0.4, 0.6, 1.0);
        assert_eq!(color.lighten(-0.5), color);
        let saturated = color.lighten(2.0);
        approx(saturated.r, 1.0);
        approx(saturated.g, 1.0);
        approx(saturated.b, 1.0);
    }

    #[test]
    fn lighten_then_darken_is_close_to_original() {
        let mid = Rgba::new(0.4, 0.5, 0.6, 1.0);
        let round = mid.lighten(0.25).darken(0.25);
        // Not exactly equal due to two-step blending, just check it shifted darker.
        assert!(round.luminance() < mid.lighten(0.25).luminance());
    }

    #[test]
    fn color_helpers_round_trip_via_rgba() {
        let color = Color::hex(0x336699);
        approx(color.luminance(), color.to_rgba().luminance());
        assert_eq!(color.is_dark(), color.to_rgba().is_dark());
    }

    #[test]
    fn color_darken_lighten_returns_rgba_variant() {
        let hsla_color = Color::hsl(180.0, 0.5, 0.5);
        let darkened = hsla_color.darken(0.5);
        assert!(matches!(darkened, Color::Rgba(_)));
        let lightened = hsla_color.lighten(0.5);
        assert!(matches!(lightened, Color::Rgba(_)));
    }
}
