//! Text with runs of different styles in one paragraph.

use super::{FontWeight, TextAlign, TextStyle, TextStyleRefinement};
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::event::Cursor;
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::core::{ElementId, SharedStr};
use crate::elements::element::{
    Capabilities, Element, EventContext, LayoutContext, PaintContext, PointerEvent, StableId,
    PointerEventKind, RichLines, RichWrapRequest, style_to_taffy,
};
use crate::elements::selection::{LeafFont, LeafLayout, SelectableLeaf};
use crate::elements::{SelectionScope, selection_scope};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use crate::renderer::text::{RichPiece, RichPieceText, RichSpan, RichWrapped};
use std::any::Any;
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use taffy::prelude::*;

static ELLIPSIS: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from("\u{2026}"));

/// Called with the target of a clicked link span, shared by the paragraphs
/// of a document.
pub type LinkHandler = Rc<dyn Fn(&str)>;

/// What a span draws around its glyphs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct SpanDecoration {
    background: Option<Color>,
    underline: bool,
    strikethrough: bool,
}

/// A run of a [`StyledText`]: its text and the style properties it sets over
/// what the paragraph inherits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextSpan {
    content: SpanText,
    refinement: TextStyleRefinement,
    italic: bool,
    decoration: SpanDecoration,
    link: Option<Arc<str>>,
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
        ..TextSpan::default()
    }
}

