use crate::core::geometry::{Bounds, Size};
use crate::renderer::text::{TextMetrics, TextRequest};
use rusttype::{Font, Scale, point};
use unicode_bidi::{BidiClass, bidi_class};
use unicode_segmentation::UnicodeSegmentation;

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
pub struct TextCluster {
    pub byte_start: usize,
    pub byte_end: usize,
    pub text: String,
    pub script: TextScript,
    pub direction: TextDirection,
    pub x_offset: f32,
    pub advance_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRun {
    pub byte_start: usize,
    pub byte_end: usize,
    pub script: TextScript,
    pub direction: TextDirection,
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
    MixedDirection {
        direction: TextDirection,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextShapePlan {
    clusters: Vec<TextCluster>,
    runs: Vec<TextRun>,
    diagnostics: Vec<TextShapeDiagnostic>,
    direction: TextDirection,
    metrics: TextMetrics,
}

impl TextShapePlan {
    pub fn empty() -> Self {
        Self {
            clusters: Vec::new(),
            runs: Vec::new(),
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

pub(crate) fn shape_with_font(font: &Font<'static>, request: TextRequest<'_>) -> TextShapePlan {
    let scale = Scale::uniform(request.font_size);
    let v_metrics = font.v_metrics(scale);
    let mut clusters = Vec::new();
    let mut diagnostics = Vec::new();
    let mut cursor_x = 0.0;
    let mut previous_glyph = None;
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for (byte_start, cluster_text) in request.content.grapheme_indices(true) {
        let byte_end = byte_start + cluster_text.len();
        let x_offset = cursor_x;
        let script = classify_script(cluster_text);
        let direction = classify_direction(cluster_text);
        let mut missing_glyph = false;

        for ch in cluster_text.chars() {
            if ch.is_control() {
                continue;
            }

            let base_glyph = font.glyph(ch);
            let glyph_id = base_glyph.id();
            if glyph_id.0 == 0 && !is_default_ignorable(ch) {
                missing_glyph = true;
            }

            if let Some(previous) = previous_glyph {
                cursor_x += font.pair_kerning(scale, previous, glyph_id);
            }

            let scaled = base_glyph.scaled(scale);
            let advance = scaled.h_metrics().advance_width;
            let glyph = scaled.positioned(point(cursor_x, v_metrics.ascent));
            if let Some(bb) = glyph.pixel_bounding_box() {
                min_x = min_x.min(bb.min.x);
                min_y = min_y.min(bb.min.y);
                max_x = max_x.max(bb.max.x);
                max_y = max_y.max(bb.max.y);
            }

            cursor_x += advance;
            previous_glyph = Some(glyph_id);
        }

        if missing_glyph {
            diagnostics.push(TextShapeDiagnostic::MissingGlyph {
                byte_start,
                byte_end,
                cluster: cluster_text.to_string(),
            });
            diagnostics.push(TextShapeDiagnostic::FallbackRequired {
                byte_start,
                byte_end,
                cluster: cluster_text.to_string(),
                script,
            });
        }

        clusters.push(TextCluster {
            byte_start,
            byte_end,
            text: cluster_text.to_string(),
            script,
            direction,
            x_offset,
            advance_width: cursor_x - x_offset,
        });
    }

    let direction = resolve_text_direction(&clusters);
    if direction == TextDirection::Mixed {
        diagnostics.push(TextShapeDiagnostic::MixedDirection { direction });
    }
    let runs = runs_from_clusters(&clusters);

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

    let width = cursor_x.max(ink_bounds.width()).ceil();
    let metrics = TextMetrics {
        size: Size::new(width, request.font_size * request.line_height),
        ink_bounds,
        advance_width: cursor_x,
    };

    TextShapePlan {
        clusters,
        runs,
        diagnostics,
        direction,
        metrics,
    }
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

fn resolve_text_direction(clusters: &[TextCluster]) -> TextDirection {
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
