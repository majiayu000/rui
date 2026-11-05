//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::resources::{
    GlyphResourceKey, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceStats,
};
use crate::renderer::system_fonts::{
    FontLoadError, ResourceSnapshot, load_system_fonts, reload_system_fonts_for_family,
    system_font_generation,
};
use crate::renderer::text_shaping::{
    ShapedParts, TextShapingFont, best_face, rasterize_with_plan, shape_parts, shape_with_fonts,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

mod atlas;
mod lru;
mod rich;
mod wrap;

pub use crate::renderer::text_shaping::{
    TextCluster, TextDirection, TextRun, TextScript, TextShapeDiagnostic, TextShapePlan,
};
pub use atlas::{
    ATLAS_PAGE_SIZE, AtlasGlyph, AtlasRect, AtlasSlot, GlyphAtlas, GlyphUpload, PlacedGlyph,
    SUBPIXEL_VARIANTS, ShapedText, TextGlyphCache,
};
pub use rich::{RichLine, RichPiece, RichPieceText, RichSpan, RichWrapped};
pub use wrap::{TextLine, WrappedText};

pub const TEXT_BOUNDS_TOLERANCE: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TextError {
    MissingFont,
    UnsupportedFontFamily(String),
    /// Rich text spans that do not tile their content.
    InvalidSpans(String),
    Resource(RendererResourceError),
}

impl From<RendererResourceError> for TextError {
    fn from(value: RendererResourceError) -> Self {
        Self::Resource(value)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TextRequest<'a> {
    pub content: &'a str,
    pub font_size: f32,
    pub font_weight: u16,
    pub font_family: Option<&'a str>,
    pub line_height: f32,
}

impl<'a> TextRequest<'a> {
    pub fn new(
        content: &'a str,
        font_size: f32,
        font_weight: u16,
        font_family: Option<&'a str>,
        line_height: f32,
    ) -> Self {
        Self {
            content,
            font_size,
            font_weight,
            font_family,
            line_height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    pub size: Size,
    pub ink_bounds: Bounds,
    pub advance_width: f32,
}

impl TextMetrics {
    pub fn empty() -> Self {
        Self {
            size: Size::ZERO,
            ink_bounds: Bounds::from_xywh(0.0, 0.0, 0.0, 0.0),
            advance_width: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextRasterEntry {
    pub id: u32,
    pub metrics: TextMetrics,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextMeasureKey {
    content: String,
    size_bits: u32,
    line_height_bits: u32,
    font_weight: u16,
    font_family: String,
}

impl TextMeasureKey {
    fn from_request(request: TextRequest<'_>, resolved_family: &str) -> Self {
        Self {
            content: request.content.to_string(),
            size_bits: request.font_size.to_bits(),
            line_height_bits: request.line_height.to_bits(),
            font_weight: request.font_weight,
            font_family: resolved_family.to_string(),
        }
    }

    fn matches(&self, request: &TextRequest<'_>, resolved_family: &str) -> bool {
        self.content == request.content
            && self.size_bits == request.font_size.to_bits()
            && self.line_height_bits == request.line_height.to_bits()
            && self.font_weight == request.font_weight
            && self.font_family == resolved_family
    }

    fn hasher(&self) -> DefaultHasher {
        let mut hasher = DefaultHasher::new();
        self.content.hash(&mut hasher);
        self.size_bits.hash(&mut hasher);
        self.line_height_bits.hash(&mut hasher);
        self.font_weight.hash(&mut hasher);
        self.font_family.hash(&mut hasher);
        hasher
    }

    fn retained_bytes(&self) -> usize {
        self.content.len().saturating_add(self.font_family.len())
    }
}

/// Hashes like [`TextMeasureKey::hasher`] without copying the text.
fn request_hasher(request: &TextRequest<'_>, resolved_family: &str) -> DefaultHasher {
    let mut hasher = DefaultHasher::new();
    request.content.hash(&mut hasher);
    request.font_size.to_bits().hash(&mut hasher);
    request.line_height.to_bits().hash(&mut hasher);
    request.font_weight.hash(&mut hasher);
    resolved_family.hash(&mut hasher);
    hasher
}

/// Measurement entries the cache trims back to at each frame, taking only text
/// no frame since the previous one used.
const RETAINED_METRICS_TARGET: usize = 4_096;
const RETAINED_METRIC_BYTES_TARGET: usize = 1024 * 1024;
/// Hard bounds, held even within one frame. Text whose content changes every
/// frame (counters, timers, cursors) would otherwise grow the cache for the life
/// of the window; past these the least recently used entries are dropped.
const MAX_RETAINED_METRICS: usize = 16_384;
const MAX_RETAINED_METRIC_BYTES: usize = 4 * 1024 * 1024;

/// Family and weight pairs whose face is remembered; past this, faces are
/// looked up every time.
const MAX_REMEMBERED_FACES: usize = 64;

/// What a [`TextMeasureCache`] did since it was made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextCacheStats {
    /// Strings run through the shaper. Every measure or wrap miss costs at
    /// least one, except a rewrap, which reuses the paragraph's kept clusters.
    pub shapes: usize,
    pub measure_hits: usize,
    pub measure_misses: usize,
    /// Wraps answered by a kept result whose width range holds the width asked.
    pub wrap_hits: usize,
    /// Wraps computed again because no kept result held the width.
    pub wrap_misses: usize,
}

pub struct TextMeasureCache {
    fonts: Arc<Vec<TextShapingFont>>,
    fonts_retryable: bool,
    font_generation: usize,
    font_error: Option<FontLoadError>,
    refreshed_missing_families: HashSet<String>,
    /// Faces chosen for requested families and weights, so a measurement does
    /// not search the font set again. Emptied whenever the font set changes.
    primary_faces: Vec<(Option<String>, u16, usize)>,
    metrics: lru::FrameLru<TextMeasureKey, TextMetrics>,
    wraps: wrap::WrapCache,
    rich_wraps: rich::RichCache,
    stats: TextCacheStats,
    #[cfg(test)]
    font_refresh_enabled: bool,
}

impl TextMeasureCache {
    pub fn new() -> Self {
        let snapshot = load_system_fonts();
        let mut cache = Self::with_fonts(snapshot.resources, snapshot.retryable, true);
        cache.font_generation = snapshot.generation;
        cache.font_error = snapshot.error;
        cache
    }

    fn with_fonts(
        fonts: Arc<Vec<TextShapingFont>>,
        fonts_retryable: bool,
        #[cfg_attr(not(test), allow(unused_variables))] font_refresh_enabled: bool,
    ) -> Self {
        Self {
            fonts,
            fonts_retryable,
            font_generation: 0,
            font_error: None,
            refreshed_missing_families: HashSet::new(),
            primary_faces: Vec::new(),
            metrics: lru::FrameLru::new(
                RETAINED_METRICS_TARGET,
                RETAINED_METRIC_BYTES_TARGET,
                MAX_RETAINED_METRICS,
                MAX_RETAINED_METRIC_BYTES,
            ),
            wraps: wrap::WrapCache::new(),
            rich_wraps: rich::RichCache::new(),
            stats: TextCacheStats::default(),
            #[cfg(test)]
            font_refresh_enabled,
        }
    }

    #[cfg(test)]
    pub(crate) fn without_font() -> Self {
        Self::with_fonts(Arc::new(Vec::new()), false, false)
    }

    #[cfg(test)]
    pub(crate) fn retryable_without_font() -> Self {
        Self::with_fonts(Arc::new(Vec::new()), true, true)
    }

    #[cfg(test)]
    pub(crate) fn with_font_error_for_test(
        path: &str,
        kind: std::io::ErrorKind,
        message: &str,
    ) -> Self {
        let mut cache = Self::with_fonts(Arc::new(Vec::new()), true, false);
        cache.font_error = Some(FontLoadError {
            path: path.to_string(),
            kind,
            message: message.to_string(),
        });
        cache
    }

    /// Mark a frame boundary. Cached measurements and wraps are trimmed back to
    /// their target size from text that neither the frame that just ended nor
    /// this one has used.
    pub fn begin_frame(&mut self) {
        self.metrics.begin_frame();
        self.wraps.begin_frame();
        self.rich_wraps.begin_frame();
    }

    pub fn stats(&self) -> TextCacheStats {
        self.stats
    }

    /// Number of measurement entries currently retained across frames.
    #[cfg(test)]
    pub(crate) fn cached_metrics_len(&self) -> usize {
        self.metrics.len()
    }

    #[cfg(test)]
    pub(crate) fn cached_metric_bytes(&self) -> usize {
        self.metrics.bytes()
    }

    #[cfg(test)]
    pub(crate) fn metric_hits(&self) -> usize {
        self.stats.measure_hits
    }

    /// Whether this cache shares its font set with `other` instead of owning a
    /// separately parsed copy.
    #[cfg(test)]
    pub(crate) fn shares_fonts_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.fonts, &other.fonts)
    }

    #[cfg(test)]
    pub(crate) fn has_fonts(&self) -> bool {
        !self.fonts.is_empty()
    }

    pub fn measure_single_line(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<TextMetrics, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(TextMetrics::empty());
        }

        let primary_index = self.primary_font_index(request.font_family, request.font_weight)?;
        Ok(self.measure_prepared(request, primary_index))
    }

    /// [`Self::measure_single_line`] once fonts are prepared.
    fn measure_prepared(&mut self, request: TextRequest<'_>, primary_index: usize) -> TextMetrics {
        let family = self.fonts[primary_index].family.as_str();
        let hash = request_hasher(&request, family).finish();
        if let Some(metrics) = self
            .metrics
            .get_mut(hash, |key| key.matches(&request, family))
        {
            self.stats.measure_hits += 1;
            return *metrics;
        }

        self.stats.measure_misses += 1;
        let metrics = self.shape_counted(request, primary_index).metrics;
        let key = TextMeasureKey::from_request(request, &self.fonts[primary_index].family);
        self.retain_metrics(key, metrics);
        metrics
    }

    /// Shape, counting it in [`TextCacheStats::shapes`].
    fn shape_counted<'a>(
        &mut self,
        request: TextRequest<'a>,
        primary_index: usize,
    ) -> ShapedParts<'a> {
        self.stats.shapes += 1;
        self.shape_parts_with_primary(request, primary_index)
    }

    pub fn shape_single_line(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<TextShapePlan, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(TextShapePlan::empty());
        }

        let primary_index = self.primary_font_index(request.font_family, request.font_weight)?;
        Ok(self.shape_with_primary(request, primary_index))
    }

    fn shape_with_primary(&self, request: TextRequest<'_>, primary_index: usize) -> TextShapePlan {
        shape_with_fonts(self.fonts.as_slice(), primary_index, request)
    }

    /// Shaping without the plan's copied strings and diagnostics.
    fn shape_parts_with_primary<'a>(
        &self,
        request: TextRequest<'a>,
        primary_index: usize,
    ) -> ShapedParts<'a> {
        shape_parts(self.fonts.as_slice(), primary_index, request)
    }

    fn primary_font_index(
        &mut self,
        font_family: Option<&str>,
        font_weight: u16,
    ) -> Result<usize, TextError> {
        self.prepare_fonts(font_family)?;
        if let Some(&(.., index)) = self
            .primary_faces
            .iter()
            .find(|(family, weight, _)| *weight == font_weight && family.as_deref() == font_family)
        {
            return Ok(index);
        }
        let index = self.primary_font_index_prepared(font_family, font_weight)?;
        if self.primary_faces.len() < MAX_REMEMBERED_FACES {
            self.primary_faces
                .push((font_family.map(str::to_string), font_weight, index));
        }
        Ok(index)
    }

    fn prepare_fonts(&mut self, font_family: Option<&str>) -> Result<(), TextError> {
        let family = font_family
            .map(str::trim)
            .filter(|family| !family.is_empty());

        self.sync_font_snapshot();

        if let Some(error) = self.font_error.as_ref() {
            return Err(font_io_text_error(error));
        }

        if self.fonts.is_empty() {
            return Err(TextError::MissingFont);
        }

        if let Some(family) = family
            && !matches!(family, "system" | "default")
            && !self
                .fonts
                .iter()
                .any(|font| font.family.eq_ignore_ascii_case(family))
            && !self.refreshed_missing_families.contains(family)
        {
            let snapshot = reload_system_fonts_for_family(family)
                .map_err(|error| font_io_text_error(&error))?;
            if let Some(snapshot) = snapshot {
                self.apply_font_snapshot(snapshot);
                if let Some(error) = self.font_error.as_ref() {
                    return Err(font_io_text_error(error));
                }
                if !self.fonts_retryable
                    && !self
                        .fonts
                        .iter()
                        .any(|font| font.family.eq_ignore_ascii_case(family))
                {
                    self.refreshed_missing_families.insert(family.to_string());
                }
            }
        }

        Ok(())
    }

    /// The face of the requested family closest to `font_weight`; no family,
    /// `system` or `default` mean the first loaded font's family.
    fn primary_font_index_prepared(
        &self,
        font_family: Option<&str>,
        font_weight: u16,
    ) -> Result<usize, TextError> {
        let family = font_family
            .map(str::trim)
            .filter(|family| !family.is_empty() && !matches!(*family, "system" | "default"));
        let family = match family {
            Some(family) => family,
            None => self
                .fonts
                .first()
                .ok_or(TextError::MissingFont)?
                .family
                .as_str(),
        };
        best_face(&self.fonts, family, font_weight)
            .ok_or_else(|| TextError::UnsupportedFontFamily(family.to_string()))
    }

    fn apply_font_snapshot(&mut self, snapshot: ResourceSnapshot<TextShapingFont>) {
        if snapshot.generation != self.font_generation {
            self.clear_metrics();
            self.refreshed_missing_families.clear();
        }
        self.fonts = snapshot.resources;
        self.primary_faces.clear();
        self.fonts_retryable = snapshot.retryable;
        self.font_generation = snapshot.generation;
        self.font_error = snapshot.error;
    }

    fn sync_font_snapshot(&mut self) {
        if self.font_refresh_enabled()
            && (self.fonts.is_empty()
                || self.fonts_retryable
                || self.font_generation != system_font_generation())
        {
            self.apply_font_snapshot(load_system_fonts());
        }
    }

    fn retain_metrics(&mut self, key: TextMeasureKey, metrics: TextMetrics) {
        let bytes = key.retained_bytes();
        let hash = key.hasher().finish();
        self.metrics.insert(hash, key, metrics, bytes);
    }

    fn clear_metrics(&mut self) {
        self.metrics.clear();
        self.wraps.clear();
        self.rich_wraps.clear();
    }

    fn font_refresh_enabled(&self) -> bool {
        #[cfg(test)]
        {
            self.font_refresh_enabled
        }
        #[cfg(not(test))]
        {
            true
        }
    }
}

