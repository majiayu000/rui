//! A resolved [`FontGlyph`] drawn at a pen position, without shaping.

use super::{GlyphSprite, PlacedGlyph, SUBPIXEL_VARIANTS, TextError, TextGlyphCache};
use crate::renderer::resources::{RendererResourceError, RendererResourceKind};
use crate::renderer::text::FontGlyph;
use crate::renderer::text_face_size::face_size;

impl TextGlyphCache {
    /// The sprite of `glyph` with its pen at `pen_x` logical pixels, at an em of
    /// `device_font_size` device pixels and `scale_factor`, rasterized at the
    /// subpixel offset the pen falls on, on first use. `baseline_y` is 0: the
    /// caller places the baseline. `None` for a glyph without ink, or one
    /// resolved in a font set that is gone.
    pub fn font_glyph_sprite(
        &mut self,
        glyph: &FontGlyph,
        pen_x: f32,
        device_font_size: f32,
        scale_factor: f32,
    ) -> Result<Option<GlyphSprite>, TextError> {
        if glyph.font_generation != self.font_generation {
            // Nothing shaped this frame yet to bring the font set up to date.
            self.measurer.sync_font_snapshot();
            self.invalidate_for_font_generation_change();
            if glyph.font_generation != self.font_generation {
                return Ok(None);
            }
        }
        let fonts = self.measurer.fonts.as_slice();
        let face = glyph.font_index as usize;
        if face >= fonts.len() {
            return Ok(None);
        }
        let placed = PlacedGlyph {
            font_index: glyph.font_index,
            glyph_id: glyph.glyph_id,
            x: 0.0,
            baseline_y: 0.0,
            font_scale: face_size(fonts, face, 1.0),
        };
        let variants = f32::from(SUBPIXEL_VARIANTS);
        let pen_x = (pen_x * scale_factor * variants).round() / variants;
        let whole_x = pen_x.floor();
        let subpixel = ((pen_x - whole_x) * variants).round() as u8;
        let Some(atlas_glyph) = self.glyph(&placed, device_font_size, subpixel)? else {
            return Ok(None);
        };
        let (slot, color) = (atlas_glyph.slot, atlas_glyph.color);
        let atlas = if color {
            &self.color_atlas
        } else {
            &self.atlas
        };
        let (page_width, page_height) = atlas.page_extent(slot.page).ok_or_else(|| {
            RendererResourceError::missing(RendererResourceKind::Glyph, slot.page as u32)
        })?;
        let rect = slot.rect;
        Ok(Some(GlyphSprite {
            x: (whole_x + atlas_glyph.left as f32) / scale_factor,
            width: rect.width as f32 / scale_factor,
            height: rect.height as f32 / scale_factor,
            baseline_y: 0.0,
            top: atlas_glyph.top,
            uv: [
                rect.x as f32 / page_width as f32,
                rect.y as f32 / page_height as f32,
                rect.width as f32 / page_width as f32,
                rect.height as f32 / page_height as f32,
            ],
            color,
            page: slot.page as u32,
        }))
    }
}
