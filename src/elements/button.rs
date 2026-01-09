//! Button element - interactive clickable component

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Size};
use crate::core::style::{Background, Corners, Style};
use crate::core::ElementId;
use crate::core::event::Cursor;
use crate::elements::element::{style_to_taffy, AnyElement, Element, LayoutContext, PaintContext};
use crate::elements::text::{text, Text};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// Button variant styles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
    Success,
}

/// Button size presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ButtonSize {
    fn padding(&self) -> (f32, f32) {
        match self {
            ButtonSize::Small => (8.0, 12.0),
            ButtonSize::Medium => (10.0, 16.0),
            ButtonSize::Large => (14.0, 24.0),
        }
    }

    fn font_size(&self) -> f32 {
        match self {
            ButtonSize::Small => 12.0,
            ButtonSize::Medium => 14.0,
            ButtonSize::Large => 16.0,
        }
    }

    fn height(&self) -> f32 {
        match self {
            ButtonSize::Small => 28.0,
            ButtonSize::Medium => 36.0,
            ButtonSize::Large => 44.0,
        }
    }
}

/// Button state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
}

/// A button component
pub struct Button {
    id: Option<ElementId>,
    label: String,
    variant: ButtonVariant,
    size: ButtonSize,
    style: Style,
    state: ButtonState,
    icon_left: Option<AnyElement>,
    icon_right: Option<AnyElement>,
    on_click: Option<Box<dyn Fn()>>,
    layout_node: Option<NodeId>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        let mut style = Style::new();
        style.border.radius = Corners::all(6.0);

