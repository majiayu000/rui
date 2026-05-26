//! Shared text measurement and rasterization.

use crate::core::geometry::{Bounds, Size};
use rusttype::{point, Font, Scale};
use std::collections::HashMap;
use std::sync::Arc;

pub const TEXT_BOUNDS_TOLERANCE: f32 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TextError {
    MissingFont,
    UnsupportedFontFamily(String),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextRasterKey {
    content: String,
    size_bits: u32,
    line_height_bits: u32,
}

impl TextRasterKey {
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
}

impl Default for TextMeasureCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextRasterCache {
    measurer: TextMeasureCache,
    next_id: u32,
    entries: HashMap<TextRasterKey, Arc<TextRasterEntry>>,
}

impl TextRasterCache {
    pub fn new() -> Self {
        Self {
            measurer: TextMeasureCache::new(),
            next_id: 1,
            entries: HashMap::new(),
        }
    }

    pub fn resolve(
        &mut self,
        request: TextRequest<'_>,
    ) -> Result<Option<Arc<TextRasterEntry>>, TextError> {
        let metrics = self.measurer.measure_single_line(request)?;
        if metrics.ink_bounds.is_empty() {
            return Ok(None);
        }

        let key = TextRasterKey::from_request(request)?;
        if let Some(entry) = self.entries.get(&key) {
            return Ok(Some(entry.clone()));
        }

        let entry = Arc::new(TextRasterEntry {
            id: self.next_entry_id(),
            metrics,
            pixels: rasterize_with_font(
                self.measurer.font.as_ref().ok_or(TextError::MissingFont)?,
                request,
                metrics,
            ),
        });
        self.entries.insert(key, entry.clone());
        Ok(Some(entry))
    }

    fn next_entry_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for TextRasterCache {
    fn default() -> Self {
        Self::new()
    }
}

fn measure_with_font(font: &Font<'static>, request: TextRequest<'_>) -> TextMetrics {
    let scale = Scale::uniform(request.font_size);
    let v_metrics = font.v_metrics(scale);
    let glyphs: Vec<_> = font
        .layout(request.content, scale, point(0.0, v_metrics.ascent))
        .collect();

    let advance_width = glyphs
        .last()
        .map(|glyph| glyph.position().x + glyph.unpositioned().h_metrics().advance_width)
        .unwrap_or(0.0);

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for glyph in &glyphs {
        if let Some(bb) = glyph.pixel_bounding_box() {
            min_x = min_x.min(bb.min.x);
            min_y = min_y.min(bb.min.y);
            max_x = max_x.max(bb.max.x);
            max_y = max_y.max(bb.max.y);
        }
    }

    let ink_bounds = if min_x < max_x && min_y < max_y {
        Bounds::from_xywh(
            min_x as f32,
            min_y as f32,
            (max_x - min_x) as f32,
            (max_y - min_y) as f32,
        )
    } else {
        Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)
    };

    let width = advance_width.max(ink_bounds.width()).ceil();
    TextMetrics {
        size: Size::new(width, request.font_size * request.line_height),
        ink_bounds,
        advance_width,
    }
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
    for glyph in font.layout(request.content, scale, point(0.0, v_metrics.ascent)) {
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
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(font) = Font::try_from_vec(bytes) {
                return Some(font);
            }
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
