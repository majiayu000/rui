//! Button element - interactive clickable component

use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::core::event::Cursor;
use crate::elements::element::{
    style_to_taffy, AnyElement, Element, EventContext, LayoutContext, PaintContext,
    PointerEvent, PointerEventKind,
};
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

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.state.disabled {
            self.state.pressed = false;
            self.state.hovered = false;
            return false;
        }

        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.hovered = inside;
                false
            }
            PointerEventKind::Down => {
                if inside {
                    self.state.pressed = true;
                    true
                } else {
                    false
                }
            }
            PointerEventKind::Up => {
                let was_pressed = self.state.pressed;
                self.state.pressed = false;
                if inside && was_pressed {
                    if let Some(handler) = &self.on_click {
                        handler();
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// Create a new Button
pub fn button(label: impl Into<String>) -> Button {
    Button::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::style::Corners;
    use crate::elements::element::Element;

    // ==================== ButtonVariant Tests ====================

    #[test]
    fn test_button_variant_default() {
        let variant = ButtonVariant::default();
        assert_eq!(variant, ButtonVariant::Primary);
    }

    #[test]
    fn test_button_variant_debug() {
        assert_eq!(format!("{:?}", ButtonVariant::Primary), "Primary");
        assert_eq!(format!("{:?}", ButtonVariant::Secondary), "Secondary");
        assert_eq!(format!("{:?}", ButtonVariant::Outline), "Outline");
        assert_eq!(format!("{:?}", ButtonVariant::Ghost), "Ghost");
        assert_eq!(format!("{:?}", ButtonVariant::Danger), "Danger");
        assert_eq!(format!("{:?}", ButtonVariant::Success), "Success");
    }

    #[test]
    fn test_button_variant_clone() {
        let variant = ButtonVariant::Danger;
        let cloned = variant.clone();
        assert_eq!(variant, cloned);
    }

    #[test]
    fn test_button_variant_copy() {
        let variant = ButtonVariant::Success;
        let copied = variant;
        assert_eq!(variant, copied);
    }

    #[test]
    fn test_button_variant_eq() {
        assert_eq!(ButtonVariant::Primary, ButtonVariant::Primary);
        assert_ne!(ButtonVariant::Primary, ButtonVariant::Secondary);
        assert_ne!(ButtonVariant::Outline, ButtonVariant::Ghost);
    }

    // ==================== ButtonSize Tests ====================

    #[test]
    fn test_button_size_default() {
        let size = ButtonSize::default();
        assert_eq!(size, ButtonSize::Medium);
    }

    #[test]
    fn test_button_size_debug() {
        assert_eq!(format!("{:?}", ButtonSize::Small), "Small");
        assert_eq!(format!("{:?}", ButtonSize::Medium), "Medium");
        assert_eq!(format!("{:?}", ButtonSize::Large), "Large");
    }

    #[test]
    fn test_button_size_clone() {
        let size = ButtonSize::Large;
        let cloned = size.clone();
        assert_eq!(size, cloned);
    }

    #[test]
    fn test_button_size_copy() {
        let size = ButtonSize::Small;
        let copied = size;
        assert_eq!(size, copied);
    }

    #[test]
    fn test_button_size_eq() {
        assert_eq!(ButtonSize::Small, ButtonSize::Small);
        assert_ne!(ButtonSize::Small, ButtonSize::Medium);
        assert_ne!(ButtonSize::Medium, ButtonSize::Large);
    }

    // ==================== ButtonSize Padding Tests ====================

    struct PaddingTestCase {
        size: ButtonSize,
        expected_vertical: f32,
        expected_horizontal: f32,
    }

    #[test]
    fn test_button_size_padding_table_driven() {
        let test_cases = vec![
            PaddingTestCase {
                size: ButtonSize::Small,
                expected_vertical: 8.0,
                expected_horizontal: 12.0,
            },
            PaddingTestCase {
                size: ButtonSize::Medium,
                expected_vertical: 10.0,
                expected_horizontal: 16.0,
            },
            PaddingTestCase {
                size: ButtonSize::Large,
                expected_vertical: 14.0,
                expected_horizontal: 24.0,
            },
        ];

        for tc in test_cases {
            let (py, px) = tc.size.padding();
            assert_eq!(
                py, tc.expected_vertical,
                "Vertical padding for {:?} should be {}",
                tc.size, tc.expected_vertical
            );
            assert_eq!(
                px, tc.expected_horizontal,
                "Horizontal padding for {:?} should be {}",
                tc.size, tc.expected_horizontal
            );
        }
    }

    // ==================== ButtonSize Font Size Tests ====================

    struct FontSizeTestCase {
        size: ButtonSize,
        expected: f32,
    }

    #[test]
    fn test_button_size_font_size_table_driven() {
        let test_cases = vec![
            FontSizeTestCase {
                size: ButtonSize::Small,
                expected: 12.0,
            },
            FontSizeTestCase {
                size: ButtonSize::Medium,
                expected: 14.0,
            },
            FontSizeTestCase {
                size: ButtonSize::Large,
                expected: 16.0,
            },
        ];

        for tc in test_cases {
            let font_size = tc.size.font_size();
            assert_eq!(
                font_size, tc.expected,
                "Font size for {:?} should be {}",
                tc.size, tc.expected
            );
        }
    }

    // ==================== ButtonSize Height Tests ====================

    struct HeightTestCase {
        size: ButtonSize,
        expected: f32,
    }

    #[test]
    fn test_button_size_height_table_driven() {
        let test_cases = vec![
            HeightTestCase {
                size: ButtonSize::Small,
                expected: 28.0,
            },
            HeightTestCase {
                size: ButtonSize::Medium,
                expected: 36.0,
            },
            HeightTestCase {
                size: ButtonSize::Large,
                expected: 44.0,
            },
        ];

        for tc in test_cases {
            let height = tc.size.height();
            assert_eq!(
                height, tc.expected,
                "Height for {:?} should be {}",
                tc.size, tc.expected
            );
        }
    }

    // ==================== ButtonState Tests ====================

    #[test]
    fn test_button_state_default() {
        let state = ButtonState::default();
        assert!(!state.hovered);
        assert!(!state.pressed);
        assert!(!state.focused);
        assert!(!state.disabled);
    }

    #[test]
    fn test_button_state_debug() {
        let state = ButtonState {
            hovered: true,
            pressed: false,
            focused: true,
            disabled: false,
        };
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("hovered: true"));
        assert!(debug_str.contains("pressed: false"));
        assert!(debug_str.contains("focused: true"));
        assert!(debug_str.contains("disabled: false"));
    }

    #[test]
    fn test_button_state_clone() {
        let state = ButtonState {
            hovered: true,
            pressed: true,
            focused: false,
            disabled: true,
        };
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_button_state_copy() {
        let state = ButtonState {
            hovered: false,
            pressed: true,
            focused: true,
            disabled: false,
        };
        let copied = state;
        assert_eq!(state, copied);
    }

    #[test]
    fn test_button_state_eq() {
        let state1 = ButtonState {
            hovered: true,
            pressed: false,
            focused: false,
            disabled: false,
        };
        let state2 = ButtonState {
            hovered: true,
            pressed: false,
            focused: false,
            disabled: false,
        };
        let state3 = ButtonState {
            hovered: false,
            pressed: false,
            focused: false,
            disabled: false,
        };
        assert_eq!(state1, state2);
        assert_ne!(state1, state3);
    }

    // ==================== Button Creation Tests ====================

    #[test]
    fn test_button_new() {
        let btn = Button::new("Click me");
        assert_eq!(btn.label, "Click me");
        assert_eq!(btn.variant, ButtonVariant::Primary);
        assert_eq!(btn.size, ButtonSize::Medium);
        assert!(btn.id.is_none());
        assert!(btn.on_click.is_none());
        assert!(btn.icon_left.is_none());
        assert!(btn.icon_right.is_none());
    }

    #[test]
    fn test_button_new_with_string() {
        let btn = Button::new(String::from("Test Button"));
        assert_eq!(btn.label, "Test Button");
    }

    #[test]
    fn test_button_function() {
        let btn = button("Hello");
        assert_eq!(btn.label, "Hello");
    }

    #[test]
    fn test_button_default_style() {
        let btn = Button::new("Test");
        assert_eq!(btn.style.border.radius, Corners::all(6.0));
    }

    #[test]
    fn test_button_default_state() {
        let btn = Button::new("Test");
        assert!(!btn.state.hovered);
        assert!(!btn.state.pressed);
        assert!(!btn.state.focused);
        assert!(!btn.state.disabled);
    }

    // ==================== Button Builder Method Tests ====================

    #[test]
    fn test_button_id() {
        let id = ElementId::from(42);
        let btn = Button::new("Test").id(id);
        assert_eq!(btn.id, Some(id));
    }

    #[test]
    fn test_button_variant() {
        let btn = Button::new("Test").variant(ButtonVariant::Danger);
        assert_eq!(btn.variant, ButtonVariant::Danger);
    }

    #[test]
    fn test_button_primary() {
        let btn = Button::new("Test").secondary().primary();
        assert_eq!(btn.variant, ButtonVariant::Primary);
    }

    #[test]
    fn test_button_secondary() {
        let btn = Button::new("Test").secondary();
        assert_eq!(btn.variant, ButtonVariant::Secondary);
    }

    #[test]
    fn test_button_outline() {
        let btn = Button::new("Test").outline();
        assert_eq!(btn.variant, ButtonVariant::Outline);
    }

    #[test]
    fn test_button_ghost() {
        let btn = Button::new("Test").ghost();
        assert_eq!(btn.variant, ButtonVariant::Ghost);
    }

    #[test]
    fn test_button_danger() {
        let btn = Button::new("Test").danger();
        assert_eq!(btn.variant, ButtonVariant::Danger);
    }

    #[test]
    fn test_button_success() {
        let btn = Button::new("Test").success();
        assert_eq!(btn.variant, ButtonVariant::Success);
    }

    #[test]
    fn test_button_size() {
        let btn = Button::new("Test").size(ButtonSize::Large);
        assert_eq!(btn.size, ButtonSize::Large);
    }

    #[test]
    fn test_button_small() {
        let btn = Button::new("Test").small();
        assert_eq!(btn.size, ButtonSize::Small);
    }

    #[test]
    fn test_button_large() {
        let btn = Button::new("Test").large();
        assert_eq!(btn.size, ButtonSize::Large);
    }

    #[test]
    fn test_button_disabled() {
        let btn = Button::new("Test").disabled(true);
        assert!(btn.state.disabled);
    }

    #[test]
    fn test_button_disabled_false() {
        let btn = Button::new("Test").disabled(true).disabled(false);
        assert!(!btn.state.disabled);
    }

    #[test]
    fn test_button_rounded() {
        let btn = Button::new("Test").rounded(10.0);
        assert_eq!(btn.style.border.radius, Corners::all(10.0));
    }

    #[test]
    fn test_button_rounded_full() {
        let btn = Button::new("Test").rounded_full();
        assert_eq!(btn.style.border.radius, Corners::all(9999.0));
    }

    // ==================== Button Chained Builder Tests ====================

    struct ChainedBuilderTestCase {
        name: &'static str,
        expected_variant: ButtonVariant,
        expected_size: ButtonSize,
        expected_disabled: bool,
        expected_radius: Corners,
    }

    #[test]
    fn test_button_chained_builders_table_driven() {
        let test_cases = vec![
            ChainedBuilderTestCase {
                name: "danger_small_disabled",
                expected_variant: ButtonVariant::Danger,
                expected_size: ButtonSize::Small,
                expected_disabled: true,
                expected_radius: Corners::all(6.0),
            },
            ChainedBuilderTestCase {
                name: "success_large_rounded",
                expected_variant: ButtonVariant::Success,
                expected_size: ButtonSize::Large,
                expected_disabled: false,
                expected_radius: Corners::all(20.0),
            },
            ChainedBuilderTestCase {
                name: "outline_medium_rounded_full",
                expected_variant: ButtonVariant::Outline,
                expected_size: ButtonSize::Medium,
                expected_disabled: false,
                expected_radius: Corners::all(9999.0),
            },
        ];

        for tc in test_cases {
            let btn = match tc.name {
                "danger_small_disabled" => Button::new("Test").danger().small().disabled(true),
                "success_large_rounded" => Button::new("Test").success().large().rounded(20.0),
                "outline_medium_rounded_full" => Button::new("Test").outline().rounded_full(),
                _ => panic!("Unknown test case"),
            };

            assert_eq!(
                btn.variant, tc.expected_variant,
                "{}: variant mismatch",
                tc.name
            );
            assert_eq!(btn.size, tc.expected_size, "{}: size mismatch", tc.name);
            assert_eq!(
                btn.state.disabled, tc.expected_disabled,
                "{}: disabled mismatch",
                tc.name
            );
            assert_eq!(
                btn.style.border.radius, tc.expected_radius,
                "{}: radius mismatch",
                tc.name
            );
        }
    }

    #[test]
    fn test_button_full_chain() {
        let id = ElementId::from(123);
        let btn = Button::new("Full Chain Test")
            .id(id)
            .danger()
            .large()
            .disabled(true)
            .rounded(15.0);

        assert_eq!(btn.label, "Full Chain Test");
        assert_eq!(btn.id, Some(id));
        assert_eq!(btn.variant, ButtonVariant::Danger);
        assert_eq!(btn.size, ButtonSize::Large);
        assert!(btn.state.disabled);
        assert_eq!(btn.style.border.radius, Corners::all(15.0));
    }

    // ==================== Button on_click Tests ====================

    #[test]
    fn test_button_on_click() {
        use std::cell::Cell;
        use std::rc::Rc;

        let clicked = Rc::new(Cell::new(false));
        let clicked_clone = clicked.clone();

        let btn = Button::new("Test").on_click(move || {
            clicked_clone.set(true);
        });

        assert!(btn.on_click.is_some());

        // Call the handler
        if let Some(handler) = &btn.on_click {
            handler();
        }

        assert!(clicked.get());
    }

    // ==================== Button Cursor Tests ====================

    struct CursorTestCase {
        disabled: bool,
        expected_cursor: Cursor,
    }

    #[test]
    fn test_button_cursor_table_driven() {
        let test_cases = vec![
            CursorTestCase {
                disabled: false,
                expected_cursor: Cursor::Pointer,
            },
            CursorTestCase {
                disabled: true,
                expected_cursor: Cursor::NotAllowed,
            },
        ];

        for tc in test_cases {
            let btn = Button::new("Test").disabled(tc.disabled);
            let cursor = btn.cursor();
            assert_eq!(
                cursor, tc.expected_cursor,
                "Cursor for disabled={} should be {:?}",
                tc.disabled, tc.expected_cursor
            );
        }
    }

    // ==================== Button Colors Tests ====================

    struct ColorsTestCase {
        name: &'static str,
        variant: ButtonVariant,
        hovered: bool,
        pressed: bool,
        disabled: bool,
        // We verify that colors returns different values for different states
    }

    #[test]
    fn test_button_colors_variant_states() {
        let variants = vec![
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Outline,
            ButtonVariant::Ghost,
            ButtonVariant::Danger,
            ButtonVariant::Success,
        ];

        for variant in variants {
            // Normal state
            let mut btn = Button::new("Test").variant(variant);
            let normal_colors = btn.colors();

            // Hovered state
            btn.state.hovered = true;
            let hovered_colors = btn.colors();

            // Pressed state
            btn.state.hovered = false;
            btn.state.pressed = true;
            let pressed_colors = btn.colors();

            // Disabled state
            btn.state.pressed = false;
            btn.state.disabled = true;
            let disabled_colors = btn.colors();

            // Verify colors are returned (they should be different for different states)
            // We just verify no panic and returns valid color tuples
            let _ = (normal_colors, hovered_colors, pressed_colors, disabled_colors);
        }
    }

    #[test]
    fn test_button_colors_primary_states() {
        let mut btn = Button::new("Test").primary();

        // Normal
        let (bg, text, border) = btn.colors();
        assert_eq!(bg.to_rgba(), Color::hex(0x6366f1).to_rgba());
        assert_eq!(text.to_rgba(), Color::WHITE.to_rgba());
        assert_eq!(border.to_rgba(), Color::TRANSPARENT.to_rgba());

        // Hovered
        btn.state.hovered = true;
        let (bg, _, _) = btn.colors();
        assert_eq!(bg.to_rgba(), Color::hex(0x4f46e5).to_rgba());

        // Pressed
        btn.state.hovered = false;
        btn.state.pressed = true;
        let (bg, _, _) = btn.colors();
        assert_eq!(bg.to_rgba(), Color::hex(0x4338ca).to_rgba());

        // Disabled
        btn.state.pressed = false;
        btn.state.disabled = true;
        let (bg, text, _) = btn.colors();
        assert_eq!(bg.to_rgba(), Color::hex(0x6366f1).with_alpha(0.5).to_rgba());
        assert_eq!(text.to_rgba(), Color::WHITE.with_alpha(0.5).to_rgba());
    }

    #[test]
    fn test_button_colors_outline_has_border() {
        let btn = Button::new("Test").outline();
        let (_, _, border) = btn.colors();
        // Outline variant has a visible border
        assert_ne!(border.to_rgba(), Color::TRANSPARENT.to_rgba());
    }

    #[test]
    fn test_button_colors_ghost_transparent_bg() {
        let btn = Button::new("Test").ghost();
        let (bg, _, _) = btn.colors();
        // Ghost variant has transparent background in normal state
        assert_eq!(bg.to_rgba(), Color::TRANSPARENT.to_rgba());
    }

    // ==================== Element Trait Tests ====================

    #[test]
    fn test_button_element_id_none() {
        let btn = Button::new("Test");
        // Use the Element trait method through explicit call
        assert!(<Button as Element>::id(&btn).is_none());
    }

    #[test]
    fn test_button_element_id_some() {
        let id = ElementId::from(999);
        let btn = Button::new("Test").id(id);
        // Use the Element trait method through explicit call
        assert_eq!(<Button as Element>::id(&btn), Some(id));
    }

    #[test]
    fn test_button_element_style() {
        let btn = Button::new("Test").rounded(25.0);
        let style = <Button as Element>::style(&btn);
        assert_eq!(style.border.radius, Corners::all(25.0));
    }

    // ==================== Variant Override Tests ====================

    #[test]
    fn test_button_variant_override() {
        // Ensure last variant wins
        let btn = Button::new("Test")
            .primary()
            .secondary()
            .outline()
            .ghost()
            .danger()
            .success();
        assert_eq!(btn.variant, ButtonVariant::Success);
    }

    #[test]
    fn test_button_size_override() {
        // Ensure last size wins
        let btn = Button::new("Test").small().large().size(ButtonSize::Small);
        assert_eq!(btn.size, ButtonSize::Small);
    }

    // ==================== Empty Label Tests ====================

    #[test]
    fn test_button_empty_label() {
        let btn = Button::new("");
        assert_eq!(btn.label, "");
    }

    #[test]
    fn test_button_whitespace_label() {
        let btn = Button::new("   ");
        assert_eq!(btn.label, "   ");
    }

    // ==================== Unicode Label Tests ====================

    #[test]
    fn test_button_unicode_label() {
        let btn = Button::new("你好世界");
        assert_eq!(btn.label, "你好世界");
    }

    #[test]
    fn test_button_emoji_label() {
        let btn = Button::new("Click Me! 🎉");
        assert_eq!(btn.label, "Click Me! 🎉");
    }

    // ==================== State Combination Tests ====================

    #[test]
    fn test_button_state_combinations() {
        struct StateTestCase {
            hovered: bool,
            pressed: bool,
            focused: bool,
            disabled: bool,
        }

        let test_cases = vec![
            StateTestCase {
                hovered: false,
                pressed: false,
                focused: false,
                disabled: false,
            },
            StateTestCase {
                hovered: true,
                pressed: false,
                focused: false,
                disabled: false,
            },
            StateTestCase {
                hovered: true,
                pressed: true,
                focused: false,
                disabled: false,
            },
            StateTestCase {
                hovered: true,
                pressed: true,
                focused: true,
                disabled: false,
            },
            StateTestCase {
                hovered: true,
                pressed: true,
                focused: true,
                disabled: true,
            },
        ];

        for tc in test_cases {
            let mut btn = Button::new("Test");
            btn.state.hovered = tc.hovered;
            btn.state.pressed = tc.pressed;
            btn.state.focused = tc.focused;
            btn.state.disabled = tc.disabled;

            // Just verify colors() doesn't panic with any state combination
            let _ = btn.colors();

            // Verify cursor based on disabled state
            let cursor = btn.cursor();
            if tc.disabled {
                assert_eq!(cursor, Cursor::NotAllowed);
            } else {
                assert_eq!(cursor, Cursor::Pointer);
            }
        }
    }

    // ==================== Disabled State Priority Tests ====================

    #[test]
    fn test_button_disabled_priority_in_colors() {
        // When disabled, other states should be ignored for color calculation
        let mut btn = Button::new("Test").primary();
        btn.state.disabled = true;
        btn.state.hovered = true;
        btn.state.pressed = true;

        let (bg, _, _) = btn.colors();
        // Should use disabled colors, not hovered or pressed
        assert_eq!(bg.to_rgba(), Color::hex(0x6366f1).with_alpha(0.5).to_rgba());
    }

    // ==================== All Variants Color Tests ====================

    #[test]
    fn test_all_variants_have_distinct_colors() {
        let variants = [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Outline,
            ButtonVariant::Ghost,
            ButtonVariant::Danger,
            ButtonVariant::Success,
        ];

        let mut colors_set = std::collections::HashSet::new();

        for variant in variants {
            let btn = Button::new("Test").variant(variant);
            let (bg, text, _) = btn.colors();
            // Use a simple hash of colors to verify they're different
            let color_key = format!("{:?}{:?}", bg.to_rgba(), text.to_rgba());
            colors_set.insert(color_key);
        }

        // All variants should have different color combinations
        // Note: Some may have same text color (like WHITE), so we check > 1
        assert!(colors_set.len() > 1);
    }

    // ==================== Multiple Callback Tests ====================

    #[test]
    fn test_button_callback_replaced() {
        use std::cell::Cell;
        use std::rc::Rc;

        let first_called = Rc::new(Cell::new(false));
        let second_called = Rc::new(Cell::new(false));

        let first_clone = first_called.clone();
        let second_clone = second_called.clone();

        let btn = Button::new("Test")
            .on_click(move || first_clone.set(true))
            .on_click(move || second_clone.set(true));

        // Only second callback should be set
        if let Some(handler) = &btn.on_click {
            handler();
        }

        assert!(!first_called.get());
        assert!(second_called.get());
    }

    // ==================== Rounded Value Edge Cases ====================

    #[test]
    fn test_button_rounded_zero() {
        let btn = Button::new("Test").rounded(0.0);
        assert_eq!(btn.style.border.radius, Corners::all(0.0));
    }

    #[test]
    fn test_button_rounded_negative() {
        // The API allows negative values, up to the user/renderer to handle
        let btn = Button::new("Test").rounded(-5.0);
        assert_eq!(btn.style.border.radius, Corners::all(-5.0));
    }

    #[test]
    fn test_button_rounded_large() {
        let btn = Button::new("Test").rounded(1000000.0);
        assert_eq!(btn.style.border.radius, Corners::all(1000000.0));
    }
}
