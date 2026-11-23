use crate::core::geometry::Size;
use crate::renderer::text::{FontFeature, TextMetrics, TextRequest};
use crate::renderer::text_ascii::AsciiCoverage;
use crate::renderer::text_bidi::{ink_of, line_levels, place_visually};
use crate::renderer::text_classify::{
    classify_direction, classify_script, is_default_ignorable, resolve_text_direction,
};
use crate::renderer::text_cluster_position::apply_exact_cluster_positions;
use crate::renderer::text_face_size::{FaceMetrics, face_size};
use crate::renderer::text_fallback::{RunScript, assign_fonts, script_runs};
use crate::renderer::text_glyph_box::{GlyphBoxes, InkBounds, PixelRect, draw_glyph};
pub use crate::renderer::text_plan::{
    ShapedGlyph, TextCluster, TextDirection, TextRun, TextScript, TextShapeDiagnostic,
    TextShapePlan,
};
use glam::Vec2;
use plans::CachedPlan;
use skrifa::GlyphId;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use unicode_segmentation::UnicodeSegmentation;

mod plans;

/// The bytes of one font file, shared by every face in it.
#[derive(Debug)]
pub(crate) enum FontBytes {
    /// Mapped from disk: only the pages that are read become resident, and they
    /// are clean file pages, not part of the process footprint.
    Mapped(crate::renderer::system_fonts::MappedFile),
    Owned(Vec<u8>),
    /// Bytes an app registered from its binary.
    Static(&'static [u8]),
}

impl AsRef<[u8]> for FontBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl FontBytes {
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Mapped(map) => map.as_slice(),
            Self::Owned(bytes) => bytes,
            Self::Static(bytes) => bytes,
        }
    }
}

/// Faces of `family`, best first for `weight`: upright before italic, then the
/// closest weight, then load order.
pub(crate) fn faces_for_weight(fonts: &[TextShapingFont], family: &str, weight: u16) -> Vec<usize> {
    let mut faces: Vec<usize> = (0..fonts.len())
        .filter(|&index| fonts[index].family.eq_ignore_ascii_case(family))
        .collect();
    faces.sort_by_key(|&index| (fonts[index].italic, fonts[index].weight.abs_diff(weight)));
    faces
}

#[derive(Clone)]
pub(crate) struct TextShapingFont {
    pub family: String,
    pub index: u32,
    /// OS/2 weight class: 400 regular, 700 bold.
    pub(crate) weight: u16,
    pub(crate) italic: bool,
    /// Descent to ascent, in ems: the height faces are scaled by.
    height_em: f32,
    metrics: FaceMetrics,
    /// The face's one parsed form, which shaping, metrics, outlines and
    /// bitmaps all read. It keeps the font's bytes alive.
    font: skrifa::Font,
    /// Recently used plans, keyed by the buffer properties and feature settings.
    plans: Arc<Mutex<VecDeque<CachedPlan>>>,
    boxes: Arc<GlyphBoxes>,
    ascii: AsciiCoverage,
}

