//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::resources::{
    GlyphResourceKey, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceStats,
};
use crate::renderer::text_shaping::{TextShapingFont, rasterize_with_plan, shape_with_fonts};
use rusttype::Font;
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

pub struct TextMeasureCache {
    fonts: Vec<TextShapingFont>,
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
            fonts: Vec::new(),
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

        let primary_index = self.primary_font_index(request.font_family)?;
        let key = TextMeasureKey::from_request(request, &self.fonts[primary_index].family);
        if let Some(metrics) = self.metrics.get(&key) {
            return Ok(*metrics);
        }

        let metrics = self.shape_with_primary(request, primary_index).metrics();
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
        shape_with_fonts(&self.fonts, primary_index, request)
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

        let pixels = rasterize_with_plan(&self.measurer.fonts, request, metrics, &plan);
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

fn load_system_fonts() -> Vec<TextShapingFont> {
    let candidates = [
        ("Arial", "/System/Library/Fonts/Supplemental/Arial.ttf"),
        ("Arial", "/Library/Fonts/Arial.ttf"),
        (
            "Arial Unicode",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ),
        ("Arial Unicode", "/Library/Fonts/Arial Unicode.ttf"),
        ("Helvetica", "/System/Library/Fonts/Helvetica.ttc"),
        ("Helvetica Neue", "/System/Library/Fonts/HelveticaNeue.ttc"),
        ("Geneva", "/System/Library/Fonts/Geneva.ttf"),
        ("SF Pro", "/System/Library/Fonts/SFNS.ttf"),
        ("SF Mono", "/System/Library/Fonts/SFNSMono.ttf"),
        ("SF Hebrew", "/System/Library/Fonts/SFHebrew.ttf"),
        ("SF Arabic", "/System/Library/Fonts/SFArabic.ttf"),
        ("Geeza Pro", "/System/Library/Fonts/GeezaPro.ttc"),
        (
            "Hiragino Sans GB",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ),
        ("STHeiti", "/System/Library/Fonts/STHeiti Medium.ttc"),
        (
            "Apple Color Emoji",
            "/System/Library/Fonts/Apple Color Emoji.ttc",
        ),
        ("Apple Symbols", "/System/Library/Fonts/Apple Symbols.ttf"),
        (
            "DejaVu Sans",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ),
        (
            "Liberation Sans",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        ),
        (
            "Noto Sans CJK",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ),
    ];

    let mut fonts = Vec::new();
    for (family, path) in candidates {
        load_font_faces(family, path, &mut fonts);
    }
    fonts
}

fn load_font_faces(family: &str, path: &str, fonts: &mut Vec<TextShapingFont>) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let data = Arc::new(bytes);

    for index in 0..16 {
        let Some(font) = Font::try_from_vec_and_index(data.as_ref().clone(), index) else {
            break;
        };
        if rustybuzz::Face::from_slice(data.as_slice(), index).is_none() {
            break;
        }
        fonts.push(TextShapingFont::new(family, index, data.clone(), font));
    }
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

    fn assert_close(left: f32, right: f32) {
        assert!(
            (left - right).abs() <= TEXT_BOUNDS_TOLERANCE,
            "{left} should be within {TEXT_BOUNDS_TOLERANCE} of {right}"
        );
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
    fn shaping_records_positioned_glyphs_that_sum_to_the_advance() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("office"));

