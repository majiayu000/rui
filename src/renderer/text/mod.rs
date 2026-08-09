//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::resources::{
    GlyphResourceKey, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceStats,
};
use crate::renderer::system_fonts::{
    FontLoadError, ResourceSnapshot, load_system_fonts, reload_system_fonts_for_family,
};
use crate::renderer::text_shaping::{TextShapingFont, rasterize_with_plan, shape_with_fonts};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub use crate::renderer::text_shaping::{
    TextCluster, TextDirection, TextRun, TextScript, TextShapeDiagnostic, TextShapePlan,
};

pub const TEXT_BOUNDS_TOLERANCE: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TextError {
    MissingFont,
    FontIo {
        path: String,
        kind: std::io::ErrorKind,
        message: String,
    },
    UnsupportedFontFamily(String),
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

    fn retained_bytes(&self) -> usize {
        self.content.len().saturating_add(self.font_family.len())
    }
}

/// Upper bound on retained measurement entries.
///
/// The cache is reused across frames, so text whose content changes every frame
/// (counters, timers, cursors) would otherwise grow this map for the lifetime of
/// the window. Reaching the bound drops every entry instead of maintaining LRU
/// bookkeeping: measurement stays correct, only the next frame re-measures.
const MAX_RETAINED_METRICS: usize = 4_096;
const MAX_RETAINED_METRIC_BYTES: usize = 4 * 1024 * 1024;

pub struct TextMeasureCache {
    fonts: Arc<Vec<TextShapingFont>>,
    fonts_retryable: bool,
    font_generation: usize,
    font_error: Option<FontLoadError>,
    refreshed_missing_families: HashSet<String>,
    metrics: HashMap<TextMeasureKey, TextMetrics>,
    retained_metric_bytes: usize,
    #[cfg(test)]
    metric_hits: usize,
    #[cfg(test)]
    font_refresh_enabled: bool,
}