        Self {
            id: None,
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            style,
            state: ButtonState::default(),
            icon_left: None,
            icon_right: None,
            on_click: None,
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn outline(mut self) -> Self {
        self.variant = ButtonVariant::Outline;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }

    pub fn success(mut self) -> Self {
        self.variant = ButtonVariant::Success;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = ButtonSize::Small;
        self
    }

    pub fn large(mut self) -> Self {
        self.size = ButtonSize::Large;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn icon_left(mut self, icon: impl Into<AnyElement>) -> Self {
        self.icon_left = Some(icon.into());
        self
    }

    pub fn icon_right(mut self, icon: impl Into<AnyElement>) -> Self {
        self.icon_right = Some(icon.into());
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border.radius = Corners::all(radius);
        self
    }

    pub fn rounded_full(mut self) -> Self {
        self.style.border.radius = Corners::all(9999.0);
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Get colors based on variant and state
    fn colors(&self) -> (Color, Color, Color) {
        let (bg, text_color, border) = match self.variant {
            ButtonVariant::Primary => {
                if self.state.disabled {
                    (Color::hex(0x6366f1).with_alpha(0.5), Color::WHITE.with_alpha(0.5), Color::TRANSPARENT)
                } else if self.state.pressed {
                    (Color::hex(0x4338ca), Color::WHITE, Color::TRANSPARENT)
                } else if self.state.hovered {
                    (Color::hex(0x4f46e5), Color::WHITE, Color::TRANSPARENT)
                } else {
                    (Color::hex(0x6366f1), Color::WHITE, Color::TRANSPARENT)
                }
            }
            ButtonVariant::Secondary => {
                if self.state.disabled {
                    (Color::hex(0x374151).with_alpha(0.5), Color::WHITE.with_alpha(0.5), Color::TRANSPARENT)
                } else if self.state.pressed {
                    (Color::hex(0x1f2937), Color::WHITE, Color::TRANSPARENT)
                } else if self.state.hovered {
                    (Color::hex(0x374151), Color::WHITE, Color::TRANSPARENT)
                } else {
                    (Color::hex(0x4b5563), Color::WHITE, Color::TRANSPARENT)
                }
            }
            ButtonVariant::Outline => {
                if self.state.disabled {
                    (Color::TRANSPARENT, Color::hex(0x6366f1).with_alpha(0.5), Color::hex(0x6366f1).with_alpha(0.5))
                } else if self.state.pressed {
                    (Color::hex(0x6366f1).with_alpha(0.1), Color::hex(0x4338ca), Color::hex(0x4338ca))
                } else if self.state.hovered {
                    (Color::hex(0x6366f1).with_alpha(0.05), Color::hex(0x4f46e5), Color::hex(0x4f46e5))
                } else {
                    (Color::TRANSPARENT, Color::hex(0x6366f1), Color::hex(0x6366f1))
                }
            }
            ButtonVariant::Ghost => {
                if self.state.disabled {
                    (Color::TRANSPARENT, Color::hex(0x6b7280).with_alpha(0.5), Color::TRANSPARENT)
                } else if self.state.pressed {
                    (Color::hex(0x6b7280).with_alpha(0.2), Color::hex(0x374151), Color::TRANSPARENT)
                } else if self.state.hovered {
                    (Color::hex(0x6b7280).with_alpha(0.1), Color::hex(0x4b5563), Color::TRANSPARENT)
                } else {
                    (Color::TRANSPARENT, Color::hex(0x6b7280), Color::TRANSPARENT)
                }
            }
            ButtonVariant::Danger => {
                if self.state.disabled {
                    (Color::hex(0xef4444).with_alpha(0.5), Color::WHITE.with_alpha(0.5), Color::TRANSPARENT)
                } else if self.state.pressed {
                    (Color::hex(0xb91c1c), Color::WHITE, Color::TRANSPARENT)
                } else if self.state.hovered {
                    (Color::hex(0xdc2626), Color::WHITE, Color::TRANSPARENT)
                } else {
                    (Color::hex(0xef4444), Color::WHITE, Color::TRANSPARENT)
                }
            }
            ButtonVariant::Success => {
                if self.state.disabled {
                    (Color::hex(0x22c55e).with_alpha(0.5), Color::WHITE.with_alpha(0.5), Color::TRANSPARENT)
                } else if self.state.pressed {
                    (Color::hex(0x15803d), Color::WHITE, Color::TRANSPARENT)
                } else if self.state.hovered {
                    (Color::hex(0x16a34a), Color::WHITE, Color::TRANSPARENT)
                } else {
                    (Color::hex(0x22c55e), Color::WHITE, Color::TRANSPARENT)
                }
            }
        };
        (bg, text_color, border)
    }

    pub fn cursor(&self) -> Cursor {
        if self.state.disabled {
            Cursor::NotAllowed
        } else {
            Cursor::Pointer
        }
    }
}

impl Element for Button {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let (py, px) = self.size.padding();
        let height = self.size.height();

        let mut style = style_to_taffy(&self.style);
        style.size.height = Dimension::Length(height);
        style.padding = taffy::Rect {
            top: LengthPercentage::Length(py),
            right: LengthPercentage::Length(px),
            bottom: LengthPercentage::Length(py),
            left: LengthPercentage::Length(px),
        };
        style.justify_content = Some(taffy::JustifyContent::Center);
        style.align_items = Some(taffy::AlignItems::Center);
        style.gap = taffy::Size {
            width: LengthPercentage::Length(8.0),
            height: LengthPercentage::Length(0.0),
        };

        let node = cx.taffy.new_leaf(style).expect("Failed to create button layout node");
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
            border_widths: if matches!(self.variant, ButtonVariant::Outline) {
                crate::core::geometry::Edges::all(1.0)
            } else {
                crate::core::geometry::Edges::ZERO
            },
            corner_radii: self.style.border.radius,
        });

        // Paint text (centered)
        let font_size = self.size.font_size();
        let text_width = self.label.len() as f32 * font_size * 0.5;
        let text_height = font_size * 1.2;
        let text_x = bounds.x() + (bounds.width() - text_width) / 2.0;
        let text_y = bounds.y() + (bounds.height() - text_height) / 2.0;

        cx.paint(Primitive::Text {
            bounds: Bounds::from_xywh(text_x, text_y, text_width, text_height),
            content: self.label.clone(),
            color: text_color.to_rgba(),
            font_size,
            font_weight: 600, // semibold
            font_family: None,
            line_height: 1.2,
            align: crate::elements::text::TextAlign::Center,
        });
    }
}

/// Create a new Button
pub fn button(label: impl Into<String>) -> Button {
    Button::new(label)
}
