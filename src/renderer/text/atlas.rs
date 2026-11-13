//! Glyphs rasterized once at device scale and packed into shared A8 atlas pages,
//! plus shaping results kept across frames.
//!
//! The cache is backend independent: it hands out atlas positions and queues the
//! pixels of newly rasterized glyphs, which a renderer copies into its own page
//! textures with [`TextGlyphCache::take_uploads`].
//!
//! Pages are evicted whole, least recently drawn first, and only once the GPU is
//! done with every frame that drew from them: the renderer reports finished
//! frames with [`TextGlyphCache::set_completed_frame`]. Eviction happens when the
//! atlas would grow past [`SOFT_ATLAS_PAGES`] or past its byte or glyph limit. An
//! evicted page is emptied and reused; its glyphs are rasterized again when next
//! drawn, and the renderer clears its texture ([`TextGlyphCache::take_cleared_pages`]).

use super::hash::FastMap;
use super::{TextError, TextMeasureCache, TextMetrics, TextRequest, font_io_text_error};
use crate::renderer::resources::{
    RendererResourceError, RendererResourceKind, RendererResourceStats,
};
use color::color_bitmap;
pub use placed::GlyphSprite;
use placed::PlacedText;
use rusttype::{GlyphId, Scale, point};
use shaped_entry::{ShapedEntry, shape_hash};
use std::sync::Arc;

/// Side of a new atlas page in pixels. A glyph larger than this gets a page of its own.
pub const ATLAS_PAGE_SIZE: u32 = 1024;
/// Horizontal positions a glyph is rasterized at within one device pixel.
pub const SUBPIXEL_VARIANTS: u8 = 4;
/// Empty pixels kept between packed glyphs so linear sampling never reads a neighbour.
const ATLAS_PADDING: u32 = 1;
/// Pages the atlas grows to before it reuses idle ones.
pub const SOFT_ATLAS_PAGES: usize = 8;
/// Shaped strings kept across frames. Past this, `begin_frame` drops the least
/// recently drawn strings down to three quarters of it, never one the previous
/// frame drew, so a frame never evicts what it is still drawing.
const MAX_RETAINED_SHAPES: usize = 4_096;
/// Glyphs of the kept shaped strings, trimmed the same way. Each costs about
/// 70 bytes with its sprite, so this keeps them near 4.5 MB: long paragraphs
/// that change every frame would otherwise keep 60 MB under the count alone.
const MAX_RETAINED_SHAPE_GLYPHS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasSlot {
    pub page: usize,
    pub rect: AtlasRect,
}

#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    next_x: u32,
}

#[derive(Debug, Clone)]
struct AtlasPage {
    width: u32,
    height: u32,
    shelves: Vec<Shelf>,
    next_y: u32,
    /// The last frame that drew from this page.
    last_used: u64,
    /// Times this page was emptied; slots from before are gone.
    generation: u64,
}

