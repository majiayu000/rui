//! Breaking a paragraph into lines that fit a width.

use super::lru::FrameLru;
use super::{
    MAX_RETAINED_METRIC_BYTES, MAX_RETAINED_METRICS, RETAINED_METRIC_BYTES_TARGET,
    RETAINED_METRICS_TARGET, TextError, TextMeasureCache, TextMeasureKey, TextRequest,
    request_hasher,
};
use crate::core::SharedStr;
use crate::core::geometry::Size;
use crate::renderer::text_shaping::{ClusterDraft, TextScript};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub(super) const ELLIPSIS: &str = "\u{2026}";
/// Wrap results kept per paragraph. Taffy asks one text for its min-content,
/// max-content and definite widths in the same pass, so a few are live at once.
pub(super) const KEPT_RESULTS: usize = 4;

/// One line of a wrapped paragraph, ready to be drawn on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub content: SharedStr,
    pub width: f32,
}

/// A paragraph broken into lines no wider than the width it was given, except
/// where a single cluster is wider than that width on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct WrappedText {
    pub lines: Vec<TextLine>,
    /// Widest line by the height of all lines.
    pub size: Size,
}

impl WrappedText {
    fn empty() -> Self {
        Self {
            lines: Vec::new(),
            size: Size::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextWrapKey {
    text: TextMeasureKey,
    max_lines: Option<usize>,
}

/// The part of a shaped cluster line breaking reads.
#[derive(Debug, Clone, Copy)]
pub(super) struct BreakCluster {
    pub(super) byte_start: usize,
    pub(super) byte_end: usize,
    pub(super) script: TextScript,
    pub(super) advance_width: f32,
}

impl From<&ClusterDraft<'_>> for BreakCluster {
    fn from(cluster: &ClusterDraft<'_>) -> Self {
        Self {
            byte_start: cluster.byte_start,
            byte_end: cluster.byte_end,
            script: cluster.script,
            advance_width: cluster.advance_width,
        }
    }
}

/// Widths `w` with `min <= w < max`; `max` infinite also takes `w` infinite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WidthRange {
    min: f32,
    max: f32,
}

impl WidthRange {
    pub(super) const ANY: Self = Self {
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
    };

    fn contains(self, width: f32) -> bool {
        width >= self.min && (width < self.max || self.max == f32::INFINITY)
    }

    /// Record that `needed > width` decided a break the way it did at `width`.
    pub(super) fn split(&mut self, needed: f32, width: f32) -> bool {
        if needed > width {
            self.max = self.max.min(needed);
            true
        } else {
            self.min = self.min.max(needed);
            false
        }
    }
}

/// A wrap, and every width it is also the answer for: the widths at which each
/// fit test the greedy breaker made comes out the same.
#[derive(Debug, Clone)]
pub(super) struct WrapResult<T> {
    pub(super) fits: WidthRange,
    /// For a cut off result, the same for the room left beside the ellipsis,
    /// and the ellipsis width that room is measured without.
    pub(super) ellipsis: Option<(WidthRange, f32)>,
    pub(super) wrapped: Arc<T>,
}

impl<T> WrapResult<T> {
    pub(super) fn holds(&self, width: f32) -> bool {
        self.fits.contains(width)
            && self
                .ellipsis
                .is_none_or(|(room, ellipsis)| room.contains(width - ellipsis))
    }
}

/// What one text keeps across wraps: its paragraphs' clusters, shaped when a
/// wrap first reaches them, and its most recent results.
struct WrapEntry {
    paragraphs: Vec<Option<BreakParagraph>>,
    results: Vec<WrapResult<WrappedText>>,
}

impl WrapEntry {
    fn retained_bytes(&self, key: &TextWrapKey) -> usize {
        let paragraphs: usize = self
            .paragraphs
            .iter()
            .flatten()
            .map(BreakParagraph::retained_bytes)
            .sum();
        let lines: usize = self
            .results
            .iter()
            .flat_map(|result| &result.wrapped.lines)
            .map(|line| line.content.len() + size_of::<TextLine>())
            .sum();
        key.text.retained_bytes() + paragraphs + lines
    }
}

/// Wrapped results kept across frames, bounded like the single-line metrics.
pub(super) struct WrapCache {
    entries: FrameLru<TextWrapKey, WrapEntry>,
}

impl WrapCache {
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

/// A line as byte offsets into its paragraph.
pub(super) struct LineSpan {
    pub(super) paragraph: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) width: f32,
}

impl TextMeasureCache {
    /// Break `request.content` into lines no wider than `max_width`.
    ///
    /// Lines break after whitespace and around CJK clusters; a word wider than
    /// the whole line is broken between clusters. `\n` always starts a new line.
    /// With `max_lines`, lines past that count are dropped and the last kept line
    /// ends in an ellipsis.
    ///
    /// A result is kept with the range of widths it holds for, so the same text
    /// asked again at any width in that range is neither wrapped nor shaped again;
    /// outside it, the text is rewrapped from its kept clusters.
    pub fn wrap(
        &mut self,
        request: TextRequest<'_>,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<WrappedText, TextError> {
        self.with_wrapped(request, max_width, max_lines, |wrapped| (**wrapped).clone())
    }

