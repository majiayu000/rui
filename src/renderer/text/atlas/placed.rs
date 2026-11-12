//! A shaped text's glyphs resolved to atlas sprites, kept for the next frame.
//!
//! Looking every glyph up in the atlas each frame costs a hash lookup per glyph
//! drawn. A text drawn again at the same horizontal position, with none of the
//! atlas evicted since, reuses its sprites and only marks their pages drawn.

use super::{SUBPIXEL_VARIANTS, ShapedText, TextError, TextGlyphCache};
use crate::renderer::resources::{RendererResourceError, RendererResourceKind};
use std::sync::Arc;

/// One inked glyph of a placed text: its atlas sprite, left edge and size in
/// logical pixels; its vertical place is left to the line it is drawn on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphSprite {
    pub x: f32,
    pub width: f32,
    pub height: f32,
    /// The glyph's baseline from the line top, in logical pixels.
    pub baseline_y: f32,
    /// The bitmap's top from the baseline, in device pixels.
    pub top: i32,
    /// Position and size in the page, as fractions of the page.
    pub uv: [f32; 4],
    /// In the color atlas rather than the coverage atlas.
    pub color: bool,
    pub page: u32,
}

/// The sprites of one shaped text at one origin and scale.
pub(super) struct PlacedText {
    /// Held so the pointer this entry is keyed by is not reused.
    pub(super) text: Arc<ShapedText>,
    origin_x: u32,
    device_font_size: u32,
    scale_factor: u32,
    /// The cache's glyph generation the sprites were resolved in.
    generation: u64,
    sprites: Vec<GlyphSprite>,
    /// Each atlas page, coverage or color, the sprites are on.
    pages: Vec<(bool, usize)>,
}

impl TextGlyphCache {
    /// The sprites of `text`'s inked glyphs, its ink origin at `origin_x`
    /// logical pixels, drawn at `scale_factor` with glyphs `device_font_size`
    /// pixels high: each glyph rasterized at the subpixel offset its pen
    /// position falls on, on first use.
    pub fn sprites(
        &mut self,
        text: &Arc<ShapedText>,
        origin_x: f32,
        device_font_size: f32,
        scale_factor: f32,
    ) -> Result<&[GlyphSprite], TextError> {
        let key = Arc::as_ptr(text) as usize;
        let current = self.placed.get(&key).is_some_and(|placed| {
            placed.origin_x == origin_x.to_bits()
                && placed.device_font_size == device_font_size.to_bits()
                && placed.scale_factor == scale_factor.to_bits()
                && placed.generation == self.glyph_generation
        });
        if current {
            let placed = &self.placed[&key];
            for &(color, page) in &placed.pages {
                let atlas = if color {
                    &mut self.color_atlas
                } else {
                    &mut self.atlas
                };
                atlas.touch(page, self.frame);
            }
            self.glyph_hits += text.glyphs.len();
        } else {
            // A text drawn at more than one x is placed again each time; its
            // buffers are reused.
            let mut placed = self.placed.remove(&key).unwrap_or_else(|| PlacedText {
                text: Arc::clone(text),
                origin_x: 0,
                device_font_size: 0,
                scale_factor: 0,
                generation: 0,
                sprites: Vec::with_capacity(text.glyphs.len()),
                pages: Vec::new(),
            });
            self.place_text(&mut placed, origin_x, device_font_size, scale_factor)?;
            self.placed.insert(key, placed);
        }
        Ok(&self.placed[&key].sprites)
    }

    fn place_text(
        &mut self,
        placed: &mut PlacedText,
        origin_x: f32,
        device_font_size: f32,
        scale_factor: f32,
    ) -> Result<(), TextError> {
        let variants = f32::from(SUBPIXEL_VARIANTS);
        let PlacedText {
            text,
            sprites,
            pages,
            ..
        } = placed;
        sprites.clear();
        pages.clear();
        for glyph in &text.glyphs {
            let pen_x = ((origin_x + glyph.x) * scale_factor * variants).round() / variants;
            let whole_x = pen_x.floor();
            let subpixel = ((pen_x - whole_x) * variants).round() as u8;
            let Some(atlas_glyph) = self.glyph(glyph, device_font_size, subpixel)? else {
                continue;
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
            if !pages.contains(&(color, slot.page)) {
                pages.push((color, slot.page));
            }
            let rect = slot.rect;
            sprites.push(GlyphSprite {
                x: (whole_x + atlas_glyph.left as f32) / scale_factor,
                width: rect.width as f32 / scale_factor,
                height: rect.height as f32 / scale_factor,
                baseline_y: glyph.baseline_y,
                top: atlas_glyph.top,
                uv: [
                    rect.x as f32 / page_width as f32,
                    rect.y as f32 / page_height as f32,
                    rect.width as f32 / page_width as f32,
                    rect.height as f32 / page_height as f32,
                ],
                color,
                page: slot.page as u32,
            });
        }
        placed.origin_x = origin_x.to_bits();
        placed.device_font_size = device_font_size.to_bits();
        placed.scale_factor = scale_factor.to_bits();
        placed.generation = self.glyph_generation;
        Ok(())
    }
}
