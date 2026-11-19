//! Planning a [`Primitive::Glyph`](crate::renderer::Primitive::Glyph): one
//! atlas sprite at its pen position, without shaping.

use super::build::bounds_rect;
use super::frame::{ItemKind, Pages};
use super::*;
use crate::core::color::Rgba;
use crate::core::geometry::Point;
use crate::renderer::text::FontGlyph;

impl<T: Clone> FramePlanner<T> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_glyph(
        &mut self,
        frame: &mut FrameData<T>,
        glyph: &FontGlyph,
        origin: Point,
        font_size: f32,
        color: Rgba,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        if font_size <= 0.0 {
            return Ok(());
        }
        let sprite = self
            .text_cache
            .font_glyph_sprite(glyph, origin.x, font_size * scale_factor, scale_factor)
            .map_err(|err| {
                RendererError::render_failed(format!("glyph rendering failed: {err:?}"))
            })?;
        let Some(sprite) = sprite else {
            return Ok(());
        };
        // Baselines on device pixels, as text lines are drawn.
        let pen_y = (origin.y * scale_factor).round();
        let gpu = GpuGlyph {
            bounds: [
                sprite.x,
                (pen_y + sprite.top as f32) / scale_factor,
                sprite.width,
                sprite.height,
            ],
            uv: sprite.uv,
            color: color.to_array(),
        };
        let rect = bounds_rect(gpu.bounds);
        let (kind, pages) = if sprite.color {
            (KIND_COLOR_GLYPH, Pages::color(sprite.page))
        } else {
            (KIND_GLYPH, Pages::coverage(sprite.page))
        };
        let inside = frame.clip_bounds[clip as usize]
            .is_none_or(|bounds| simple::inside(&bounds, simple::flipped(rect)));
        let kind = ItemKind::Sprites {
            kind,
            start: frame.glyphs.len() as u32,
            count: 1,
            pages,
            simple: !sprite.color && inside,
        };
        frame.glyphs.push(gpu);
        frame.push_item(kind, clip, rect, scale_factor);
        Ok(())
    }
}
