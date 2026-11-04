//! Breaking a paragraph into lines that fit a width.

use super::{
    MAX_RETAINED_METRIC_BYTES, MAX_RETAINED_METRICS, TextError, TextMeasureCache, TextMeasureKey,
    TextRequest,
};
use crate::core::geometry::Size;
use crate::renderer::text_shaping::{TextCluster, TextScript};
use std::collections::HashMap;

const ELLIPSIS: &str = "\u{2026}";

/// One line of a wrapped paragraph, ready to be drawn on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub content: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextWrapKey {
    text: TextMeasureKey,
    width_bits: u32,
    max_lines: Option<usize>,
}

/// Wrapped results kept across frames, bounded like the single-line metrics.
#[derive(Default)]
pub(super) struct WrapCache {
    entries: HashMap<TextWrapKey, WrappedText>,
    retained_bytes: usize,
}

impl WrapCache {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.retained_bytes = 0;
    }

    fn retain(&mut self, key: TextWrapKey, wrapped: WrappedText) {
        let bytes = key.text.retained_bytes().saturating_mul(2);
        if bytes > MAX_RETAINED_METRIC_BYTES {
            return;
        }
        let exceeds_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .is_none_or(|total| total > MAX_RETAINED_METRIC_BYTES);
        if self.entries.len() >= MAX_RETAINED_METRICS || exceeds_bytes {
            self.clear();
        }
        self.retained_bytes += bytes;
        self.entries.insert(key, wrapped);
    }
}

/// A line as byte offsets into its paragraph.
struct LineSpan {
    paragraph: usize,
    start: usize,
    end: usize,
    width: f32,
}

impl TextMeasureCache {
    /// Break `request.content` into lines no wider than `max_width`.
    ///
    /// Lines break after whitespace and around CJK clusters; a word wider than
    /// the whole line is broken between clusters. `\n` always starts a new line.
    /// With `max_lines`, lines past that count are dropped and the last kept line
    /// ends in an ellipsis.
    pub fn wrap(
        &mut self,
        request: TextRequest<'_>,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> Result<WrappedText, TextError> {
        if request.content.is_empty() || request.font_size <= 0.0 || request.line_height <= 0.0 {
            return Ok(WrappedText::empty());
        }
        let primary_index = self.primary_font_index(request.font_family)?;
        let key = TextWrapKey {
            text: TextMeasureKey::from_request(request, &self.fonts[primary_index].family),
            width_bits: max_width.to_bits(),
            max_lines,
        };
        if let Some(wrapped) = self.wraps.entries.get(&key) {
            return Ok(wrapped.clone());
        }

        let wrapped = self.wrap_uncached(request, primary_index, max_width, max_lines);
        self.wraps.retain(key, wrapped.clone());
        Ok(wrapped)
    }

    fn wrap_uncached(
        &self,
        request: TextRequest<'_>,
        primary_index: usize,
        max_width: f32,
        max_lines: Option<usize>,
    ) -> WrappedText {
        let line_limit = max_lines.unwrap_or(usize::MAX);
        let paragraphs: Vec<&str> = request.content.split('\n').collect();
        let mut clusters = Vec::with_capacity(paragraphs.len());
        let mut spans = Vec::new();
        for (index, paragraph) in paragraphs.iter().enumerate() {
            if spans.len() > line_limit {
                break;
            }
            let paragraph_clusters = self
                .shape_with_primary(
                    TextRequest {
                        content: paragraph,
                        ..request
                    },
                    primary_index,
                )
                .clusters()
                .to_vec();
            break_paragraph(index, &paragraph_clusters, max_width, &mut spans);
            clusters.push(paragraph_clusters);
        }

        let truncated = spans.len() > line_limit;
        spans.truncate(line_limit);
        let mut lines: Vec<TextLine> = spans
            .iter()
            .map(|span| TextLine {
                content: paragraphs[span.paragraph][span.start..span.end].to_string(),
                width: span.width.ceil(),
            })
            .collect();
        if truncated && let (Some(span), Some(line)) = (spans.last(), lines.last_mut()) {
            *line = self.ellipsize(
                request,
                primary_index,
                paragraphs[span.paragraph],
                &clusters[span.paragraph],
                span,
                max_width,
            );
        }

        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = lines.len() as f32 * request.font_size * request.line_height;
        WrappedText {
            lines,
            size: Size::new(width, height),
        }
    }

    /// The line in `span`, cut so it still fits `max_width` with an ellipsis.
    fn ellipsize(
        &self,
        request: TextRequest<'_>,
        primary_index: usize,
        paragraph: &str,
        clusters: &[TextCluster],
        span: &LineSpan,
        max_width: f32,
    ) -> TextLine {
        let measure = |content: &str| {
            self.shape_with_primary(TextRequest { content, ..request }, primary_index)
                .metrics()
                .size
                .width
        };
        let room = max_width - measure(ELLIPSIS);
        let mut end = span.start;
        let mut width = 0.0;
        for cluster in clusters
            .iter()
            .filter(|cluster| cluster.byte_start >= span.start && cluster.byte_end <= span.end)
        {
            if width + cluster.advance_width > room {
                break;
            }
            width += cluster.advance_width;
            end = cluster.byte_end;
        }
        let content = format!("{}{ELLIPSIS}", paragraph[span.start..end].trim_end());
        let width = measure(&content);
        TextLine { content, width }
    }
}

/// Greedy line breaking of one paragraph's clusters into `spans`.
fn break_paragraph(
    paragraph: usize,
    clusters: &[TextCluster],
    max_width: f32,
    spans: &mut Vec<LineSpan>,
) {
    let first_line = spans.len();
    // Byte offset where the current line starts, and its width so far.
    let mut start = 0;
    let mut width = 0.0;
    // End of the last non-whitespace cluster and the line width up to it.
    let mut content_end = 0;
    let mut content_width = 0.0;
    // Last place the line may break: (line end, its width, next line start,
    // width consumed up to that start).
    let mut candidate: Option<(usize, f32, usize, f32)> = None;
    let mut previous_script = None;

    for cluster in clusters {
        let advance = cluster.advance_width;
        if cluster.script == TextScript::Whitespace {
            if content_end > start {
                candidate = Some((
                    content_end,
                    content_width,
                    cluster.byte_end,
                    width + advance,
                ));
            }
            width += advance;
            previous_script = Some(cluster.script);
            continue;
        }
        if content_end > start
            && previous_script != Some(TextScript::Whitespace)
            && (cluster.script == TextScript::Cjk || previous_script == Some(TextScript::Cjk))
        {
            candidate = Some((cluster.byte_start, width, cluster.byte_start, width));
        }
        while width + advance > max_width && content_end > start {
            let (end, end_width, next, next_width) =
                candidate
                    .take()
                    .unwrap_or((content_end, content_width, cluster.byte_start, width));
            spans.push(LineSpan {
                paragraph,
                start,
                end,
                width: end_width,
            });
            start = next;
            width -= next_width;
            content_width -= next_width;
        }
        width += advance;
        content_end = cluster.byte_end;
        content_width = width;
        previous_script = Some(cluster.script);
    }

    if content_end > start || spans.len() == first_line {
        spans.push(LineSpan {
            paragraph,
            start,
            end: content_end.max(start),
            width: if content_end > start {
                content_width
            } else {
                0.0
            },
        });
    }
}
