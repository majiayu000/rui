//! Display order of bidirectional text (UAX #9).
//!
//! Text is kept, edited and shaped in logical order. Each piece shaped on its
//! own has one embedding level, and the pieces of a line are placed in the
//! order rule L2 gives their levels; a piece at an odd level is shaped right to
//! left. Text with nothing that can start a right-to-left run skips all of it.

use crate::renderer::text::TextRequest;
use crate::renderer::text_face_size::face_size;
use crate::renderer::text_glyph_box::InkBounds;
use crate::renderer::text_shaping::{ClusterDraft, GlyphDraft, ShapedRun, TextShapingFont};
use rusttype::{Scale, point};
use unicode_bidi::{BidiClass, BidiInfo, Direction, Level, bidi_class, get_base_direction};

/// The base direction of the paragraph a piece of text belongs to (UAX #9
/// rules P2 and P3).
///
/// A line wrapped out of a paragraph is laid out at its paragraph's level, not
/// at a level found from the line's own first strong character.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BaseDirection {
    /// From the text's own first strong character; left to right without one.
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

impl BaseDirection {
    /// `Auto` resolved for `text` as a paragraph of its own.
    pub fn resolve(self, text: &str) -> Self {
        match self {
            Self::Auto if has_rtl(text) && get_base_direction(text) == Direction::Rtl => {
                Self::RightToLeft
            }
            Self::Auto => Self::LeftToRight,
            resolved => resolved,
        }
    }

    fn level(self) -> Option<Level> {
        match self {
            Self::Auto => None,
            Self::LeftToRight => Some(Level::ltr()),
            Self::RightToLeft => Some(Level::rtl()),
        }
    }
}

/// Whether `text` has anything that can start a right-to-left run.
fn has_rtl(text: &str) -> bool {
    // Nothing below U+0590 is right to left or starts a right-to-left run.
    text.chars().any(|ch| {
        ch >= '\u{590}'
            && matches!(
                bidi_class(ch),
                BidiClass::R | BidiClass::AL | BidiClass::RLE | BidiClass::RLO | BidiClass::RLI
            )
    })
}

/// The resolved level of each byte of `text` laid out as one line (rule L1
/// applied) at the paragraph level `base` gives, or `None` when every level
/// would be left to right.
pub(crate) fn line_levels(text: &str, base: BaseDirection) -> Option<Vec<Level>> {
    if base != BaseDirection::RightToLeft && !has_rtl(text) {
        return None;
    }
    let bidi = BidiInfo::new(text, base.level());
    let mut levels = bidi.levels.clone();
    for paragraph in &bidi.paragraphs {
        let range = paragraph.range.clone();
        levels[range.clone()]
            .copy_from_slice(&bidi.reordered_levels(paragraph, range.clone())[range]);
    }
    Some(levels)
}

/// Indices of pieces at `levels`, left to right on screen (rule L2).
pub(crate) fn visual_order(levels: &[u8]) -> Vec<usize> {
    let levels: Vec<Level> = levels
        .iter()
        .map(|&level| Level::new(level).expect("levels come from unicode-bidi"))
        .collect();
    BidiInfo::reorder_visual(&levels)
}

/// Move `runs`, shaped one after another in logical order, to their places
/// left to right on screen, with their clusters and glyphs.
pub(crate) fn place_visually(
    runs: &mut [ShapedRun],
    clusters: &mut [ClusterDraft],
    glyphs: &mut [GlyphDraft],
) {
    let levels: Vec<u8> = runs.iter().map(|run| run.bidi_level).collect();
    let mut x = 0.0;
    for index in visual_order(&levels) {
        let run = &mut runs[index];
        let shift = x - run.x_offset;
        for cluster in &mut clusters[run.clusters.clone()] {
            cluster.x_offset += shift;
        }
        for glyph in &mut glyphs[run.glyphs.clone()] {
            glyph.x_offset += shift;
        }
        run.x_offset = x;
        x += run.advance_width;
    }
}

/// The ink of `glyphs` where they are now, as shaping measures it.
pub(crate) fn ink_of(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
    glyphs: &[GlyphDraft],
) -> InkBounds {
    let baseline = fonts[primary_index]
        .font()
        .v_metrics(Scale::uniform(request.font_size))
        .ascent;
    let mut ink = InkBounds::empty();
    for glyph in glyphs {
        let font = &fonts[glyph.font_index];
        let size = face_size(fonts, primary_index, glyph.font_index, request.font_size);
        let position = point(glyph.x_offset, baseline + glyph.y_offset);
        if let Some(bounds) = font.pixel_bounds(glyph.glyph_id, size, position) {
            ink.include(bounds);
        }
    }
    ink
}
