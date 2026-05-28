//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::resources::{
    GlyphResourceKey, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceStats,
};
use crate::renderer::text_shaping::shape_with_font;
use rusttype::{Font, Scale, point};
use std::borrow::Cow;
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
}

impl TextMeasureKey {
    fn from_request(request: TextRequest<'_>) -> Result<Self, TextError> {
        resolve_font_family(request.font_family)?;
        Ok(Self {
            content: request.content.to_string(),
            size_bits: request.font_size.to_bits(),
            line_height_bits: request.line_height.to_bits(),
        })
    }
}

pub struct TextMeasureCache {
    font: Option<Font<'static>>,
    metrics: HashMap<TextMeasureKey, TextMetrics>,
}

impl TextMeasureCache {
    pub fn new() -> Self {
        Self {
            font: load_system_font(),
            metrics: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn without_font() -> Self {
        Self {
            font: None,
            metrics: HashMap::new(),
        }
    }

    pub fn measure_single_line(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<TextMetrics, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(TextMetrics::empty());
        }

        let key = TextMeasureKey::from_request(request)?;
        if let Some(metrics) = self.metrics.get(&key) {
            return Ok(*metrics);
        }

        let font = self.font.as_ref().ok_or(TextError::MissingFont)?;
        let metrics = measure_with_font(font, request);
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

        resolve_font_family(request.font_family)?;
        let font = self.font.as_ref().ok_or(TextError::MissingFont)?;
        Ok(shape_with_font(font, request))
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
        let metrics = self.measurer.measure_single_line(request)?;
        if metrics.ink_bounds.is_empty() {
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

        let pixels = rasterize_with_font(
            self.measurer.font.as_ref().ok_or(TextError::MissingFont)?,
            request,
            metrics,
        );
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

fn measure_with_font(font: &Font<'static>, request: TextRequest<'_>) -> TextMetrics {
    shape_with_font(font, request).metrics()
}

fn rasterize_with_font(
    font: &Font<'static>,
    request: TextRequest<'_>,
    metrics: TextMetrics,
) -> Vec<u8> {
    let width = metrics.ink_bounds.width().ceil().max(1.0) as u32;
    let height = metrics.ink_bounds.height().ceil().max(1.0) as u32;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    let scale = Scale::uniform(request.font_size);
    let v_metrics = font.v_metrics(scale);
    let raster_content = rasterizable_text(request.content);
    for glyph in font.layout(&raster_content, scale, point(0.0, v_metrics.ascent)) {
        if let Some(bb) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                let px = x as i32 + bb.min.x - metrics.ink_bounds.x() as i32;
                let py = y as i32 + bb.min.y - metrics.ink_bounds.y() as i32;
                if px < 0 || py < 0 {
                    return;
                }
                let px = px as u32;
                let py = py as u32;
                if px >= width || py >= height {
                    return;
                }
                let idx = ((py * width + px) * 4) as usize;
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = (v * 255.0) as u8;
            });
        }
    }

    pixels
}

fn rasterizable_text(content: &str) -> Cow<'_, str> {
    if content.chars().any(char::is_control) {
        Cow::Owned(content.chars().filter(|ch| !ch.is_control()).collect())
    } else {
        Cow::Borrowed(content)
    }
}

fn resolve_font_family(font_family: Option<&str>) -> Result<(), TextError> {
    match font_family
        .map(str::trim)
        .filter(|family| !family.is_empty())
    {
        None | Some("system") | Some("default") => Ok(()),
        Some(family) => Err(TextError::UnsupportedFontFamily(family.to_string())),
    }
}