impl Default for TextMeasureCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextRasterCache {
    measurer: TextMeasureCache,
    font_generation: usize,
    resources: RendererResourceCache<GlyphResourceKey>,
    entries: HashMap<GlyphResourceKey, Arc<TextRasterEntry>>,
}

impl TextRasterCache {
    pub fn new() -> Self {
        let measurer = TextMeasureCache::new();
        let font_generation = measurer.font_generation;
        Self {
            measurer,
            font_generation,
            resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
            entries: HashMap::new(),
        }
    }

    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        let measurer = TextMeasureCache::new();
        let font_generation = measurer.font_generation;
        Self {
            measurer,
            font_generation,
            resources: RendererResourceCache::new(
                RendererResourceKind::Glyph,
                max_entries,
                max_bytes,
            ),
            entries: HashMap::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.resources.begin_frame();
    }

    pub fn resource_stats(&self) -> RendererResourceStats {
        self.resources.stats()
    }

    pub fn dispose(
        &mut self,
        key: &GlyphResourceKey,
    ) -> Result<crate::renderer::RendererResourceHandle, TextError> {
        self.entries.remove(key);
        Ok(self.resources.dispose(key)?)
    }

    pub fn resolve(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<Option<Arc<TextRasterEntry>>, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(None);
        }

        let key = GlyphResourceKey::new(
            request.content,
            request.font_size,
            request.font_weight,
            request.font_family,
            request.line_height,
        );
        self.measurer.sync_font_snapshot();
        self.invalidate_for_font_generation_change();
        if let Some(error) = self.measurer.font_error.as_ref() {
            return Err(font_io_text_error(error));
        }
        if let Some(entry) = self.entries.get(&key).cloned() {
            let allocation = self.resources.resolve(key.clone(), entry.pixels.len())?;
            self.drop_evicted(allocation.evicted);
            return Ok(Some(entry));
        }

