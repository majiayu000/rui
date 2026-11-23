//! Glyph bounding boxes and outlines.
//!
//! Shaping asks for the box of every glyph it places, so the boxes are read
//! from the font once per glyph, in font units, and placed in pixels here. A
//! glyph is drawn by scaling its outline by the same factor, its y
//! flipped, into the pixel box, and rasterizing it with ab_glyph_rasterizer.

use crate::core::geometry::Bounds;
use crate::renderer::text_shaping::TextShapingFont;
use ab_glyph_rasterizer::{Point, Rasterizer};
use glam::Vec2;
use skrifa::GlyphId;
use skrifa::font::glyph::outline::{ControlBoundsSink, DrawMemory, DrawOptions, PathSink};
use std::sync::{Mutex, PoisonError};

/// A box of whole pixels: `min` inclusive, `max` exclusive, y down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub(crate) min_x: i32,
    pub(crate) min_y: i32,
    pub(crate) max_x: i32,
    pub(crate) max_y: i32,
}

impl PixelRect {
    pub(crate) fn width(&self) -> i32 {
        self.max_x - self.min_x
    }

    pub(crate) fn height(&self) -> i32 {
        self.max_y - self.min_y
    }
}

/// Font-unit boxes by glyph id: `x_min, y_min, x_max, y_max`, `None` for a glyph
/// without an outline, missing until first asked.
#[derive(Debug, Default)]
pub(crate) struct GlyphBoxes(Mutex<Vec<Option<Option<[i16; 4]>>>>);

impl GlyphBoxes {
    /// The pixel box of glyph `id` of `font` at `font_size`, drawn with its
    /// origin at `position`. The fraction of the position joins the box and the
    /// whole part offsets it, so a glyph's box at one subpixel phase is the same
    /// anywhere on the line.
    pub(crate) fn pixel_bounds(
        &self,
        font: &TextShapingFont,
        id: u16,
        font_size: f32,
        position: Vec2,
    ) -> Option<PixelRect> {
        let [x_min, y_min, x_max, y_max] = self.font_units(font, id)?;
        let scale = font.scale_for_pixel_height(font_size);
        let (x_trunc, x_fract) = (position.x.trunc() as i32, position.x.fract());
        let (y_trunc, y_fract) = (position.y.trunc() as i32, position.y.fract());
        Some(PixelRect {
            min_x: x_trunc + (x_min as f32 * scale + x_fract).floor() as i32,
            min_y: y_trunc + (-y_max as f32 * scale + y_fract).floor() as i32,
            max_x: x_trunc + (x_max as f32 * scale + x_fract).ceil() as i32,
            max_y: y_trunc + (-y_min as f32 * scale + y_fract).ceil() as i32,
        })
    }

    fn font_units(&self, font: &TextShapingFont, id: u16) -> Option<[i16; 4]> {
        let mut boxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let index = usize::from(id);
        if index >= boxes.len() {
            boxes.resize(index + 1, None);
        }
        *boxes[index].get_or_insert_with(|| font_units_box(font.font(), id))
    }
}

/// Glyph `id`'s outline box in font units: the box of the outline it is drawn
/// with, control points included. `None` when it has no outline or a box of
/// nothing; bitmaps are not measured. The box a `glyf` header states is not
/// used: it can be wrong, and the outline is placed by `hmtx`'s side bearing.
fn font_units_box(font: &skrifa::Font, id: u16) -> Option<[i16; 4]> {
    let glyph = font.outline_glyphs().get(GlyphId::new(u32::from(id)))?;
    let mut sink = ControlBoundsSink::new();
    glyph
        .draw(&DrawOptions::new(), DrawMemory::Scratch, &mut sink)
        .ok()?;
    let bounds = sink.bounding_box()?;
    let units = [
        bounds.x_min.floor() as i16,
        bounds.y_min.floor() as i16,
        bounds.x_max.ceil() as i16,
        bounds.y_max.ceil() as i16,
    ];
    (units != [0; 4]).then_some(units)
}

/// Draws glyph `id` of `font` at `font_size` with its origin at `position`,
/// calling `o(x, y, coverage)` for every pixel of `bounds`, its
/// [`TextShapingFont::pixel_bounds`], row by row from the box's top left.
pub(crate) fn draw_glyph(
    font: &TextShapingFont,
    id: u16,
    font_size: f32,
    position: Vec2,
    bounds: PixelRect,
    o: impl FnMut(u32, u32, f32),
) {
    let Some(glyph) = font
        .font()
        .outline_glyphs()
        .get(GlyphId::new(u32::from(id)))
    else {
        return;
    };
    let scale_y = font.scale_for_pixel_height(font_size);
    let mut sink = RasterSink {
        rasterizer: Rasterizer::new(bounds.width() as usize, bounds.height() as usize),
        scale: (scale_y * font_size / font_size, scale_y),
        offset: (
            position.x - bounds.min_x as f32,
            position.y - bounds.min_y as f32,
        ),
        last: Point { x: 0.0, y: 0.0 },
        last_move: None,
    };
    if glyph
        .draw(&DrawOptions::new(), DrawMemory::Scratch, &mut sink)
        .is_ok()
    {
        sink.rasterizer.for_each_pixel_2d(o);
    }
}

/// Font units scaled (y flipped), then offset into the pixel box.
struct RasterSink {
    rasterizer: Rasterizer,
    scale: (f32, f32),
    offset: (f32, f32),
    last: Point,
    last_move: Option<Point>,
}

impl RasterSink {
    fn point(&self, x: f32, y: f32) -> Point {
        Point {
            x: x * self.scale.0 + self.offset.0,
            y: y * -self.scale.1 + self.offset.1,
        }
    }
}

impl PathSink for RasterSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last = self.point(x, y);
        self.last_move = Some(self.last);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.rasterizer.draw_line(self.last, p);
        self.last = p;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.point(cx0, cy0), self.point(x, y));
        self.rasterizer.draw_quad(self.last, c, p);
        self.last = p;
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));
        self.rasterizer.draw_cubic(self.last, c0, c1, p);
        self.last = p;
    }

    fn close(&mut self) {
        if let Some(m) = self.last_move {
            self.rasterizer.draw_line(self.last, m);
        }
    }
}

/// The pixel box around every glyph included, empty until one is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InkBounds(PixelRect);

impl InkBounds {
    pub(crate) fn empty() -> Self {
        Self(PixelRect {
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
        })
    }

    pub(crate) fn include(&mut self, bounds: PixelRect) {
        let ink = &mut self.0;
        ink.min_x = ink.min_x.min(bounds.min_x);
        ink.min_y = ink.min_y.min(bounds.min_y);
        ink.max_x = ink.max_x.max(bounds.max_x);
        ink.max_y = ink.max_y.max(bounds.max_y);
    }

    pub(crate) fn to_bounds(self) -> Bounds {
        let ink = self.0;
        if ink.min_x < ink.max_x && ink.min_y < ink.max_y {
            Bounds::from_xywh(
                ink.min_x as f32,
                ink.min_y as f32,
                ink.width() as f32,
                ink.height() as f32,
            )
        } else {
            Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)
        }
    }
}