impl std::fmt::Debug for TextShapingFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextShapingFont")
            .field("family", &self.family)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl TextShapingFont {
    /// `None` when face `index` of `data` cannot be parsed or lacks `head` or
    /// `hhea`.
    pub(crate) fn new(family: impl Into<String>, index: u32, data: Arc<FontBytes>) -> Option<Self> {
        let blob: Arc<dyn AsRef<[u8]> + Send + Sync> = data;
        let font = skrifa::Font::new(skrifa::font::Blob::from(blob), index)?;
        let metrics = FaceMetrics::read(&font)?;
        let height_em = (f32::from(metrics.ascender) - f32::from(metrics.descender))
            / f32::from(metrics.units_per_em);
        Some(Self {
            family: family.into(),
            index,
            weight: metrics.weight,
            italic: metrics.italic,
            height_em,
            metrics,
            font,
            plans: Arc::default(),
            boxes: Arc::default(),
            ascii: AsciiCoverage::default(),
        })
    }

    pub(crate) fn font(&self) -> &skrifa::Font {
        &self.font
    }

    pub(crate) fn height_em(&self) -> f32 {
        self.height_em
    }

    pub(crate) fn units_per_em(&self) -> u16 {
        self.metrics.units_per_em
    }

    /// Pixels per font unit for a face drawn `height` pixels from descent to
    /// ascent.
    pub(crate) fn scale_for_pixel_height(&self, height: f32) -> f32 {
        height / (f32::from(self.metrics.ascender) - f32::from(self.metrics.descender))
    }

    /// Ascent above the baseline and descent below it (negative), in pixels,
    /// at `height`.
    pub(crate) fn ascent_descent(&self, height: f32) -> (f32, f32) {
        let scale = self.scale_for_pixel_height(height);
        (
            f32::from(self.metrics.ascender) * scale,
            f32::from(self.metrics.descender) * scale,
        )
    }

    /// The advance of glyph `id` in pixels at `height`.
    pub(crate) fn glyph_advance(&self, id: u16, height: f32) -> f32 {
        let scale_x = self.scale_for_pixel_height(height) * height / height;
        self.font
            .glyph_metrics()
            .h_advance(GlyphId::new(u32::from(id)))
            * scale_x
    }

    /// The glyph `ch` maps to, if any.
    pub(crate) fn glyph_index(&self, ch: char) -> Option<u16> {
        let glyph = self.font.charmap().map_unicode(ch)?;
        u16::try_from(glyph.to_u32()).ok()
    }

    /// The pixel box of glyph `id` at `font_size` drawn at `position`, from the
    /// glyph's box in font units without decoding its outline.
    pub(crate) fn pixel_bounds(
        &self,
        id: u16,
        font_size: f32,
        position: Vec2,
    ) -> Option<PixelRect> {
        self.boxes.pixel_bounds(self, id, font_size, position)
    }

    pub(crate) fn supports_cluster(&self, cluster: &str) -> bool {
        if let &[byte] = cluster.as_bytes()
            && byte.is_ascii()
        {
            return char::from(byte).is_control() || self.ascii.covers(self, byte);
        }
        cluster
            .chars()
            .all(|ch| ch.is_control() || is_default_ignorable(ch) || self.glyph_index(ch).is_some())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClusterDraft<'a> {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) text: &'a str,
    pub(crate) script: TextScript,
    pub(crate) direction: TextDirection,
    pub(crate) bidi_level: u8,
    pub(crate) font_index: usize,
    pub(crate) x_offset: f32,
    pub(crate) advance_width: f32,
}

/// A [`ShapedGlyph`] before its font family is copied out.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphDraft {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) glyph_id: u16,
    pub(crate) x_offset: f32,
    pub(crate) y_offset: f32,
    pub(crate) advance_width: f32,
    pub(crate) font_index: usize,
}

/// A run the font shaped into fewer glyphs than it has graphemes.
#[derive(Debug, Clone, Copy)]
struct LigatureDraft {
    byte_start: usize,
    byte_end: usize,
    glyph_count: usize,
    grapheme_count: usize,
}

/// Clusters shaped together, in one font at one embedding level.
pub(crate) struct ShapedRun {
    pub(crate) clusters: Range<usize>,
    pub(crate) glyphs: Range<usize>,
    script: RunScript,
    rtl: bool,
    pub(crate) bidi_level: u8,
    pub(crate) advance_width: f32,
    pub(crate) x_offset: f32,
}

/// What shaping produces, borrowing the text it shaped. Layout and the glyph
/// atlas read it as is; [`Self::into_plan`] copies it into a [`TextShapePlan`]
/// for callers that want clusters, runs and diagnostics of their own.
pub(crate) struct ShapedParts<'a> {
    content: &'a str,
    pub(crate) clusters: Vec<ClusterDraft<'a>>,
    pub(crate) glyphs: Vec<GlyphDraft>,
    runs: Vec<ShapedRun>,
    ligatures: Vec<LigatureDraft>,
    pub(crate) metrics: TextMetrics,
}

impl ShapedParts<'_> {
    pub(crate) fn into_plan(
        self,
        fonts: &[TextShapingFont],
        primary_index: usize,
    ) -> TextShapePlan {
        let direction = resolve_text_direction(&self.clusters);
        let mut diagnostics = diagnostics_for_clusters(fonts, primary_index, &self.clusters);
        diagnostics.extend(self.ligatures.iter().map(|ligature| {
            TextShapeDiagnostic::LigatureSubstitution {
                byte_start: ligature.byte_start,
                byte_end: ligature.byte_end,
                text: self.content[ligature.byte_start..ligature.byte_end].to_string(),
                glyph_count: ligature.glyph_count,
                grapheme_count: ligature.grapheme_count,
            }
        }));
        let clusters = self
            .clusters
            .into_iter()
            .map(|cluster| TextCluster {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                text: cluster.text.to_string(),
                script: cluster.script,
                direction: cluster.direction,
                bidi_level: cluster.bidi_level,
                font_family: fonts[cluster.font_index].family.clone(),
                x_offset: cluster.x_offset,
                advance_width: cluster.advance_width,
            })
            .collect::<Vec<_>>();
        let runs = self
            .runs
            .into_iter()
            .map(|run| text_run(&clusters[run.clusters.clone()], &run))
            .collect();
        let glyphs = self
            .glyphs
            .into_iter()
            .map(|glyph| ShapedGlyph {
                byte_start: glyph.byte_start,
                byte_end: glyph.byte_end,
                glyph_id: glyph.glyph_id,
                font_family: fonts[glyph.font_index].family.clone(),
                x_offset: glyph.x_offset,
                y_offset: glyph.y_offset,
                advance_width: glyph.advance_width,
                font_index: glyph.font_index,
            })
            .collect();
        TextShapePlan {
            clusters,
            runs,
            glyphs,
            diagnostics,
            direction,
            metrics: self.metrics,
        }
    }
}

