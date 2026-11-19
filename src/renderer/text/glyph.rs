//! Characters resolved once to a face and a glyph, for grids that place every
//! glyph on its own cell, such as terminals.
//!
//! A [`Primitive::Text`](crate::renderer::Primitive::Text) shapes its whole
//! string, so a row drawn as one text is shaped again whenever one of its cells
//! changes, and glyphs advance by their own font's widths: a CJK or fallback
//! glyph pushes every later column off the grid. A [`GlyphResolver`] does the
//! font lookup and fallback once per character; the [`FontGlyph`] it returns is
//! drawn at a pen position of the caller's choosing with
//! [`Primitive::Glyph`](crate::renderer::Primitive::Glyph), which shapes nothing.

use super::{TextMeasureCache, TextRequest};
use crate::renderer::system_fonts::system_font_generation;
use crate::renderer::text_face_size::face_size;
use rusttype::Scale;
use std::collections::HashMap;

/// The em characters are shaped at to read their advance; advances scale
/// linearly with the em, so any size gives the same advance per em.
const RESOLVE_EM: f32 = 64.0;

/// A glyph of a face in the process font set.
///
/// It names the face by its place in the font set, so it holds only while that
/// set stays the same: one resolved before the set changed (a family loaded
/// on demand, a failed load retried) draws nothing. [`Self::is_current`] says
/// whether it still holds; resolve it again when it does not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontGlyph {
    pub(crate) font_index: u32,
    pub(crate) glyph_id: u16,
    pub(crate) font_generation: usize,
    /// Advance in ems.
    pub(crate) advance: f32,
}

impl FontGlyph {
    /// How far the pen moves past this glyph at an em of `font_size`.
    pub fn advance(&self, font_size: f32) -> f32 {
        self.advance * font_size
    }

    /// Whether the font set it was resolved in is still the process's.
    pub fn is_current(&self) -> bool {
        self.font_generation == system_font_generation()
    }
}

/// Resolves characters of one family and weight to [`FontGlyph`]s, falling back
/// to other faces as text does, and remembers each answer.
pub struct GlyphResolver {
    measurer: TextMeasureCache,
    family: Option<String>,
    weight: u16,
    resolved: HashMap<char, Option<FontGlyph>>,
    generation: usize,
}

impl GlyphResolver {
    /// A resolver for `font_family` (`None` for the default family) at `font_weight`.
    pub fn new(font_family: Option<&str>, font_weight: u16) -> Self {
        let measurer = TextMeasureCache::new();
        let generation = measurer.font_generation;
        Self {
            measurer,
            family: font_family.map(str::to_string),
            weight: font_weight,
            resolved: HashMap::new(),
            generation,
        }
    }

    /// The glyph `ch` is drawn with: from the requested face, else from the
    /// face text would fall back to. `None` when no face has it, when it shapes
    /// to no glyph, or when no font could be loaded. A character that shapes to
    /// several glyphs gets the first.
    pub fn resolve(&mut self, ch: char) -> Option<FontGlyph> {
        self.forget_old_fonts();
        if let Some(glyph) = self.resolved.get(&ch) {
            return *glyph;
        }
        let mut buffer = [0u8; 4];
        let content: &str = ch.encode_utf8(&mut buffer);
        let request = TextRequest::new(
            content,
            RESOLVE_EM,
            self.weight,
            self.family.as_deref(),
            1.0,
        );
        let glyph = self
            .measurer
            .shape_single_line(request)
            .ok()
            .and_then(|plan| plan.glyphs().first().cloned())
            .map(|glyph| FontGlyph {
                font_index: glyph.font_index as u32,
                glyph_id: glyph.glyph_id,
                font_generation: self.measurer.font_generation,
                advance: glyph.advance_width / RESOLVE_EM,
            });
        // Shaping may have loaded the font set again.
        self.forget_old_fonts();
        self.resolved.insert(ch, glyph);
        glyph
    }

    /// Ascent above the baseline and descent below it (negative) of the
    /// requested face at an em of `font_size`, as text lines are placed by;
    /// `None` when no font could be loaded.
    pub fn ascent_descent(&mut self, font_size: f32) -> Option<(f32, f32)> {
        let face = self
            .measurer
            .primary_font_index(self.family.as_deref(), self.weight)
            .ok()?;
        self.forget_old_fonts();
        let fonts = self.measurer.fonts.as_slice();
        let metrics = fonts[face]
            .font()
            .v_metrics(Scale::uniform(face_size(fonts, face, font_size)));
        Some((metrics.ascent, metrics.descent))
    }

    fn forget_old_fonts(&mut self) {
        if self.measurer.font_generation != self.generation {
            self.resolved.clear();
            self.generation = self.measurer.font_generation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_resolves_once_to_a_glyph_with_its_advance() {
        let mut resolver = GlyphResolver::new(None, 400);
        let Some(zero) = resolver.resolve('0') else {
            return; // No font on this machine.
        };
        assert!(zero.is_current());
        assert!(zero.advance(14.0) > 0.0);
        assert!((zero.advance(28.0) - 2.0 * zero.advance(14.0)).abs() < 1e-4);
        assert_eq!(resolver.resolve('0'), Some(zero));
        assert_eq!(resolver.resolved.len(), 1, "answered from memory");
        let (ascent, descent) = resolver.ascent_descent(14.0).expect("metrics");
        assert!(ascent > 0.0 && descent < 0.0);
    }
}
