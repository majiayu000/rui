//! The size each face of a string is drawn at.
//!
//! Font sizes here are rusttype's: the height from descent to ascent, which is a
//! different share of the em in every font. Arial's is 1.12 em, Hiragino Sans
//! GB's 1.0, Kohinoor Devanagari's 1.4. Drawn at the same height, a fallback
//! face's letters would come out up to a quarter larger or smaller than the
//! requested face's, so a fallback face is drawn at the height that gives it the
//! requested face's em, as CoreText and browsers size fallback text. The
//! requested face itself is drawn at the requested size, unchanged.

use crate::renderer::text_shaping::TextShapingFont;

/// The height `fonts[face]` is drawn at in text of `font_size` whose requested
/// face is `fonts[primary]`.
pub(crate) fn face_size(
    fonts: &[TextShapingFont],
    primary: usize,
    face: usize,
    font_size: f32,
) -> f32 {
    if face == primary {
        return font_size;
    }
    font_size * fonts[face].height_em() / fonts[primary].height_em()
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::renderer::system_fonts::load_system_fonts;

    #[test]
    fn a_fallback_face_gets_the_requested_faces_em() {
        let fonts = load_system_fonts().resources;
        let face = |family: &str| {
            fonts
                .iter()
                .position(|font| font.family == family)
                .unwrap_or_else(|| panic!("{family} is a macOS system font"))
        };
        let (arial, hiragino) = (face("Arial"), face("Hiragino Sans GB"));
        assert_eq!(face_size(&fonts, arial, arial, 14.0), 14.0);
        // Arial's height is 1.117 em and Hiragino's 1.0: 14 px of Arial is 12.5
        // px of em, which Hiragino draws at a height of 12.5 px.
        let size = face_size(&fonts, arial, hiragino, 14.0);
        assert!((size - 12.53).abs() < 0.01, "{size}");
        let em = |face: usize, size: f32| size / fonts[face].height_em();
        assert!((em(arial, 14.0) - em(hiragino, size)).abs() < 1e-4);
    }
}