    /// [`Self::wrap`] sharing the kept result instead of copying its lines.
    pub fn wrap_shared(
        &mut self,
        request: TextRequest<'_>,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<Arc<WrappedText>, TextError> {
        self.with_wrapped(request, max_width, max_lines, Arc::clone)
    }

    /// The size [`Self::wrap`] would report, without copying its lines.
    pub fn wrapped_size(
        &mut self,
        request: TextRequest<'_>,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<Size, TextError> {
        self.with_wrapped(request, max_width, max_lines, |wrapped| wrapped.size)
    }

    fn with_wrapped<R>(
        &mut self,
        request: TextRequest<'_>,
        max_width: f32,
        max_lines: Option<usize>,
        read: impl FnOnce(&Arc<WrappedText>) -> R,
    ) -> Result<R, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(read(&Arc::new(WrappedText::empty())));
        }
        let primary_index = self.primary_font_index(request.font_family, request.font_weight)?;
        let family = self.fonts[primary_index].family.as_str();
        let mut hasher = request_hasher(&request, family);
        max_lines.hash(&mut hasher);
        let hash = hasher.finish();
        let matches =
            |key: &TextWrapKey| key.max_lines == max_lines && key.text.matches(&request, family);

        if let Some(entry) = self.wraps.entries.get_mut(hash, matches)
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
        let key = TextWrapKey {
            text: TextMeasureKey::from_request(request, family),
            max_lines,
        };
        let mut entry = self
            .wraps
            .entries
            .remove(hash, |stored| *stored == key)
            .unwrap_or_else(|| WrapEntry {
                paragraphs: request.content.split('\n').map(|_| None).collect(),
                results: Vec::new(),
            });
        let result = self.wrap_uncached(request, primary_index, max_width, max_lines, &mut entry);
        let value = read(&result.wrapped);
        entry.results.truncate(KEPT_RESULTS - 1);
        entry.results.insert(0, result);
        let bytes = entry.retained_bytes(&key);
        self.wraps.entries.insert(hash, key, entry, bytes);
        Ok(value)
    }

    fn wrap_uncached(
        &mut self,
        request: TextRequest<'_>,
        primary_index: usize,
        max_width: f32,
        max_lines: Option<usize>,
        entry: &mut WrapEntry,
    ) -> WrapResult<WrappedText> {
        let line_limit = max_lines.unwrap_or(usize::MAX);
        let paragraphs: Vec<&str> = request.content.split('\n').collect();
        let mut fits = WidthRange::ANY;
        let mut spans = Vec::new();
        for (index, paragraph) in paragraphs.iter().enumerate() {
            if spans.len() > line_limit {
                break;
            }
            if entry.paragraphs[index].is_none() {
                let parts = self.shape_counted(
                    TextRequest {
                        content: paragraph,
                        ..request
                    },
                    primary_index,
                );
                entry.paragraphs[index] = Some(BreakParagraph::new(
                    parts.clusters.iter().map(Into::into).collect(),
                ));
            }
            let layout = entry.paragraphs[index]
                .as_ref()
                .expect("paragraph clusters are shaped above");
            break_paragraph(index, layout, max_width, &mut fits, &mut spans);
        }

        let truncated = spans.len() > line_limit;
        spans.truncate(line_limit);
        // One copy of the text that every line slices.
        let text: Arc<str> = Arc::from(request.content);
        let mut lines: Vec<TextLine> = spans
            .iter()
            .map(|span| {
                let start = paragraphs[span.paragraph].as_ptr() as usize
                    - request.content.as_ptr() as usize;
                TextLine {
                    content: SharedStr::slice(&text, start + span.start..start + span.end),
                    width: span.width.ceil(),
                }
            })
            .collect();
        let mut ellipsis = None;
        if truncated && let (Some(span), Some(line)) = (spans.last(), lines.last_mut()) {
            let clusters = &entry.paragraphs[span.paragraph]
                .as_ref()
                .expect("a paragraph with lines was shaped")
                .clusters;
            let (cut, room) = self.ellipsize(
                request,
                primary_index,
                paragraphs[span.paragraph],
                clusters,
                span,
                max_width,
            );
            *line = cut;
            ellipsis = Some(room);
        }

        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = lines.len() as f32 * request.font_size * request.line_height;
        WrapResult {
            fits,
            ellipsis,
            wrapped: Arc::new(WrappedText {
                lines,
                size: Size::new(width, height),
            }),
        }
    }

    /// The line in `span`, cut so it still fits `max_width` with an ellipsis,
    /// and the room range and ellipsis width that cut holds for.
    fn ellipsize(
        &mut self,
        request: TextRequest<'_>,
        primary_index: usize,
        paragraph: &str,
        clusters: &[BreakCluster],
        span: &LineSpan,
        max_width: f32,
    ) -> (TextLine, (WidthRange, f32)) {
        let ellipsis_width = self
            .measure_prepared(
                TextRequest {
                    content: ELLIPSIS,
                    ..request
                },
                primary_index,
            )
            .size
            .width;
        let room = max_width - ellipsis_width;
        let mut room_range = WidthRange::ANY;
        let mut end = span.start;
        let mut width = 0.0;
        for cluster in clusters
            .iter()
            .filter(|cluster| cluster.byte_start >= span.start && cluster.byte_end <= span.end)
        {
            if room_range.split(width + cluster.advance_width, room) {
                break;
            }
            width += cluster.advance_width;
            end = cluster.byte_end;
        }
        let content = format!("{}{ELLIPSIS}", paragraph[span.start..end].trim_end());
        let width = self
            .measure_prepared(
                TextRequest {
                    content: &content,
                    ..request
                },
                primary_index,
            )
            .size
            .width;
        let content = content.into();
        (TextLine { content, width }, (room_range, ellipsis_width))
    }
}

