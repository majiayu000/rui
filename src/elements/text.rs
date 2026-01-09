//! Text element for rendering text

use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
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

    /// Estimate text width (simplified - real implementation would use font metrics)
    fn estimate_width(&self) -> f32 {
        // Rough estimate: average char width is about 0.5 * font_size
        self.content.len() as f32 * self.font_size * 0.5
    }

    /// Estimate text height
    fn estimate_height(&self) -> f32 {
        self.font_size * self.line_height
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
        // For text, we compute the size based on content
        let width = self.estimate_width();
        let height = self.estimate_height();

        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        };

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create text layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
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
