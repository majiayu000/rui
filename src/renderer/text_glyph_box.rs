//! Glyph bounding boxes, read from the font once per glyph.
//!
//! rusttype's `positioned` asks ttf-parser for the glyph's bounding box, which
//! decodes the whole outline every time. Shaping asks for every glyph it places,
//! so the boxes are kept per font, in font units, and placed as rusttype places
//! them.

use rustybuzz::Face;
use rusttype::{Font, Point, Rect, point};
use std::sync::{Mutex, PoisonError};

/// Font-unit boxes by glyph id: `x_min, y_min, x_max, y_max`, `None` for a glyph
/// without an outline, missing until first asked.
#[derive(Debug, Default)]
pub(crate) struct GlyphBoxes(Mutex<Vec<Option<Option<[i16; 4]>>>>);

impl GlyphBoxes {
    /// The pixel box of glyph `id` of `font` at `font_size`, drawn at `position`:
    /// what `font.glyph(id).scaled(..).positioned(position).pixel_bounding_box()`
    /// returns. `face` is the same font, which holds its boxes.
    pub(crate) fn pixel_bounds(
        &self,
        font: &Font<'_>,
        face: &Face<'_>,
        id: u16,
        font_size: f32,
        position: Point<f32>,
    ) -> Option<Rect<i32>> {
        let [x_min, y_min, x_max, y_max] = self.font_units(face, id)?;
        // As `ScaledGlyph` scales for `Scale::uniform(font_size)`.
        let scale = font.scale_for_pixel_height(font_size);
        // As rusttype rounds: the fraction joins the box, the whole part offsets it.
        let (x_trunc, x_fract) = (position.x.trunc() as i32, position.x.fract());
        let (y_trunc, y_fract) = (position.y.trunc() as i32, position.y.fract());
        Some(Rect {
            min: point(
                x_trunc + (x_min as f32 * scale + x_fract).floor() as i32,
                y_trunc + (-y_max as f32 * scale + y_fract).floor() as i32,
            ),
            max: point(
                x_trunc + (x_max as f32 * scale + x_fract).ceil() as i32,
                y_trunc + (-y_min as f32 * scale + y_fract).ceil() as i32,
            ),
        })
    }

    fn font_units(&self, face: &Face<'_>, id: u16) -> Option<[i16; 4]> {
        let mut boxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let index = usize::from(id);
        if index >= boxes.len() {
            boxes.resize(index + 1, None);
        }
        *boxes[index].get_or_insert_with(|| {
            face.glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(id))
                .map(|rect| [rect.x_min, rect.y_min, rect.x_max, rect.y_max])
        })
    }
}
