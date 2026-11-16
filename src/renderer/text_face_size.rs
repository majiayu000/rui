//! The size each face of a string is drawn at.
//!
//! A requested `font_size` is the em, as in CSS, CoreText and GPUI. rusttype
//! scales by the height from descent to ascent, which is a different share of the
//! em in every font: Arial's is 1.12 em, Hiragino Sans GB's 1.0, Kohinoor
//! Devanagari's 1.4. So every face, the requested one and each fallback, is drawn
//! at the height that gives it an em of `font_size`.

use crate::renderer::text_shaping::TextShapingFont;

/// The rusttype height `fonts[face]` is drawn at for an em of `font_size`.
pub(crate) fn face_size(fonts: &[TextShapingFont], face: usize, font_size: f32) -> f32 {
    font_size * fonts[face].height_em()
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::renderer::system_fonts::load_system_fonts;
    use crate::renderer::text::{TextMeasureCache, TextRequest};

    #[test]
    fn every_face_gets_an_em_of_the_font_size() {
        let fonts = load_system_fonts().resources;
        let face = |family: &str| {
            fonts
                .iter()
                .position(|font| font.family == family)
                .unwrap_or_else(|| panic!("{family} is a macOS system font"))
        };
        let (arial, hiragino) = (face("Arial"), face("Hiragino Sans GB"));
        // Arial's height is 1.117 em and Hiragino's 1.0.
        assert!((face_size(&fonts, arial, 14.0) - 15.64).abs() < 0.01);
        assert!((face_size(&fonts, hiragino, 14.0) - 14.0).abs() < 0.01);
        let em = |face: usize, size: f32| size / fonts[face].height_em();
        assert!((em(arial, face_size(&fonts, arial, 14.0)) - 14.0).abs() < 1e-4);
    }

    /// `font_size` is the em: the advance of `0` is its advance in font units
    /// times `font_size / units_per_em`, in fonts whose ascent to descent spans
    /// different shares of the em (SF Mono 1.18, Hiragino Sans GB 1.0).
    #[test]
    fn font_size_is_the_em_size() {
        let fonts = load_system_fonts().resources;
        let mut cache = TextMeasureCache::new();
        for family in ["SF Mono", "Hiragino Sans GB"] {
            let font = fonts
                .iter()
                .find(|font| font.family == family)
                .unwrap_or_else(|| panic!("{family} is a macOS system font"))
                .font();
            let units = font
                .glyph('0')
                .scaled(rusttype::Scale::uniform(1.0))
                .h_metrics()
                .advance_width
                / font.scale_for_pixel_height(1.0);
            let want = units * 18.0 / f32::from(font.units_per_em());
            let advance = cache
                .measure_single_line(TextRequest::new("0", 18.0, 400, Some(family), 1.2))
                .unwrap_or_else(|err| panic!("{family}: {err:?}"))
                .advance_width;
            assert!(
                (advance - want).abs() < 0.01,
                "{family}: {advance} against {want}"
            );
        }
        let sf_mono = cache
            .measure_single_line(TextRequest::new("0", 18.0, 400, Some("SF Mono"), 1.2))
            .map(|metrics| metrics.advance_width);
        assert!(
            matches!(sf_mono, Ok(advance) if (advance - 11.13).abs() < 0.01),
            "{sf_mono:?}"
        );
    }
}
