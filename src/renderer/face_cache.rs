//! What text measurement asks of a face again and again, found once.
//!
//! rusttype finds a glyph's box by walking its whole outline, and
//! `ScaledGlyph::positioned` does that on every call; choosing a font for a
//! cluster looks each character up in the face's `cmap`. Measuring text asks
//! both for the same few glyphs thousands of times, so the box in font units
//! and the face's ASCII coverage are kept here.

use rusttype::{Font, GlyphId, Point, Rect, Scale, point};
use rustybuzz::Face;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

/// A glyph's box in font units, `None` for a glyph with no outline.
type UnitBox = Option<(i16, i16, i16, i16)>;

#[derive(Default)]
pub(crate) struct FaceCache {
    boxes: Mutex<HashMap<u16, UnitBox>>,
    /// Bit `n` is set when the face maps ASCII character `n` to a glyph.
    ascii: OnceLock<u128>,
}

impl FaceCache {
    /// The pixel box of glyph `id` of `font` at `scale` placed at `position`,
    /// equal to `font.glyph(id).scaled(scale).positioned(position).pixel_bounding_box()`.
    pub(crate) fn pixel_bounds(
        &self,
        font: &Font<'_>,
        id: u16,
        scale: Scale,
        position: Point<f32>,
    ) -> Option<Rect<i32>> {
        let Font::Ref(face) = font else {
            return font
                .glyph(GlyphId(id))
                .scaled(scale)
                .positioned(position)
                .pixel_bounding_box();
        };
        let unit_box = *self
            .boxes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id)
            .or_insert_with(|| {
                face.glyph_bounding_box(GlyphId(id).into())
                    .map(|rect| (rect.x_min, rect.y_min, rect.x_max, rect.y_max))
            });
        let (x_min, y_min, x_max, y_max) = unit_box?;
        // As `Glyph::scaled` and `ScaledGlyph::pixel_bounds_at` compute it.
        let scale_y = font.scale_for_pixel_height(scale.y);
        let scale_x = scale_y * scale.x / scale.y;
        let (x_trunc, x_fract) = (position.x.trunc() as i32, position.x.fract());
        let (y_trunc, y_fract) = (position.y.trunc() as i32, position.y.fract());
        Some(Rect {
            min: point(
                x_trunc + (f32::from(x_min) * scale_x + x_fract).floor() as i32,
                y_trunc + (-f32::from(y_max) * scale_y + y_fract).floor() as i32,
            ),
            max: point(
                x_trunc + (f32::from(x_max) * scale_x + x_fract).ceil() as i32,
                y_trunc + (-f32::from(y_min) * scale_y + y_fract).ceil() as i32,
            ),
        })
    }

    /// Whether `face` maps `ch` to a glyph, for an ASCII `ch`.
    pub(crate) fn has_ascii(&self, face: &Face<'_>, ch: u8) -> bool {
        let ascii = self.ascii.get_or_init(|| {
            (0..128u8)
                .filter(|&ch| face.glyph_index(char::from(ch)).is_some())
                .fold(0, |mask, ch| mask | 1 << ch)
        });
        ch < 128 && ascii & 1 << ch != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_boxes_and_coverage_match_the_font() {
        for path in [
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Geneva.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let font = Font::try_from_bytes(&bytes).expect("font parses");
            let face = Face::from_slice(&bytes, 0).expect("face parses");
            let cache = FaceCache::default();
            for id in 0..font.glyph_count().min(400) as u16 {
                for (size, x, y) in [(14.0, 0.0, 10.5), (13.3, 17.25, 11.0), (30.0, -3.7, 0.4)] {
                    // Twice: the second answer comes from the cache.
                    for _ in 0..2 {
                        let position = point(x, y);
                        let expected = font
                            .glyph(GlyphId(id))
                            .scaled(Scale::uniform(size))
                            .positioned(position)
                            .pixel_bounding_box();
                        let found = cache.pixel_bounds(&font, id, Scale::uniform(size), position);
                        assert_eq!(found, expected, "{path} glyph {id} at {size}");
                    }
                }
            }
            for ch in 0..128u8 {
                assert_eq!(
                    cache.has_ascii(&face, ch),
                    face.glyph_index(char::from(ch)).is_some(),
                    "{path} {ch}"
                );
            }
        }
    }
}