/// A paragraph's clusters grouped at the places a line may break, with prefix
/// sums of their advances, so the width of any run of clusters is a subtraction.
pub(super) struct BreakParagraph {
    pub(super) clusters: Vec<BreakCluster>,
    /// `prefix[i]` is the advance of the clusters before `i`.
    prefix: Vec<f32>,
    segments: Vec<Segment>,
}

/// Clusters from one break opportunity to the next: content, then the
/// whitespace that hangs after it. The paragraph's leading whitespace belongs
/// to its first segment.
#[derive(Debug, Clone, Copy)]
struct Segment {
    start: usize,
    content_start: usize,
    content_end: usize,
}

impl BreakParagraph {
    /// Lines break after whitespace and before or after a CJK cluster.
    pub(super) fn new(clusters: Vec<BreakCluster>) -> Self {
        let mut prefix = Vec::with_capacity(clusters.len() + 1);
        let mut total = 0.0;
        prefix.push(total);
        let mut segments: Vec<Segment> = Vec::new();
        let mut current: Option<Segment> = None;
        for (index, cluster) in clusters.iter().enumerate() {
            total += cluster.advance_width;
            prefix.push(total);
            if cluster.script == TextScript::Whitespace {
                continue;
            }
            let segment = current.get_or_insert(Segment {
                start: 0,
                content_start: index,
                content_end: index,
            });
            let has_content = segment.content_end > segment.content_start;
            let opportunity = index > 0 && {
                let previous = clusters[index - 1].script;
                previous == TextScript::Whitespace
                    || cluster.script == TextScript::Cjk
                    || previous == TextScript::Cjk
            };
            if has_content && opportunity {
                segments.push(*segment);
                *segment = Segment {
                    start: index,
                    content_start: index,
                    content_end: index,
                };
            }
            segment.content_end = index + 1;
        }
        segments.extend(current);
        Self {
            clusters,
            prefix,
            segments,
        }
    }

    pub(super) fn width(&self, from: usize, to: usize) -> f32 {
        self.prefix[to] - self.prefix[from]
    }