pub(crate) fn shape_with_fonts(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
) -> TextShapePlan {
    shape_parts(fonts, primary_index, request).into_plan(fonts, primary_index)
}

pub(crate) fn shape_parts<'a>(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'a>,
) -> ShapedParts<'a> {
    let mut clusters = build_cluster_drafts(primary_index, request);
    let levels = line_levels(request.content, request.direction);
    if let Some(levels) = &levels {
        for cluster in &mut clusters {
            cluster.bidi_level = levels[cluster.byte_start].number();
        }
    }
    let script_runs = script_runs(&clusters);
    for run in &script_runs {
        assign_fonts(
            fonts,
            primary_index,
            run.script,
            request.font_weight,
            &mut clusters[run.clusters.clone()],
        );
    }
    let mut glyphs = Vec::with_capacity(clusters.len());
    let mut ligatures = Vec::new();
    let mut ink_bounds = InkBounds::empty();
    let mut cursor_x = 0.0;

    let mut runs = Vec::with_capacity(script_runs.len());
    for run in &script_runs {
        // One script run is shaped in pieces where its clusters' fonts or
        // embedding levels differ.
        let mut start = run.clusters.start;
        while start < run.clusters.end {
            let (font_index, bidi_level) = (clusters[start].font_index, clusters[start].bidi_level);
            let end = clusters[start..run.clusters.end]
                .iter()
                .position(|cluster| {
                    cluster.font_index != font_index || cluster.bidi_level != bidi_level
                })
                .map_or(run.clusters.end, |offset| start + offset);
            let rtl = match levels {
                Some(_) => bidi_level % 2 == 1,
                None => run.script.is_rtl(),
            };
            let run_result = shape_run(
                fonts,
                primary_index,
                request,
                &mut clusters[start..end],
                run.script,
                rtl,
                cursor_x,
                &mut ink_bounds,
            );
            ligatures.extend(run_result.ligature);
            let glyph_start = glyphs.len();
            glyphs.extend(run_result.glyphs);
            runs.push(ShapedRun {
                clusters: start..end,
                glyphs: glyph_start..glyphs.len(),
                script: run.script,
                rtl,
                bidi_level,
                advance_width: run_result.advance_width,
                x_offset: cursor_x,
            });
            cursor_x += run_result.advance_width;
            start = end;
        }
    }
    if levels.is_some() {
        place_visually(&mut runs, &mut clusters, &mut glyphs);
        ink_bounds = ink_of(fonts, primary_index, request, &glyphs);
    }

    let ink_bounds = ink_bounds.to_bounds();
    let width = cursor_x.max(ink_bounds.width()).ceil();
    let metrics = TextMetrics {
        size: Size::new(width, request.font_size * request.line_height),
        ink_bounds,
        advance_width: cursor_x,
    };

    ShapedParts {
        content: request.content,
        clusters,
        glyphs,
        runs,
        ligatures,
        metrics,
    }
}

