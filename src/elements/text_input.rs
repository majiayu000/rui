//! TextInput component - A text input field with builder pattern
//!
//! This component provides a text input field with support for:
//! - Value management
//! - Placeholder text
//! - Password masking
//! - Focus state
//! - Cursor position tracking
//! - Change and submit callbacks

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::core::event::Cursor;
use crate::elements::element::{
    style_to_taffy, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    PointerEventKind,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// TextInput component with builder pattern
pub struct TextInput {
    id: Option<ElementId>,
    value: String,
    placeholder: String,
    mask: Option<char>,
    focused: bool,
    cursor_position: usize,
    style: Style,
    width: Option<f32>,
    on_change: Option<Box<dyn Fn(&str)>>,
    on_submit: Option<Box<dyn Fn(&str)>>,
    layout_node: Option<NodeId>,
}

impl TextInput {
    /// Create a new TextInput with default settings
    pub fn new() -> Self {
        let mut style = Style::new();
        style.border.radius = Corners::all(6.0);
        style.border.color = Color::hex(0xd1d5db);
        style.border.width = Edges::all(1.0);

        Self {
            id: None,
            value: String::new(),
            placeholder: String::new(),
            mask: None,
            focused: false,
            cursor_position: 0,
            style,
            width: None,
            on_change: None,
            on_submit: None,
            layout_node: None,
        }
    }

    /// Set the element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Get the current value
    pub fn get_value(&self) -> &str {
        &self.value
    }

    /// Set the input value
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_position = self.value.len();
        self
    }

    /// Get the placeholder text
    pub fn get_placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Set the placeholder text
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Get the mask character (if any)
    pub fn get_mask(&self) -> Option<char> {
        self.mask
    }

    /// Set a mask character for password input
    pub fn mask(mut self, mask_char: char) -> Self {
        self.mask = Some(mask_char);
        self
    }

    /// Check if the input is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set the focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Get the current cursor position
    pub fn get_cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Set the cursor position
    pub fn cursor_position(mut self, position: usize) -> Self {
        self.cursor_position = position.min(self.value.len());
        self
    }

    /// Set the width of the input
    pub fn w(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the border radius
    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border.radius = Corners::all(radius);
        self
    }

    /// Set the border color
    pub fn border_color(mut self, color: impl Into<Color>) -> Self {
        self.style.border.color = color.into();
        self
    }

    /// Set the on_change callback
    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Check if on_change handler is set
    pub fn has_on_change(&self) -> bool {
        self.on_change.is_some()
    }

    /// Trigger the on_change callback with the current value
    pub fn trigger_change(&self) {
        if let Some(ref handler) = self.on_change {
            handler(&self.value);
        }
    }

    /// Set the on_submit callback
    pub fn on_submit(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    /// Check if on_submit handler is set
    pub fn has_on_submit(&self) -> bool {
        self.on_submit.is_some()
    }

    /// Trigger the on_submit callback with the current value
    pub fn trigger_submit(&self) {
        if let Some(ref handler) = self.on_submit {
            handler(&self.value);
        }
    }

    /// Get the display text (masked if mask is set)
    pub fn display_text(&self) -> String {
        if let Some(mask_char) = self.mask {
            mask_char.to_string().repeat(self.value.len())
        } else {
            self.value.clone()
        }
    }

    /// Get colors based on state
    fn colors(&self) -> (Color, Color, Color) {
        let bg = Color::WHITE;
        let text = if self.value.is_empty() {
            Color::hex(0x9ca3af) // placehlor
        } else {
            Color::hex(0x111827)
        };
        let border = if self.focused {
            Color::hex(0x6366f1) // focus ring
        } else {
            Color::hex(0xd1d5db)
        };
        (bg, text, border)
    }

    /// Get the cursor type for this input
    pub fn cursor(&self) -> Cursor {
        Cursor::Text
    }

    /// Convert to Element (returns self since TextInput implements Element)
    pub fn into_element(self) -> Self {
        self
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for TextInput {
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

        let node = cx.taffy.new_leaf(style).expect("Failed to create text input layout node");
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
            corneii: self.style.border.radius,
        });

        // Paint focus ring
        if self.focused {
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
        let display = if self.value.is_empty() {
            self.placeholder.clone()
        } else {
            self.display_text()
        };

        if !display.is_empty() {
            let text_x = bounds.x() + 12.0;
            let text_y = bounds.y() + (bounds.height() - 14.0) / 2.0;
            let text_width = bounds.width() - 24.0;

            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(text_x, text_y, text_width, 14.0),
                content: display,
                color: texo_rgba(),
                font_size: 14.0,
                font_weight: 400,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Left,
            });
        }

        // Paint cursor when focused
        if self.focused {
            let cursor_x = bounds.x() + 12.0 + (self.cursor_position as f32 * 7.0);
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

/// Create a new TextInput
pub fn text_input() -> TextInput {
    TextInput::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ==================== Creation Tests ====================

    #[test]
    fn test_text_input_new() {
   nput = TextInput::new();
        assert_eq!(input.get_value(), "");
        assert_eq!(input.get_placeholder(), "");
        assert_eq!(input.get_mask(), None);
        assert!(!input.is_focused());
        assert_eq!(input.get_cursor_position(), 0);
    }

    #[test]
    fn test_text_input_default() {
        let input = TextInput::default();
        assert_eq!(input.get_value(), "");
        assert_eq!(input.get_placeholder(), "");
        assert_eq!(input.get_mask(), None);
        assert!(!input.is_focused());
        assert_eq!(input.get_cursor_position(), 0);
    }

    #[test]
    fn test_text_input_helper_function() {
        let input = text_input();
        assert_eq!(input.get_value(), "");
        assert_eq!(input.get_placeholder(), "");
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_builder_chain() {
        let input = TextInput::new()
            .value("test")
            .placeholder("Enter text")
            .mask('*')
            .focused(true)
            .cursor_position(2);

        assert_eq!(input.get_value(), "test");
        assert_eq!(input.get_placeholder(), "Enter text");
        assert_eq!(isk(), Some('*'));
        assert!(input.is_focused());
        assert_eq!(input.get_cursor_position(), 2);
    }

    #[test]
    fn test_builder_with_id() {
        let id = ElementId::new();
        let input = TextInput::new().id(id);
        assert_eq!(input.id, Some(id));
    }

    #[test]
    fn test_builder_with_width() {
        let input = TextInput::new().w(200.0);
        assert_eq!(input.width, Some(200.0));
    }

    #[test]
    fn test_builder_with_rounded() {
        let input = TextInput::new().rounded(10.0);
        assert_eq!(input.style.border.radius, Corners::all(10.0));
    }

    #[test]
    fn test_builder_with_border_color() {
        let input = TextInput::new().border_color(Color::RED);
        assert_eq!(input.style.border.color, Color::RED);
    }

    // ==================== Value Tests ====================

    #[test]
    fn test_value_setting() {
        let input = TextInput::new().value("hello");
        assert_eq!(input.get_value(), "hello");
    }

    #[test]
    fn test_value_with_string() {
        let input = TextInput::new().value(String::from("world"));
        assert_eq!(input.get_value(), "world");
    }

    #[test]
    fn test_value_empty() {
        let input = TextInput::new().value("");
        assert_eq!(input.get_value(), "");
    }

    #[test]
    fn test_value_with_unicode() {
        let input = TextInput::new().value("Hello, World!");
        assert_eq!(input.get_value(), "Hello, World!");
    }

    #[test]
    fn test_value_sets_cursor_to_end() {
        let input = TextInput::new().value("hello");
        assert_eq!(input.get_cursor_position(), 5);
    }

    // ==================== Placeholder Tests ====================

    #[test]
    fn test_placeholder_setting() {
        let input = TextInput::new().placeholder("Enter your name");
        assert_eq!(input.get_placeholder(), "Enter your name");
    }

    #[test]
    fn test_placeholder_with_string() {
        let input = TextInput::new().placeholder(String::from("Type here..."));
        assert_eq!(input.get_placeholder(), "Type here...");
    }

    #[test]
    fn test_placeholder_empty() {
        let input = TextInput::new().placeholder("");
        assert_eq!(input.get_placeholder(), "");
    }

    // ==================== Mask (Password) Tests ====================

    #[test]
    fn test_mask_setting() {
        let input = TextInput::new().mask('*');
        assert_eq!(input.get_mask(), Some('*'));
    }

    #[test]
    fn test_mask_with_bullet() {
        let input = TextInput::new().mask('\u{2022}'); // bullet character
        assert_eq!(input.get_mask(), Some('\u{2022}'));
    }

    #[test]
    fn test_display_text_without_mask() {
        let input = TextInput::new().value("password123");
        assert_eq!(input.display_text(), "password123");
    }

    #[test]
    fn test_display_text_with_mask() {
        let input = TextInput::new().value("password123").mask('*');
        assert_eq!(input.display_text(), "***********");
    }

    #[test]
    fn test_display_text_with_bullet_mask() {
        let input = TextInput::new().value("abc").mask('\u{2022}');
        assert_eq!(input.display_text(), "\u{2022}\u{2022}\u{2022}");
    }

    #[test]
    fn test_display_text_empty_with_mask() {
        let input = TextInput::new().value("").mask('*');
        assert_eq!(input.display_text(), "");
    }

    // ==================== Cursor Position Tests ====================

    #[test]
    fn test_cursor_position_setting() {
        let input = TextInput::new().value("hello").cursor_position(3);
        assert_eq!(input.get_cursor_position(), 3);
    }

    #[test]
    fn test_cursor_position_at_start() {
        let input = TextInput::new().value("hello").cursor_position(0);
        assert_eq!(input.get_cursor_position(), 0);
    }

    #[test]
    fn test_cursor_position_at_end() {
        let input = TextInput::new().value("hello").cursor_position(5);
        assert_eq!(input.get_cursor_position(), 5);
    }

    #[test]
    fn test_cursor_position_clamped_to_value_length() {
        let input = TextInput::new().value("hi").cursor_position(100);
        assert_eq!(input.get_cursor_position(), 2);
    }

    #[test]
    fn test_cursor_position_empty_value() {
        let input = TextInput::new().value("").cursor_position(5);
        assert_eq!(input.get_cursor_position(), 0);
    }

    // ==================== Focus State Tests ====================

    #[test]
    fn test_focus_state_default() {
        let input = TextInput::new();
        assert!(!input.is_focused());
    }

    #[test]
    fn test_focus_state_true() {
        let input = TextInput::new().focused(true);
        assert!(input.is_focused());
    }

    #[test]
    fn test_focus_state_false() {
        let input = ew().focused(false);
        assert!(!input.is_focused());
    }

    #[test]
    fn test_focus_state_toggle() {
        let input = TextInput::new().focused(true).focused(false);
        assert!(!input.is_focused());
    }

    // ==================== on_change Callback Tests ====================

    #[test]
    fn test_on_change_not_set() {
        let input = TextInput::new();
        assert!(!input.has_on_change());
    }

    #[test]
    fn test_on_change_set() {
        let input = TextInput::new().on_change(|_| {});
        assert!(input.has_on_change());
    }

    #[test]
    fn test_on_change_callback_triggered() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let input = TextInput::new()
            .value("test")
            .on_change(move |_| {
                *called_clone.borrow_mut() = true;
            });

        input.trigger_change();
        assert!(*called.borrow());
    }

    #[test]
    fn test_on_change_receives_value() {
        let received_value = Rc::new(RefCell::new(String::new()));
        let received_clone = received_value.clone();

        let input = TextInput::        .value("hello world")
            .on_change(move |val| {
                *received_clone.borrow_mut() = val.to_string();
            });

        input.trigger_change();
        assert_eq!(*received_value.borrow(), "hello world");
    }

    #[test]
    fn test_on_change_not_triggered_when_not_set() {
        let input = TextInput::new().value("test");
        // Should not panic
        input.trigger_change();
    }

    // ==================== on_submit Callback Tests ====================

    #[test]
    fn test_on_submit_not_set() {
        let input = TextInput::new();
        assert!(!input.has_on_submit());
    }

    #[test]
    fn test_on_submit_set() {
        let input = TextInput::new().on_submit(|_| {});
        assert!(input.has_on_submit());
    }

    #[test]
    fn test_on_submit_callback_triggered() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let input = TextInput::new()
            .value("test")
            .on_submit(move |_| {
                *called_clone.borrow_mut() = true;
            });

        input.trigger_submit();
        assert!(*called.borrow());
    }

    #[test]
    fn tt_receives_value() {
        let received_value = Rc::new(RefCell::new(String::new()));
        let received_clone = received_value.clone();

        let input = TextInput::new()
            .value("submitted text")
            .on_submit(move |val| {
                *received_clone.borrow_mut() = val.to_string();
            });

        input.trigger_submit();
        assert_eq!(*received_value.borrow(), "submitted text");
    }

    #[test]
    fn test_on_submit_not_triggered_when_not_set() {
        let input = TextInput::new().value("test");
        // Should not panic
        input.trigger_submit();
    }

    // ==================== into_element Tests ====================

    #[test]
    fn test_into_element_preserves_value() {
        let input = TextInput::new().value("test").into_element();
        assert_eq!(input.get_value(), "test");
    }

    #[test]
    fn test_into_element_preserves_placeholder() {
        let input = TextInput::new().placeholder("hint").into_element();
        assert_eq!(input.get_placeholder(), "hint");
    }

    #[test]
    fn test_into_element_preserves_mask() {
        let input = TextInput::new().mask('*').into_element();
        assert_eq!(input.get_mask(), Some('*'));
    }

    #[test]
    fn test_into_element_preserves_focus() {
        let input = TextInput::new().focused(true).into_element();
        assert!(input.is_focused());
    }

    #[test]
    fn test_into_element_preserves_cursor_position() {
        let input = TextInput::new().value("hello").cursor_position(3).into_element();
        assert_eq!(input.get_cursor_position(), 3);
    }

    // ==================== Element Trait Tests ====================

    #[test]
    fn test_element_id_none_by_defau      let input = TextInput::new();
        assert!(Element::id(&input).is_none());
    }

    #[test]
    fn test_element_id_when_set() {
        let id = ElementId::new();
        let input = TextInput::new().id(id);
        assert_eq!(Element::id(&input), Some(id));
    }

    #[test]
    fn test_element_style_returns_style() {
        let input = TextInput::new();
        let style = Element::style(&input);
        // Verify style has expected default values
        assert_eq!(style.border.radius, Corners::all(6.0));
    }

    // ==================== Cursor Type Tests ====================

    #[test]
    fn test_cursor_type() {
        let input = TextInput::new();
        assert_eq!(input.cursor(), Cursor::Text);
    }

    // ==================== Color Tests ====================

    #[test]
    fn test_colors_unfocused_empty() {
        let input = TextInput::new();
        let (bg, text, border) = input.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(text, Color::hex(0x9ca3af)); // placeholder color
        assert_eq!(border, Color::hex(0xd1d5db));
    }

    #[test]
    fn test_colors_unfocused_with_value() {
        let input = TextInput::new().value("test");
        let (bg, text, border) = input.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(text, Color::hex(0x111827)); // text color
        assert_eq!(border, Color::hex(0xd1d5db));
    }

    #[test]
    fn test_colors_focused() {
        let input = TextInput::new().focused(true);
        let (bg, _text, border) = input.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(border, Color::hex(0x6366f1)); // focus ring color
    }

    // ==================== Complex Scenario Tests ====================

    #[test]
    fn test_password_input_scenario() {
        let input = TextInput::new()
            .placeholder("Enter password")
            .mask('\u{2022}')
            .value("secret123")
    cused(true);

        assert_eq!(input.get_placeholder(), "Enter password");
        assert_eq!(input.get_value(), "secret123");
        assert_eq!(input.display_text(), "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}");
        assert!(input.is_focused());
    }

    #[test]
    fn test_search_input_scenario() {
        let search_results = Rc::new(RefCell::new(Vec::new()));
        let results_clone = search_results.clone();

        let input = TextInput::new()
            .placeholder("Search...")
            .value("rust")
            .w(300.0)
            .on_change(move |val| {
                results_clone.borrow_mut().push(val.to_string());
            });

        input.trigger_change();
        assert_eq!(search_results.borrow().len(), 1);
        assert_eq!(search_results.borrow()[0], "rust");
    }

    #[test]
    fn test_form_input_scenario() {
        let submitted = Rc::new(RefCell::new(false));
        let submitted_clone = submitted.clone();

        let input = TextInput::new()
            .placeholder("Enter your email")
            .value("user@example.com")
            .on_submit(move |_| {
                *submitted_clone.borrow_mut() = true;
            });

        input.trigger_submit();
        assert!(*submitted.borrow());
    }

    #[test]
    fn test_full_builder_chain() {
        let id = ElementId::new();
        let change_count = Rc::new(RefCell::new(0));
        let submit_count = Rc::new(RefCell::new(0));
        let change_clone = change_count.clone();
        let submit_clone = submit_count.clone();

        let input = TextInput::new()
            .id(id)
            .value("initial")
            .placeholder("Type here")
            .mask('*')
            .focused(true)
            .cursor_position(3)
            .w(250.0)
            .rounded(8.0)
            .border_color(Color::BLUE)
            .on_change(move |_| {
                *change_clone.borrow_mut() += 1;
            })
            .on_submit(move |_| {
                *submit_clone.borrow_mut() += 1;
            });

        // Verify all properties
        assert_eq!(input.id, Some(id));
        assert_eq!(input.get_value(), "initial");
        assert_eq!(input.get_placeholder(), "Type here");
        assert_eq!(input.get_mask(), Some('*'));
        assert!(input.is_focused());
        assert_eq!(input.get_cursor_position(), 3);
        assert_eq!(input.width, Some(250.0));
        assert_eq!(input.style.border.radius, Corners::all(8.0));
        assert_eq!(input.style.border.color, Color::BLUE);
        assert!(input.has_on_change());
        assert!(input.has_on_submit());

        // Trigger callbacks
        input.trigger_change();
        input.trigger_submit();
        assert_eq!(*change_count.borrow(), 1);
        assert_eq!(*submit_count.borrow(), 1);
    }
}