    fn line(&self, paragraph: usize, start: usize, content_end: Option<usize>) -> LineSpan {
        let start_byte = if start == 0 {
            0
        } else {
            self.clusters[start].byte_start
        };
        match content_end {
            Some(end) => LineSpan {
                paragraph,
                start: start_byte,
                end: self.clusters[end - 1].byte_end,
                width: self.width(start, end),
            },
            None => LineSpan {
                paragraph,
                start: start_byte,
                end: start_byte,
                width: 0.0,
            },
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.clusters.len() * (size_of::<BreakCluster>() + size_of::<f32>())
            + self.segments.len() * size_of::<Segment>()
    }
}

/// Greedy line breaking of one paragraph into `spans`: each line takes whole
/// segments while their content fits, and a segment too wide for a line of its
/// own is broken between clusters.
///
/// `fits` narrows to the widths at which every fit test here comes out as it
/// did at `max_width`; the lines are the same anywhere in it.
pub(super) fn break_paragraph(
    paragraph: usize,
    layout: &BreakParagraph,
    max_width: f32,
    fits: &mut WidthRange,
    spans: &mut Vec<LineSpan>,
) {
    let first_line = spans.len();
    // First cluster of the current line, and the end of its content so far.
    let mut start = 0;
    let mut content_end: Option<usize> = None;
    for segment in &layout.segments {
        if content_end.is_some() && fits.split(layout.width(start, segment.content_end), max_width)
        {
            spans.push(layout.line(paragraph, start, content_end));
            start = segment.start;
            content_end = None;
        }
        if content_end.is_none() {
            // The line starts with this segment, whose first cluster is placed
            // whatever its width.
            for cluster in segment.content_start + 1..segment.content_end {
                if fits.split(layout.width(start, cluster + 1), max_width) {
                    spans.push(layout.line(paragraph, start, Some(cluster)));
                    start = cluster;
                }
            }
        }
        content_end = Some(segment.content_end);
    }
    if content_end.is_some() || spans.len() == first_line {
        spans.push(layout.line(paragraph, start, content_end));
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::PARAGRAPH;
    use super::*;

    fn request(content: &str) -> TextRequest<'_> {
        TextRequest::new(content, 20.0, 400, None, 1.2)
    }

    #[test]
    fn a_kept_wrap_answers_every_width_exactly_as_breaking_again_would() {
        let mut cache = TextMeasureCache::new();
        let mut reference = TextMeasureCache::new();
        if !cache.has_fonts() {
            return;
        }
        for max_lines in [None, Some(2)] {
            // Grow, shrink, then jump about, so results are found from either side.
            let widths = (0..=1200)
                .map(|step| step as f32 * 0.5)
                .chain((0..=1200).rev().map(|step| step as f32 * 0.5))
                .chain((0..400).map(|step| (step * 7919 % 600) as f32 + 0.25))
                .chain([f32::INFINITY, 0.0]);
            for width in widths {
                let kept = cache.wrap(request(PARAGRAPH), width, max_lines);
                // Forget every kept result, keeping only the shaped clusters.
                reference
                    .wraps
                    .entries
                    .for_each_value(|entry| entry.results.clear());
                let fresh = reference.wrap(request(PARAGRAPH), width, max_lines);
                assert_eq!(kept, fresh, "width {width}, max_lines {max_lines:?}");
                assert_eq!(
                    cache.wrapped_size(request(PARAGRAPH), width, max_lines),
                    fresh.map(|fresh| fresh.size)
                );
            }
        }
        assert!(cache.stats().wrap_hits > cache.stats().wrap_misses);
    }

    #[test]
    fn words_stay_whole_and_cjk_breaks_between_clusters() {
        let mut cache = TextMeasureCache::new();
        if !cache.has_fonts() {
            return;
        }
        let lines = |cache: &mut TextMeasureCache, content: &str, width: f32| {
            cache
                .wrap(request(content), width, None)
                .expect("wrap")
                .lines
                .into_iter()
                .map(|line| line.content.to_string())
                .collect::<Vec<_>>()
        };
        let widest_word = ["alpha", "beta", "gamma"]
            .map(|word| {
                cache
                    .measure_single_line(request(word))
                    .expect("measure")
                    .advance_width
            })
            .into_iter()
            .fold(0.0, f32::max);
        assert_eq!(
            lines(&mut cache, "alpha beta gamma", widest_word + 1.0),
            ["alpha", "beta", "gamma"]
        );
        assert_eq!(lines(&mut cache, "ab", 0.0), ["a", "b"]);
        assert_eq!(lines(&mut cache, "中文换行", 0.0), ["中", "文", "换", "行"]);
        assert_eq!(lines(&mut cache, "  lead", f32::INFINITY), ["  lead"]);
        assert_eq!(lines(&mut cache, "a\n\nb", f32::INFINITY), ["a", "", "b"]);
        let narrow = lines(&mut cache, "unbreakable", 30.0);
        assert!(narrow.len() > 1, "{narrow:?}");
        assert_eq!(narrow.concat(), "unbreakable");
    }
}