impl AtlasPage {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shelves: Vec::new(),
            next_y: 0,
            last_used: 0,
            generation: 0,
        }
    }

    /// Shelf packing: the lowest shelf that fits, else a new shelf below the others.
    fn allocate(&mut self, width: u32, height: u32) -> Option<AtlasRect> {
        let padded_width = width + ATLAS_PADDING;
        let padded_height = height + ATLAS_PADDING;
        if padded_width > self.width || padded_height > self.height {
            return None;
        }

        let shelf = self
            .shelves
            .iter_mut()
            .filter(|shelf| {
                shelf.height >= padded_height && shelf.next_x + padded_width <= self.width
            })
            .min_by_key(|shelf| shelf.height);
        let shelf = match shelf {
            Some(shelf) => shelf,
            None => {
                if self.next_y + padded_height > self.height {
                    return None;
                }
                self.shelves.push(Shelf {
                    y: self.next_y,
                    height: padded_height,
                    next_x: 0,
                });
                self.next_y += padded_height;
                self.shelves.last_mut()?
            }
        };

        let rect = AtlasRect {
            x: shelf.next_x,
            y: shelf.y,
            width,
            height,
        };
        shelf.next_x += padded_width;
        Some(rect)
    }

    fn byte_size(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

/// Packs rectangles into pages, adding a page whenever the current ones are full.
#[derive(Debug, Clone)]
pub struct GlyphAtlas {
    page_size: u32,
    max_bytes: usize,
    /// 1 for coverage pages, 4 for RGBA color pages.
    bytes_per_pixel: usize,
    pages: Vec<AtlasPage>,
    /// Pages emptied since the renderer last took them.
    cleared: Vec<usize>,
    pressure_events: usize,
}

impl GlyphAtlas {
    /// An atlas of A8 coverage pages.
    pub fn new(page_size: u32, max_bytes: usize) -> Self {
        Self::with_bytes_per_pixel(page_size, max_bytes, 1)
    }

    fn with_bytes_per_pixel(page_size: u32, max_bytes: usize, bytes_per_pixel: usize) -> Self {
        Self {
            page_size,
            max_bytes,
            bytes_per_pixel,
            pages: Vec::new(),
            cleared: Vec::new(),
            pressure_events: 0,
        }
    }

    /// How many times `page` was emptied; a slot placed in an earlier
    /// generation no longer holds what was put there.
    pub fn page_generation(&self, page: usize) -> Option<u64> {
        self.pages.get(page).map(|page| page.generation)
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page_extent(&self, page: usize) -> Option<(u32, u32)> {
        self.pages.get(page).map(|page| (page.width, page.height))
    }

    pub fn byte_size(&self) -> usize {
        self.pages.iter().map(AtlasPage::byte_size).sum::<usize>() * self.bytes_per_pixel
    }

    /// Fails with resource pressure when a new page would exceed the byte budget;
    /// a glyph is never dropped silently.
    pub fn allocate(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<AtlasSlot, RendererResourceError> {
        Self::check_size(width, height)?;
        match self.allocate_in_pages(width, height) {
            Some(slot) => Ok(slot),
            None => self.add_page(width, height),
        }
    }

    fn check_size(width: u32, height: u32) -> Result<(), RendererResourceError> {
        if width == 0 || height == 0 {
            return Err(RendererResourceError::invalid(
                RendererResourceKind::Glyph,
                "atlas allocations must have a positive size",
            ));
        }
        Ok(())
    }

    /// Room in a page there already is.
    fn allocate_in_pages(&mut self, width: u32, height: u32) -> Option<AtlasSlot> {
        self.pages
            .iter_mut()
            .enumerate()
            .find_map(|(page, atlas_page)| {
                Some(AtlasSlot {
                    page,
                    rect: atlas_page.allocate(width, height)?,
                })
            })
    }

    /// A new page sized for the allocation, within the byte budget.
    fn add_page(&mut self, width: u32, height: u32) -> Result<AtlasSlot, RendererResourceError> {
        let page_width = self.page_size.max(width + ATLAS_PADDING);
        let page_height = self.page_size.max(height + ATLAS_PADDING);
        let requested_bytes = page_width as usize * page_height as usize * self.bytes_per_pixel;
        let active_bytes = self.byte_size();
        if active_bytes
            .checked_add(requested_bytes)
            .is_none_or(|bytes| bytes > self.max_bytes)
        {
            self.pressure_events += 1;
            return Err(RendererResourceError::ResourcePressure {
                kind: RendererResourceKind::Glyph,
                requested_bytes,
                max_bytes: self.max_bytes,
                active_bytes,
            });
        }

        let mut atlas_page = AtlasPage::new(page_width, page_height);
        let rect = atlas_page.allocate(width, height).ok_or_else(|| {
            RendererResourceError::invalid(
                RendererResourceKind::Glyph,
                "a new atlas page could not hold the glyph it was sized for",
            )
        })?;
        self.pages.push(atlas_page);
        Ok(AtlasSlot {
            page: self.pages.len() - 1,
            rect,
        })
    }

    fn touch(&mut self, page: usize, frame: u64) {
        if let Some(page) = self.pages.get_mut(page) {
            page.last_used = page.last_used.max(frame);
        }
    }

    /// The least recently drawn page that no frame after `completed`, nor the
    /// current `frame`, drew from, emptied for reuse.
    fn evict(&mut self, completed: u64, frame: u64) -> Option<usize> {
        let (index, page) = self
            .pages
            .iter_mut()
            .enumerate()
            .filter(|(_, page)| page.last_used <= completed && page.last_used < frame)
            .min_by_key(|(_, page)| page.last_used)?;
        page.shelves.clear();
        page.next_y = 0;
        page.generation += 1;
        self.cleared.push(index);
        Some(index)
    }

    fn clear(&mut self) {
        self.pages.clear();
        self.cleared.clear();
    }
}

/// One glyph of a shaped string, in logical pixels from the string's ink origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    pub font_index: u32,
    pub glyph_id: u16,
    pub x: f32,
    /// Baseline, measured down from the top of the line box.
    pub baseline_y: f32,
    /// Font size over the text's: 1, or what gives a fallback face the text's em.
    pub font_scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    pub metrics: TextMetrics,
    pub glyphs: Vec<PlacedGlyph>,
    /// Ascent and descent of the primary font, the same for any content, so
    /// lines can be placed on a common baseline.
    pub ascent: f32,
    pub descent: f32,
}

/// A rasterized glyph: where it sits in the atlas and its offset from the pen
/// position on the baseline, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasGlyph {
    pub slot: AtlasSlot,
    pub left: i32,
    pub top: i32,
    /// In the color atlas, as RGBA, rather than in the coverage atlas.
    pub color: bool,
}

/// Pixels of a newly rasterized glyph: one byte of coverage per pixel, or for
/// the color atlas, four bytes of sRGB RGBA with straight alpha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphUpload {
    pub slot: AtlasSlot,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_index: u32,
    glyph_id: u16,
    size_bits: u32,
    subpixel: u8,
}

