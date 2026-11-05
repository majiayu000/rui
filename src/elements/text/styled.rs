//! Text with runs of different styles in one paragraph.

use super::{FontWeight, TextAlign, TextStyle, TextStyleRefinement};
use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::elements::element::{
    Element, LayoutContext, PaintContext, RichLines, RichWrapRequest, style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::{RichPieceText, RichSpan};
use std::any::Any;
use taffy::prelude::*;

/// A run of a [`StyledText`]: its text and the style properties it sets over
/// what the paragraph inherits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextSpan {
    content: String,
    refinement: TextStyleRefinement,
}

/// A run of text for [`styled_text`].
pub fn span(content: impl Into<String>) -> TextSpan {
    TextSpan {
        content: content.into(),
        refinement: TextStyleRefinement::default(),
    }
}

impl TextSpan {
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
        self.refinement.font_family = Some(family.into());
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
    spans: Vec<TextSpan>,
    content: String,
    align: TextAlign,
    max_lines: Option<usize>,
    /// Each span's style once inherited, set in layout.
    resolved: Vec<TextStyle>,
    lines: RichLines,
    layout_node: Option<NodeId>,
}

/// A paragraph of differently styled spans.
pub fn styled_text(spans: impl IntoIterator<Item = TextSpan>) -> StyledText {
    let spans: Vec<TextSpan> = spans.into_iter().collect();
    let content = spans.iter().map(|span| span.content.as_str()).collect();
    StyledText {
        id: None,
        style: Style::new(),
        spans,
        content,
        align: TextAlign::Left,
        max_lines: None,
        resolved: Vec::new(),
        lines: RichLines::default(),
        layout_node: None,
    }
}

impl StyledText {
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Add `span` after the others.
    pub fn span(mut self, span: TextSpan) -> Self {
        self.content.push_str(&span.content);
        self.spans.push(span);
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

    fn memoizable(&self) -> bool {
        true
    }

    /// Kept when every span sets the same text and style properties. What
    /// the spans inherit is compared in layout, against the style the kept
    /// text resolved them from.
    fn reconcile(&mut self, previous: &mut dyn Element) -> bool {
        let Some(previous) = (previous as &mut dyn Any).downcast_ref::<StyledText>() else {
            return false;
        };
        self.id == previous.id
            && self.spans == previous.spans
            && self.style == previous.style
            && self.align == previous.align
            && self.max_lines == previous.max_lines
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let inherited = cx.text_style();
        self.resolved = self
            .spans
            .iter()
            .map(|span| inherited.refined(&span.refinement))
            .collect();
        let request = RichWrapRequest {
            content: self.content.clone(),
            spans: self
                .spans
                .iter()
                .zip(&self.resolved)
                .map(|(span, style)| RichSpan {
                    len: span.content.len(),
                    font_size: style.font_size,
                    font_weight: style.font_weight.to_value(),
                    font_family: style.font_family.clone(),
                    line_height: style.line_height,
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
                let style = &self.resolved[piece.span];
                let content = match &piece.text {
                    RichPieceText::Content(bytes) => self.content[bytes.clone()].to_string(),
                    RichPieceText::Ellipsis => "\u{2026}".to_string(),
                };
                cx.paint(Primitive::Text {
                    bounds: Bounds::from_xywh(
                        bounds.x() + offset + piece.x + piece.ink_x,
                        y + piece.top,
                        piece.width,
                        piece.height,
                    ),
                    content,
                    color: style.color.to_rgba(),
                    font_size: style.font_size,
                    font_weight: style.font_weight.to_value(),
                    font_family: style.font_family.clone(),
                    line_height: style.line_height,
                    align: TextAlign::Left,
                });
            }
            y += line.height;
        }
    }
}
