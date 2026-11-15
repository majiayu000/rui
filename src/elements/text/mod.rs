//! Text element for rendering text

use crate::core::ElementId;
use crate::core::SharedStr;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::elements::element::{
    Capabilities, Element, LayoutContext, PaintContext, StableId, TextWrapRequest, WrappedLines,
    no_reach, style_to_taffy,
};
use crate::renderer::text::BaseDirection;
use crate::renderer::text::{TextMeasureCache, TextMetrics, TextRequest};
use crate::renderer::{Primitive, text_reach};
use std::any::Any;
use std::sync::Arc;
use taffy::prelude::*;

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Font weight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    Thin,
    Light,
    #[default]
    Regular,
    Medium,
    Semibold,
    Bold,
    Black,
}

impl FontWeight {
    pub fn to_value(&self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::Semibold => 600,
            FontWeight::Bold => 700,
            FontWeight::Black => 900,
        }
    }
}

/// Text style a [`Text`] resolves against during layout: its own settings
/// first, then the nearest ancestor that set the property, then these defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub font_family: Option<Arc<str>>,
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            font_size: 14.0,
            font_weight: FontWeight::Regular,
            font_family: None,
            line_height: 1.4,
        }
    }
}

impl TextStyle {
    /// This style with every property `refinement` sets replaced.
    pub fn refined(&self, refinement: &TextStyleRefinement) -> Self {
        Self {
            color: refinement.color.unwrap_or(self.color),
            font_size: refinement.font_size.unwrap_or(self.font_size),
            font_weight: refinement.font_weight.unwrap_or(self.font_weight),
            font_family: refinement
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
            line_height: refinement.line_height.unwrap_or(self.line_height),
        }
    }
}

/// Text properties an element sets for the text inside it; unset ones are
/// inherited from further up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextStyleRefinement {
    pub color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub font_family: Option<Arc<str>>,
    pub line_height: Option<f32>,
}

impl TextStyleRefinement {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Which text properties were set on the element itself rather than inherited.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct OwnTextStyle {
    color: bool,
    font_size: bool,
    font_weight: bool,
    font_family: bool,
    line_height: bool,
}

/// Text element
///
/// Text is one line by default and sized to its content. [`Text::wrap`] makes
/// it wrap to the width its container offers, and [`Text::max_lines`] cuts it
/// off with an ellipsis. [`Text::accessible`] exposes it to assistive
/// technology.
pub struct Text {
    id: Option<ElementId>,
    /// Set by [`Text::accessible`]: the id of its accessibility node.
    accessible: Option<StableId>,
    content: SharedStr,
    style: Style,
    color: Color,
    font_size: f32,
    font_weight: FontWeight,
    font_family: Option<Arc<str>>,
    line_height: f32,
    own: OwnTextStyle,
    align: TextAlign,
    max_lines: Option<usize>,
    /// Lines laid out for wrapped text; `None` keeps the text on one line.
    wrapped: Option<WrappedLines>,
    layout_node: Option<NodeId>,
}

impl Text {
    pub fn new(content: impl Into<SharedStr>) -> Self {
        let defaults = TextStyle::default();
        Self {
            id: None,
            accessible: None,
            content: content.into(),
            style: Style::new(),
            color: defaults.color,
            font_size: defaults.font_size,
            font_weight: defaults.font_weight,
            font_family: defaults.font_family,
            line_height: defaults.line_height,
            own: OwnTextStyle::default(),
            align: TextAlign::Left,
            max_lines: None,
            wrapped: None,
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

    /// Expose the text as an accessibility node labelled with its content,
    /// for text that is the only description of what it sits in. Such text is
    /// laid out and painted every frame: it registers an accessibility region.
    pub fn accessible(mut self) -> Self {
        self.accessible = Some(self.id.map_or_else(StableId::new, StableId::explicit));
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self.own.color = true;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self.own.font_size = true;
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self.own.font_weight = true;
        self
    }

    pub fn bold(mut self) -> Self {
        self.font_weight = FontWeight::Bold;
        self.own.font_weight = true;
        self
    }

    pub fn semibold(mut self) -> Self {
        self.font_weight = FontWeight::Semibold;
        self.own.font_weight = true;
        self
    }

    pub fn medium(mut self) -> Self {
        self.font_weight = FontWeight::Medium;
        self.own.font_weight = true;
        self
    }

    pub fn light(mut self) -> Self {
        self.font_weight = FontWeight::Light;
        self.own.font_weight = true;
        self
    }

    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(Arc::from(family.into()));
        self.own.font_family = true;
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self.own.line_height = true;
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    pub fn right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }

    /// Wrap onto as many lines as the container's width needs. The height
    /// reported to layout covers every line.
    pub fn wrap(mut self) -> Self {
        self.wrapped.get_or_insert_with(WrappedLines::default);
        self
    }

    /// Wrap, showing at most `lines` lines and ending the last one in an
    /// ellipsis when text is cut off.
    ///
    /// # Panics
    ///
    /// When `lines` is zero.
    pub fn max_lines(mut self, lines: usize) -> Self {
        assert!(lines > 0, "Text::max_lines needs at least one line");
        self.max_lines = Some(lines);
        self.wrap()
    }

    /// One line cut off with an ellipsis where it would overflow its container.
    pub fn truncate(self) -> Self {
        self.max_lines(1)
    }

    /// Take every property not set on this text from `inherited`.
    fn inherit(&mut self, inherited: &TextStyle) {
        if !self.own.color {
            self.color = inherited.color;
        }
        if !self.own.font_size {
            self.font_size = inherited.font_size;
        }
        if !self.own.font_weight {
            self.font_weight = inherited.font_weight;
        }
        if !self.own.font_family && self.font_family != inherited.font_family {
            self.font_family = inherited.font_family.clone();
        }
        if !self.own.line_height {
            self.line_height = inherited.line_height;
        }
    }

    fn primitive(&self, bounds: Bounds, content: SharedStr, direction: BaseDirection) -> Primitive {
        Primitive::Text {
            bounds,
            content,
            color: self.color.to_rgba(),
            font_size: self.font_size,
            font_weight: self.font_weight.to_value(),
            font_family: self.font_family.clone(),
            line_height: self.line_height,
            align: self.align,
            direction,
        }
    }

    fn measure_with(&self, cache: &mut TextMeasureCache) -> TextMetrics {
        match cache.measure_single_line(TextRequest::new(
            &self.content,
            self.font_size,
            self.font_weight.to_value(),
            self.font_family.as_deref(),
            self.line_height,
        )) {
            Ok(metrics) => metrics,
            Err(err) => panic!("text layout failed: {:?}", err),
        }
    }

    /// Measure text width using the configured font metrics.
    #[cfg(test)]
    fn estimate_width(&self) -> f32 {
        self.measure_with(&mut TextMeasureCache::new()).size.width
    }

    /// Measure text height using the configured line box.
    #[cfg(test)]
    fn estimate_height(&self) -> f32 {
        self.measure_with(&mut TextMeasureCache::new()).size.height
    }
}

impl Element for Text {
    fn id(&self) -> Option<ElementId> {
        self.accessible.map(|id| id.get()).or(self.id)
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        self.accessible.as_mut()
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn capabilities(&self) -> Capabilities {
        if self.accessible.is_some() {
            // Its accessibility region is registered in paint, which a
            // replay would skip.
            return Capabilities::default();
        }
        Capabilities::PURE_INERT
    }

    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<Text>() else {
            return false;
        };
        // Inherited properties are compared in layout, against the style the
        // previous text resolved them from.
        let own = self.own;
        own == previous.own
            && (!own.color || self.color == previous.color)
            && (!own.font_size || self.font_size == previous.font_size)
            && (!own.font_weight || self.font_weight == previous.font_weight)
            && (!own.font_family || self.font_family == previous.font_family)
            && (!own.line_height || self.line_height == previous.line_height)
            && self.id == previous.id
            && self.content == previous.content
            && self.style == previous.style
            && self.align == previous.align
            && self.max_lines == previous.max_lines
            && self.wrapped.is_some() == previous.wrapped.is_some()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inherit(cx.text_style());
        if let Some(lines) = &self.wrapped {
            let request = TextWrapRequest {
                content: self.content.share(),
                font_size: self.font_size,
                font_weight: self.font_weight.to_value(),
                font_family: self.font_family.clone(),
                line_height: self.line_height,
                max_lines: self.max_lines,
            };
            let node =
                match cx.new_wrapped_text_leaf(style_to_taffy(&self.style), request, lines.clone())
                {
                    Ok(node) => node,
                    Err(err) => panic!("failed to create text layout node: {:?}", err),
                };
            self.layout_node = Some(node);
            return node;
        }

        let metrics = self.measure_with(cx.text_measurer());

        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: Dimension::Length(metrics.size.width),
            height: Dimension::Length(metrics.size.height),
        };

