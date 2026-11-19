//! OpenType feature settings text is shaped with.

use std::sync::Arc;

/// One OpenType feature setting: `value` 0 turns the feature off, 1 on, and
/// larger values pick an alternate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFeature {
    pub tag: [u8; 4],
    pub value: u32,
}

impl FontFeature {
    pub const fn new(tag: [u8; 4], value: u32) -> Self {
        Self { tag, value }
    }
}

/// `features` as rustybuzz takes them, each over the whole text.
pub(crate) fn rustybuzz_features(features: &[FontFeature]) -> Vec<rustybuzz::Feature> {
    use rustybuzz::ttf_parser::Tag;
    let feature = |f: &FontFeature| rustybuzz::Feature::new(Tag::from_bytes(&f.tag), f.value, ..);
    features.iter().map(feature).collect()
}

/// A shared list of feature settings, applied in order over the font's
/// defaults. Empty, the default, shapes with the font's own features.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FontFeatures(Option<Arc<[FontFeature]>>);

impl FontFeatures {
    pub const NONE: Self = Self(None);

    pub fn new(features: impl IntoIterator<Item = FontFeature>) -> Self {
        let features: Arc<[FontFeature]> = features.into_iter().collect();
        Self((!features.is_empty()).then_some(features))
    }

    /// Contextual alternates (`calt`) and standard ligatures (`liga`) off, as
    /// terminals and many code editors draw `->` or `!=`.
    pub fn no_ligatures() -> Self {
        Self::new([FontFeature::new(*b"calt", 0), FontFeature::new(*b"liga", 0)])
    }

    pub fn as_slice(&self) -> &[FontFeature] {
        self.0.as_deref().unwrap_or(&[])
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
}
