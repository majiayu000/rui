//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::resources::{
    GlyphResourceKey, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceStats,
};
use crate::renderer::system_fonts::load_system_fonts;
use crate::renderer::text_shaping::{TextShapingFont, rasterize_with_plan, shape_with_fonts};
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::renderer::text_shaping::{
    TextCluster, TextDirection, TextRun, TextScript, TextShapeDiagnostic, TextShapePlan,
};

pub const TEXT_BOUNDS_TOLERANCE: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TextError {
    MissingFont,
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
}

/// Upper bound on retained measurement entries.
///
/// The cache is reused across frames, so text whose content changes every frame
/// (counters, timers, cursors) would otherwise grow this map for the lifetime of
/// the window. Reaching the bound drops every entry instead of maintaining LRU
/// bookkeeping: measurement stays correct, only the next frame re-measures.
const MAX_RETAINED_METRICS: usize = 4_096;

pub struct TextMeasureCache {
    fonts: Arc<Vec<TextShapingFont>>,
    metrics: HashMap<TextMeasureKey, TextMetrics>,
}

impl TextMeasureCache {
    pub fn new() -> Self {
        Self {
            fonts: load_system_fonts(),
            metrics: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn without_font() -> Self {
        Self {
            fonts: Arc::new(Vec::new()),
            metrics: HashMap::new(),
        }
    }

    /// Number of measurement entries currently retained across frames.
    #[cfg(test)]
    pub(crate) fn cached_metrics_len(&self) -> usize {
        self.metrics.len()
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
            return Ok(*metrics);
        }

        let metrics = self.shape_with_primary(request, primary_index).metrics();
        if self.metrics.len() >= MAX_RETAINED_METRICS {
            self.metrics.clear();
        }
        self.metrics.insert(key, metrics);
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

    fn primary_font_index(&self, font_family: Option<&str>) -> Result<usize, TextError> {
        if self.fonts.is_empty() {
            return Err(TextError::MissingFont);
        }

        let Some(family) = font_family
            .map(str::trim)
            .filter(|family| !family.is_empty())
        else {
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
}

impl Default for TextRasterCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