        let node = match cx.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create text layout node: {:?}", err),
        };

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if let Some(id) = self.accessible {
            cx.register_accessibility_region(id.get(), cx.bounds());
        }
        if self.content.is_empty() || self.font_size <= 0.0 || self.line_height <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let Some(wrapped) = &self.wrapped else {
            let content = self.content.share();
            cx.paint(self.primitive(bounds, content, BaseDirection::Auto));
            return;
        };

        let lines = wrapped.borrow();
        let Some(lines) = lines.as_ref() else {
            panic!("wrapped text painted without LayoutContext::compute_layout measuring it");
        };
        let line_box = self.font_size * self.line_height;
        for (index, line) in lines.lines.iter().enumerate() {
            let line_bounds = Bounds::from_xywh(
                bounds.x(),
                bounds.y() + index as f32 * line_box,
                bounds.width(),
                line_box,
            );
            cx.paint(self.primitive(line_bounds, line.content.clone(), line.direction));
        }
    }

    fn unseen_reach(
        &self,
        _taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        visible: &Bounds,
    ) -> Option<Bounds> {
        if self.accessible.is_some() {
            return None;
        }
        if self.content.is_empty() || self.font_size <= 0.0 || self.line_height <= 0.0 {
            return Some(no_reach());
        }
        // The lines paint one below the other from the top of `bounds`.
        let height = match &self.wrapped {
            None => bounds.height(),
            Some(wrapped) => {
                wrapped.borrow().as_ref()?.lines.len() as f32 * self.font_size * self.line_height
            }
        };
        let painted = Bounds::from_xywh(bounds.x(), bounds.y(), bounds.width(), height);
        let reach = text_reach(painted, self.font_size);
        (!reach.intersects(visible)).then_some(reach)
    }

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
        let node = AccessibilityNode::new(id, AccessibilityRole::Text)
            .with_label(self.content.to_string())
            .with_focused(cx.a11y_has_focus(id));
        Ok(Some(node))
    }
}

/// Create a new Text element
pub fn text(content: impl Into<SharedStr>) -> Text {
    Text::new(content)
}

mod styled;

pub use styled::{StyledText, TextSpan, shared_styled_text, span, span_range, styled_text};

#[cfg(test)]
mod tests;
