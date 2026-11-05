use crate::core::geometry::{Bounds, Size};
use crate::renderer::text::{TextMetrics, TextRequest};
use crate::renderer::text_cluster_position::apply_exact_cluster_positions;
use crate::renderer::text_fallback::{RunScript, assign_fonts, script_runs};
use rusttype::{Font, GlyphId, Scale, point};
use rustybuzz::{
    BufferClusterLevel, Direction as HbDirection, Face, Language, Script, ShapePlan, UnicodeBuffer,
};
use std::sync::{Arc, Mutex, PoisonError};
use unicode_bidi::{BidiClass, bidi_class};
use unicode_segmentation::UnicodeSegmentation;

/// The bytes of one font file, shared by every face in it.
#[derive(Debug)]
pub(crate) enum FontBytes {
    /// Mapped from disk: only the pages that are read become resident, and they
    /// are clean file pages, not part of the process footprint.
    Mapped(crate::renderer::system_fonts::MappedFile),
    #[cfg_attr(all(unix, not(test)), allow(dead_code))]
    Owned(Vec<u8>),
}

impl FontBytes {
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Self::Mapped(map) => map.as_slice(),
            Self::Owned(bytes) => bytes,
        }
    }
}

/// A shaping plan and the buffer properties it was built for.
type CachedPlan = (HbDirection, Script, Option<Language>, Arc<ShapePlan>);

#[derive(Clone)]
pub(crate) struct TextShapingFont {
    pub family: String,
    pub index: u32,
    // Both borrow from `data`. Declared first so they are dropped first, and private
    // so no `Font<'static>`, `Face<'static>` or glyph outlives the bytes it reads.
    font: Font<'static>,
    /// Parsed once: building a face reads the font's layout tables, far more
    /// work than shaping a short string with it.
    face: Face<'static>,
    /// Plans built so far, one per direction, script and language shaped.
    plans: Arc<Mutex<Vec<CachedPlan>>>,
    /// Owns the bytes `font` and `face` read; never read directly.
    #[allow(dead_code)]
    data: Arc<FontBytes>,
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
    /// `None` when face `index` of `data` cannot be parsed by both rusttype and
    /// rustybuzz.
    pub(crate) fn new(family: impl Into<String>, index: u32, data: Arc<FontBytes>) -> Option<Self> {
        // SAFETY: the slice points into `data`, whose heap or mapping does not move
        // while the `Arc` lives. The font and face are stored next to that `Arc`,
        // dropped before it, and only handed out with their lifetime shortened to
        // `&self`.
        let bytes: &'static [u8] = unsafe { std::mem::transmute(data.as_slice()) };
        let font = Font::try_from_bytes_and_index(bytes, index)?;
        let face = Face::from_slice(bytes, index)?;
        Some(Self {
            family: family.into(),
            index,
            font,
            face,
            plans: Arc::new(Mutex::new(Vec::new())),
            data,
        })
    }

