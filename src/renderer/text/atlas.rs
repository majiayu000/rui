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

use super::{TextError, TextMeasureCache, TextMetrics, TextRequest, font_io_text_error};
use crate::renderer::resources::{
    RendererResourceError, RendererResourceKind, RendererResourceStats,
};
use rusttype::{GlyphId, Scale, point};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};
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

struct ShapedEntry {
    content: String,
    font_size: f32,
    font_weight: u16,
    font_family: Option<String>,
    line_height: f32,
    text: Option<Arc<ShapedText>>,
    last_used: u64,
}

impl ShapedEntry {
    fn matches(&self, request: &TextRequest<'_>) -> bool {
        self.content == request.content
            && self.font_size.to_bits() == request.font_size.to_bits()
            && self.font_weight == request.font_weight
            && self.font_family.as_deref() == request.font_family
            && self.line_height.to_bits() == request.line_height.to_bits()
    }
}

/// A multiply-rotate hash, as rustc's FxHash: far cheaper than the default
/// SipHash for the small keys looked up for every glyph drawn. Not resistant to
/// chosen collisions, which only cost time here: entries are compared in full.
#[derive(Default, Clone, Copy)]
struct FastHasher(u64);

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// Hashes a request without copying its text, so a hit allocates nothing.
fn shape_hash(request: &TextRequest<'_>) -> u64 {
    let mut hasher = FastHasher::default();
    request.content.hash(&mut hasher);
    request.font_size.to_bits().hash(&mut hasher);
    request.font_weight.hash(&mut hasher);
    request.font_family.hash(&mut hasher);
    request.line_height.to_bits().hash(&mut hasher);
    hasher.finish()
}

/// The bitmap of `glyph_id` at `device_font_size`, for fonts whose glyphs are
/// images (`sbix`, `CBDT`): its offset from the pen position
/// on the baseline, its size and its sRGB RGBA pixels with straight alpha.
/// `None` when the font has no image for the glyph. The strike closest above
/// the size is scaled down to it, in premultiplied alpha so transparent pixels
/// do not darken the edges.
#[allow(clippy::type_complexity)]
fn color_bitmap(
    font: &crate::renderer::text_shaping::TextShapingFont,
    glyph_id: u16,
    device_font_size: f32,
) -> Result<Option<(i32, i32, u32, u32, Vec<u8>)>, TextError> {
    use rustybuzz::ttf_parser::RasterImageFormat;
    // Font sizes here are rusttype's: the height from descent to ascent, as
    // outlines and shaping use them. Strikes are chosen by pixels per em.
    let unscaled = font.font().v_metrics_unscaled();
    let height_units = unscaled.ascent - unscaled.descent;
    if height_units <= 0.0 {
        return Ok(None);
    }
    let em = device_font_size * f32::from(font.font().units_per_em()) / height_units;
    let pixels_per_em = em.ceil().clamp(1.0, f32::from(u16::MAX)) as u16;
    let Some(image) = font.raster_image(glyph_id, pixels_per_em) else {
        return Ok(None);
    };
    let invalid = |reason: String| {
        TextError::Resource(RendererResourceError::invalid(
            RendererResourceKind::Glyph,
            reason,
        ))
    };
    if image.format != RasterImageFormat::PNG {
        return Err(invalid(format!(
            "glyph {glyph_id} is a {:?} bitmap; only PNG bitmaps are drawn",
            image.format
        )));
    }
    let decoded = image::load_from_memory_with_format(image.data, image::ImageFormat::Png)
        .map_err(|err| invalid(format!("glyph {glyph_id} bitmap does not decode: {err}")))?
        .to_rgba8();
    let scale = em / f32::from(image.pixels_per_em.max(1));
    let width = ((decoded.width() as f32 * scale).round() as u32).max(1);
    let height = ((decoded.height() as f32 * scale).round() as u32).max(1);
    let mut premultiplied = decoded;
    for pixel in premultiplied.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let mut scaled = image::imageops::resize(
        &premultiplied,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    for pixel in scaled.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            // Fully transparent pixels keep their (zero) color.
            if let Some(straight) = (u16::from(*channel) * 255 + alpha / 2).checked_div(alpha) {
                *channel = straight.min(255) as u8;
            }
        }
    }
    // The image's bottom left sits `x, y` strike pixels from the origin, y up.
    let left = (f32::from(image.x) * scale).round() as i32;
    let top = -((f32::from(image.y) * scale).round() as i32) - height as i32;
    Ok(Some((left, top, width, height, scaled.into_raw())))
}

