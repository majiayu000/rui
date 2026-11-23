//! Coverage masks of outline glyphs.

use crate::renderer::text_glyph_box::{PixelRect, draw_glyph};
use crate::renderer::text_shaping::TextShapingFont;
use glam::Vec2;

/// The pixel box of glyph `id` at `size` drawn at `position`, and its coverage
/// row by row, a byte per pixel. `None` for a glyph without an outline.
pub(super) fn coverage(
    font: &TextShapingFont,
    id: u16,
    size: f32,
    position: Vec2,
) -> Option<(PixelRect, Vec<u8>)> {
    let bounds = font.pixel_bounds(id, size, position)?;
    let width = bounds.width() as u32;
    let height = bounds.height() as u32;
    let mut pixels = vec![0u8; width as usize * height as usize];
    draw_glyph(font, id, size, position, bounds, |x, y, coverage| {
        if x < width && y < height {
            pixels[(y * width + x) as usize] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    });
    Some((bounds, pixels))
}