    pub(crate) fn font(&self) -> &Font<'_> {
        &self.font
    }

    fn face(&self) -> &Face<'_> {
        &self.face
    }

    /// The plan for `buffer`'s direction, script and language, built on first use.
    fn plan_for(&self, buffer: &UnicodeBuffer) -> Arc<ShapePlan> {
        let (direction, script, language) =
            (buffer.direction(), buffer.script(), buffer.language());
        let mut plans = self.plans.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((.., plan)) = plans
            .iter()
            .find(|(d, s, l, _)| *d == direction && *s == script && *l == language)
        {
            return Arc::clone(plan);
        }
        let plan = Arc::new(ShapePlan::new(
            &self.face,
            direction,
            Some(script),
            language.as_ref(),
            &[],
        ));
        plans.push((direction, script, language, Arc::clone(&plan)));
        plan
    }

    pub(crate) fn supports_cluster(&self, cluster: &str) -> bool {
        let face = self.face();
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
    /// ISO 15924 code of the script the run was shaped as: `Latn`, `Arab`,
    /// `Hani` and so on, `Zsye` for emoji, `Zyyy` for text no script owns.
    pub script_tag: [u8; 4],
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
pub(crate) struct ClusterDraft {
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) text: String,
    pub(crate) script: TextScript,
    pub(crate) direction: TextDirection,
    pub(crate) font_index: usize,
    pub(crate) x_offset: f32,
    pub(crate) advance_width: f32,
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
    let mut clusters = build_cluster_drafts(primary_index, request);
    let runs = script_runs(&clusters);
    for run in &runs {
        assign_fonts(
            fonts,
            primary_index,
            run.script,
            &mut clusters[run.clusters.clone()],
        );
    }
    let direction = resolve_text_direction(&clusters);
    let mut diagnostics = diagnostics_for_clusters(fonts, primary_index, &clusters, direction);
    let mut glyphs = Vec::new();
    let mut ink_bounds = InkBounds::empty();
    let mut cursor_x = 0.0;

    let mut shaped_runs = Vec::with_capacity(runs.len());
    for run in &runs {
        // One script run is shaped in pieces where its clusters' fonts differ.
        let mut start = run.clusters.start;
        while start < run.clusters.end {
            let font_index = clusters[start].font_index;
            let end = clusters[start..run.clusters.end]
                .iter()
                .position(|cluster| cluster.font_index != font_index)
                .map_or(run.clusters.end, |offset| start + offset);
            let run_result = shape_run(
                fonts,
                request,
                &mut clusters[start..end],
                run.script,
                cursor_x,
                &mut ink_bounds,
            );
            if let Some(diagnostic) = run_result.ligature {
                diagnostics.push(diagnostic);
            }
            shaped_runs.push((start..end, run.script, run_result.advance_width, cursor_x));
            cursor_x += run_result.advance_width;
            glyphs.extend(run_result.glyphs);
            start = end;
        }
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
    let runs = shaped_runs
        .into_iter()
        .map(|(range, script, advance_width, x_offset)| {
            text_run(&clusters[range], script, advance_width, x_offset)
        })
        .collect();
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
        let v_metrics = font.font().v_metrics(scale);
        let glyph = font
            .font()
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

/// Grapheme clusters of `request`, all in the primary font until
/// [`assign_fonts`] picks theirs.
pub(crate) fn build_cluster_drafts(
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

            Some(ClusterDraft {
                byte_start,
                byte_end,
                text: cluster_text.to_string(),
                script,
                direction,
                font_index: primary_index,
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

fn shape_run(
    fonts: &[TextShapingFont],
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    script: RunScript,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> RunShapeResult {
    let font = &fonts[clusters[0].font_index];
    let run_text = &request.content[clusters[0].byte_start..clusters.last().unwrap().byte_end];
    let glyphs = shape_run_glyphs(font, request, clusters, script, start_x, ink_bounds);
    let advance_width = glyphs.iter().map(|glyph| glyph.advance_width).sum::<f32>();
    distribute_cluster_advances(
        font,
        request,
        clusters,
        script.is_rtl(),
        start_x,
        &glyphs,
        advance_width,
    );

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
    script: RunScript,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> Vec<ShapedGlyph> {
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
    buffer.set_direction(if script.is_rtl() {
        HbDirection::RightToLeft
    } else {
        HbDirection::LeftToRight
    });
    if let Some(script) = script.script() {
        buffer.set_script(script);
    }
    for cluster in clusters {
        for ch in cluster.text.chars().filter(|ch| !ch.is_control()) {
            buffer.add(ch, cluster.byte_start as u32);
        }
    }

    buffer.guess_segment_properties();
    let plan = font.plan_for(&buffer);
    let shaped = rustybuzz::shape_with_plan(font.face(), &plan, buffer);
    let position_scale = font.font().scale_for_pixel_height(request.font_size);
    let rust_scale = Scale::uniform(request.font_size);
    let v_metrics = font.font().v_metrics(rust_scale);
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
            .font()
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
    rtl: bool,
    start_x: f32,
    glyphs: &[ShapedGlyph],
    shaped_advance: f32,
) {
    if apply_exact_cluster_positions(clusters, glyphs) {
        return;
    }

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

fn simple_cluster_advance(font: &TextShapingFont, scale: Scale, cluster: &str) -> f32 {
    cluster
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| {
            font.font()
                .glyph(ch)
                .scaled(scale)
                .h_metrics()
                .advance_width
        })
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

/// The run of `clusters`, shaped together as `script` from `x_offset`.
fn text_run(
    clusters: &[TextCluster],
    script: RunScript,
    advance_width: f32,
    x_offset: f32,
) -> TextRun {
    let coarse = clusters
        .iter()
        .map(|cluster| cluster.script)
        .find(|script| !matches!(script, TextScript::Whitespace | TextScript::Number))
        .unwrap_or(clusters[0].script);
    let direction = if script.is_rtl() {
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
        script_tag: script.tag(),
        direction,
        font_family: clusters[0].font_family.clone(),
        x_offset,
        advance_width,
    }
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