pub struct TextGlyphCache {
    measurer: TextMeasureCache,
    font_generation: usize,
    frame: u64,
    shapes: FastMap<u64, ShapedEntry>,
    /// Glyphs of the shaped texts in `shapes`.
    shape_glyphs: usize,
    glyphs: FastMap<GlyphKey, Option<AtlasGlyph>>,
    max_glyphs: usize,
    atlas: GlyphAtlas,
    uploads: Vec<GlyphUpload>,
    /// Color glyphs, such as emoji, from bitmap fonts.
    color_atlas: GlyphAtlas,
    color_uploads: Vec<GlyphUpload>,
    epoch: u64,
    /// The newest frame the GPU is done with.
    completed_frame: u64,
    shape_misses: usize,
    glyph_hits: usize,
    glyph_misses: usize,
    evicted_shapes: usize,
    evicted_glyphs: usize,
    glyph_pressure_events: usize,
    /// Sprites of each text last drawn, by the address of its shaped text.
    placed: FastMap<usize, PlacedText>,
    /// Bumped whenever glyphs leave the atlas, which outdates every placement.
    glyph_generation: u64,
}

impl TextGlyphCache {
    pub fn new() -> Self {
        Self::with_limits(usize::MAX, usize::MAX)
    }

    /// `max_glyphs` bounds rasterized glyphs and `max_bytes` the atlas pages; going
    /// past either is a resource pressure error.
    pub fn with_limits(max_glyphs: usize, max_bytes: usize) -> Self {
        Self::with_measurer(
            TextMeasureCache::new(),
            GlyphAtlas::new(ATLAS_PAGE_SIZE, max_bytes),
            max_glyphs,
        )
    }

