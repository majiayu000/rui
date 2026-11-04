//! Glyphs rasterized once at device scale and packed into shared A8 atlas pages,
//! plus shaping results kept across frames.
//!
//! The cache is backend independent: it hands out atlas positions and queues the
//! pixels of newly rasterized glyphs, which a renderer copies into its own page
//! textures with [`TextGlyphCache::take_uploads`].

use super::{TextError, TextMeasureCache, TextMetrics, TextRequest, font_io_text_error};
use crate::renderer::resources::{
    RendererResourceError, RendererResourceKind, RendererResourceStats,
};
use rusttype::{GlyphId, Scale, point};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Side of a new atlas page in pixels. A glyph larger than this gets a page of its own.
pub const ATLAS_PAGE_SIZE: u32 = 1024;
/// Horizontal positions a glyph is rasterized at within one device pixel.
pub const SUBPIXEL_VARIANTS: u8 = 4;
/// Empty pixels kept between packed glyphs so linear sampling never reads a neighbour.
const ATLAS_PADDING: u32 = 1;
/// Shaped strings kept across frames. Past this, `begin_frame` keeps only the strings
/// the previous frame drew, so a frame never evicts what it is still drawing.
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
}

impl AtlasPage {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shelves: Vec::new(),
            next_y: 0,
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

/// Packs rectangles into A8 pages, adding a page whenever the current ones are full.
#[derive(Debug, Clone)]
pub struct GlyphAtlas {
    page_size: u32,
    max_bytes: usize,
    pages: Vec<AtlasPage>,
    pressure_events: usize,
}

impl GlyphAtlas {
    pub fn new(page_size: u32, max_bytes: usize) -> Self {
        Self {
            page_size,
            max_bytes,
            pages: Vec::new(),
            pressure_events: 0,
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page_extent(&self, page: usize) -> Option<(u32, u32)> {
        self.pages.get(page).map(|page| (page.width, page.height))
    }

    pub fn byte_size(&self) -> usize {
        self.pages.iter().map(AtlasPage::byte_size).sum()
    }

    /// Fails with resource pressure when a new page would exceed the byte budget;
    /// a glyph is never dropped silently.
    pub fn allocate(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<AtlasSlot, RendererResourceError> {
        if width == 0 || height == 0 {
            return Err(RendererResourceError::invalid(
                RendererResourceKind::Glyph,
                "atlas allocations must have a positive size",
            ));
        }
        for (page, atlas_page) in self.pages.iter_mut().enumerate() {
            if let Some(rect) = atlas_page.allocate(width, height) {
                return Ok(AtlasSlot { page, rect });
            }
        }

        let page_width = self.page_size.max(width + ATLAS_PADDING);
        let page_height = self.page_size.max(height + ATLAS_PADDING);
        let requested_bytes = page_width as usize * page_height as usize;
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

    fn clear(&mut self) {
        self.pages.clear();
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
}

/// A rasterized glyph: where it sits in the atlas and its offset from the pen
/// position on the baseline, in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasGlyph {
    pub slot: AtlasSlot,
    pub left: i32,
    pub top: i32,
}

/// Pixels of a newly rasterized glyph, one byte of coverage per pixel.
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

/// Hashes a request without copying its text, so a hit allocates nothing.
fn shape_hash(request: &TextRequest<'_>) -> u64 {
    let mut hasher = DefaultHasher::new();
    request.content.hash(&mut hasher);
    request.font_size.to_bits().hash(&mut hasher);
    request.font_weight.hash(&mut hasher);
    request.font_family.hash(&mut hasher);
    request.line_height.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub struct TextGlyphCache {
    measurer: TextMeasureCache,
    font_generation: usize,
    frame: u64,
    shapes: HashMap<u64, ShapedEntry>,
    glyphs: HashMap<GlyphKey, Option<AtlasGlyph>>,
    max_glyphs: usize,
    atlas: GlyphAtlas,
    uploads: Vec<GlyphUpload>,
    epoch: u64,
    shape_misses: usize,
    glyph_hits: usize,
    glyph_misses: usize,
    evicted_shapes: usize,
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
            shapes: HashMap::new(),
            glyphs: HashMap::new(),
            max_glyphs,
            atlas,
            uploads: Vec::new(),
            epoch: 0,
            shape_misses: 0,
            glyph_hits: 0,
            glyph_misses: 0,
            evicted_shapes: 0,
            glyph_pressure_events: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        if self.shapes.len() > MAX_RETAINED_SHAPES {
            let previous = self.frame;
            let before = self.shapes.len();
            self.shapes.retain(|_, entry| entry.last_used == previous);
            self.evicted_shapes += before - self.shapes.len();
        }
        self.frame += 1;
    }

    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
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
            live_bytes: self.atlas.byte_size(),
            disposed_entries: 0,
            evicted_entries: self.evicted_shapes,
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
            .primary_font_index_prepared(request.font_family)?;
        let plan = self.measurer.shape_with_primary(request, primary_index);
        let metrics = plan.metrics();
        self.shape_misses += 1;

        let text = (!metrics.ink_bounds.is_empty()).then(|| {
            let fonts = self.measurer.fonts.as_slice();
            let scale = Scale::uniform(request.font_size);
            let glyphs = plan
                .glyphs()
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
            Arc::new(ShapedText { metrics, glyphs })
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
        let Some(bounds) = positioned.pixel_bounding_box() else {
            self.glyphs.insert(key, None);
            return Ok(None);
        };

        if self.glyphs.len() >= self.max_glyphs {
            self.glyph_pressure_events += 1;
            return Err(TextError::Resource(
                RendererResourceError::ResourcePressure {
                    kind: RendererResourceKind::Glyph,
                    requested_bytes: (bounds.width() * bounds.height()) as usize,
                    max_bytes: self.atlas.max_bytes,
                    active_bytes: self.atlas.byte_size(),
                },
            ));
        }

        let width = bounds.width() as u32;
        let height = bounds.height() as u32;
        let mut pixels = vec![0u8; width as usize * height as usize];
        positioned.draw(|x, y, coverage| {
            if x < width && y < height {
                pixels[(y * width + x) as usize] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });

        let slot = self.atlas.allocate(width, height)?;
        let entry = AtlasGlyph {
            slot,
            left: bounds.min.x,
            top: bounds.min.y,
        };
        self.uploads.push(GlyphUpload { slot, pixels });
        self.glyphs.insert(key, Some(entry));
        Ok(Some(entry))
    }

    fn invalidate_for_font_generation_change(&mut self) {
        if self.measurer.font_generation != self.font_generation {
            self.shapes.clear();
            self.glyphs.clear();
            self.atlas.clear();
            self.uploads.clear();
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