/// A run of the bytes `bytes` of the text given to [`shared_styled_text`].
pub fn span_range(bytes: Range<usize>) -> TextSpan {
    TextSpan {
        content: SpanText::Bytes(bytes),
        ..TextSpan::default()
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

    /// This span placed at `bytes` of the paragraph's text.
    fn placed(self, bytes: Range<usize>) -> Self {
        Self {
            content: SpanText::Bytes(bytes),
            ..self
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

    /// Draw with the italic face of the span's family, where it has one.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Fill the span's line box behind its glyphs, as inline code is.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.decoration.background = Some(color.into());
        self
    }

    /// A line under the glyphs, in the span's color.
    pub fn underline(mut self) -> Self {
        self.decoration.underline = true;
        self
    }

    /// A line through the glyphs, in the span's color.
    pub fn strikethrough(mut self) -> Self {
        self.decoration.strikethrough = true;
        self
    }

    /// Make the span a link to `target`: [`StyledText::on_link`] is called
    /// with it when the span is clicked, and the pointer shows over it.
    pub fn link(mut self, target: impl Into<Arc<str>>) -> Self {
        self.link = Some(target.into());
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
    /// The id of its accessibility node, read as static text; `None` once
    /// [`StyledText::accessibility_hidden`] leaves it out.
    accessible: Option<StableId>,
    style: Style,
    /// Every span as bytes of `content`, in order, covering it.
    spans: Vec<TextSpan>,
    content: Arc<str>,
    align: TextAlign,
    max_lines: Option<usize>,
    on_link: Option<LinkHandler>,
    /// The style the spans refine, set in layout.
    inherited: TextStyle,
    /// Each span's font family, resolved in layout.
    families: Vec<Option<Arc<str>>>,
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
            span.placed(start..content.len())
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
            accessible: Some(StableId::new()),
            style: Style::new(),
            spans,
            content,
            align: TextAlign::Left,
            max_lines: None,
            on_link: None,
            inherited: TextStyle::default(),
            families: Vec::new(),
            lines: RichLines::default(),
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        if self.accessible.is_some() {
            self.accessible = Some(StableId::explicit(id));
        }
        self
    }

    /// Leave the paragraph out of the accessibility tree.
    pub fn accessibility_hidden(mut self) -> Self {
        self.accessible = None;
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
        let end = self.content.len();
        self.spans.push(span.placed(start..end));
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

    /// Called with the target of a span made with [`TextSpan::link`] when a
    /// click presses and releases on it.
    pub fn on_link(self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_link_rc(Rc::new(handler))
    }

    /// [`StyledText::on_link`] with a handler several paragraphs share.
    pub fn on_link_rc(mut self, handler: LinkHandler) -> Self {
        self.on_link = Some(handler);
        self
    }

    /// This paragraph in a [`SelectionScope`] of its own, so it can be
    /// selected and copied. Text inside a scope is selectable without this.
    pub fn selectable(self) -> SelectionScope {
        selection_scope(self)
    }

    /// What a selection scope needs to know of this paragraph painted at
    /// `bounds`, once laid out.
    pub(crate) fn selectable_leaf(
        &self,
        bounds: Bounds,
        clip: Option<Bounds>,
    ) -> Option<SelectableLeaf> {
        let lines = self.lines.borrow().clone()?;
        if self.content.is_empty() {
            return None;
        }
        let inherited = &self.inherited;
        let fonts = self
            .spans
            .iter()
            .zip(&self.families)
            .map(|(span, family)| {
                let refinement = &span.refinement;
                LeafFont {
                    size: refinement.font_size.unwrap_or(inherited.font_size),
                    weight: refinement
                        .font_weight
                        .unwrap_or(inherited.font_weight)
                        .to_value(),
                    family: family.clone(),
                    line_height: refinement.line_height.unwrap_or(inherited.line_height),
                }
            })
            .collect();
        let layout = LeafLayout::Rich {
            fonts,
            align: self.align,
            lines,
        };
        Some(SelectableLeaf::new(
            bounds,
            clip,
            SharedStr::new(Arc::clone(&self.content)),
            layout,
        ))
    }

    /// The whole text, every span's content in order.
    pub fn content(&self) -> &str {
        &self.content
    }

    fn span_color(&self, span: &TextSpan) -> Color {
        span.refinement.color.unwrap_or(self.inherited.color)
    }

    /// The link under `point`, in window coordinates, in a paragraph laid out
    /// at `bounds`.
    fn link_at(&self, bounds: Bounds, point: Point) -> Option<Arc<str>> {
        let lines = self.lines.borrow();
        let wrapped = lines.as_ref()?;
        let mut hit = None;
        self.for_each_piece(wrapped, bounds, |piece, x, y| {
            let link = self.spans[piece.span].link.as_ref();
            if let Some(link) = link
                && point.x >= x + piece.x
                && point.x < x + piece.x + piece.width
                && point.y >= y + piece.top
                && point.y < y + piece.top + piece.height
            {
                hit = Some(Arc::clone(link));
            }
        });
        hit
    }

    /// Calls `visit` with each piece and the left edge and top of its line.
    fn for_each_piece(
        &self,
        wrapped: &RichWrapped,
        bounds: Bounds,
        mut visit: impl FnMut(&RichPiece, f32, f32),
    ) {
        let mut y = bounds.y();
        for line in &wrapped.lines {
            let offset = match self.align {
                TextAlign::Left => 0.0,
                TextAlign::Center => (bounds.width() - line.width) * 0.5,
                TextAlign::Right => bounds.width() - line.width,
            };
            for piece in &line.pieces {
                visit(piece, bounds.x() + offset, y);
            }
            y += line.height;
        }
    }
}

fn fill(bounds: Bounds, color: Color) -> Primitive {
    Primitive::Quad {
        bounds,
        background: color.to_rgba(),
        border_color: Color::TRANSPARENT.to_rgba(),
        border_widths: Edges::all(0.0),
        corner_radii: Corners::all(0.0),
    }
}

impl Element for StyledText {
    // Only an id the application set: the accessibility node's adopted id is
    // derived from the text's place, and layout derives that again each pass.
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        self.accessible.as_mut()
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            pure: true,
            pointer_inert: self.on_link.is_none(),
        }
    }

    /// Kept when every span sets the same text and style properties. What
    /// the spans inherit is compared in layout, against the style the kept
    /// text resolved them from. A link handler cannot be compared, so text
    /// with one keeps the new instance.
    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<StyledText>() else {
            return false;
        };
        self.on_link.is_none()
            && previous.on_link.is_none()
            && self.id == previous.id
            && self.spans == previous.spans
            && self.content == previous.content
            && self.style == previous.style
            && self.align == previous.align
            && self.max_lines == previous.max_lines
            && self.accessible.is_some() == previous.accessible.is_some()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inherited = cx.text_style().clone();
        let inherited = &self.inherited;
        // Shared once for every span that does not set its own.
        self.families = self
            .spans
            .iter()
            .map(|span| {
                let family = span
                    .refinement
                    .font_family
                    .as_ref()
                    .or(inherited.font_family.as_ref());
                match (span.italic, family) {
                    (false, family) => family.cloned(),
                    (true, Some(family)) => Some(Arc::from(format!("{family} Italic"))),
                    (true, None) => Some(Arc::from("Italic")),
                }
            })
            .collect();
        let request = RichWrapRequest {
            content: Arc::clone(&self.content),
            spans: self
                .spans
                .iter()
                .zip(&self.families)
                .map(|(span, family)| {
                    let refinement = &span.refinement;
                    RichSpan {
                        len: span.len(),
                        font_size: refinement.font_size.unwrap_or(inherited.font_size),
                        font_weight: refinement
                            .font_weight
                            .unwrap_or(inherited.font_weight)
                            .to_value(),
                        font_family: family.clone(),
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

    /// Read as one piece of static text: the paragraph's characters, in
    /// order, without the styling its spans add.
    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let Some(id) = self.accessible.map(|id| id.get()) else {
            return Ok(None);
        };
        if self.content.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(
            AccessibilityNode::new(id, AccessibilityRole::Text)
                .with_label(self.content.to_string())
                .with_focused(cx.a11y_has_focus(id)),
        ))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if let Some(id) = self.accessible {
            cx.register_accessibility_region(id.get(), cx.bounds());
        }
        if self.content.is_empty() {
            return;
        }
        let bounds = cx.bounds();
        let lines = self.lines.borrow();
        let Some(wrapped) = lines.as_ref() else {
            panic!("styled text painted without LayoutContext::compute_layout measuring it");
        };
        // Backgrounds first. They cover the span's advance, from its pen
        // position, not its ink: reaching past it would cover the spaces of
        // the spans beside it.
        self.for_each_piece(wrapped, bounds, |piece, x, y| {
            if let Some(color) = self.spans[piece.span].decoration.background {
                let radius = piece.height * 0.15;
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(
                        x + piece.x,
                        y + piece.top,
                        piece.width,
                        piece.height,
                    ),
                    background: color.to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::all(0.0),
                    corner_radii: Corners::all(radius),
                });
            }
        });
        self.for_each_piece(wrapped, bounds, |piece, x, y| {
            let span = &self.spans[piece.span];
            let refinement = &span.refinement;
            let inherited = &self.inherited;
            let content = match &piece.text {
                RichPieceText::Content(bytes) => SharedStr::slice(&self.content, bytes.clone()),
                RichPieceText::Ellipsis => SharedStr::from(ELLIPSIS.clone()),
            };
            let font_size = refinement.font_size.unwrap_or(inherited.font_size);
            let color = self.span_color(span);
            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(
                    x + piece.x + piece.ink_x,
                    y + piece.top,
                    piece.width,
                    piece.height,
                ),
                content,
                color: color.to_rgba(),
                font_size,
                font_weight: refinement
                    .font_weight
                    .unwrap_or(inherited.font_weight)
                    .to_value(),
                font_family: self.families.get(piece.span).cloned().flatten(),
                line_height: refinement.line_height.unwrap_or(inherited.line_height),
                align: TextAlign::Left,
                direction: BaseDirection::Auto,
            });
            let decoration = span.decoration;
            if !decoration.underline && !decoration.strikethrough {
                return;
            }
            // The line box centers the font's em: the baseline sits about
            // 0.3 em below its middle, the x-height's middle 0.25 em above it.
            let middle = y + piece.top + piece.height * 0.5;
            let thickness = (font_size / 14.0).max(1.0);
            let line = |offset: f32| {
                fill(
                    Bounds::from_xywh(
                        x + piece.x,
                        middle + offset * font_size - thickness * 0.5,
                        piece.width,
                        thickness,
                    ),
                    color,
                )
            };
            if decoration.underline {
                cx.paint(line(0.42));
            }
            if decoration.strikethrough {
                cx.paint(line(0.05));
            }
        });
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let Some(handler) = self.on_link.clone() else {
            return false;
        };
        if !cx.contains_pointer(event.position) {
            return false;
        }
        let Some(link) = self.link_at(cx.bounds(), event.position) else {
            return false;
        };
        match event.kind {
            PointerEventKind::Move => {
                cx.set_cursor(Cursor::Pointer);
                false
            }
            PointerEventKind::Down => true,
            PointerEventKind::Up => {
                handler(&link);
                true
            }
        }
    }
}
