use crate::core::geometry::Size;
use crate::renderer::text::{TextMetrics, TextRequest};
use crate::renderer::text_ascii::AsciiCoverage;
use crate::renderer::text_bidi::{ink_of, line_levels, place_visually};
use crate::renderer::text_classify::{
    classify_direction, classify_script, is_default_ignorable, resolve_text_direction,
};
use crate::renderer::text_cluster_position::apply_exact_cluster_positions;
use crate::renderer::text_fallback::{RunScript, assign_fonts, script_runs};
use crate::renderer::text_glyph_box::{GlyphBoxes, InkBounds};
pub use crate::renderer::text_plan::{
    ShapedGlyph, TextCluster, TextDirection, TextRun, TextScript, TextShapeDiagnostic,
    TextShapePlan,
};
use rusttype::{Font, GlyphId, Scale, point};
use rustybuzz::{
    BufferClusterLevel, Direction as HbDirection, Face, Language, Script, ShapePlan, UnicodeBuffer,
};
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};
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

/// The face of `family` best for `weight`, as [`faces_for_weight`] orders them.
pub(crate) fn best_face(fonts: &[TextShapingFont], family: &str, weight: u16) -> Option<usize> {
    (0..fonts.len())
        .filter(|&index| fonts[index].family.eq_ignore_ascii_case(family))
        .min_by_key(|&index| (fonts[index].italic, fonts[index].weight.abs_diff(weight)))
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

/// A shaping plan and the buffer properties it was built for.
type CachedPlan = (HbDirection, Script, Option<Language>, Arc<ShapePlan>);

#[derive(Clone)]
pub(crate) struct TextShapingFont {
    pub family: String,
    pub index: u32,
    /// OS/2 weight class: 400 regular, 700 bold.
    pub(crate) weight: u16,
    pub(crate) italic: bool,
    // Both borrow from `data`. Declared first so they are dropped first, and private
    // so no `Font<'static>`, `Face<'static>` or glyph outlives the bytes it reads.
    font: Font<'static>,
    /// Parsed once: building a face reads the font's layout tables, far more
    /// work than shaping a short string with it.
    face: Face<'static>,
    /// Plans built so far, one per direction, script and language shaped.
    plans: Arc<Mutex<Vec<CachedPlan>>>,
    boxes: Arc<GlyphBoxes>,
    ascii: AsciiCoverage,
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
            weight: face.weight().to_number(),
            italic: face.is_italic() || face.is_oblique(),
            font,
            face,
            plans: Arc::new(Mutex::new(Vec::new())),
            boxes: Arc::default(),
            ascii: AsciiCoverage::default(),
            data,
        })
    }

    pub(crate) fn font(&self) -> &Font<'_> {
        &self.font
    }

    /// The pixel box of glyph `id` at `font_size` drawn at `position`, as
    /// rusttype's `pixel_bounding_box` gives it, without decoding the outline.
    pub(crate) fn pixel_bounds(
        &self,
        id: u16,
        font_size: f32,
        position: rusttype::Point<f32>,
    ) -> Option<rusttype::Rect<i32>> {
        self.boxes
            .pixel_bounds(&self.font, &self.face, id, font_size, position)
    }

    /// The glyph's bitmap from the strike closest above `pixels_per_em`, for
    /// bitmap color fonts such as Apple Color Emoji (`sbix`).
    pub(crate) fn raster_image(
        &self,
        glyph_id: u16,
        pixels_per_em: u16,
    ) -> Option<rustybuzz::ttf_parser::RasterGlyphImage<'_>> {
        self.face
            .glyph_raster_image(rustybuzz::ttf_parser::GlyphId(glyph_id), pixels_per_em)
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
        if let &[byte] = cluster.as_bytes()
            && byte.is_ascii()
        {
            return char::from(byte).is_control() || self.ascii.covers(face, byte);
        }
        cluster
            .chars()
            .all(|ch| ch.is_control() || is_default_ignorable(ch) || face.glyph_index(ch).is_some())
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
    let levels = line_levels(request.content);
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
        ink_bounds = ink_of(fonts, request, &glyphs);
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

fn shape_run(
    fonts: &[TextShapingFont],
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    script: RunScript,
    rtl: bool,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> RunShapeResult {
    let font = &fonts[clusters[0].font_index];
    let glyphs = shape_run_glyphs(font, request, clusters, script, rtl, start_x, ink_bounds);
    let advance_width = glyphs.iter().map(|glyph| glyph.advance_width).sum::<f32>();
    distribute_cluster_advances(
        font,
        request,
        clusters,
        rtl,
        start_x,
        &glyphs,
        advance_width,
    );

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

fn shape_run_glyphs(
    font: &TextShapingFont,
    request: TextRequest<'_>,
    clusters: &[ClusterDraft],
    script: RunScript,
    rtl: bool,
    start_x: f32,
    ink_bounds: &mut InkBounds,
) -> Vec<GlyphDraft> {
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
    buffer.set_direction(if rtl {
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
    let mut glyphs = Vec::with_capacity(shaped.len());
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
        let position = point(x_offset, v_metrics.ascent + y_offset);
        if let Some(bounds) = font.pixel_bounds(glyph_id, request.font_size, position) {
            ink_bounds.include(bounds);
        }

        let byte_start = info.cluster as usize;
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
    request: TextRequest<'_>,
    clusters: &mut [ClusterDraft],
    rtl: bool,
    start_x: f32,
    glyphs: &[GlyphDraft],
    shaped_advance: f32,
) {
    if apply_exact_cluster_positions(clusters, glyphs) {
        return;
    }

    let scale = Scale::uniform(request.font_size);
    let simple_advances = clusters
        .iter()
        .map(|cluster| simple_cluster_advance(font, scale, cluster.text))
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
