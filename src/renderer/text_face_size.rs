//! The size each face of a string is drawn at.
//!
//! A requested `font_size` is the em, as in CSS, CoreText and GPUI. Faces are
//! scaled by the height from descent to ascent, which is a different share of the
//! em in every font: Arial's is 1.12 em, Hiragino Sans GB's 1.0, Kohinoor
//! Devanagari's 1.4. So every face, the requested one and each fallback, is drawn
//! at the height that gives it an em of `font_size`.

use crate::renderer::text_shaping::TextShapingFont;
use skrifa::raw::TableProvider;
use skrifa::raw::tables::os2::{Os2, SelectionFlags};

/// What a face's `head`, `hhea`, `OS/2` and `post` tables say about its size
/// and style.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FaceMetrics {
    /// Above the baseline, in font units.
    pub(crate) ascender: i16,
    /// Below the baseline, negative, in font units.
    pub(crate) descender: i16,
    pub(crate) units_per_em: u16,
    /// OS/2 weight class: 400 regular, 700 bold; 400 without `OS/2`.
    pub(crate) weight: u16,
    /// Italic or oblique in `OS/2`, or slanted in `post`.
    pub(crate) italic: bool,
}

impl FaceMetrics {
    /// `None` without `head` or `hhea`. The ascender and descender are the
    /// typographic ones when `OS/2` (version 4 and up) asks for them, else
    /// `hhea`'s, else, where `hhea` has zero, the typographic and then the
    /// Windows ones.
    pub(crate) fn read(font: &skrifa::Font) -> Option<Self> {
        let tables = font.tables();
        let units_per_em = tables.head().ok()?.units_per_em();
        let hhea = tables.hhea().ok()?;
        let os2 = tables.os2().ok();
        let version_4 = |flag| {
            os2.as_ref()
                .is_some_and(|os2| os2.version() >= 4 && os2.fs_selection().contains(flag))
        };
        let typo_metrics = version_4(SelectionFlags::USE_TYPO_METRICS);
        let os2_value = |read: fn(&Os2) -> i16| os2.as_ref().map_or(0, read);
        let line = |hhea_value: i16, typo: i16, windows: i16| {
            if typo_metrics {
                typo
            } else if hhea_value != 0 || os2.is_none() {
                hhea_value
            } else if typo != 0 {
                typo
            } else {
                windows
            }
        };
        let ascender = line(
            hhea.ascender().to_i16(),
            os2_value(|os2| os2.s_typo_ascender()),
            os2_value(|os2| os2.us_win_ascent() as i16),
        );
        let descender = line(
            hhea.descender().to_i16(),
            os2_value(|os2| os2.s_typo_descender()),
            os2_value(|os2| (os2.us_win_descent() as i16).wrapping_neg()),
        );
        let slanted = tables
            .post()
            .is_ok_and(|post| post.italic_angle().to_f32() != 0.0);
        let italic = os2
            .as_ref()
            .is_some_and(|os2| os2.fs_selection().contains(SelectionFlags::ITALIC))
            || version_4(SelectionFlags::OBLIQUE)
            || slanted;
        Some(Self {
            ascender,
            descender,
            units_per_em,
            weight: os2.as_ref().map_or(400, |os2| os2.us_weight_class()),
            italic,
        })
    }
}

/// The height `fonts[face]` is drawn at for an em of `font_size`.
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
            let zero = font.charmap().map_unicode('0').expect("a 0");
            let units = font.glyph_metrics().h_advance(zero);
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
