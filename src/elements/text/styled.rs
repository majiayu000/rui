//! Text with runs of different styles in one paragraph.

use super::{FontWeight, TextAlign, TextStyle, TextStyleRefinement};
use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::core::{ElementId, SharedStr};
use crate::elements::element::{
    Capabilities, Element, LayoutContext, PaintContext, RichLines, RichWrapRequest, style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use crate::renderer::text::{RichPieceText, RichSpan};
use std::any::Any;
use std::ops::Range;
use std::sync::{Arc, LazyLock};
use taffy::prelude::*;

static ELLIPSIS: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from("\u{2026}"));

/// A run of a [`StyledText`]: its text and the style properties it sets over
/// what the paragraph inherits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextSpan {
    content: SpanText,
    refinement: TextStyleRefinement,
}

/// A span's text: its own, or bytes of the text [`shared_styled_text`] shares.
#[derive(Debug, Clone, PartialEq)]
enum SpanText {
    Own(String),
    Bytes(Range<usize>),
}

impl Default for SpanText {
    fn default() -> Self {
        Self::Own(String::new())
    }
}

/// A run of text for [`styled_text`].
pub fn span(content: impl Into<String>) -> TextSpan {
    TextSpan {
        content: SpanText::Own(content.into()),
        refinement: TextStyleRefinement::default(),
    }
}

/// A run of the bytes `bytes` of the text given to [`shared_styled_text`].
pub fn span_range(bytes: Range<usize>) -> TextSpan {
    TextSpan {
        content: SpanText::Bytes(bytes),
        refinement: TextStyleRefinement::default(),
    }
}

impl TextSpan {
    /// Bytes of the paragraph's text the span covers.
    fn len(&self) -> usize {
        match &self.content {
            SpanText::Own(text) => text.len(),
            SpanText::Bytes(bytes) => bytes.len(),
        }
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.refinement.color = Some(color.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.refinement.font_size = Some(size);
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.refinement.font_weight = Some(weight);
        self
    }

    pub fn bold(self) -> Self {
        self.weight(FontWeight::Bold)
    }

    pub fn semibold(self) -> Self {
        self.weight(FontWeight::Semibold)
    }

    pub fn medium(self) -> Self {
        self.weight(FontWeight::Medium)
    }

    pub fn light(self) -> Self {
        self.weight(FontWeight::Light)
    }

    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.refinement.font_family = Some(Arc::from(family.into()));
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.refinement.line_height = Some(height);
        self
    }
}

/// One paragraph made of [`TextSpan`]s of different colors, sizes, weights or
/// fonts. It wraps to the width its container offers, breaking lines across
/// spans as if they were one text; every span inherits the text style of its
/// parents for what it does not set. A line is as tall as its tallest span,
/// and all spans on a line share one baseline.
pub struct StyledText {
    id: Option<ElementId>,
    style: Style,
    /// Every span as bytes of `content`, in order, covering it.
    spans: Vec<TextSpan>,
    content: Arc<str>,
    align: TextAlign,
    max_lines: Option<usize>,
    /// The style the spans refine, set in layout.
    inherited: TextStyle,
    lines: RichLines,
    layout_node: Option<NodeId>,
}

/// A paragraph of differently styled spans.
///
/// # Panics
///
/// When a span was made with [`span_range`]; those belong to
/// [`shared_styled_text`].
pub fn styled_text(spans: impl IntoIterator<Item = TextSpan>) -> StyledText {
    let mut content = String::new();
    let spans = spans
        .into_iter()
        .map(|span| {
            let start = content.len();
            match &span.content {
                SpanText::Own(text) => content.push_str(text),
                SpanText::Bytes(bytes) => panic!(
                    "span_range({bytes:?}) has no text of its own; place it with shared_styled_text"
                ),
            }
            TextSpan {
                content: SpanText::Bytes(start..content.len()),
                refinement: span.refinement,
            }
        })
        .collect();
    StyledText::new(content.into(), spans)
}

/// A paragraph of spans made with [`span_range`] over one shared `text`, such
/// as a line an editor keeps: the text is not copied, and spans need no text
/// of their own.
///
/// # Panics
///
/// When the spans do not cover `text` in order, from its first byte to its
/// last, each starting and ending on a character boundary, or when a span was
/// made with [`span`].
pub fn shared_styled_text(
    text: impl Into<Arc<str>>,
    spans: impl IntoIterator<Item = TextSpan>,
) -> StyledText {
    let text = text.into();
    let mut end = 0;
    let spans: Vec<TextSpan> = spans
        .into_iter()
        .inspect(|span| {
            let SpanText::Bytes(bytes) = &span.content else {
                panic!("shared_styled_text takes spans made with span_range, not span");
            };
            if bytes.start != end
                || bytes.end < bytes.start
                || !text.is_char_boundary(bytes.start)
                || !text.is_char_boundary(bytes.end)
            {
                panic!(
                    "span_range({bytes:?}) does not follow byte {end} of a {}-byte text on character boundaries",
                    text.len()
                );
            }
            end = bytes.end;
        })
        .collect();
    if end != text.len() {
        panic!(
            "the spans of shared_styled_text cover {end} of its {} bytes",
            text.len()
        );
    }
    StyledText::new(text, spans)
}