    fn with_measurer(measurer: TextMeasureCache, atlas: GlyphAtlas, max_glyphs: usize) -> Self {
        let font_generation = measurer.font_generation;
        Self {
            measurer,
            font_generation,
            frame: 0,
            shapes: FastMap::default(),
            shape_glyphs: 0,
            glyphs: FastMap::default(),
            max_glyphs,
            color_atlas: GlyphAtlas::with_bytes_per_pixel(atlas.page_size, atlas.max_bytes, 4),
            color_uploads: Vec::new(),
            atlas,
            uploads: Vec::new(),
            epoch: 0,
            completed_frame: 0,
            shape_misses: 0,
            glyph_hits: 0,
            glyph_misses: 0,
            evicted_shapes: 0,
            evicted_glyphs: 0,
            glyph_pressure_events: 0,
            placed: FastMap::default(),
            glyph_generation: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        if self.shapes.len() > MAX_RETAINED_SHAPES || self.shape_glyphs > MAX_RETAINED_SHAPE_GLYPHS
        {
            self.trim_shapes(MAX_RETAINED_SHAPES * 3 / 4, MAX_RETAINED_SHAPE_GLYPHS * 3 / 4);
        }
        self.frame += 1;
    }

    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    /// The frame being drawn, counted by `begin_frame`.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Reports that the GPU is done with every frame up to `frame`, so pages
    /// only those frames drew from may be evicted. Without reports, none are.
    pub fn set_completed_frame(&mut self, frame: u64) {
        self.completed_frame = self
            .completed_frame
            .max(frame.min(self.frame.saturating_sub(1)));
    }

    /// Pages emptied since the last call, whose textures must be cleared before
    /// this frame's uploads.
    pub fn take_cleared_pages(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.atlas.cleared)
    }

    /// The atlas of color glyphs, whose pages are RGBA.
    pub fn color_atlas(&self) -> &GlyphAtlas {
        &self.color_atlas
    }

