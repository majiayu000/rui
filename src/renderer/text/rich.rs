//! Wrapping one paragraph whose runs have different styles.
//!
//! Each run is shaped in its own font and size, and the clusters of all runs are
//! broken into lines together, so a word split across two runs stays whole and a
//! line breaks between runs like anywhere else. Results are kept by width range
//! exactly like single-style wraps.

use super::hash::FastHasher;
use super::lru::FrameLru;
use super::wrap::{
    BreakCluster, BreakParagraph, ELLIPSIS, KEPT_RESULTS, LineSpan, WidthRange, WrapResult,
    break_paragraph,
};
use super::{
    BaseDirection, MAX_RETAINED_METRIC_BYTES, MAX_RETAINED_METRICS, RETAINED_METRIC_BYTES_TARGET,
    RETAINED_METRICS_TARGET, TextError, TextMeasureCache, TextRequest,
};
use crate::core::geometry::Size;
use crate::renderer::text_bidi::{line_levels, visual_order};
use rusttype::Scale;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

/// The style of `len` bytes of a rich paragraph, as far as shaping cares.
#[derive(Debug, Clone, PartialEq)]
pub struct RichSpan {
    pub len: usize,
    pub font_size: f32,
    pub font_weight: u16,
    /// Shared, so the spans of one paragraph do not each copy it.
    pub font_family: Option<Arc<str>>,
    pub line_height: f32,
}

impl RichSpan {
    fn request<'a>(&'a self, content: &'a str) -> TextRequest<'a> {
        TextRequest::new(
            content,
            self.font_size,
            self.font_weight,
            self.font_family.as_deref(),
            self.line_height,
        )
    }

    fn hash(&self, hasher: &mut FastHasher) {
        self.len.hash(hasher);
        self.font_size.to_bits().hash(hasher);
        self.font_weight.hash(hasher);
        self.font_family.hash(hasher);
        self.line_height.to_bits().hash(hasher);
    }
}

/// What a piece of a line draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichPieceText {
    /// These bytes of the paragraph.
    Content(Range<usize>),
    /// An ellipsis ending a cut off paragraph.
    Ellipsis,
}