impl StyledText {
    fn new(content: Arc<str>, spans: Vec<TextSpan>) -> Self {
        Self {
            id: None,
            style: Style::new(),
            spans,
            content,
            align: TextAlign::Left,
            max_lines: None,
            inherited: TextStyle::default(),
            lines: RichLines::default(),
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Add `span` after the others.
    ///
    /// # Panics
    ///
    /// When `span` was made with [`span_range`].
    pub fn span(mut self, span: TextSpan) -> Self {
        let SpanText::Own(text) = &span.content else {
            panic!("StyledText::span takes a span with its own text, made with span");
        };
        let start = self.content.len();
        self.content = format!("{}{text}", self.content).into();
        self.spans.push(TextSpan {
            content: SpanText::Bytes(start..self.content.len()),
            refinement: span.refinement,
        });
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn center(self) -> Self {
        self.align(TextAlign::Center)
    }

    pub fn right(self) -> Self {
        self.align(TextAlign::Right)
    }

    /// Show at most `lines` lines, ending the last one in an ellipsis when text
    /// is cut off.
    ///
    /// # Panics
    ///
    /// When `lines` is zero.
    pub fn max_lines(mut self, lines: usize) -> Self {
        assert!(lines > 0, "StyledText::max_lines needs at least one line");
        self.max_lines = Some(lines);
        self
    }

    /// One line cut off with an ellipsis where it would overflow.
    pub fn truncate(self) -> Self {
        self.max_lines(1)
    }

    /// The whole text, every span's content in order.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Element for StyledText {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::PURE_INERT
    }

    /// Kept when every span sets the same text and style properties. What
    /// the spans inherit is compared in layout, against the style the kept
    /// text resolved them from.
    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<StyledText>() else {
            return false;
        };
        self.id == previous.id
            && self.spans == previous.spans
            && self.content == previous.content
            && self.style == previous.style
            && self.align == previous.align
            && self.max_lines == previous.max_lines
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inherited = cx.text_style().clone();
        let inherited = &self.inherited;
        // Copied once for every span that does not set its own.
        let request = RichWrapRequest {
            content: Arc::clone(&self.content),
            spans: self
                .spans
                .iter()
                .map(|span| {
                    let refinement = &span.refinement;
                    RichSpan {
                        len: span.len(),
                        font_size: refinement.font_size.unwrap_or(inherited.font_size),
                        font_weight: refinement
                            .font_weight
                            .unwrap_or(inherited.font_weight)
                            .to_value(),
                        font_family: refinement
                            .font_family
                            .clone()
                            .or_else(|| inherited.font_family.clone()),
                        line_height: refinement.line_height.unwrap_or(inherited.line_height),
                    }
                })
                .collect(),
            max_lines: self.max_lines,
        };
        let node =
            match cx.new_rich_text_leaf(style_to_taffy(&self.style), request, self.lines.clone()) {
                Ok(node) => node,
                Err(err) => panic!("failed to create styled text layout node: {:?}", err),
            };
        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if self.content.is_empty() {
            return;
        }
        let bounds = cx.bounds();
        let lines = self.lines.borrow();
        let Some(wrapped) = lines.as_ref() else {
            panic!("styled text painted without LayoutContext::compute_layout measuring it");
        };
        let mut y = bounds.y();
        for line in &wrapped.lines {
            let offset = match self.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (bounds.width() - line.width) * 0.5,
                TextAlign::Right => bounds.width() - line.width,
            };
            for piece in &line.pieces {
                let refinement = &self.spans[piece.span].refinement;
                let inherited = &self.inherited;
                let content = match &piece.text {
                    RichPieceText::Content(bytes) => SharedStr::slice(&self.content, bytes.clone()),
                    RichPieceText::Ellipsis => SharedStr::from(ELLIPSIS.clone()),
                };
                cx.paint(Primitive::Text {
                    bounds: Bounds::from_xywh(
                        bounds.x() + offset + piece.x + piece.ink_x,
                        y + piece.top,
                        piece.width,
                        piece.height,
                    ),
                    content,
                    color: refinement.color.unwrap_or(inherited.color).to_rgba(),
                    font_size: refinement.font_size.unwrap_or(inherited.font_size),
                    font_weight: refinement
                        .font_weight
                        .unwrap_or(inherited.font_weight)
                        .to_value(),
                    font_family: refinement
                        .font_family
                        .as_ref()
                        .or(inherited.font_family.as_ref())
                        .cloned(),
                    line_height: refinement.line_height.unwrap_or(inherited.line_height),
                    align: TextAlign::Left,
                    direction: BaseDirection::Auto,
                });
            }
            y += line.height;
        }
    }
}