    /// [`Self::take_cleared_pages`] for the color atlas.
    pub fn take_cleared_color_pages(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.color_atlas.cleared)
    }

    /// [`Self::take_uploads`] for the color atlas.
    pub fn take_color_uploads(&mut self) -> Vec<GlyphUpload> {
        std::mem::take(&mut self.color_uploads)
    }

    /// Whether a slot placed in `generation` still holds its pixels; marks its
    /// page drawn this frame if so.
    pub(crate) fn use_sprite(&mut self, slot: AtlasSlot, generation: u64) -> bool {
        let live = self.atlas.page_generation(slot.page) == Some(generation);
        if live {
            self.atlas.touch(slot.page, self.frame);
        }
        live
    }

    /// Changes whenever the atlas is emptied (after a font change); page textures
    /// built before then no longer match it.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// How many strings were shaped rather than found in the cache.
    pub fn shape_misses(&self) -> usize {
        self.shape_misses
    }

    pub fn take_uploads(&mut self) -> Vec<GlyphUpload> {
        std::mem::take(&mut self.uploads)
    }

    pub fn has_uploads(&self) -> bool {
        !self.uploads.is_empty()
    }

    pub fn resource_stats(&self) -> RendererResourceStats {
        RendererResourceStats {
            kind: RendererResourceKind::Glyph,
            live_entries: self.glyphs.len(),
            live_bytes: self.atlas.byte_size() + self.color_atlas.byte_size(),
            disposed_entries: 0,
            evicted_entries: self.evicted_shapes + self.evicted_glyphs,
            cache_hits: self.glyph_hits,
            cache_misses: self.glyph_misses,
            pressure_events: self.glyph_pressure_events + self.atlas.pressure_events,
        }
    }

    /// Shapes `request`, or returns the result kept from an earlier frame. `None`
    /// when the text draws no ink.
    pub fn shape(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<Option<Arc<ShapedText>>, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(None);
        }

        self.measurer.sync_font_snapshot();
        self.invalidate_for_font_generation_change();
        if let Some(error) = self.measurer.font_error.as_ref() {
            return Err(font_io_text_error(error));
        }

        let hash = shape_hash(&request);
        if let Some(entry) = self.shapes.get_mut(&hash)
            && entry.matches(&request)
        {
            entry.last_used = self.frame;
            return Ok(entry.text.clone());
        }

        let prepared = self.measurer.prepare_fonts(request.font_family);
        self.invalidate_for_font_generation_change();
        prepared?;
        let primary_index = self
            .measurer
            .primary_font_index_prepared(request.font_family, request.font_weight)?;
        let parts = self
            .measurer
            .shape_parts_with_primary(request, primary_index);
        let metrics = parts.metrics;
        self.shape_misses += 1;

        let text = (!metrics.ink_bounds.is_empty()).then(|| {
            let fonts = self.measurer.fonts.as_slice();
            let scale = Scale::uniform(request.font_size);
            let v_metrics = fonts[primary_index].font().v_metrics(scale);
            let glyphs =
                placed::place_glyphs(fonts, primary_index, &parts.glyphs, v_metrics.ascent);
            Arc::new(ShapedText {
                metrics,
                glyphs,
                ascent: v_metrics.ascent,
                descent: v_metrics.descent,
            })
        });

        self.shape_glyphs += text.as_ref().map_or(0, |text| text.glyphs.len());
        let replaced = self.shapes.insert(
            hash,
            ShapedEntry {
                content: request.content.to_string(),
                font_size: request.font_size,
                font_weight: request.font_weight,
                font_family: request.font_family.map(str::to_string),
                line_height: request.line_height,
                direction: request.direction,
                text: text.clone(),
                last_used: self.frame,
            },
        );
        self.shape_glyphs -= replaced.map_or(0, |entry| entry.glyphs());
        Ok(text)
    }

    /// The atlas entry for `glyph` at `device_font_size` pixels, offset right by
    /// `subpixel / SUBPIXEL_VARIANTS` of a pixel, rasterizing it on first use.
    /// `None` for glyphs without ink, such as spaces.
    pub fn glyph(
        &mut self,
        glyph: &PlacedGlyph,
        device_font_size: f32,
        subpixel: u8,
    ) -> Result<Option<AtlasGlyph>, TextError> {
        let device_font_size = device_font_size * glyph.font_scale;
        let key = GlyphKey {
            font_index: glyph.font_index,
            glyph_id: glyph.glyph_id,
            size_bits: device_font_size.to_bits(),
            subpixel,
        };
        if let Some(entry) = self.glyphs.get(&key) {
            self.glyph_hits += 1;
            if let Some(glyph) = entry {
                let atlas = if glyph.color {
                    &mut self.color_atlas
                } else {
                    &mut self.atlas
                };
                atlas.touch(glyph.slot.page, self.frame);
            }
            return Ok(*entry);
        }
        self.glyph_misses += 1;

        let font = self
            .measurer
            .fonts
            .get(glyph.font_index as usize)
            .ok_or(TextError::MissingFont)?;
        let positioned = font
            .font()
            .glyph(GlyphId(glyph.glyph_id))
            .scaled(Scale::uniform(device_font_size))
            .positioned(point(
                f32::from(subpixel) / f32::from(SUBPIXEL_VARIANTS),
                0.0,
            ));
        // A bitmap color font, as emoji fonts are, may also carry outlines, which
        // are only placeholders: its images come first.
        if let Some((left, top, width, height, pixels)) =
            color_bitmap(font, glyph.glyph_id, device_font_size)?
        {
            return self.insert_glyph(key, true, left, top, width, height, pixels);
        }
        let Some(bounds) = positioned.pixel_bounding_box() else {
            self.glyphs.insert(key, None);
            return Ok(None);
        };
        let width = bounds.width() as u32;
        let height = bounds.height() as u32;
        let mut pixels = vec![0u8; width as usize * height as usize];
        positioned.draw(|x, y, coverage| {
            if x < width && y < height {
                pixels[(y * width + x) as usize] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
        self.insert_glyph(
            key,
            false,
            bounds.min.x,
            bounds.min.y,
            width,
            height,
            pixels,
        )
    }

    /// Places a rasterized glyph in the coverage atlas, or the color atlas if
    /// `color`, and remembers it under `key`.
    #[allow(clippy::too_many_arguments)]
    fn insert_glyph(
        &mut self,
        key: GlyphKey,
        color: bool,
        left: i32,
        top: i32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<Option<AtlasGlyph>, TextError> {
        if self.glyphs.len() >= self.max_glyphs && !self.evict_page(color) {
            self.glyph_pressure_events += 1;
            let atlas = self.atlas_mut(color);
            return Err(TextError::Resource(
                RendererResourceError::ResourcePressure {
                    kind: RendererResourceKind::Glyph,
                    requested_bytes: pixels.len(),
                    max_bytes: atlas.max_bytes,
                    active_bytes: atlas.byte_size(),
                },
            ));
        }

        let slot = self.place(color, width, height)?;
        let entry = AtlasGlyph {
            slot,
            left,
            top,
            color,
        };
        let upload = GlyphUpload { slot, pixels };
        if color {
            self.color_uploads.push(upload);
        } else {
            self.uploads.push(upload);
        }
        self.glyphs.insert(key, Some(entry));
        Ok(Some(entry))
    }

    /// Places a coverage bitmap that is not a glyph, such as a rasterized path,
    /// in the atlas and queues its upload. Returns its slot and the page's
    /// generation: [`Self::use_sprite`] says whether it still holds the bitmap.
    pub(crate) fn place_sprite(
        &mut self,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<(AtlasSlot, u64), RendererResourceError> {
        let slot = self.place(false, width, height)?;
        self.uploads.push(GlyphUpload { slot, pixels });
        let generation = self.atlas.page_generation(slot.page).unwrap_or_default();
        Ok((slot, generation))
    }

    /// Room for a bitmap drawn this frame, in the color atlas if `color`: in a
    /// page there is, else in an idle page once the atlas has
    /// [`SOFT_ATLAS_PAGES`], else in a new page, else in an idle page when a new
    /// one is over budget.
    fn place(
        &mut self,
        color: bool,
        width: u32,
        height: u32,
    ) -> Result<AtlasSlot, RendererResourceError> {
        GlyphAtlas::check_size(width, height)?;
        let mut slot = self.atlas_mut(color).allocate_in_pages(width, height);
        if slot.is_none()
            && self.atlas_mut(color).page_count() >= SOFT_ATLAS_PAGES
            && self.evict_page(color)
        {
            slot = self.atlas_mut(color).allocate_in_pages(width, height);
        }
        let slot = match slot {
            Some(slot) => slot,
            None => match self.atlas_mut(color).add_page(width, height) {
                Ok(slot) => slot,
                Err(err) if err.is_pressure() && self.evict_page(color) => {
                    let atlas = self.atlas_mut(color);
                    match atlas.allocate_in_pages(width, height) {
                        Some(slot) => slot,
                        None => atlas.add_page(width, height)?,
                    }
                }
                Err(err) => return Err(err),
            },
        };
        let frame = self.frame;
        self.atlas_mut(color).touch(slot.page, frame);
        Ok(slot)
    }

    fn atlas_mut(&mut self, color: bool) -> &mut GlyphAtlas {
        if color {
            &mut self.color_atlas
        } else {
            &mut self.atlas
        }
    }

    /// Empties the least recently drawn page, of the color atlas if `color`,
    /// that the GPU is done with, forgetting its glyphs. False when every page
    /// is still in use.
    fn evict_page(&mut self, color: bool) -> bool {
        let (completed, frame) = (self.completed_frame, self.frame);
        let Some(page) = self.atlas_mut(color).evict(completed, frame) else {
            return false;
        };
        let before = self.glyphs.len();
        self.glyphs.retain(|_, entry| {
            entry.is_none_or(|glyph| glyph.color != color || glyph.slot.page != page)
        });
        self.evicted_glyphs += before - self.glyphs.len();
        self.glyph_generation += 1;
        let uploads = if color {
            &mut self.color_uploads
        } else {
            &mut self.uploads
        };
        uploads.retain(|upload| upload.slot.page != page);
        true
    }

    fn invalidate_for_font_generation_change(&mut self) {
        if self.measurer.font_generation != self.font_generation {
            self.shapes.clear();
            self.shape_glyphs = 0;
            self.glyphs.clear();
            self.atlas.clear();
            self.uploads.clear();
            self.color_atlas.clear();
            self.color_uploads.clear();
            self.placed.clear();
            self.glyph_generation += 1;
            self.epoch += 1;
            self.font_generation = self.measurer.font_generation;
        }
    }
}

impl Default for TextGlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

mod color;
mod placed;
mod shaped_entry;
#[cfg(test)]
mod tests;
