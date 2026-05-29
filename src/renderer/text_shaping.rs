use crate::core::geometry::{Bounds, Size};
use crate::renderer::text::{TextMetrics, TextRequest};
use rusttype::{Font, GlyphId, Scale, point};
use rustybuzz::{BufferClusterLevel, Direction as HbDirection, Face, UnicodeBuffer};
use std::sync::Arc;
use unicode_bidi::{BidiClass, bidi_class};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub(crate) struct TextShapingFont {
    pub family: String,
    pub index: u32,
    pub data: Arc<Vec<u8>>,
    pub font: Font<'static>,
}

impl TextShapingFont {
    pub(crate) fn new(
        family: impl Into<String>,
        index: u32,
        data: Arc<Vec<u8>>,
        font: Font<'static>,
    ) -> Self {
        Self {
            family: family.into(),
            index,
            data,
            font,
        }
    }

    fn face(&self) -> Option<Face<'_>> {
        Face::from_slice(self.data.as_slice(), self.index)
    }

    fn supports_cluster(&self, cluster: &str) -> bool {
        let Some(face) = self.face() else {
            return false;
        };

        cluster
            .chars()
            .all(|ch| ch.is_control() || is_default_ignorable(ch) || face.glyph_index(ch).is_some())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextScript {
    Latin,
    Cjk,
    Emoji,
    Rtl,
    Number,
    Whitespace,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Neutral,
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub byte_start: usize,
    pub byte_end: usize,
    pub glyph_id: u16,
    pub font_family: String,
    pub x_offset: f32,
    pub y_offset: f32,
    pub advance_width: f32,
    pub(crate) font_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    pub byte_start: usize,
    pub byte_end: usize,
    pub text: String,
    pub script: TextScript,
    pub direction: TextDirection,
    pub font_family: String,
    pub x_offset: f32,
    pub advance_width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub byte_start: usize,
    pub byte_end: usize,
    pub script: TextScript,
    pub direction: TextDirection,
    pub font_family: String,
    pub x_offset: f32,
    pub advance_width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextShapeDiagnostic {
    MissingGlyph {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
    },
    FallbackRequired {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    FallbackApplied {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        requested_family: String,
        fallback_family: String,
    },
    FallbackFailed {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    UnsupportedScript {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    MixedDirection {
        direction: TextDirection,
    },
    LigatureSubstitution {
        byte_start: usize,
        byte_end: usize,
        text: String,
        glyph_count: usize,
        grapheme_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextShapePlan {
    clusters: Vec<TextCluster>,
    runs: Vec<TextRun>,
    glyphs: Vec<ShapedGlyph>,
    diagnostics: Vec<TextShapeDiagnostic>,
    direction: TextDirection,
    metrics: TextMetrics,
}

impl TextShapePlan {
    pub fn empty() -> Self {
        Self {
            clusters: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            diagnostics: Vec::new(),
            direction: TextDirection::Neutral,
            metrics: TextMetrics::empty(),
        }
    }

    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }

    pub fn runs(&self) -> &[TextRun] {
        &self.runs
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.glyphs
    }

    pub fn diagnostics(&self) -> &[TextShapeDiagnostic] {
        &self.diagnostics
    }

    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }
}

#[derive(Debug, Clone)]
struct ClusterDraft {
    byte_start: usize,
    byte_end: usize,
    text: String,
    script: TextScript,
    direction: TextDirection,
    font_index: usize,
    x_offset: f32,
    advance_width: f32,
}

#[derive(Debug, Clone, Copy)]
struct InkBounds {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl InkBounds {
    fn empty() -> Self {
        Self {
            min_x: i32::MAX,
            min_y: i32::MAX,
            max_x: i32::MIN,
            max_y: i32::MIN,
        }
    }

    fn include(&mut self, bounds: rusttype::Rect<i32>) {
        self.min_x = self.min_x.min(bounds.min.x);
        self.min_y = self.min_y.min(bounds.min.y);
        self.max_x = self.max_x.max(bounds.max.x);
        self.max_y = self.max_y.max(bounds.max.y);
    }

    fn to_bounds(self) -> Bounds {
        if self.min_x < self.max_x && self.min_y < self.max_y {
            Bounds::from_xywh(
                self.min_x as f32,
                self.min_y as f32,
                (self.max_x - self.min_x) as f32,
                (self.max_y - self.min_y) as f32,
            )
        } else {
            Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)
        }
    }
}

pub(crate) fn shape_with_fonts(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
) -> TextShapePlan {
    let mut clusters = build_cluster_drafts(fonts, primary_index, request);
    let direction = resolve_text_direction(&clusters);
    let mut diagnostics = diagnostics_for_clusters(fonts, primary_index, &clusters, direction);
    let mut glyphs = Vec::new();
    let mut ink_bounds = InkBounds::empty();
    let mut cursor_x = 0.0;

    let mut run_start = 0;
    while run_start < clusters.len() {
        let run_end = next_run_boundary(&clusters, run_start);
        let run_result = shape_run(
            fonts,
            request,
            &mut clusters[run_start..run_end],
            cursor_x,
            &mut ink_bounds,
        );
        if let Some(diagnostic) = run_result.ligature {
            diagnostics.push(diagnostic);
        }
        cursor_x += run_result.advance_width;
        glyphs.extend(run_result.glyphs);
        run_start = run_end;
    }

    let clusters = clusters
        .into_iter()
        .map(|cluster| TextCluster {
            byte_start: cluster.byte_start,
            byte_end: cluster.byte_end,
            text: cluster.text,
            script: cluster.script,
            direction: cluster.direction,
            font_family: fonts[cluster.font_index].family.clone(),
            x_offset: cluster.x_offset,
            advance_width: cluster.advance_width,
        })
        .collect::<Vec<_>>();
    let runs = runs_from_clusters(&clusters);
    let ink_bounds = ink_bounds.to_bounds();
    let width = cursor_x.max(ink_bounds.width()).ceil();
    let metrics = TextMetrics {
        size: Size::new(width, request.font_size * request.line_height),
        ink_bounds,
        advance_width: cursor_x,
    };

    TextShapePlan {
        clusters,
        runs,
        glyphs,
        diagnostics,
        direction,
        metrics,
    }
}

pub(crate) fn rasterize_with_plan(
    fonts: &[TextShapingFont],
    request: TextRequest<'_>,
    metrics: TextMetrics,
    plan: &TextShapePlan,
) -> Vec<u8> {
    let width = metrics.ink_bounds.width().ceil().max(1.0) as u32;
    let height = metrics.ink_bounds.height().ceil().max(1.0) as u32;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for shaped in plan.glyphs() {
        let Some(font) = fonts.get(shaped.font_index) else {
            continue;
        };
        let scale = Scale::uniform(request.font_size);
        let v_metrics = font.font.v_metrics(scale);
        let glyph = font
            .font
            .glyph(GlyphId(shaped.glyph_id))
            .scaled(scale)
            .positioned(point(shaped.x_offset, v_metrics.ascent + shaped.y_offset));

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

struct RunShapeResult {
    advance_width: f32,
    glyphs: Vec<ShapedGlyph>,
    ligature: Option<TextShapeDiagnostic>,
}

fn build_cluster_drafts(
    fonts: &[TextShapingFont],
    primary_index: usize,
    request: TextRequest<'_>,
) -> Vec<ClusterDraft> {
    request
        .content
        .grapheme_indices(true)
        .filter_map(|(byte_start, cluster_text)| {
            if cluster_text.chars().all(char::is_control) {
                return None;
            }

            let byte_end = byte_start + cluster_text.len();
            let script = classify_script(cluster_text);
            let direction = classify_direction(cluster_text);
            let font_index =
                choose_font(fonts, primary_index, cluster_text).unwrap_or(primary_index);

            Some(ClusterDraft {
                byte_start,
                byte_end,
                text: cluster_text.to_string(),
                script,
                direction,
                font_index,
                x_offset: 0.0,
                advance_width: 0.0,
            })
        })
        .collect()
}

fn diagnostics_for_clusters(
    fonts: &[TextShapingFont],
    primary_index: usize,
    clusters: &[ClusterDraft],
    direction: TextDirection,
) -> Vec<TextShapeDiagnostic> {
    let mut diagnostics = Vec::new();
    let requested_family = fonts[primary_index].family.clone();

    for cluster in clusters {
        if cluster.script == TextScript::Other {
            diagnostics.push(TextShapeDiagnostic::UnsupportedScript {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.clone(),
                script: cluster.script,
            });
        }

        if fonts[primary_index].supports_cluster(&cluster.text) {
            continue;
        }

        diagnostics.push(TextShapeDiagnostic::FallbackRequired {
            byte_start: cluster.byte_start,
            byte_end: cluster.byte_end,
            cluster: cluster.text.clone(),
            script: cluster.script,
        });

        if cluster.font_index != primary_index
            && fonts[cluster.font_index].supports_cluster(&cluster.text)
        {
            diagnostics.push(TextShapeDiagnostic::FallbackApplied {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.clone(),
                requested_family: requested_family.clone(),
                fallback_family: fonts[cluster.font_index].family.clone(),
            });
        } else {
            diagnostics.push(TextShapeDiagnostic::MissingGlyph {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.clone(),
            });
            diagnostics.push(TextShapeDiagnostic::FallbackFailed {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                cluster: cluster.text.clone(),
                script: cluster.script,
            });
        }
    }

    if direction == TextDirection::Mixed {
        diagnostics.push(TextShapeDiagnostic::MixedDirection { direction });
    }
    diagnostics
}

fn choose_font(fonts: &[TextShapingFont], primary_index: usize, cluster: &str) -> Option<usize> {
    if fonts.get(primary_index)?.supports_cluster(cluster) {
        return Some(primary_index);
    }
    fonts.iter().position(|font| font.supports_cluster(cluster))
}

fn next_run_boundary(clusters: &[ClusterDraft], start: usize) -> usize {
    let first = &clusters[start];
    clusters[start + 1..]
        .iter()
        .position(|cluster| {
            cluster.font_index != first.font_index
                || cluster.script != first.script
                || cluster.direction != first.direction
        })
        .map(|offset| start + 1 + offset)
        .unwrap_or(clusters.len())
}

fn shape_run(
    fonts: &[TextShapingFont],
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> RunShapeResult {
    let font = &fonts[clusters[0].font_index];
    let run_text = &request.content[clusters[0].byte_start..clusters.last().unwrap().byte_end];
    let glyphs = shape_run_glyphs(font, request, clusters, start_x, ink_bounds);
    let advance_width = glyphs.iter().map(|glyph| glyph.advance_width).sum::<f32>();
    distribute_cluster_advances(font, request, clusters, start_x, advance_width);

    let grapheme_count = clusters
        .iter()
        .filter(|cluster| !cluster.text.chars().all(char::is_control))
        .count();
    let ligature = if glyphs.len() < grapheme_count && grapheme_count > 1 {
        Some(TextShapeDiagnostic::LigatureSubstitution {
            byte_start: clusters[0].byte_start,
            byte_end: clusters.last().unwrap().byte_end,
            text: run_text.to_string(),
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

fn shape_run_glyphs(
    font: &TextShapingFont,
    request: TextRequest<'_>,
    clusters: &[ClusterDraft],
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> Vec<ShapedGlyph> {
    let Some(face) = font.face() else {
        return Vec::new();
    };

    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
    buffer.set_direction(match clusters[0].direction {
        TextDirection::RightToLeft => HbDirection::RightToLeft,
        _ => HbDirection::LeftToRight,
    });
    for cluster in clusters {
        for ch in cluster.text.chars().filter(|ch| !ch.is_control()) {
            buffer.add(ch, cluster.byte_start as u32);
        }
    }

    let shaped = rustybuzz::shape(&face, &[], buffer);
    let position_scale = font.font.scale_for_pixel_height(request.font_size);
    let rust_scale = Scale::uniform(request.font_size);
    let v_metrics = font.font.v_metrics(rust_scale);
    let mut cursor_x = start_x;
    let mut glyphs = Vec::new();
    let run_end = clusters.last().map(|cluster| cluster.byte_end).unwrap_or(0);
    let mut shaped_cluster_starts = shaped
        .glyph_infos()
        .iter()
        .map(|info| info.cluster as usize)
        .collect::<Vec<_>>();
    shaped_cluster_starts.sort_unstable();
    shaped_cluster_starts.dedup();

    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let x_offset = cursor_x + position.x_offset as f32 * position_scale;
        let y_offset = -(position.y_offset as f32 * position_scale);
        let advance_width = position.x_advance as f32 * position_scale;
        let glyph_id = info.glyph_id as u16;
        let glyph = font
            .font
            .glyph(GlyphId(glyph_id))
            .scaled(rust_scale)
            .positioned(point(x_offset, v_metrics.ascent + y_offset));
        if let Some(bounds) = glyph.pixel_bounding_box() {
            ink_bounds.include(bounds);
        }

        let byte_start = info.cluster as usize;
        glyphs.push(ShapedGlyph {
            byte_start,
            byte_end: glyph_byte_end_for_offset(&shaped_cluster_starts, run_end, byte_start),
            glyph_id,
            font_family: font.family.clone(),
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
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    start_x: f32,
    shaped_advance: f32,
) {
    let scale = Scale::uniform(request.font_size);
    let simple_advances = clusters
        .iter()
        .map(|cluster| simple_cluster_advance(font, scale, &cluster.text))
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
    if clusters
        .first()
        .is_some_and(|cluster| cluster.direction == TextDirection::RightToLeft)
    {
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

fn simple_cluster_advance(font: &TextShapingFont, scale: Scale, cluster: &str) -> f32 {
    cluster
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| font.font.glyph(ch).scaled(scale).h_metrics().advance_width)
        .sum()
}

fn glyph_byte_end_for_offset(
    shaped_cluster_starts: &[usize],
    run_end: usize,
    byte_start: usize,
) -> usize {
    shaped_cluster_starts
        .iter()
        .copied()
        .find(|cluster_start| *cluster_start > byte_start)
        .unwrap_or(run_end)
}

fn classify_script(cluster: &str) -> TextScript {
    if cluster.chars().all(char::is_whitespace) {
        return TextScript::Whitespace;
    }
    if cluster.chars().any(is_emoji) {
        return TextScript::Emoji;
    }
    if cluster.chars().any(is_cjk) {
        return TextScript::Cjk;
    }
    if cluster.chars().any(is_rtl) {
        return TextScript::Rtl;
    }
    if cluster.chars().all(char::is_numeric) {
        return TextScript::Number;
    }
    if cluster.chars().any(is_latin) {
        return TextScript::Latin;
    }
    TextScript::Other
}

fn classify_direction(cluster: &str) -> TextDirection {
    if cluster.chars().any(is_rtl) {
        TextDirection::RightToLeft
    } else if cluster.chars().any(is_ltr) {
        TextDirection::LeftToRight
    } else {
        TextDirection::Neutral
    }
}

fn resolve_text_direction(clusters: &[ClusterDraft]) -> TextDirection {
    let has_ltr = clusters
        .iter()
        .any(|cluster| cluster.direction == TextDirection::LeftToRight);
    let has_rtl = clusters
        .iter()
        .any(|cluster| cluster.direction == TextDirection::RightToLeft);
    match (has_ltr, has_rtl) {
        (true, true) => TextDirection::Mixed,
        (true, false) => TextDirection::LeftToRight,
        (false, true) => TextDirection::RightToLeft,
        (false, false) => TextDirection::Neutral,
    }
}

fn runs_from_clusters(clusters: &[TextCluster]) -> Vec<TextRun> {
    let mut runs: Vec<TextRun> = Vec::new();
    for cluster in clusters {
        if let Some(run) = runs.last_mut()
            && run.script == cluster.script
            && run.direction == cluster.direction
            && run.font_family == cluster.font_family
        {
            run.byte_end = cluster.byte_end;
            run.advance_width = cluster.x_offset + cluster.advance_width - run.x_offset;
            continue;
        }

        runs.push(TextRun {
            byte_start: cluster.byte_start,
            byte_end: cluster.byte_end,
            script: cluster.script,
            direction: cluster.direction,
            font_family: cluster.font_family.clone(),
            x_offset: cluster.x_offset,
            advance_width: cluster.advance_width,
        });
    }
    runs
}

fn is_latin(ch: char) -> bool {
    matches!(ch as u32, 0x0041..=0x024f | 0x1e00..=0x1eff)
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
    )
}

fn is_emoji(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2600..=0x27bf | 0x1f1e6..=0x1f1ff | 0x1f300..=0x1faff
    )
}

fn is_ltr(ch: char) -> bool {
    ch.is_alphabetic() && bidi_class(ch) == BidiClass::L
}

fn is_rtl(ch: char) -> bool {
    ch.is_alphabetic() && matches!(bidi_class(ch), BidiClass::R | BidiClass::AL)
}

fn is_default_ignorable(ch: char) -> bool {
    matches!(
        ch as u32,
        0x00ad
            | 0x034f
            | 0x061c
            | 0x115f..=0x1160
            | 0x17b4..=0x17b5
            | 0x180b..=0x180f
            | 0x200b..=0x200f
            | 0x202a..=0x202e
            | 0x2060..=0x206f
            | 0xfe00..=0xfe0f
            | 0xfeff
            | 0xfff0..=0xfff8
            | 0xe0100..=0xe01ef
    )
}