pub struct TextGlyphCache {
    measurer: TextMeasureCache,
    font_generation: usize,
    frame: u64,
    shapes: FastMap<u64, ShapedEntry>,
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
        }
    }

    pub fn begin_frame(&mut self) {
        if self.shapes.len() > MAX_RETAINED_SHAPES {
            let previous = self.frame;
            let mut frames: Vec<u64> = self
                .shapes
                .values()
                .map(|entry| entry.last_used)
                .filter(|&frame| frame < previous)
                .collect();
            let evict = (self.shapes.len() - MAX_RETAINED_SHAPES * 3 / 4).min(frames.len());
            if evict > 0 {
                let (_, &mut newest_evicted, _) = frames.select_nth_unstable(evict - 1);
                let before = self.shapes.len();
                self.shapes
                    .retain(|_, entry| entry.last_used > newest_evicted);
                self.evicted_shapes += before - self.shapes.len();
            }
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
            let glyphs = parts
                .glyphs
                .iter()
                .filter_map(|glyph| {
                    let font = fonts.get(glyph.font_index)?;
                    let ascent = font.font().v_metrics(scale).ascent;
                    Some(PlacedGlyph {
                        font_index: glyph.font_index as u32,
                        glyph_id: glyph.glyph_id,
                        x: glyph.x_offset,
                        baseline_y: ascent + glyph.y_offset,
                    })
                })
                .collect();
            let v_metrics = fonts[primary_index].font().v_metrics(scale);
            Arc::new(ShapedText {
                metrics,
                glyphs,
                ascent: v_metrics.ascent,
                descent: v_metrics.descent,
            })
        });

        self.shapes.insert(
            hash,
            ShapedEntry {
                content: request.content.to_string(),
                font_size: request.font_size,
                font_weight: request.font_weight,
                font_family: request.font_family.map(str::to_string),
                line_height: request.line_height,
                text: text.clone(),
                last_used: self.frame,
            },
        );
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
            self.glyphs.clear();
            self.atlas.clear();
            self.uploads.clear();
            self.color_atlas.clear();
            self.color_uploads.clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_packs_glyphs_into_one_page_without_overlap() {
        let mut atlas = GlyphAtlas::new(64, usize::MAX);
        let slots = (0..20)
            .map(|_| atlas.allocate(10, 12).expect("allocate"))
            .collect::<Vec<_>>();

        assert_eq!(atlas.page_count(), 1);
        for (index, a) in slots.iter().enumerate() {
            assert!(a.rect.x + a.rect.width <= 64 && a.rect.y + a.rect.height <= 64);
            for b in &slots[index + 1..] {
                let apart = a.rect.x + a.rect.width + ATLAS_PADDING <= b.rect.x
                    || b.rect.x + b.rect.width + ATLAS_PADDING <= a.rect.x
                    || a.rect.y + a.rect.height + ATLAS_PADDING <= b.rect.y
                    || b.rect.y + b.rect.height + ATLAS_PADDING <= a.rect.y;
                assert!(apart, "{a:?} and {b:?} overlap");
            }
        }
    }

    #[test]
    fn atlas_adds_a_page_when_full_and_sizes_oversized_glyphs_to_fit() {
        let mut atlas = GlyphAtlas::new(32, usize::MAX);
        for _ in 0..4 {
            assert_eq!(atlas.allocate(15, 15).expect("allocate").page, 0);
        }
        assert_eq!(atlas.allocate(15, 15).expect("allocate").page, 1);

        let big = atlas.allocate(100, 40).expect("allocate");
        assert_eq!(big.page, 2);
        assert_eq!(atlas.page_extent(2), Some((101, 41)));
    }

    #[test]
    fn atlas_reports_pressure_instead_of_dropping_a_glyph() {
        let mut atlas = GlyphAtlas::new(32, 32 * 32);
        for _ in 0..4 {
            atlas.allocate(15, 15).expect("allocate");
        }
        let err = atlas
            .allocate(15, 15)
            .expect_err("second page is over budget");
        assert!(err.is_pressure());
        assert_eq!(atlas.page_count(), 1);
    }

    fn request(content: &str) -> TextRequest<'_> {
        TextRequest::new(content, 14.0, 400, None, 1.2)
    }

    #[test]
    fn shaped_text_and_glyphs_are_reused_across_frames() {
        let mut cache = TextGlyphCache::new();
        let draw_frame = |cache: &mut TextGlyphCache| {
            cache.begin_frame();
            let shaped = cache
                .shape(request("Hello atlas"))
                .expect("shape")
                .expect("ink");
            for glyph in &shaped.glyphs {
                cache.glyph(glyph, 28.0, 1).expect("glyph");
            }
            shaped
        };

        let first = draw_frame(&mut cache);
        let uploads = cache.take_uploads();
        assert!(!uploads.is_empty());
        let rasterized = cache.resource_stats().cache_misses;
        assert_eq!(cache.shape_misses(), 1);

        let second = draw_frame(&mut cache);
        assert!(Arc::ptr_eq(&first, &second), "shaping is kept");
        assert_eq!(cache.shape_misses(), 1);
        assert_eq!(cache.resource_stats().cache_misses, rasterized);
        assert!(
            cache.take_uploads().is_empty(),
            "nothing is rasterized again"
        );
        assert!(
            uploads
                .iter()
                .all(|upload| upload.pixels.iter().any(|&p| p > 0))
        );
    }

    #[test]
    fn glyph_variants_are_rasterized_separately() {
        let mut cache = TextGlyphCache::new();
        let shaped = cache.shape(request("l")).expect("shape").expect("ink");
        let glyph = shaped.glyphs[0];
        let a = cache.glyph(&glyph, 28.0, 0).expect("glyph").expect("ink");
        let b = cache.glyph(&glyph, 28.0, 2).expect("glyph").expect("ink");
        let c = cache.glyph(&glyph, 14.0, 0).expect("glyph").expect("ink");
        assert_ne!(a.slot, b.slot);
        assert_ne!(a.slot, c.slot);
        assert!(
            c.slot.rect.height < a.slot.rect.height,
            "device size is honoured"
        );
        assert_eq!(cache.glyph(&glyph, 28.0, 0).expect("glyph"), Some(a));
    }

    #[test]
    fn strings_drawn_last_frame_survive_shape_eviction() {
        let mut cache = TextGlyphCache::new();
        cache.begin_frame();
        for index in 0..=MAX_RETAINED_SHAPES as u64 {
            let entry = ShapedEntry {
                content: format!("old {index}"),
                font_size: 14.0,
                font_weight: 400,
                font_family: None,
                line_height: 1.2,
                text: None,
                last_used: cache.frame,
            };
            cache.shapes.insert(index, entry);
        }
        cache.begin_frame();
        let kept = cache.shape(request("kept")).expect("shape");
        let misses = cache.shape_misses();

        cache.begin_frame();
        cache.begin_frame();
        assert!(cache.shapes.len() <= MAX_RETAINED_SHAPES);
        let again = cache.shape(request("kept")).expect("shape");
        assert_eq!(
            cache.shape_misses(),
            misses,
            "string from the last frame is kept"
        );
        assert_eq!(kept, again);
    }

    /// A cache over an atlas of `pages` pages of 32 x 32, each holding four
    /// 15 x 15 sprites.
    fn small_cache(pages: usize) -> TextGlyphCache {
        TextGlyphCache::with_measurer(
            TextMeasureCache::new(),
            GlyphAtlas::new(32, pages * 32 * 32),
            usize::MAX,
        )
    }

    fn sprite(cache: &mut TextGlyphCache) -> Result<(AtlasSlot, u64), RendererResourceError> {
        cache.place_sprite(15, 15, vec![255; 15 * 15])
    }

    #[test]
    fn pages_unfinished_frames_drew_from_are_kept_until_they_finish() {
        let mut cache = small_cache(1);
        cache.begin_frame();
        let first = sprite(&mut cache).expect("room");
        for _ in 0..3 {
            sprite(&mut cache).expect("room");
        }
        cache.take_cleared_pages();
        assert!(cache.use_sprite(first.0, first.1));

        // The next frame needs room while the GPU may still sample the page.
        cache.begin_frame();
        let err = sprite(&mut cache).expect_err("the only page is in flight");
        assert!(err.is_pressure());

        // Once that frame is done, its page is emptied and reused.
        cache.begin_frame();
        cache.set_completed_frame(cache.frame() - 1);
        let reused = sprite(&mut cache).expect("an idle page");
        assert_eq!(reused.0.page, 0);
        assert_ne!(reused.1, first.1, "the page's generation moved");
        assert_eq!(cache.take_cleared_pages(), [0]);
        assert!(!cache.use_sprite(first.0, first.1), "the old slot is gone");
        assert!(cache.use_sprite(reused.0, reused.1));
    }

    #[test]
    fn the_frame_being_drawn_never_loses_a_page() {
        let mut cache = small_cache(1);
        cache.begin_frame();
        cache.begin_frame();
        cache.set_completed_frame(cache.frame() - 1);
        for _ in 0..4 {
            sprite(&mut cache).expect("room");
        }
        let err = sprite(&mut cache).expect_err("the page holds this frame's sprites");
        assert!(err.is_pressure());
        assert!(cache.take_cleared_pages().is_empty());
    }

    #[test]
    fn glyphs_on_an_evicted_page_are_rasterized_again() {
        let mut cache = TextGlyphCache::with_measurer(
            TextMeasureCache::new(),
            GlyphAtlas::new(64, 64 * 64),
            usize::MAX,
        );
        let shaped = cache.shape(request("ab")).expect("shape").expect("ink");
        // One frame draws the glyph and fills the rest of the only page.
        cache.begin_frame();
        let a = cache
            .glyph(&shaped.glyphs[0], 28.0, 0)
            .expect("glyph")
            .expect("ink");
        while cache.place(false, 15, 15).is_ok() {}
        cache.take_uploads();
        cache.take_cleared_pages();

        // Once it is done, the next frame's sprite empties the page.
        cache.begin_frame();
        cache.set_completed_frame(cache.frame() - 1);
        sprite(&mut cache).expect("an idle page");
        assert_eq!(cache.take_cleared_pages(), [0]);
        assert_eq!(
            cache.resource_stats().evicted_entries,
            1,
            "the glyph was evicted"
        );

        cache.take_uploads();
        let again = cache
            .glyph(&shaped.glyphs[0], 28.0, 0)
            .expect("glyph")
            .expect("ink");
        assert_eq!((again.left, again.top), (a.left, a.top));
        assert_eq!(cache.take_uploads().len(), 1, "rasterized again");
    }

    #[test]
    fn past_the_soft_page_count_idle_pages_are_reused_before_growing() {
        let mut cache = small_cache(usize::MAX / (32 * 32));
        cache.begin_frame();
        for _ in 0..SOFT_ATLAS_PAGES * 4 {
            sprite(&mut cache).expect("room");
        }
        assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES);
        cache.take_cleared_pages();

        // The next frame reuses every idle page before adding one.
        cache.begin_frame();
        cache.set_completed_frame(cache.frame() - 1);
        for _ in 0..SOFT_ATLAS_PAGES * 4 {
            sprite(&mut cache).expect("an idle page");
        }
        assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES, "no new page");
        assert_eq!(cache.take_cleared_pages().len(), SOFT_ATLAS_PAGES);
        // With none idle, the atlas grows.
        sprite(&mut cache).expect("a new page");
        assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES + 1);
    }

    #[test]
    fn bitmap_emoji_become_color_glyphs() {
        let mut cache = TextGlyphCache::new();
        let Some(shaped) = cache.shape(request("\u{1F600}")).expect("shape") else {
            return;
        };
        let glyph = shaped.glyphs[0];
        let family = &cache.measurer.fonts[glyph.font_index as usize].family;
        if !family.contains("Emoji") {
            // No color emoji font on this machine.
            return;
        }
        let emoji = cache
            .glyph(&glyph, 28.0, 0)
            .expect("glyph")
            .expect("an emoji has pixels");
        assert!(emoji.color, "{emoji:?}");
        let size = (emoji.slot.rect.width, emoji.slot.rect.height);
        // A 28 px font, descent to ascent, is about 21 px per em in Apple Color
        // Emoji, the size of its square images and of its advance.
        let advance = cache.measurer.fonts[glyph.font_index as usize]
            .font()
            .glyph(GlyphId(glyph.glyph_id))
            .scaled(Scale::uniform(28.0))
            .h_metrics()
            .advance_width;
        assert!(
            (size.0 as f32 - advance).abs() <= 1.0 && size.0 == size.1,
            "{size:?} against an advance of {advance}"
        );
        assert!(emoji.top < 0, "drawn above the baseline: {emoji:?}");
        let uploads = cache.take_color_uploads();
        assert_eq!(uploads.len(), 1);
        let pixels = &uploads[0].pixels;
        assert_eq!(pixels.len() as u32, size.0 * size.1 * 4);
        // Not gray: an emoji has color.
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[3] > 200 && p[0].abs_diff(p[2]) > 60)
        );
        assert!(
            cache.take_uploads().is_empty(),
            "nothing in the coverage atlas"
        );
        assert_eq!(cache.glyph(&glyph, 28.0, 0).expect("glyph"), Some(emoji));
    }

    #[test]
    fn glyph_limit_is_a_pressure_error() {
        let mut cache = TextGlyphCache::with_measurer(
            TextMeasureCache::new(),
            GlyphAtlas::new(64, usize::MAX),
            1,
        );
        let shaped = cache.shape(request("ab")).expect("shape").expect("ink");
        cache
            .glyph(&shaped.glyphs[0], 14.0, 0)
            .expect("first glyph fits");
        let err = cache
            .glyph(&shaped.glyphs[1], 14.0, 0)
            .expect_err("second glyph is over the limit");
        assert!(matches!(err, TextError::Resource(err) if err.is_pressure()));
    }
}