pub(crate) fn rasterize_with_plan(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
    metrics: TextMetrics,
    plan: &TextShapePlan,
) -> Vec<u8> {
    let width = metrics.ink_bounds.width().ceil().max(1.0) as u32;
    let height = metrics.ink_bounds.height().ceil().max(1.0) as u32;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let baseline = fonts[primary_index]
        .ascent_descent(face_size(fonts, primary_index, request.font_size))
        .0;

    for shaped in plan.glyphs() {
        let Some(font) = fonts.get(shaped.font_index) else {
            continue;
        };
        let size = face_size(fonts, shaped.font_index, request.font_size);
        let position = Vec2::new(shaped.x_offset, baseline + shaped.y_offset);
        if let Some(bb) = font.pixel_bounds(shaped.glyph_id, size, position) {
            draw_glyph(font, shaped.glyph_id, size, position, bb, |x, y, v| {
                let px = x as i32 + bb.min_x - metrics.ink_bounds.x() as i32;
                let py = y as i32 + bb.min_y - metrics.ink_bounds.y() as i32;
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

struct RunShapeResult {
    advance_width: f32,
    glyphs: Vec<GlyphDraft>,
    ligature: Option<LigatureDraft>,
}

/// Grapheme clusters of `request`, all in the primary font until
/// [`assign_fonts`] picks theirs.
pub(crate) fn build_cluster_drafts<'a>(
    primary_index: usize,
    request: TextRequest<'a>,
) -> Vec<ClusterDraft<'a>> {
    let draft = |(byte_start, cluster_text): (usize, &'a str)| {
        if cluster_text.chars().all(char::is_control) {
            return None;
        }
        Some(ClusterDraft {
            byte_start,
            byte_end: byte_start + cluster_text.len(),
            text: cluster_text,
            script: classify_script(cluster_text),
            direction: classify_direction(cluster_text),
            bidi_level: 0,
            font_index: primary_index,
            x_offset: 0.0,
            advance_width: 0.0,
        })
    };
    let content = request.content;
    if content.is_ascii() {
        // Every ASCII character is a grapheme of its own, but for CR LF, which
        // is control and dropped either way.
        (0..content.len())
            .filter_map(|index| draft((index, &content[index..index + 1])))
            .collect()
    } else {
        content.grapheme_indices(true).filter_map(draft).collect()
    }
}

fn diagnostics_for_clusters(
    fonts: &[TextShapingFont],
    primary_index: usize,
    clusters: &[ClusterDraft],
) -> Vec<TextShapeDiagnostic> {
    let mut diagnostics = Vec::new();
    let requested_family = fonts[primary_index].family.clone();

    for cluster in clusters {
        if cluster.script == TextScript::Other {
            diagnostics.push(TextShapeDiagnostic::UnsupportedScript {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.to_string(),
                script: cluster.script,
            });
        }

        if fonts[primary_index].supports_cluster(cluster.text) {
            continue;
        }

        diagnostics.push(TextShapeDiagnostic::FallbackRequired {
            byte_start: cluster.byte_start,
            byte_end: cluster.byte_end,
            cluster: cluster.text.to_string(),
            script: cluster.script,
        });

        if cluster.font_index != primary_index
            && fonts[cluster.font_index].supports_cluster(cluster.text)
        {
            diagnostics.push(TextShapeDiagnostic::FallbackApplied {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.to_string(),
                requested_family: requested_family.clone(),
                fallback_family: fonts[cluster.font_index].family.clone(),
            });
        } else {
            diagnostics.push(TextShapeDiagnostic::MissingGlyph {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.to_string(),
            });
            diagnostics.push(TextShapeDiagnostic::FallbackFailed {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.to_string(),
                script: cluster.script,
            });
        }
    }

    diagnostics
}

#[allow(clippy::too_many_arguments)]
fn shape_run(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    script: RunScript,
    rtl: bool,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> RunShapeResult {
    let font = &fonts[clusters[0].font_index];
    // Every face sits on the requested face's baseline.
    let size = face_size(fonts, clusters[0].font_index, request.font_size);
    let baseline = fonts[primary_index]
        .ascent_descent(face_size(fonts, primary_index, request.font_size))
        .0;
    let glyphs = shape_run_glyphs(
        (font, request.font_features),
        (size, baseline),
        clusters,
        script,
        rtl,
        start_x,
        ink_bounds,
    );
    let advance_width = glyphs.iter().map(|glyph| glyph.advance_width).sum::<f32>();
    distribute_cluster_advances(font, size, clusters, rtl, start_x, &glyphs, advance_width);

    let grapheme_count = clusters
        .iter()
        .filter(|cluster| !cluster.text.chars().all(char::is_control))
        .count();
    let ligature = if glyphs.len() < grapheme_count && grapheme_count > 1 {
        Some(LigatureDraft {
            byte_start: clusters[0].byte_start,
            byte_end: clusters[clusters.len() - 1].byte_end,
            glyph_count: glyphs.len(),
            grapheme_count,
        })
    } else {
        None
    };

    RunShapeResult {
        advance_width,
        glyphs,
        ligature,
    }
}

/// `font` shaped with `features`; `(size, baseline)`: the height it is drawn
/// at and the baseline's distance from the top of the line.
fn shape_run_glyphs(
    (font, features): (&TextShapingFont, &[FontFeature]),
    (size, baseline): (f32, f32),
    clusters: &[ClusterDraft],
    script: RunScript,
    rtl: bool,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> Vec<GlyphDraft> {
    let shaped = font.shape(features, clusters, script, rtl);
    let position_scale = font.scale_for_pixel_height(size);
    let mut cursor_x = start_x;
    let mut glyphs = Vec::with_capacity(shaped.len());
    let run_end = clusters.last().map(|cluster| cluster.byte_end).unwrap_or(0);
    let mut shaped_cluster_starts = shaped
        .iter()
        .map(|glyph| glyph.1 as usize)
        .collect::<Vec<_>>();
    shaped_cluster_starts.sort_unstable();
    shaped_cluster_starts.dedup();

    for &(glyph_id, cluster, x_advance, x_offset, y_offset) in &shaped {
        let x_offset = cursor_x + x_offset as f32 * position_scale;
        let y_offset = -(y_offset as f32 * position_scale);
        let advance_width = x_advance as f32 * position_scale;
        let glyph_id = glyph_id as u16;
        let position = Vec2::new(x_offset, baseline + y_offset);
        if let Some(bounds) = font.pixel_bounds(glyph_id, size, position) {
            ink_bounds.include(bounds);
        }

        let byte_start = cluster as usize;
        glyphs.push(GlyphDraft {
            byte_start,
            byte_end: glyph_byte_end_for_offset(&shaped_cluster_starts, run_end, byte_start),
            glyph_id,
            x_offset,
            y_offset,
            advance_width,
            font_index: clusters[0].font_index,
        });
        cursor_x += advance_width;
    }

    glyphs
}

fn distribute_cluster_advances(
    font: &TextShapingFont,
    size: f32,
    clusters: &mut [ClusterDraft],
    rtl: bool,
    start_x: f32,
    glyphs: &[GlyphDraft],
    shaped_advance: f32,
) {
    if apply_exact_cluster_positions(clusters, glyphs) {
        return;
    }

    let simple_advances = clusters
        .iter()
        .map(|cluster| simple_cluster_advance(font, size, cluster.text))
        .collect::<Vec<_>>();
    let simple_total = simple_advances.iter().sum::<f32>();
    let fallback_advance = if clusters.is_empty() {
        0.0
    } else {
        shaped_advance / clusters.len() as f32
    };
    let advance_widths = simple_advances
        .iter()
        .map(|simple_advance| {
            if simple_total > 0.0 {
                simple_advance / simple_total * shaped_advance
            } else {
                fallback_advance
            }
        })
        .collect::<Vec<_>>();

    let mut cursor_x = start_x;
    if rtl {
        for index in (0..clusters.len()).rev() {
            clusters[index].x_offset = cursor_x;
            clusters[index].advance_width = advance_widths[index];
            cursor_x += clusters[index].advance_width;
        }
    } else {
        for (cluster, advance_width) in clusters.iter_mut().zip(advance_widths) {
            cluster.x_offset = cursor_x;
            cluster.advance_width = advance_width;
            cursor_x += cluster.advance_width;
        }
    }
}

/// The advances of `cluster`'s characters' own glyphs (`.notdef` for one the
/// font lacks) at `size`.
fn simple_cluster_advance(font: &TextShapingFont, size: f32, cluster: &str) -> f32 {
    cluster
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| font.glyph_advance(font.glyph_index(ch).unwrap_or(0), size))
        .sum()
}

fn glyph_byte_end_for_offset(
    shaped_cluster_starts: &[usize],
    run_end: usize,
    byte_start: usize,
) -> usize {
    let next = shaped_cluster_starts.partition_point(|&start| start <= byte_start);
    shaped_cluster_starts.get(next).copied().unwrap_or(run_end)
}

/// The [`TextRun`] of `run`, whose clusters are `clusters`.
fn text_run(clusters: &[TextCluster], run: &ShapedRun) -> TextRun {
    let coarse = clusters
        .iter()
        .map(|cluster| cluster.script)
        .find(|script| !matches!(script, TextScript::Whitespace | TextScript::Number))
        .unwrap_or(clusters[0].script);
    let direction = if run.rtl {
        TextDirection::RightToLeft
    } else if clusters
        .iter()
        .any(|cluster| cluster.direction == TextDirection::LeftToRight)
    {
        TextDirection::LeftToRight
    } else {
        TextDirection::Neutral
    };
    TextRun {
        byte_start: clusters[0].byte_start,
        byte_end: clusters[clusters.len() - 1].byte_end,
        script: coarse,
        script_tag: run.script.tag(),
        direction,
        font_family: clusters[0].font_family.clone(),
        x_offset: run.x_offset,
        advance_width: run.advance_width,
    }
}