impl TextMeasureCache {
    pub fn new() -> Self {
        let snapshot = load_system_fonts();
        Self {
            fonts: snapshot.resources,
            fonts_retryable: snapshot.retryable,
            font_generation: snapshot.generation,
            font_error: snapshot.error,
            refreshed_missing_families: HashSet::new(),
            metrics: HashMap::new(),
            retained_metric_bytes: 0,
            #[cfg(test)]
            metric_hits: 0,
            #[cfg(test)]
            font_refresh_enabled: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn without_font() -> Self {
        Self {
            fonts: Arc::new(Vec::new()),
            fonts_retryable: false,
            font_generation: 0,
            font_error: None,
            refreshed_missing_families: HashSet::new(),
            metrics: HashMap::new(),
            retained_metric_bytes: 0,
            metric_hits: 0,
            font_refresh_enabled: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn retryable_without_font() -> Self {
        Self {
            fonts: Arc::new(Vec::new()),
            fonts_retryable: true,
            font_generation: 0,
            font_error: None,
            refreshed_missing_families: HashSet::new(),
            metrics: HashMap::new(),
            retained_metric_bytes: 0,
            metric_hits: 0,
            font_refresh_enabled: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_font_error_for_test(
        path: &str,
        kind: std::io::ErrorKind,
        message: &str,
    ) -> Self {
        Self {
            fonts: Arc::new(Vec::new()),
            fonts_retryable: true,
            font_generation: 0,
            font_error: Some(FontLoadError {
                path: path.to_string(),
                kind,
                message: message.to_string(),
            }),
            refreshed_missing_families: HashSet::new(),
            metrics: HashMap::new(),
            retained_metric_bytes: 0,
            metric_hits: 0,
            font_refresh_enabled: false,
        }
    }

    /// Number of measurement entries currently retained across frames.
    #[cfg(test)]
    pub(crate) fn cached_metrics_len(&self) -> usize {
        self.metrics.len()
    }

    #[cfg(test)]
    pub(crate) fn cached_metric_bytes(&self) -> usize {
        self.retained_metric_bytes
    }

    #[cfg(test)]
    pub(crate) fn metric_hits(&self) -> usize {
        self.metric_hits
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

        let primary_index = self.primary_font_index(request.font_family)?;
        let key = TextMeasureKey::from_request(request, &self.fonts[primary_index].family);
        if let Some(metrics) = self.metrics.get(&key) {
            #[cfg(test)]
            {
                self.metric_hits += 1;
            }
            return Ok(*metrics);
        }

        let metrics = self.shape_with_primary(request, primary_index).metrics();
        self.retain_metrics(key, metrics);
        Ok(metrics)
    }

    pub fn shape_single_line(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<TextShapePlan, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(TextShapePlan::empty());
        }

        let primary_index = self.primary_font_index(request.font_family)?;
        Ok(self.shape_with_primary(request, primary_index))
    }

    fn shape_with_primary(&self, request: TextRequest<'_>, primary_index: usize) -> TextShapePlan {
        shape_with_fonts(self.fonts.as_slice(), primary_index, request)
    }

    fn primary_font_index(&mut self, font_family: Option<&str>) -> Result<usize, TextError> {
        self.prepare_fonts(font_family)?;
        self.primary_font_index_prepared(font_family)
    }

    fn prepare_fonts(&mut self, font_family: Option<&str>) -> Result<(), TextError> {
        let family = font_family
            .map(str::trim)
            .filter(|family| !family.is_empty());

        if self.font_refresh_enabled() && (self.fonts.is_empty() || self.fonts_retryable) {
            self.apply_font_snapshot(load_system_fonts());
        }

        if let Some(error) = self.font_error.as_ref() {
            return Err(TextError::FontIo {
                path: error.path.clone(),
                kind: error.kind,
                message: error.message.clone(),
            });
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
            && let Some(snapshot) = reload_system_fonts_for_family(family)
        {
            self.refreshed_missing_families.insert(family.to_string());
            self.apply_font_snapshot(snapshot);
            if let Some(error) = self.font_error.as_ref() {
                return Err(TextError::FontIo {
                    path: error.path.clone(),
                    kind: error.kind,
                    message: error.message.clone(),
                });
            }
        }

        Ok(())
    }

    fn primary_font_index_prepared(&self, font_family: Option<&str>) -> Result<usize, TextError> {
        let family = font_family
            .map(str::trim)
            .filter(|family| !family.is_empty());

        let Some(family) = family else {
            return Ok(0);
        };

        if matches!(family, "system" | "default") {
            return Ok(0);
        }

        self.fonts
            .iter()
            .position(|font| font.family.eq_ignore_ascii_case(family))
            .ok_or_else(|| TextError::UnsupportedFontFamily(family.to_string()))
    }

    fn apply_font_snapshot(&mut self, snapshot: ResourceSnapshot<TextShapingFont>) {
        if snapshot.generation != self.font_generation {
            self.clear_metrics();
            self.refreshed_missing_families.clear();
        }
        self.fonts = snapshot.resources;
        self.fonts_retryable = snapshot.retryable;
        self.font_generation = snapshot.generation;
        self.font_error = snapshot.error;
    }

    fn retain_metrics(&mut self, key: TextMeasureKey, metrics: TextMetrics) {
        let key_bytes = key.retained_bytes();
        if key_bytes > MAX_RETAINED_METRIC_BYTES {
            return;
        }
        let exceeds_bytes = self
            .retained_metric_bytes
            .checked_add(key_bytes)
            .is_none_or(|bytes| bytes > MAX_RETAINED_METRIC_BYTES);
        if self.metrics.len() >= MAX_RETAINED_METRICS || exceeds_bytes {
            self.clear_metrics();
        }
        self.retained_metric_bytes += key_bytes;
        self.metrics.insert(key, metrics);
    }

    fn clear_metrics(&mut self) {
        self.metrics.clear();
        self.retained_metric_bytes = 0;
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
    resources: RendererResourceCache<GlyphResourceKey>,
    entries: HashMap<GlyphResourceKey, Arc<TextRasterEntry>>,
}

impl TextRasterCache {
    pub fn new() -> Self {
        Self {
            measurer: TextMeasureCache::new(),
            resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
            entries: HashMap::new(),
        }
    }

    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            measurer: TextMeasureCache::new(),
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
        if self.measurer.fonts_retryable {
            let previous_generation = self.measurer.font_generation;
            self.measurer.prepare_fonts(request.font_family)?;
            self.invalidate_for_font_generation_change(previous_generation);
        }
        if let Some(entry) = self.entries.get(&key).cloned() {
            let allocation = self.resources.resolve(key.clone(), entry.pixels.len())?;
            self.drop_evicted(allocation.evicted);
            return Ok(Some(entry));
        }

        let primary_index = self.measurer.primary_font_index(request.font_family)?;
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

    fn invalidate_for_font_generation_change(&mut self, previous_generation: usize) {
        if self.measurer.font_generation != previous_generation {
            self.entries.clear();
            self.resources.clear();
        }
    }
}

impl Default for TextRasterCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