        assert!(!plan.glyphs().is_empty());
        assert!(
            plan.glyphs()
                .iter()
                .all(|glyph| glyph.byte_end > glyph.byte_start)
        );
        let glyph_advance = plan
            .glyphs()
            .iter()
            .map(|glyph| glyph.advance_width)
            .sum::<f32>();
        assert_close(glyph_advance, plan.metrics().advance_width);
    }

    #[test]
    fn shaping_cluster_offsets_follow_positioned_glyphs() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("AV"));

        if plan.glyphs().len() != plan.clusters().len() {
            return;
        }

        for cluster in plan.clusters() {
            let glyph = plan
                .glyphs()
                .iter()
                .find(|glyph| glyph.byte_start == cluster.byte_start)
                .expect("cluster should map to a positioned glyph");
            assert_close(cluster.x_offset, glyph.x_offset);
            assert_close(cluster.advance_width, glyph.advance_width);
        }
    }

    #[test]
    fn shaping_reports_ligature_substitution_when_the_font_applies_one() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("office"));

        if let Some(TextShapeDiagnostic::LigatureSubstitution {
            glyph_count,
            grapheme_count,
            ..
        }) = plan.diagnostics().iter().find(|diagnostic| {
            matches!(diagnostic, TextShapeDiagnostic::LigatureSubstitution { .. })
        }) {
            assert!(*glyph_count < *grapheme_count);
        } else {
            assert_eq!(plan.glyphs().len(), plan.clusters().len());
        }
    }

    #[test]
    fn shaped_ligature_glyph_ranges_cover_collapsed_clusters() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("office"));

        if !plan.diagnostics().iter().any(|diagnostic| {
            matches!(diagnostic, TextShapeDiagnostic::LigatureSubstitution { .. })
        }) {
            return;
        }

        let collapsed = plan.glyphs().iter().any(|glyph| {
            plan.clusters()
                .iter()
                .filter(|cluster| {
                    cluster.byte_start >= glyph.byte_start && cluster.byte_end <= glyph.byte_end
                })
                .count()
                > 1
        });
        assert!(
            collapsed,
            "expected one glyph range to cover every cluster collapsed into a ligature"
        );
    }

    #[test]
    fn rtl_cluster_offsets_follow_visual_order() {
        let mut cache = TextMeasureCache::new();
        let plan = shape(&mut cache, request("שלום"));
        let rtl_clusters = plan
            .clusters()
            .iter()
            .filter(|cluster| cluster.direction == TextDirection::RightToLeft)
            .collect::<Vec<_>>();

        assert!(rtl_clusters.len() > 1);
        assert!(
            rtl_clusters
                .windows(2)
                .all(|pair| { pair[0].x_offset > pair[1].x_offset })
        );
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
    fn empty_raster_requests_do_not_require_fonts() {
        let mut cache = TextRasterCache {
            measurer: TextMeasureCache::without_font(),
            resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
            entries: HashMap::new(),
        };

        assert!(matches!(cache.resolve(request("")), Ok(None)));
        assert!(matches!(
            cache.resolve(TextRequest::new("Hello", 0.0, 400, None, 1.2)),
            Ok(None)
        ));
        assert!(matches!(
            cache.resolve(TextRequest::new("Hello", 20.0, 400, None, 0.0)),
            Ok(None)
        ));
    }

    #[test]
    fn raster_cache_hits_do_not_require_reshaping() {
        let mut cache = TextRasterCache {
            measurer: TextMeasureCache::without_font(),
            resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
            entries: HashMap::new(),
        };
        let key = GlyphResourceKey::new("Cached", 20.0, 400, None, 1.2);
        let entry = Arc::new(TextRasterEntry {
            id: 7,
            metrics: TextMetrics {
                size: Size::new(1.0, 1.0),
                ink_bounds: Bounds::from_xywh(0.0, 0.0, 1.0, 1.0),
                advance_width: 1.0,
            },
            pixels: vec![255, 255, 255, 255],
        });
        cache.entries.insert(key, entry.clone());

        let resolved = match cache.resolve(request("Cached")) {
            Ok(Some(entry)) => entry,
            Ok(None) => panic!("expected cached raster entry"),
            Err(err) => panic!("cache hit should not shape text: {:?}", err),
        };
        assert!(Arc::ptr_eq(&resolved, &entry));
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
        assert!(
            plan.diagnostics()
                .iter()
                .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::FallbackFailed { .. }))
        );
    }

    #[test]
    fn shaping_surfaces_font_fallback_when_primary_lacks_a_cluster() {
        let mut cache = TextMeasureCache::new();
        let families = cache
            .fonts
            .iter()
            .map(|font| font.family.clone())
            .collect::<Vec<_>>();

        let fallback_plan = families.into_iter().find_map(|family| {
            let plan = shape(
                &mut cache,
                TextRequest::new("A界", 20.0, 400, Some(&family), 1.2),
            );
            plan.diagnostics()
                .iter()
                .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::FallbackApplied { .. }))
                .then_some(plan)
        });

        let plan = match fallback_plan {
            Some(plan) => plan,
            None => panic!("expected an installed primary font to need deterministic CJK fallback"),
        };
        assert!(plan.clusters().iter().any(|cluster| {
            cluster.script == TextScript::Cjk && !cluster.font_family.is_empty()
        }));
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