fn load_system_font() -> Option<Font<'static>> {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Geneva.ttf",
        "/System/Library/Fonts/Monaco.ttf",
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];

    for path in candidates {
        if let Ok(bytes) = std::fs::read(path)
            && let Some(font) = Font::try_from_vec(bytes)
        {
            return Some(font);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(content: &str) -> TextRequest<'_> {
        TextRequest::new(content, 20.0, 400, None, 1.2)
    }

    fn measure(cache: &mut TextMeasureCache, request: TextRequest<'_>) -> TextMetrics {
        match cache.measure_single_line(request) {
            Ok(metrics) => metrics,
            Err(err) => panic!("text measurement failed: {:?}", err),
        }
    }

    fn shape(cache: &mut TextMeasureCache, request: TextRequest<'_>) -> TextShapePlan {
        match cache.shape_single_line(request) {
            Ok(plan) => plan,
            Err(err) => panic!("text shaping failed: {:?}", err),
        }
    }

    #[test]
    fn empty_text_measures_zero_without_font() {
        let mut cache = TextMeasureCache::without_font();
        let metrics = measure(&mut cache, request(""));
        assert_eq!(metrics, TextMetrics::empty());
    }

    #[test]
    fn missing_font_is_an_explicit_error() {
        let mut cache = TextMeasureCache::without_font();
        assert_eq!(
            cache.measure_single_line(request("Hello")),
            Err(TextError::MissingFont)
        );
    }

    #[test]
    fn unsupported_font_family_is_an_explicit_error() {
        let mut cache = TextMeasureCache::new();
        let result = cache.measure_single_line(TextRequest::new(
            "Hello",
            20.0,
            400,
            Some("Unknown Family"),
            1.2,
        ));
        let err = match result {
            Ok(metrics) => panic!("expected font family error, got {:?}", metrics),
            Err(err) => err,
        };
        assert_eq!(
            err,
            TextError::UnsupportedFontFamily("Unknown Family".to_string())
        );
    }

    #[test]
    fn same_count_wide_and_narrow_text_measure_differently() {
        let mut cache = TextMeasureCache::new();
        let narrow = measure(&mut cache, request("iiii"));
        let wide = measure(&mut cache, request("WWWW"));
        assert!(wide.size.width > narrow.size.width);
    }

    #[test]
    fn line_height_changes_height_not_width() {
        let mut cache = TextMeasureCache::new();
        let compact = measure(&mut cache, TextRequest::new("Hello", 18.0, 400, None, 1.0));
        let loose = measure(&mut cache, TextRequest::new("Hello", 18.0, 400, None, 1.8));

        assert_eq!(compact.size.width, loose.size.width);
        assert!((loose.size.height - 32.4).abs() < 0.01);
    }

    #[test]
    fn shaping_splits_mixed_script_runs_and_keeps_grapheme_clusters() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("A界e\u{301}"));

        assert_eq!(plan.clusters().len(), 3);
        assert_eq!(plan.clusters()[0].script, TextScript::Latin);
        assert_eq!(plan.clusters()[1].script, TextScript::Cjk);
        assert_eq!(plan.clusters()[2].text, "e\u{301}");
        assert_eq!(plan.clusters()[2].script, TextScript::Latin);
        assert!(plan.runs().iter().any(|run| run.script == TextScript::Cjk));
        assert_eq!(measure(&mut cache, request("A界e\u{301}")), plan.metrics());
    }

    #[test]
    fn shaping_marks_emoji_clusters_without_splitting_zwj_sequences() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("build 🧑‍💻"));

        assert!(plan.clusters().iter().any(|cluster| {
            cluster.text == "🧑‍💻" && cluster.script == TextScript::Emoji
        }));
    }

    #[test]
    fn shaping_reports_mixed_direction_as_observable_diagnostic() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("abc שלום"));

        assert_eq!(plan.direction(), TextDirection::Mixed);
        assert!(plan.runs().iter().any(|run| {
            run.direction == TextDirection::RightToLeft && run.script == TextScript::Rtl
        }));
        assert!(plan.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            TextShapeDiagnostic::MixedDirection {
                direction: TextDirection::Mixed
            }
        )));
    }

    #[test]
    fn shaping_recognizes_non_latin_ltr_scripts() {
        let mut cache = TextMeasureCache::new();

        let cyrillic = shape(&mut cache, request("Привет"));
        assert_eq!(cyrillic.direction(), TextDirection::LeftToRight);

        let mixed = shape(&mut cache, request("Привет שלום"));
        assert_eq!(mixed.direction(), TextDirection::Mixed);
    }

    #[test]
    fn shaping_treats_emoji_as_neutral_for_bidi_diagnostics() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("שלום 🙂"));

        assert_eq!(plan.direction(), TextDirection::RightToLeft);
        assert!(plan.clusters().iter().any(|cluster| {
            cluster.text == "🙂" && cluster.direction == TextDirection::Neutral
        }));
        assert!(
            !plan
                .diagnostics()
                .iter()
                .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MixedDirection { .. }))
        );
    }

    #[test]
    fn shaping_treats_arabic_indic_digits_as_numeric_neutral() {
        let mut cache = TextMeasureCache::new();

        let digits = shape(&mut cache, request("١٢٣"));
        assert_eq!(digits.direction(), TextDirection::Neutral);
        assert!(
            digits
                .clusters()
                .iter()
                .all(|cluster| cluster.script == TextScript::Number)
        );

        let latin_with_digits = shape(&mut cache, request("abc ١٢٣"));
        assert_eq!(latin_with_digits.direction(), TextDirection::LeftToRight);
        assert!(
            !latin_with_digits
                .diagnostics()
                .iter()
                .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MixedDirection { .. }))
        );
    }

    #[test]
    fn shaping_recognizes_rtl_scripts_from_bidi_properties() {
        let mut cache = TextMeasureCache::new();

        let adlam = shape(&mut cache, request("\u{1e900}\u{1e901}"));
        assert_eq!(adlam.direction(), TextDirection::RightToLeft);
        assert!(
            adlam
                .clusters()
                .iter()
                .all(|cluster| cluster.script == TextScript::Rtl)
        );

        let old_hungarian = shape(&mut cache, request("\u{10c80}\u{10c81}"));
        assert_eq!(old_hungarian.direction(), TextDirection::RightToLeft);

        let mixed = shape(&mut cache, request("abc \u{1e900}"));
        assert_eq!(mixed.direction(), TextDirection::Mixed);
        assert!(mixed.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic,
            TextShapeDiagnostic::MixedDirection {
                direction: TextDirection::Mixed
            }
        )));
    }

    #[test]
    fn rasterization_filters_control_characters_like_measurement() {
        let mut cache = TextRasterCache::new();
        let with_control = match cache.resolve(request("A\tW")) {
            Ok(Some(entry)) => entry,
            Ok(None) => panic!("expected rasterized control-character text entry"),
            Err(err) => panic!("text rasterization failed: {:?}", err),
        };
        let without_control = match cache.resolve(request("AW")) {
            Ok(Some(entry)) => entry,
            Ok(None) => panic!("expected rasterized filtered text entry"),
            Err(err) => panic!("text rasterization failed: {:?}", err),
        };

        assert_eq!(with_control.metrics, without_control.metrics);
        assert_eq!(with_control.pixels, without_control.pixels);
    }

    #[test]
    fn shaping_reports_missing_glyph_and_required_fallback() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("\u{10ffff}"));

        assert!(
            plan.diagnostics()
                .iter()
                .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MissingGlyph { .. }))
        );
        assert!(
            plan.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic,
                TextShapeDiagnostic::FallbackRequired { .. }
            ))
        );
    }

    #[test]
    fn raster_bounds_match_measured_ink_bounds_within_tolerance() {
        let mut cache = TextRasterCache::new();
        let entry = match cache.resolve(request("Bounds")) {
            Ok(Some(entry)) => entry,
            Ok(None) => panic!("expected rasterized text entry"),
            Err(err) => panic!("text rasterization failed: {:?}", err),
        };
        let raster_height = entry.metrics.ink_bounds.height().ceil().max(1.0);
        let raster_width = (entry.pixels.len() / 4) as f32 / raster_height;

        assert!((raster_width - entry.metrics.ink_bounds.width()).abs() <= TEXT_BOUNDS_TOLERANCE);
    }
}