        let prepared = self.measurer.prepare_fonts(request.font_family);
        self.invalidate_for_font_generation_change();
        prepared?;
        let primary_index = self
            .measurer
            .primary_font_index_prepared(request.font_family, request.font_weight)?;
        let plan = self.measurer.shape_with_primary(request, primary_index);
        let metrics = plan.metrics();
        if metrics.ink_bounds.is_empty() {
            return Ok(None);
        }

        let pixels = rasterize_with_plan(self.measurer.fonts.as_slice(), request, metrics, &plan);
        let allocation = self.resources.resolve(key.clone(), pixels.len())?;
        self.drop_evicted(allocation.evicted);

        let entry = Arc::new(TextRasterEntry {
            id: allocation.handle.id.as_u32(),
            metrics,
            pixels,
        });
        self.entries.insert(key, entry.clone());
        Ok(Some(entry))
    }

    fn drop_evicted(&mut self, evicted: Vec<GlyphResourceKey>) {
        for key in evicted {
            self.entries.remove(&key);
        }
    }

    fn invalidate_for_font_generation_change(&mut self) {
        if self.measurer.font_generation != self.font_generation {
            self.entries.clear();
            self.resources.clear();
            self.font_generation = self.measurer.font_generation;
        }
    }
}

fn font_io_text_error(error: &FontLoadError) -> TextError {
    TextError::Resource(RendererResourceError::invalid(
        RendererResourceKind::Glyph,
        format!(
            "failed to load font at {} ({:?}): {}",
            error.path, error.kind, error.message
        ),
    ))
}

impl Default for TextRasterCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
