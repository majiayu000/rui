//! Glyph bounding boxes, read from the font once per glyph.
//!
//! rusttype's `positioned` asks ttf-parser for the glyph's bounding box, which
//! decodes the whole outline every time. Shaping asks for every glyph it places,
//! so the boxes are kept per font, in font units, and placed as rusttype places
//! them.

use crate::core::geometry::Bounds;
use rusttype::{Font, Point, Rect, point};
use rustybuzz::Face;
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

/// The pixel box around every glyph included, empty until one is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InkBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl InkBounds {
    pub(crate) fn empty() -> Self {
        Self {
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
        }
    }

    pub(crate) fn include(&mut self, bounds: rusttype::Rect<i32>) {
        self.min_x = self.min_x.min(bounds.min.x);
        self.min_y = self.min_y.min(bounds.min.y);
        self.max_x = self.max_x.max(bounds.max.x);
        self.max_y = self.max_y.max(bounds.max.y);
    }

    pub(crate) fn to_bounds(self) -> Bounds {
        if self.min_x < self.max_x && self.min_y < self.max_y {
            Bounds::from_xywh(
                self.min_x as f32,
                self.min_y as f32,
                (self.max_x - self.min_x) as f32,
                (self.max_y - self.min_y) as f32,
            )
        } else {
            Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)
        }
    }
}
