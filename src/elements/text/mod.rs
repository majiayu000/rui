//! Text element for rendering text

use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::style::Style;
use crate::elements::element::{Element, LayoutContext, PaintContext, style_to_taffy};
use crate::renderer::Primitive;
use crate::renderer::text::{TextMeasureCache, TextMetrics, TextRequest};
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

/// Text element
pub struct Text {
    id: Option<ElementId>,
    content: String,
    style: Style,
    color: Color,
    font_size: f32,
    font_weight: FontWeight,
    font_family: Option<String>,
    line_height: f32,
    align: TextAlign,
    layout_node: Option<NodeId>,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: None,
            content: content.into(),
            style: Style::new(),
            color: Color::BLACK,
            font_size: 14.0,
            font_weight: FontWeight::Regular,
            font_family: None,
            line_height: 1.4,
            align: TextAlign::Left,
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    pub fn bold(mut self) -> Self {
        self.font_weight = FontWeight::Bold;
        self
    }

    pub fn semibold(mut self) -> Self {
        self.font_weight = FontWeight::Semibold;
        self
    }

    pub fn medium(mut self) -> Self {
        self.font_weight = FontWeight::Medium;
        self
    }

    pub fn light(mut self) -> Self {
        self.font_weight = FontWeight::Light;
        self
    }

    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
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
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let metrics = self.measure_with(cx.text_measurer());

        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: Dimension::Length(metrics.size.width),
            height: Dimension::Length(metrics.size.height),
        };

        let node = match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create text layout node: {:?}", err),
        };

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if self.content.is_empty() || self.font_size <= 0.0 || self.line_height <= 0.0 {
            return;
        }

        let bounds = cx.bounds();

        cx.paint(Primitive::Text {
            bounds,
            content: self.content.clone(),
            color: self.color.to_rgba(),
            font_size: self.font_size,
            font_weight: self.font_weight.to_value(),
            font_family: self.font_family.clone(),
            line_height: self.line_height,
            align: self.align,
        });
    }
}

/// Create a new Text element
pub fn text(content: impl Into<String>) -> Text {
    Text::new(content)
}

#[cfg(test)]
mod tests;