/// Consecutive clusters of one line in one span.
#[derive(Debug, Clone, PartialEq)]
pub struct RichPiece {
    /// Index of the span the piece is styled by.
    pub span: usize,
    pub text: RichPieceText,
    /// Pen position from the start of the line.
    pub x: f32,
    /// Ink left edge relative to the pen, which is where a text primitive puts
    /// its bounds' left edge.
    pub ink_x: f32,
    pub width: f32,
    /// Top of the piece's own line box below the top of the line; every piece's
    /// box puts its baseline on the line's baseline.
    pub top: f32,
    /// Height of the piece's own line box: its font size times its line height.
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichLine {
    pub pieces: Vec<RichPiece>,
    pub width: f32,
    pub height: f32,
}

/// A rich paragraph broken into lines.
#[derive(Debug, Clone, PartialEq)]
pub struct RichWrapped {
    pub lines: Vec<RichLine>,
    /// Widest line by the height of all lines.
    pub size: Size,
}

#[derive(Debug, Clone, PartialEq)]
struct RichKey {
    content: String,
    spans: Vec<RichSpan>,
    max_lines: Option<usize>,
}

/// A `\n`-separated paragraph's clusters, each with the span it came from.
struct RichParagraph {
    /// Byte offset of the paragraph in the content.
    offset: usize,
    layout: BreakParagraph,
    spans: Vec<usize>,
    /// Each cluster's embedding level, when any is right to left.
    levels: Option<Vec<u8>>,
}

struct RichEntry {
    paragraphs: Vec<RichParagraph>,
    results: Vec<WrapResult<RichWrapped>>,
}

pub(super) struct RichCache {
    entries: FrameLru<RichKey, RichEntry>,
}

impl RichCache {
    pub(super) fn new() -> Self {
        Self {
            entries: FrameLru::new(
                RETAINED_METRICS_TARGET,
                RETAINED_METRIC_BYTES_TARGET,
                MAX_RETAINED_METRICS,
                MAX_RETAINED_METRIC_BYTES,
            ),
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn begin_frame(&mut self) {
        self.entries.begin_frame();
    }
}

/// Where a line's baseline sits and how tall it is, from its pieces' styles.
struct LineBox {
    baseline: f32,
    height: f32,
}

impl TextMeasureCache {
    /// Break `content`, styled by `spans` in order, into lines no wider than
    /// `max_width`, as [`Self::wrap`] does for one style. The spans' lengths must
    /// add up to the content's and fall on character boundaries.
    pub fn wrap_rich(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<Arc<RichWrapped>, TextError> {
        self.with_rich(content, spans, max_width, max_lines, Arc::clone)
    }

    /// The size [`Self::wrap_rich`] would report.
    pub fn wrapped_rich_size(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<Size, TextError> {
        self.with_rich(content, spans, max_width, max_lines, |wrapped| wrapped.size)
    }

    fn with_rich<R>(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        max_width: f32,
        max_lines: Option<usize>,
        read: impl FnOnce(&Arc<RichWrapped>) -> R,
    ) -> Result<R, TextError> {
        validate_spans(content, spans)?;
        if content.is_empty() {
            return Ok(read(&Arc::new(RichWrapped {
                lines: Vec::new(),
                size: Size::ZERO,
            })));
        }
        let mut hasher = FastHasher::default();
        content.hash(&mut hasher);
        spans.iter().for_each(|span| span.hash(&mut hasher));
        max_lines.hash(&mut hasher);
        let hash = hasher.finish();
        let matches = |key: &RichKey| {
            key.max_lines == max_lines && key.content == content && key.spans == spans
        };

        if let Some(entry) = self.rich_wraps.entries.get_mut(hash, matches)
            && let Some(index) = entry
                .results
                .iter()
                .position(|result| result.holds(max_width))
        {
            self.stats.wrap_hits += 1;
            entry.results[..=index].rotate_right(1);
            return Ok(read(&entry.results[0].wrapped));
        }

        self.stats.wrap_misses += 1;
        let primaries = spans
            .iter()
            .map(|span| self.primary_font_index(span.font_family.as_deref(), span.font_weight))
            .collect::<Result<Vec<_>, _>>()?;
        let mut entry = match self.rich_wraps.entries.remove(hash, matches) {
            Some(entry) => entry,
            None => RichEntry {
                paragraphs: self.shape_rich(content, spans, &primaries),
                results: Vec::new(),
            },
        };
        let result = self.wrap_rich_uncached(
            content,
            spans,
            &primaries,
            &entry.paragraphs,
            max_width,
            max_lines,
        );
        let value = read(&result.wrapped);
        entry.results.truncate(KEPT_RESULTS - 1);
        entry.results.insert(0, result);
        let bytes = content.len()
            + entry
                .paragraphs
                .iter()
                .map(|paragraph| {
                    paragraph.layout.retained_bytes() + paragraph.spans.len() * size_of::<usize>()
                })
                .sum::<usize>()
            + entry
                .results
                .iter()
                .flat_map(|result| &result.wrapped.lines)
                .map(|line| size_of::<RichLine>() + line.pieces.len() * size_of::<RichPiece>())
                .sum::<usize>();
        let key = RichKey {
            content: content.to_string(),
            spans: spans.to_vec(),
            max_lines,
        };
        self.rich_wraps.entries.insert(hash, key, entry, bytes);
        Ok(value)
    }

    /// Shape each span's part of each paragraph in the span's own style.
    fn shape_rich(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        primaries: &[usize],
    ) -> Vec<RichParagraph> {
        let mut paragraphs = Vec::new();
        let mut offset = 0;
        for paragraph in content.split('\n') {
            let end = offset + paragraph.len();
            let mut clusters = Vec::new();
            let mut cluster_spans = Vec::new();
            let mut span_start = 0;
            for (index, span) in spans.iter().enumerate() {
                let span_end = span_start + span.len;
                let (from, to) = (span_start.max(offset), span_end.min(end));
                span_start = span_end;
                if from >= to {
                    continue;
                }
                let parts = self.shape_counted(span.request(&content[from..to]), primaries[index]);
                for cluster in &parts.clusters {
                    let mut cluster = BreakCluster::from(cluster);
                    cluster.byte_start += from - offset;
                    cluster.byte_end += from - offset;
                    clusters.push(cluster);
                    cluster_spans.push(index);
                }
            }
            let levels = line_levels(&content[offset..end], BaseDirection::Auto).map(|levels| {
                clusters
                    .iter()
                    .map(|cluster| levels[cluster.byte_start].number())
                    .collect()
            });
            paragraphs.push(RichParagraph {
                offset,
                layout: BreakParagraph::new(clusters, &content[offset..end]),
                spans: cluster_spans,
                levels,
            });
            offset = end + 1;
        }
        paragraphs
    }

    fn wrap_rich_uncached(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        primaries: &[usize],
        paragraphs: &[RichParagraph],
        max_width: f32,
        max_lines: Option<usize>,
    ) -> WrapResult<RichWrapped> {
        let line_limit = max_lines.unwrap_or(usize::MAX);
        let mut fits = WidthRange::ANY;
        let mut line_spans = Vec::new();
        for (index, paragraph) in paragraphs.iter().enumerate() {
            if line_spans.len() > line_limit {
                break;
            }
            break_paragraph(
                index,
                &paragraph.layout,
                max_width,
                &mut fits,
                &mut line_spans,
            );
        }
        let truncated = line_spans.len() > line_limit;
        line_spans.truncate(line_limit);

        let last = line_spans.len().saturating_sub(1);
        let mut ellipsis = None;
        let mut lines = Vec::with_capacity(line_spans.len());
        for (index, line) in line_spans.iter().enumerate() {
            let paragraph = &paragraphs[line.paragraph];
            let (first, mut end) = cluster_range(paragraph, line);
            let mut trailing = None;
            if truncated && index == last {
                let cut = self.cut_for_ellipsis(spans, primaries, paragraph, first, end, max_width);
                end = cut.end;
                trailing = Some((cut.span, cut.ellipsis_width));
                ellipsis = Some((cut.room, cut.ellipsis_width));
            }
            lines.push(self.rich_line(
                content, spans, primaries, paragraph, line, first, end, trailing,
            ));
        }

        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = lines.iter().map(|line| line.height).sum();
        WrapResult {
            fits,
            ellipsis,
            wrapped: Arc::new(RichWrapped {
                lines,
                size: Size::new(width, height),
            }),
        }
    }

    /// Where the last kept line is cut so an ellipsis in the style of its last
    /// cluster still fits.
    fn cut_for_ellipsis(
        &mut self,
        spans: &[RichSpan],
        primaries: &[usize],
        paragraph: &RichParagraph,
        first: usize,
        end: usize,
        max_width: f32,
    ) -> EllipsisCut {
        let span = paragraph
            .spans
            .get(end.saturating_sub(1).max(first))
            .copied()
            .unwrap_or_else(|| span_at(spans, paragraph.offset));
        let ellipsis_width = self
            .measure_prepared(spans[span].request(ELLIPSIS), primaries[span])
            .size
            .width;
        let room = max_width - ellipsis_width;
        let mut room_range = WidthRange::ANY;
        let mut cut = first;
        for cluster in first..end {
            if room_range.split(paragraph.layout.width(first, cluster + 1), room) {
                break;
            }
            cut = cluster + 1;
        }
        while cut > first
            && paragraph.layout.clusters[cut - 1].script
                == crate::renderer::text_shaping::TextScript::Whitespace
        {
            cut -= 1;
        }
        EllipsisCut {
            end: cut,
            span,
            ellipsis_width,
            room: room_range,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rich_line(
        &mut self,
        content: &str,
        spans: &[RichSpan],
        primaries: &[usize],
        paragraph: &RichParagraph,
        line: &LineSpan,
        first: usize,
        end: usize,
        ellipsis: Option<(usize, f32)>,
    ) -> RichLine {
        let layout = &paragraph.layout;
        let level = |cluster: usize| {
            paragraph
                .levels
                .as_ref()
                .map_or(0, |levels| levels[cluster])
        };
        let mut pieces = Vec::new();
        let mut piece_levels = Vec::new();
        let mut start = first;
        while start < end {
            let span = paragraph.spans[start];
            // A piece is one style at one embedding level.
            let stop = (start..end)
                .find(|&cluster| paragraph.spans[cluster] != span || level(cluster) != level(start))
                .unwrap_or(end);
            if paragraph.levels.is_some() {
                piece_levels.push(level(start));
            }
            let bytes = paragraph.offset + layout.clusters[start].byte_start
                ..paragraph.offset + layout.clusters[stop - 1].byte_end;
            let ink_x = self
                .measure_prepared(
                    spans[span].request(&content[bytes.clone()]),
                    primaries[span],
                )
                .ink_bounds
                .x();
            pieces.push(RichPiece {
                span,
                text: RichPieceText::Content(bytes),
                x: layout.width(first, start),
                ink_x,
                width: layout.width(start, stop),
                top: 0.0,
                height: 0.0,
            });
            start = stop;
        }
        if paragraph.levels.is_some() {
            let mut x = 0.0;
            for index in visual_order(&piece_levels) {
                pieces[index].x = x;
                x += pieces[index].width;
            }
        }
        let mut width = layout.width(first, end);
        if let Some((span, ellipsis_width)) = ellipsis {
            let ink_x = self
                .measure_prepared(spans[span].request(ELLIPSIS), primaries[span])
                .ink_bounds
                .x();
            pieces.push(RichPiece {
                span,
                text: RichPieceText::Ellipsis,
                x: width,
                ink_x,
                width: ellipsis_width,
                top: 0.0,
                height: 0.0,
            });
            width += ellipsis_width;
        }

        // An empty line is as tall as the style its position is in.
        let styles: Vec<usize> = if pieces.is_empty() {
            vec![span_at(spans, paragraph.offset + line.start)]
        } else {
            pieces.iter().map(|piece| piece.span).collect()
        };
        let line_box = self.line_box(spans, primaries, &styles);
        for piece in &mut pieces {
            let (own_baseline, own_height) =
                self.own_baseline(&spans[piece.span], primaries[piece.span]);
            piece.top = line_box.baseline - own_baseline;
            piece.height = own_height;
        }
        RichLine {
            pieces,
            width: width.ceil(),
            height: line_box.height,
        }
    }

    /// A style's baseline below the top of its own line box, as a text
    /// primitive places it, and that box's height.
    fn own_baseline(&self, span: &RichSpan, primary: usize) -> (f32, f32) {
        let v_metrics = self.fonts[primary]
            .font()
            .v_metrics(Scale::uniform(span.font_size));
        let height = span.font_size * span.line_height;
        let font_height = v_metrics.ascent - v_metrics.descent;
        ((height - font_height) * 0.5 + v_metrics.ascent, height)
    }

    fn line_box(&self, spans: &[RichSpan], primaries: &[usize], styles: &[usize]) -> LineBox {
        let boxes: Vec<(f32, f32)> = styles
            .iter()
            .map(|&span| self.own_baseline(&spans[span], primaries[span]))
            .collect();
        let baseline = boxes
            .iter()
            .map(|&(baseline, _)| baseline)
            .fold(0.0, f32::max);
        let height = boxes
            .iter()
            .map(|&(own, height)| baseline - own + height)
            .fold(0.0, f32::max);
        LineBox { baseline, height }
    }
}

struct EllipsisCut {
    end: usize,
    span: usize,
    ellipsis_width: f32,
    room: WidthRange,
}

/// Clusters of `paragraph` inside `line`: the first, and one past the last.
fn cluster_range(paragraph: &RichParagraph, line: &LineSpan) -> (usize, usize) {
    let clusters = &paragraph.layout.clusters;
    let first = clusters.partition_point(|cluster| cluster.byte_start < line.start);
    let end = clusters.partition_point(|cluster| cluster.byte_end <= line.end);
    (first, end.max(first))
}

/// The span byte `offset` of the content falls in; the last span past the end.
fn span_at(spans: &[RichSpan], offset: usize) -> usize {
    let mut end = 0;
    for (index, span) in spans.iter().enumerate() {
        end += span.len;
        if offset < end {
            return index;
        }
    }
    spans.len().saturating_sub(1)
}

fn validate_spans(content: &str, spans: &[RichSpan]) -> Result<(), TextError> {
    let mut end = 0;
    for span in spans {
        if span.font_size <= 0.0 || span.line_height <= 0.0 {
            return Err(TextError::InvalidSpans(format!(
                "span at byte {end} has font size {} and line height {}",
                span.font_size, span.line_height
            )));
        }
        end += span.len;
        if !content.is_char_boundary(end.min(content.len())) {
            return Err(TextError::InvalidSpans(format!(
                "span ending at byte {end} splits a character"
            )));
        }
    }
    if end != content.len() || spans.is_empty() && !content.is_empty() {
        return Err(TextError::InvalidSpans(format!(
            "spans cover {end} bytes of {}",
            content.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(len: usize, font_size: f32, font_weight: u16) -> RichSpan {
        RichSpan {
            len,
            font_size,
            font_weight,
            font_family: None,
            line_height: 1.4,
        }
    }

    /// Each line's text, put back together from its pieces.
    fn texts(content: &str, wrapped: &RichWrapped) -> Vec<String> {
        wrapped
            .lines
            .iter()
            .map(|line| {
                line.pieces
                    .iter()
                    .map(|piece| match &piece.text {
                        RichPieceText::Content(bytes) => &content[bytes.clone()],
                        RichPieceText::Ellipsis => ELLIPSIS,
                    })
                    .collect()
            })
            .collect()
    }

    fn width_of(cache: &mut TextMeasureCache, content: &str, size: f32, weight: u16) -> f32 {
        cache
            .measure_single_line(TextRequest::new(content, size, weight, None, 1.4))
            .expect("measure")
            .advance_width
    }

    #[test]
    fn one_span_wraps_like_plain_text() {
        let mut cache = TextMeasureCache::new();
        let content = "The quick brown fox jumps over the lazy dog,\nthen 中文换行 again.";
        let spans = [style(content.len(), 14.0, 400)];
        for width in [0.0, 40.0, 90.0, 133.5, 200.0, 400.0, f32::INFINITY] {
            let rich = cache.wrap_rich(content, &spans, width, None).expect("rich");
            let plain = cache
                .wrap(TextRequest::new(content, 14.0, 400, None, 1.4), width, None)
                .expect("plain");
            let plain_lines: Vec<String> = plain
                .lines
                .iter()
                .map(|line| line.content.to_string())
                .collect();
            assert_eq!(texts(content, &rich), plain_lines, "width {width}");
            assert_eq!(rich.size.width, plain.size.width, "width {width}");
            assert!(
                (rich.size.height - plain.size.height).abs() < 1e-2,
                "width {width}"
            );
        }
    }

    #[test]
    fn a_word_split_across_spans_stays_whole() {
        let mut cache = TextMeasureCache::new();
        let content = "hello world";
        let spans = [style(3, 14.0, 700), style(8, 14.0, 400)];
        let hello = width_of(&mut cache, "hel", 14.0, 700) + width_of(&mut cache, "lo", 14.0, 400);
        let world = width_of(&mut cache, "world", 14.0, 400);
        let wrapped = cache
            .wrap_rich(content, &spans, hello.max(world) + 2.0, None)
            .expect("rich");

        assert_eq!(texts(content, &wrapped), ["hello", "world"]);
        let first = &wrapped.lines[0].pieces;
        assert_eq!(first.len(), 2);
        assert_eq!((first[0].span, first[1].span), (0, 1));
        assert_eq!(first[0].x, 0.0);
        assert!((first[1].x - first[0].width).abs() < 1e-3);
        assert_eq!(wrapped.lines[1].pieces[0].x, 0.0);
    }

    #[test]
    fn mixed_sizes_share_a_baseline_and_the_tallest_sets_the_line() {
        let mut cache = TextMeasureCache::new();
        let content = "small BIG small";
        let spans = [
            style(6, 14.0, 400),
            style(3, 28.0, 400),
            style(6, 14.0, 400),
        ];
        let wrapped = cache
            .wrap_rich(content, &spans, f32::INFINITY, None)
            .expect("rich");
        let line = &wrapped.lines[0];

        assert_eq!(line.pieces.len(), 3);
        assert_eq!(line.height, 28.0 * 1.4);
        assert_eq!(line.pieces[1].top, 0.0, "the tallest span starts the line");
        let baselines: Vec<f32> = line
            .pieces
            .iter()
            .map(|piece| piece.top + cache.own_baseline(&spans[piece.span], 0).0)
            .collect();
        assert!(
            baselines
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() < 1e-3),
            "{baselines:?}"
        );
        assert_eq!(wrapped.size.height, line.height);
    }

    #[test]
    fn max_lines_ends_in_an_ellipsis_that_fits() {
        let mut cache = TextMeasureCache::new();
        let content = "one two three four five six seven eight nine ten";
        let spans = [style(8, 14.0, 700), style(content.len() - 8, 14.0, 400)];
        for width in [60.0, 80.0, 120.5] {
            let wrapped = cache
                .wrap_rich(content, &spans, width, Some(2))
                .expect("rich");
            assert_eq!(wrapped.lines.len(), 2, "width {width}");
            let last = wrapped.lines[1].pieces.last().expect("pieces");
            assert_eq!(last.text, RichPieceText::Ellipsis);
            assert!(wrapped.lines[1].width <= width.ceil(), "width {width}");
        }
    }

    #[test]
    fn a_kept_result_answers_every_width_in_its_range_without_shaping() {
        let mut cache = TextMeasureCache::new();
        let content = "Rich text keeps its wraps across frames like plain text does.";
        let spans = [style(10, 14.0, 700), style(content.len() - 10, 18.0, 400)];
        let mut reference = TextMeasureCache::new();
        cache.wrap_rich(content, &spans, 100.0, None).expect("rich");
        let shapes = cache.stats().shapes;
        for step in 0..400 {
            let width = 100.0 + step as f32 * 0.5;
            let kept = cache
                .wrap_rich(content, &spans, width, Some(3))
                .expect("rich");
            reference.rich_wraps.clear();
            let fresh = reference
                .wrap_rich(content, &spans, width, Some(3))
                .expect("rich");
            assert_eq!(kept, fresh, "width {width}");
        }
        let after = cache.stats();
        assert!(after.wrap_hits > after.wrap_misses, "{after:?}");
        assert!(
            after.shapes - shapes <= after.wrap_misses * 3,
            "only ellipsis and piece measurements shape again: {after:?}"
        );
    }

    #[test]
    fn spans_that_do_not_tile_the_content_are_an_error() {
        let mut cache = TextMeasureCache::new();
        let short = cache.wrap_rich("abc", &[style(2, 14.0, 400)], 100.0, None);
        assert!(matches!(short, Err(TextError::InvalidSpans(_))));
        let split = cache.wrap_rich(
            "é",
            &[style(1, 14.0, 400), style(1, 14.0, 400)],
            100.0,
            None,
        );
        assert!(matches!(split, Err(TextError::InvalidSpans(_))));
        let empty = cache.wrap_rich("", &[], 100.0, None).expect("empty");
        assert!(empty.lines.is_empty());
    }
}
