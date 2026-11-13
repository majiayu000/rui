//! What a kept shaping result was shaped from, and trimming the kept results.

use super::super::hash::FastHasher;
use super::super::{BaseDirection, TextRequest};
use super::{ShapedText, TextGlyphCache};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub(super) struct ShapedEntry {
    pub(super) content: String,
    pub(super) font_size: f32,
    pub(super) font_weight: u16,
    pub(super) font_family: Option<String>,
    pub(super) line_height: f32,
    pub(super) direction: BaseDirection,
    pub(super) text: Option<Arc<ShapedText>>,
    pub(super) last_used: u64,
}

impl ShapedEntry {
    pub(super) fn glyphs(&self) -> usize {
        self.text.as_ref().map_or(0, |text| text.glyphs.len())
    }

    pub(super) fn matches(&self, request: &TextRequest<'_>) -> bool {
        self.content == request.content
            && self.font_size.to_bits() == request.font_size.to_bits()
            && self.font_weight == request.font_weight
            && self.font_family.as_deref() == request.font_family
            && self.line_height.to_bits() == request.line_height.to_bits()
            && self.direction == request.direction
    }
}

/// Hashes a request without copying its text, so a hit allocates nothing.
pub(super) fn shape_hash(request: &TextRequest<'_>) -> u64 {
    let mut hasher = FastHasher::default();
    request.content.hash(&mut hasher);
    request.font_size.to_bits().hash(&mut hasher);
    request.font_weight.hash(&mut hasher);
    request.font_family.hash(&mut hasher);
    request.line_height.to_bits().hash(&mut hasher);
    request.direction.hash(&mut hasher);
    hasher.finish()
}

impl TextGlyphCache {
    /// Drops the least recently drawn shaped strings until at most `entries`
    /// strings of at most `glyphs` glyphs are kept, never one the previous
    /// frame drew.
    pub(super) fn trim_shapes(&mut self, entries: usize, glyphs: usize) {
        let previous = self.frame;
        let mut old: Vec<(u64, usize)> = self
            .shapes
            .values()
            .filter(|entry| entry.last_used < previous)
            .map(|entry| (entry.last_used, entry.glyphs()))
            .collect();
        old.sort_unstable();
        let (mut kept, mut kept_glyphs) = (self.shapes.len(), self.shape_glyphs);
        let mut newest_evicted = None;
        for &(frame, entry_glyphs) in &old {
            if kept <= entries && kept_glyphs <= glyphs {
                break;
            }
            (kept, kept_glyphs) = (kept - 1, kept_glyphs - entry_glyphs);
            newest_evicted = Some(frame);
        }
        let Some(newest_evicted) = newest_evicted else {
            return;
        };
        let before = self.shapes.len();
        let mut evicted_glyphs = 0;
        self.shapes.retain(|_, entry| {
            let keep = entry.last_used > newest_evicted;
            if !keep {
                evicted_glyphs += entry.glyphs();
            }
            keep
        });
        self.shape_glyphs -= evicted_glyphs;
        self.evicted_shapes += before - self.shapes.len();
        // A placement whose text is no longer kept is never asked for again.
        self.placed
            .retain(|_, placed| Arc::strong_count(&placed.text) > 1);
    }
}
