//! What a kept shaping result was shaped from.

use super::super::hash::FastHasher;
use super::super::{BaseDirection, TextRequest};
use super::ShapedText;
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
