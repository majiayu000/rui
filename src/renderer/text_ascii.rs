//! Which ASCII characters a font maps, read from its `cmap` once.
//!
//! Font fallback asks whether a font has each cluster it places; for ASCII text
//! that is one `cmap` binary search per character per shape.

use rustybuzz::Face;
use std::sync::OnceLock;

/// Bit `c` set when the font maps ASCII `c`, filled on first use.
#[derive(Debug, Clone, Default)]
pub(crate) struct AsciiCoverage(OnceLock<u128>);

impl AsciiCoverage {
    /// Whether `face`, the font this coverage belongs to, maps `byte`, an
    /// ASCII character.
    pub(crate) fn covers(&self, face: &Face<'_>, byte: u8) -> bool {
        let bits = self.0.get_or_init(|| {
            (0..128u8)
                .filter(|&byte| face.glyph_index(char::from(byte)).is_some())
                .fold(0, |bits, byte| bits | 1 << byte)
        });
        bits & 1 << byte != 0
    }
}
