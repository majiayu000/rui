//! Display order of bidirectional text (UAX #9).
//!
//! Text is kept, edited and shaped in logical order. Each piece shaped on its
//! own has one embedding level, and the pieces of a line are placed in the
//! order rule L2 gives their levels; a piece at an odd level is shaped right to
//! left. Text with nothing that can start a right-to-left run skips all of it.

use crate::renderer::text::TextRequest;
use crate::renderer::text_glyph_box::InkBounds;
use crate::renderer::text_shaping::{ClusterDraft, GlyphDraft, ShapedRun, TextShapingFont};
use rusttype::{Scale, point};
use unicode_bidi::{BidiClass, BidiInfo, Level, bidi_class};

/// The resolved level of each byte of `text`, each paragraph laid out as one
/// line (rule L1 applied), or `None` when every level would be left to right.
pub(crate) fn line_levels(text: &str) -> Option<Vec<Level>> {
    // Nothing below U+0590 is right to left or starts a right-to-left run.
    let rtl = text.chars().any(|ch| {
        ch >= '\u{590}'
            && matches!(
                bidi_class(ch),
                BidiClass::R | BidiClass::AL | BidiClass::RLE | BidiClass::RLO | BidiClass::RLI
            )
    });
    if !rtl {
        return None;
    }
    let bidi = BidiInfo::new(text, None);
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
    request: TextRequest<'_>,
    glyphs: &[GlyphDraft],
) -> InkBounds {
    let scale = Scale::uniform(request.font_size);
    let mut ink = InkBounds::empty();
    for glyph in glyphs {
        let font = &fonts[glyph.font_index];
        let ascent = font.font().v_metrics(scale).ascent;
        let position = point(glyph.x_offset, ascent + glyph.y_offset);
        if let Some(bounds) = font.pixel_bounds(glyph.glyph_id, request.font_size, position) {
            ink.include(bounds);
        }
    }
    ink
}
