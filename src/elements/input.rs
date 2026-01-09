//! Text input element

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::core::event::Cursor;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// Input type variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    #[default]
    Text,
    Password,
    Email,
    Number,
    Search,
}

/// Input state
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub value: String,
    pub cursor_position: usize,
    pub selection_start: Option<usize>,
    pub selection_end: Option<usize>,
    pub focused: bool,
    pub hovered: bool,
}

/// Text input component
pub struct Input {
    id: Option<ElementId>,
    placeholder: String,
    input_type: InputType,
    style: Style,
    state: InputState,
    width: Option<f32>,
    on_change: Option<Box<dyn Fn(&str)>>,
    on_submit: Option<Box<dyn Fn(&str)>>,
    on_focus: Option<Box<dyn Fn()>>,
    on_blur: Option<Box<dyn Fn()>>,
    layout_node: Option<NodeId>,
}

impl Input {
    pub fn new() -> Self {
        let mut style = Style::new();
        style.border.radius = Corners::all(6.0);
        style.border.color = Color::hex(0xd1d5db);
        style.border.width = Edges::all(1.0);

        Self {
            id: None,
            placeholder: String::new(),
            input_type: InputType::default(),
            style,
            state: InputState::default(),
            width: None,
            on_change: None,
            on_submit: None,
            on_focus: None,
            on_blur: None,
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.state.value = value.into();
        self.state.cursor_position = self.state.value.len();
        self
    }

    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.input_type = input_type;
        self
    }

    pub fn password(mut self) -> Self {
        self.input_type = InputType::Password;
        self
    }

    pub fn email(mut self) -> Self {
        self.input_type = InputType::Email;
        self
    }

    pub fn number(mut self) -> Self {
        self.input_type = InputType::Number;
        self
    }

    pub fn search(mut self) -> Self {
        self.input_type = InputType::Search;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border.radius = Corners::all(radius);
        self
    }

    pub fn border_color(mut self, color: impl Into<Color>) -> Self {
        self.style.border.color = color.into();
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    pub fn on_focus(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }

    pub fn on_blur(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_blur = Some(Box::new(handler));
        self
    }

    /// Get display text (masked for password)
    fn display_text(&self) -> String {
        if self.input_type == InputType::Password {
            "•".repeat(self.state.value.len())
        } else {
            self.state.value.clone()
        }
    }

    fn colors(&self) -> (Color, Color, Color) {
        let bg = Color::WHITE;
        let text = if self.state.value.is_empty() {
            Color::hex(0x9ca3af) // placeholder color
        } else {
            Color::hex(0x111827)
        };
        let border = if self.state.focused {
            Color::hex(0x6366f1) // focus ring
        } else if self.state.hovered {
            Color::hex(0x9ca3af)
        } else {
            Color::hex(0xd1d5db)
        };
        (bg, text, border)
    }

    pub fn cursor(&self) -> Cursor {
        Cursor::Text
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for Input {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.height = Dimension::Length(40.0);
        if let Some(w) = self.width {
            style.size.width = Dimension::Length(w);
        } else {
            style.flex_grow = 1.0;
        }
        style.padding = taffy::Rect {
            top: LengthPercentage::Length(8.0),
            right: LengthPercentage::Length(12.0),
            bottom: LengthPercentage::Length(8.0),
            left: LengthPercentage::Length(12.0),
        };

        let node = cx.taffy.new_leaf(style).expect("Failed to create input layout node");
        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let (bg, text_color, border_color) = self.colors();

        // Paint background
        cx.paint(Primitive::Quad {
            bounds,
            background: bg.to_rgba(),
            border_color: border_color.to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: self.style.border.radius,
        });

        // Paint focus ring
        if self.state.focused {
            let ring_bounds = Bounds::from_xywh(
                bounds.x() - 2.0,
                bounds.y() - 2.0,
                bounds.width() + 4.0,
                bounds.height() + 4.0,
            );
            cx.paint(Primitive::Quad {
                bounds: ring_bounds,
                background: crate::core::color::Rgba::TRANSPARENT,
                border_color: Color::hex(0x6366f1).with_alpha(0.3).to_rgba(),
                border_widths: Edges::all(2.0),
                corner_radii: Corners::all(8.0),
            });
        }

        // Paint text or placeholder
        let display = if self.state.value.is_empty() {
            &self.placeholder
        } else {
            &self.display_text()
        };

        if !display.is_empty() {
            let text_x = bounds.x() + 12.0;
            let text_y = bounds.y() + (bounds.height() - 14.0) / 2.0;
            let text_width = bounds.width() - 24.0;

            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(text_x, text_y, text_width, 14.0),
                content: display.to_string(),
                color: text_color.to_rgba(),
                font_size: 14.0,
                font_weight: 400,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Left,
            });
        }

        // Paint cursor when focused
        if self.state.focused {
            let cursor_x = bounds.x() + 12.0 + (self.state.cursor_position as f32 * 7.0);
            let cursor_y = bounds.y() + 10.0;
            let cursor_height = bounds.height() - 20.0;

            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(cursor_x, cursor_y, 1.5, cursor_height),
                background: Color::hex(0x6366f1).to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            });
        }
    }
}

/// Create a new Input
pub fn input() -> Input {
    Input::new()
}
