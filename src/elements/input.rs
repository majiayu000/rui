//! Text input element

use crate::core::color::Color;
use crate::core::event::Cursor;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{
    style_to_taffy, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    PointerEventKind,
};
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

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create input layout node");
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

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let should_be_focused = cx.is_focused(self.id);
        if self.state.focused != should_be_focused {
            self.state.focused = should_be_focused;
            if self.state.focused {
                if let Some(handler) = &self.on_focus {
                    handler();
                }
            } else if let Some(handler) = &self.on_blur {
                handler();
            }
        }

        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.hovered = inside;
                false
            }
            PointerEventKind::Down => {
                if inside {
                    if !self.state.focused {
                        self.state.focused = true;
                        if let Some(handler) = &self.on_focus {
                            handler();
                        }
                    }
                    cx.request_focus(self.id);
                    true
                } else if self.state.focused {
                    self.state.focused = false;
                    if let Some(handler) = &self.on_blur {
                        handler();
                    }
                    cx.clear_focus();
                    false
                } else {
                    false
                }
            }
            PointerEventKind::Up => inside,
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &crate::core::event::KeyEvent) -> bool {
        if !cx.is_focused(self.id) && !self.state.focused {
            return false;
        }

        let mut handled = false;
        match event.key {
            crate::core::event::KeyCode::Backspace => {
                if self.state.cursor_position > 0 && !self.state.value.is_empty() {
                    let idx = self.state.cursor_position - 1;
                    self.state.value.remove(idx);
                    self.state.cursor_position = idx;
                    handled = true;
                }
            }
            crate::core::event::KeyCode::Delete => {
                if self.state.cursor_position < self.state.value.len() {
                    self.state.value.remove(self.state.cursor_position);
                    handled = true;
                }
            }
            crate::core::event::KeyCode::ArrowLeft => {
                if self.state.cursor_position > 0 {
                    self.state.cursor_position -= 1;
                    handled = true;
                }
            }
            crate::core::event::KeyCode::ArrowRight => {
                if self.state.cursor_position < self.state.value.len() {
                    self.state.cursor_position += 1;
                    handled = true;
                }
            }
            crate::core::event::KeyCode::Home => {
                self.state.cursor_position = 0;
                handled = true;
            }
            crate::core::event::KeyCode::End => {
                self.state.cursor_position = self.state.value.len();
                handled = true;
            }
            crate::core::event::KeyCode::Enter => {
                if let Some(handler) = &self.on_submit {
                    handler(&self.state.value);
                }
                handled = true;
            }
            _ => {}
        }

        if !handled {
            if let Some(ch) = event.char {
                if ch == '\n' || ch == '\r' {
                    if let Some(handler) = &self.on_submit {
                        handler(&self.state.value);
                    }
                    handled = true;
                } else if !ch.is_control() {
                    let idx = self.state.cursor_position.min(self.state.value.len());
                    self.state.value.insert(idx, ch);
                    self.state.cursor_position = idx + 1;
                    handled = true;
                }
            }
        }

        if handled {
            if let Some(handler) = &self.on_change {
                handler(&self.state.value);
            }
        }

        handled
    }
}

/// Create a new Input
pub fn input() -> Input {
    Input::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ==================== InputType Enum Tests ====================

    #[test]
    fn test_input_type_default() {
        let input_type = InputType::default();
        assert_eq!(input_type, InputType::Text);
    }

    #[test]
    fn test_input_type_text() {
        assert_eq!(InputType::Text, InputType::Text);
    }

    #[test]
    fn test_input_type_password() {
        assert_eq!(InputType::Password, InputType::Password);
    }

    #[test]
    fn test_input_type_email() {
        assert_eq!(InputType::Email, InputType::Email);
    }

    #[test]
    fn test_input_type_number() {
        assert_eq!(InputType::Number, InputType::Number);
    }

    #[test]
    fn test_input_type_search() {
        assert_eq!(InputType::Search, InputType::Search);
    }

    #[test]
    fn test_input_type_inequality() {
        assert_ne!(InputType::Text, InputType::Password);
        assert_ne!(InputType::Email, InputType::Number);
        assert_ne!(InputType::Search, InputType::Text);
    }

    #[test]
    fn test_input_type_clone() {
        let input_type = InputType::Password;
        let cloned = input_type.clone();
        assert_eq!(input_type, cloned);
    }

    #[test]
    fn test_input_type_copy() {
        let input_type = InputType::Email;
        let copied: InputType = input_type; // Copy
        assert_eq!(input_type, copied);
    }

    #[test]
    fn test_input_type_debug() {
        let debug_str = format!("{:?}", InputType::Text);
        assert_eq!(debug_str, "Text");

        let debug_str = format!("{:?}", InputType::Password);
        assert_eq!(debug_str, "Password");

        let debug_str = format!("{:?}", InputType::Email);
        assert_eq!(debug_str, "Email");

        let debug_str = format!("{:?}", InputType::Number);
        assert_eq!(debug_str, "Number");

        let debug_str = format!("{:?}", InputType::Search);
        assert_eq!(debug_str, "Search");
    }

    // ==================== InputType Table-Driven Tests ====================

    #[test]
    fn test_input_type_variants_table() {
        struct TestCase {
            input_type: InputType,
            expected_debug: &'static str,
        }

        let test_cases = [
            TestCase {
                input_type: InputType::Text,
                expected_debug: "Text",
            },
            TestCase {
                input_type: InputType::Password,
                expected_debug: "Password",
            },
            TestCase {
                input_type: InputType::Email,
                expected_debug: "Email",
            },
            TestCase {
                input_type: InputType::Number,
                expected_debug: "Number",
            },
            TestCase {
                input_type: InputType::Search,
                expected_debug: "Search",
            },
        ];

        for tc in test_cases {
            assert_eq!(format!("{:?}", tc.input_type), tc.expected_debug);
        }
    }

    // ==================== InputState Tests ====================

    #[test]
    fn test_input_state_default() {
        let state = InputState::default();
        assert_eq!(state.value, "");
        assert_eq!(state.cursor_position, 0);
        assert_eq!(state.selection_start, None);
        assert_eq!(state.selection_end, None);
        assert!(!state.focused);
        assert!(!state.hovered);
    }

    #[test]
    fn test_input_state_clone() {
        let mut state = InputState::default();
        state.value = "test".to_string();
        state.cursor_position = 4;
        state.focused = true;
        state.hovered = true;
        state.selection_start = Some(1);
        state.selection_end = Some(3);

        let cloned = state.clone();
        assert_eq!(cloned.value, "test");
        assert_eq!(cloned.cursor_position, 4);
        assert!(cloned.focused);
        assert!(cloned.hovered);
        assert_eq!(cloned.selection_start, Some(1));
        assert_eq!(cloned.selection_end, Some(3));
    }

    #[test]
    fn test_input_state_debug() {
        let state = InputState::default();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("InputState"));
        assert!(debug_str.contains("value"));
        assert!(debug_str.contains("cursor_position"));
    }

    // ==================== Input Creation Tests ====================

    #[test]
    fn test_input_new() {
        let input = Input::new();
        assert_eq!(input.placeholder, "");
        assert_eq!(input.input_type, InputType::Text);
        assert_eq!(input.state.value, "");
        assert!(!input.state.focused);
        assert!(!input.state.hovered);
        assert!(input.id.is_none());
        assert!(input.width.is_none());
        assert!(input.on_change.is_none());
        assert!(input.on_submit.is_none());
        assert!(input.on_focus.is_none());
        assert!(input.on_blur.is_none());
    }

    #[test]
    fn test_input_default() {
        let input = Input::default();
        assert_eq!(input.placeholder, "");
        assert_eq!(input.input_type, InputType::Text);
        assert_eq!(input.state.value, "");
    }

    #[test]
    fn test_input_helper_function() {
        let inp = input();
        assert_eq!(inp.placeholder, "");
        assert_eq!(inp.input_type, InputType::Text);
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_builder_id() {
        let id = ElementId::new();
        let inp = Input::new().id(id);
        assert_eq!(inp.id, Some(id));
    }

    #[test]
    fn test_builder_placeholder() {
        let inp = Input::new().placeholder("Enter your name");
        assert_eq!(inp.placeholder, "Enter your name");
    }

    #[test]
    fn test_builder_placeholder_with_string() {
        let inp = Input::new().placeholder(String::from("Type here..."));
        assert_eq!(inp.placeholder, "Type here...");
    }

    #[test]
    fn test_builder_value() {
        let inp = Input::new().value("hello");
        assert_eq!(inp.state.value, "hello");
        assert_eq!(inp.state.cursor_position, 5);
    }

    #[test]
    fn test_builder_value_with_string() {
        let inp = Input::new().value(String::from("world"));
        assert_eq!(inp.state.value, "world");
    }

    #[test]
    fn test_builder_value_empty() {
        let inp = Input::new().value("");
        assert_eq!(inp.state.value, "");
        assert_eq!(inp.state.cursor_position, 0);
    }

    #[test]
    fn test_builder_value_unicode() {
        let inp = Input::new().value("Hello, World!");
        assert_eq!(inp.state.value, "Hello, World!");
    }

    #[test]
    fn test_builder_value_sets_cursor_to_end() {
        let inp = Input::new().value("testing");
        assert_eq!(inp.state.cursor_position, 7);
    }

    #[test]
    fn test_builder_input_type() {
        let inp = Input::new().input_type(InputType::Password);
        assert_eq!(inp.input_type, InputType::Password);
    }

    #[test]
    fn test_builder_password() {
        let inp = Input::new().password();
        assert_eq!(inp.input_type, InputType::Password);
    }

    #[test]
    fn test_builder_email() {
        let inp = Input::new().email();
        assert_eq!(inp.input_type, InputType::Email);
    }

    #[test]
    fn test_builder_number() {
        let inp = Input::new().number();
        assert_eq!(inp.input_type, InputType::Number);
    }

    #[test]
    fn test_builder_search() {
        let inp = Input::new().search();
        assert_eq!(inp.input_type, InputType::Search);
    }

    #[test]
    fn test_builder_width() {
        let inp = Input::new().w(200.0);
        assert_eq!(inp.width, Some(200.0));
    }

    #[test]
    fn test_builder_rounded() {
        let inp = Input::new().rounded(10.0);
        assert_eq!(inp.style.border.radius, Corners::all(10.0));
    }

    #[test]
    fn test_builder_border_color() {
        let inp = Input::new().border_color(Color::RED);
        assert_eq!(inp.style.border.color, Color::RED);
    }

    #[test]
    fn test_builder_border_color_hex() {
        let inp = Input::new().border_color(Color::hex(0xFF00FF));
        let _ = inp.style.border.color; // Just verify no panic
    }

    // ==================== Builder Chain Tests ====================

    #[test]
    fn test_builder_chain() {
        let id = ElementId::new();
        let inp = Input::new()
            .id(id)
            .value("test")
            .placeholder("Enter text")
            .password()
            .w(300.0)
            .rounded(8.0)
            .border_color(Color::BLUE);

        assert_eq!(inp.id, Some(id));
        assert_eq!(inp.state.value, "test");
        assert_eq!(inp.placeholder, "Enter text");
        assert_eq!(inp.input_type, InputType::Password);
        assert_eq!(inp.width, Some(300.0));
        assert_eq!(inp.style.border.radius, Corners::all(8.0));
        assert_eq!(inp.style.border.color, Color::BLUE);
    }

    #[test]
    fn test_builder_chain_all_input_types() {
        // Test that we can switch input types multiple times
        let inp = Input::new()
            .password()
            .email()
            .number()
            .search()
            .input_type(InputType::Text);

        assert_eq!(inp.input_type, InputType::Text);
    }

    // ==================== Input Type Table-Driven Builder Tests ====================

    #[test]
    fn test_input_type_shortcut_methods_table() {
        struct TestCase {
            name: &'static str,
            build: fn(Input) -> Input,
            expected_type: InputType,
        }

        let test_cases = [
            TestCase {
                name: "password",
                build: |i| i.password(),
                expected_type: InputType::Password,
            },
            TestCase {
                name: "email",
                build: |i| i.email(),
                expected_type: InputType::Email,
            },
            TestCase {
                name: "number",
                build: |i| i.number(),
                expected_type: InputType::Number,
            },
            TestCase {
                name: "search",
                build: |i| i.search(),
                expected_type: InputType::Search,
            },
        ];

        for tc in test_cases {
            let inp = (tc.build)(Input::new());
            assert_eq!(
                inp.input_type, tc.expected_type,
                "Failed for input type shortcut: {}",
                tc.name
            );
        }
    }

    // ==================== on_change Callback Tests ====================

    #[test]
    fn test_on_change_not_set() {
        let inp = Input::new();
        assert!(inp.on_change.is_none());
    }

    #[test]
    fn test_on_change_set() {
        let inp = Input::new().on_change(|_| {});
        assert!(inp.on_change.is_some());
    }

    #[test]
    fn test_on_change_callback_called() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let inp = Input::new().value("test").on_change(move |_| {
            *called_clone.borrow_mut() = true;
        });

        // Simulate calling the handler
        if let Some(handler) = &inp.on_change {
            handler(&inp.state.value);
        }
        assert!(*called.borrow());
    }

    #[test]
    fn test_on_change_receives_value() {
        let received_value = Rc::new(RefCell::new(String::new()));
        let received_clone = received_value.clone();

        let inp = Input::new().value("hello world").on_change(move |val| {
            *received_clone.borrow_mut() = val.to_string();
        });

        if let Some(handler) = &inp.on_change {
            handler(&inp.state.value);
        }
        assert_eq!(*received_value.borrow(), "hello world");
    }

    // ==================== on_submit Callback Tests ====================

    #[test]
    fn test_on_submit_not_set() {
        let inp = Input::new();
        assert!(inp.on_submit.is_none());
    }

    #[test]
    fn test_on_submit_set() {
        let inp = Input::new().on_submit(|_| {});
        assert!(inp.on_submit.is_some());
    }

    #[test]
    fn test_on_submit_callback_called() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let inp = Input::new().value("test").on_submit(move |_| {
            *called_clone.borrow_mut() = true;
        });

        if let Some(handler) = &inp.on_submit {
            handler(&inp.state.value);
        }
        assert!(*called.borrow());
    }

    #[test]
    fn test_on_submit_receives_value() {
        let received_value = Rc::new(RefCell::new(String::new()));
        let received_clone = received_value.clone();

        let inp = Input::new().value("submitted text").on_submit(move |val| {
            *received_clone.borrow_mut() = val.to_string();
        });

        if let Some(handler) = &inp.on_submit {
            handler(&inp.state.value);
        }
        assert_eq!(*received_value.borrow(), "submitted text");
    }

    // ==================== on_focus Callback Tests ====================

    #[test]
    fn test_on_focus_not_set() {
        let inp = Input::new();
        assert!(inp.on_focus.is_none());
    }

    #[test]
    fn test_on_focus_set() {
        let inp = Input::new().on_focus(|| {});
        assert!(inp.on_focus.is_some());
    }

    #[test]
    fn test_on_focus_callback_called() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let inp = Input::new().on_focus(move || {
            *called_clone.borrow_mut() = true;
        });

        if let Some(handler) = &inp.on_focus {
            handler();
        }
        assert!(*called.borrow());
    }

    // ==================== on_blur Callback Tests ====================

    #[test]
    fn test_on_blur_not_set() {
        let inp = Input::new();
        assert!(inp.on_blur.is_none());
    }

    #[test]
    fn test_on_blur_set() {
        let inp = Input::new().on_blur(|| {});
        assert!(inp.on_blur.is_some());
    }

    #[test]
    fn test_on_blur_callback_called() {
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let inp = Input::new().on_blur(move || {
            *called_clone.borrow_mut() = true;
        });

        if let Some(handler) = &inp.on_blur {
            handler();
        }
        assert!(*called.borrow());
    }

    // ==================== display_text Tests ====================

    #[test]
    fn test_display_text_normal() {
        let inp = Input::new().value("hello world");
        assert_eq!(inp.display_text(), "hello world");
    }

    #[test]
    fn test_display_text_password() {
        let inp = Input::new().value("secret123").password();
        assert_eq!(
            inp.display_text(),
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
        );
    }

    #[test]
    fn test_display_text_password_empty() {
        let inp = Input::new().value("").password();
        assert_eq!(inp.display_text(), "");
    }

    #[test]
    fn test_display_text_password_single_char() {
        let inp = Input::new().value("a").password();
        assert_eq!(inp.display_text(), "\u{2022}");
    }

    #[test]
    fn test_display_text_email() {
        let inp = Input::new().value("user@example.com").email();
        assert_eq!(inp.display_text(), "user@example.com");
    }

    #[test]
    fn test_display_text_number() {
        let inp = Input::new().value("12345").number();
        assert_eq!(inp.display_text(), "12345");
    }

    #[test]
    fn test_display_text_search() {
        let inp = Input::new().value("search term").search();
        assert_eq!(inp.display_text(), "search term");
    }

    // ==================== display_text Table-Driven Tests ====================

    #[test]
    fn test_display_text_table() {
        struct TestCase {
            value: &'static str,
            input_type: InputType,
            expected: &'static str,
        }

        let test_cases = [
            TestCase {
                value: "hello",
                input_type: InputType::Text,
                expected: "hello",
            },
            TestCase {
                value: "secret",
                input_type: InputType::Password,
                expected: "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
            },
            TestCase {
                value: "",
                input_type: InputType::Password,
                expected: "",
            },
            TestCase {
                value: "user@test.com",
                input_type: InputType::Email,
                expected: "user@test.com",
            },
            TestCase {
                value: "42",
                input_type: InputType::Number,
                expected: "42",
            },
            TestCase {
                value: "query",
                input_type: InputType::Search,
                expected: "query",
            },
        ];

        for tc in test_cases {
            let inp = Input::new().value(tc.value).input_type(tc.input_type);
            assert_eq!(
                inp.display_text(),
                tc.expected,
                "Failed for value: {}, type: {:?}",
                tc.value,
                tc.input_type
            );
        }
    }

    // ==================== colors Tests ====================

    #[test]
    fn test_colors_default_empty() {
        let inp = Input::new();
        let (bg, text, border) = inp.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(text, Color::hex(0x9ca3af)); // placeholder color
        assert_eq!(border, Color::hex(0xd1d5db));
    }

    #[test]
    fn test_colors_with_value() {
        let mut inp = Input::new().value("test");
        inp.state.value = "test".to_string();
        let (bg, text, border) = inp.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(text, Color::hex(0x111827)); // text color
        assert_eq!(border, Color::hex(0xd1d5db));
    }

    #[test]
    fn test_colors_focused() {
        let mut inp = Input::new();
        inp.state.focused = true;
        let (bg, _text, border) = inp.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(border, Color::hex(0x6366f1)); // focus ring
    }

    #[test]
    fn test_colors_hovered() {
        let mut inp = Input::new();
        inp.state.hovered = true;
        let (bg, _text, border) = inp.colors();
        assert_eq!(bg, Color::WHITE);
        assert_eq!(border, Color::hex(0x9ca3af)); // hover border
    }

    #[test]
    fn test_colors_focused_takes_priority_over_hovered() {
        let mut inp = Input::new();
        inp.state.focused = true;
        inp.state.hovered = true;
        let (_bg, _text, border) = inp.colors();
        assert_eq!(border, Color::hex(0x6366f1)); // focus ring takes priority
    }

    // ==================== cursor Tests ====================

    #[test]
    fn test_cursor_type() {
        let inp = Input::new();
        assert_eq!(inp.cursor(), Cursor::Text);
    }

    #[test]
    fn test_cursor_type_password() {
        let inp = Input::new().password();
        assert_eq!(inp.cursor(), Cursor::Text);
    }

    #[test]
    fn test_cursor_type_all_input_types() {
        let types = [
            InputType::Text,
            InputType::Password,
            InputType::Email,
            InputType::Number,
            InputType::Search,
        ];

        for input_type in types {
            let inp = Input::new().input_type(input_type);
            assert_eq!(
                inp.cursor(),
                Cursor::Text,
                "Cursor should be Text for {:?}",
                input_type
            );
        }
    }

    // ==================== Element Trait Tests ====================

    #[test]
    fn test_element_id_none_by_default() {
        let inp = Input::new();
        assert!(Element::id(&inp).is_none());
    }

    #[test]
    fn test_element_id_when_set() {
        let id = ElementId::new();
        let inp = Input::new().id(id);
        assert_eq!(Element::id(&inp), Some(id));
    }

    #[test]
    fn test_element_style_returns_style() {
        let inp = Input::new();
        let style = Element::style(&inp);
        // Verify style has expected default values
        assert_eq!(style.border.radius, Corners::all(6.0));
    }

    #[test]
    fn test_element_style_after_rounded() {
        let inp = Input::new().rounded(12.0);
        let style = Element::style(&inp);
        assert_eq!(style.border.radius, Corners::all(12.0));
    }

    #[test]
    fn test_element_style_after_border_color() {
        let inp = Input::new().border_color(Color::GREEN);
        let style = Element::style(&inp);
        assert_eq!(style.border.color, Color::GREEN);
    }

    // ==================== Default Style Tests ====================

    #[test]
    fn test_default_border_radius() {
        let inp = Input::new();
        assert_eq!(inp.style.border.radius, Corners::all(6.0));
    }

    #[test]
    fn test_default_border_color() {
        let inp = Input::new();
        assert_eq!(inp.style.border.color, Color::hex(0xd1d5db));
    }

    #[test]
    fn test_default_border_width() {
        let inp = Input::new();
        assert_eq!(inp.style.border.width, Edges::all(1.0));
    }

    // ==================== Complex Scenario Tests ====================

    #[test]
    fn test_password_input_scenario() {
        let focus_count = Rc::new(RefCell::new(0));
        let blur_count = Rc::new(RefCell::new(0));
        let focus_clone = focus_count.clone();
        let blur_clone = blur_count.clone();

        let inp = Input::new()
            .placeholder("Enter password")
            .password()
            .value("secret123")
            .on_focus(move || {
                *focus_clone.borrow_mut() += 1;
            })
            .on_blur(move || {
                *blur_clone.borrow_mut() += 1;
            });

        assert_eq!(inp.placeholder, "Enter password");
        assert_eq!(inp.state.value, "secret123");
        assert_eq!(
            inp.display_text(),
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
        );

        // Simulate focus
        if let Some(handler) = &inp.on_focus {
            handler();
        }
        assert_eq!(*focus_count.borrow(), 1);

        // Simulate blur
        if let Some(handler) = &inp.on_blur {
            handler();
        }
        assert_eq!(*blur_count.borrow(), 1);
    }

    #[test]
    fn test_search_input_scenario() {
        let search_results = Rc::new(RefCell::new(Vec::new()));
        let results_clone = search_results.clone();

        let inp = Input::new()
            .placeholder("Search...")
            .value("rust")
            .search()
            .w(300.0)
            .on_change(move |val| {
                results_clone.borrow_mut().push(val.to_string());
            });

        if let Some(handler) = &inp.on_change {
            handler(&inp.state.value);
        }
        assert_eq!(search_results.borrow().len(), 1);
        assert_eq!(search_results.borrow()[0], "rust");
    }

    #[test]
    fn test_form_input_scenario() {
        let submitted = Rc::new(RefCell::new(false));
        let submitted_clone = submitted.clone();

        let inp = Input::new()
            .placeholder("Enter your email")
            .email()
            .value("user@example.com")
            .on_submit(move |_| {
                *submitted_clone.borrow_mut() = true;
            });

        if let Some(handler) = &inp.on_submit {
            handler(&inp.state.value);
        }
        assert!(*submitted.borrow());
    }

    #[test]
    fn test_full_builder_chain() {
        let id = ElementId::new();
        let change_count = Rc::new(RefCell::new(0));
        let submit_count = Rc::new(RefCell::new(0));
        let focus_count = Rc::new(RefCell::new(0));
        let blur_count = Rc::new(RefCell::new(0));
        let change_clone = change_count.clone();
        let submit_clone = submit_count.clone();
        let focus_clone = focus_count.clone();
        let blur_clone = blur_count.clone();

        let inp = Input::new()
            .id(id)
            .value("initial")
            .placeholder("Type here")
            .password()
            .w(250.0)
            .rounded(8.0)
            .border_color(Color::BLUE)
            .on_change(move |_| {
                *change_clone.borrow_mut() += 1;
            })
            .on_submit(move |_| {
                *submit_clone.borrow_mut() += 1;
            })
            .on_focus(move || {
                *focus_clone.borrow_mut() += 1;
            })
            .on_blur(move || {
                *blur_clone.borrow_mut() += 1;
            });

        // Verify all properties
        assert_eq!(inp.id, Some(id));
        assert_eq!(inp.state.value, "initial");
        assert_eq!(inp.placeholder, "Type here");
        assert_eq!(inp.input_type, InputType::Password);
        assert_eq!(inp.width, Some(250.0));
        assert_eq!(inp.style.border.radius, Corners::all(8.0));
        assert_eq!(inp.style.border.color, Color::BLUE);
        assert!(inp.on_change.is_some());
        assert!(inp.on_submit.is_some());
        assert!(inp.on_focus.is_some());
        assert!(inp.on_blur.is_some());

        // Trigger all callbacks
        if let Some(handler) = &inp.on_change {
            handler(&inp.state.value);
        }
        if let Some(handler) = &inp.on_submit {
            handler(&inp.state.value);
        }
        if let Some(handler) = &inp.on_focus {
            handler();
        }
        if let Some(handler) = &inp.on_blur {
            handler();
        }

        assert_eq!(*change_count.borrow(), 1);
        assert_eq!(*submit_count.borrow(), 1);
        assert_eq!(*focus_count.borrow(), 1);
        assert_eq!(*blur_count.borrow(), 1);
    }

    // ==================== Callback Multiple Invocations Tests ====================

    #[test]
    fn test_on_change_multiple_calls() {
        let call_count = Rc::new(RefCell::new(0));
        let count_clone = call_count.clone();

        let inp = Input::new().value("test").on_change(move |_| {
            *count_clone.borrow_mut() += 1;
        });

        if let Some(handler) = &inp.on_change {
            handler("a");
            handler("ab");
            handler("abc");
        }
        assert_eq!(*call_count.borrow(), 3);
    }

    #[test]
    fn test_on_submit_multiple_calls() {
        let call_count = Rc::new(RefCell::new(0));
        let count_clone = call_count.clone();

        let inp = Input::new().value("test").on_submit(move |_| {
            *count_clone.borrow_mut() += 1;
        });

        if let Some(handler) = &inp.on_submit {
            handler("first");
            handler("second");
        }
        assert_eq!(*call_count.borrow(), 2);
    }

    // ==================== Input State Modification Tests ====================

    #[test]
    fn test_state_modification() {
        let mut inp = Input::new().value("test");

        inp.state.focused = true;
        assert!(inp.state.focused);

        inp.state.hovered = true;
        assert!(inp.state.hovered);

        inp.state.cursor_position = 2;
        assert_eq!(inp.state.cursor_position, 2);

        inp.state.selection_start = Some(0);
        inp.state.selection_end = Some(2);
        assert_eq!(inp.state.selection_start, Some(0));
        assert_eq!(inp.state.selection_end, Some(2));
    }

    // ==================== Width Tests ====================

    #[test]
    fn test_width_table() {
        struct TestCase {
            width: f32,
        }

        let test_cases = [
            TestCase { width: 100.0 },
            TestCase { width: 200.0 },
            TestCase { width: 300.0 },
            TestCase { width: 0.0 },
            TestCase { width: 500.5 },
        ];

        for tc in test_cases {
            let inp = Input::new().w(tc.width);
            assert_eq!(inp.width, Some(tc.width));
        }
    }

    // ==================== Rounded Corners Tests ====================

    #[test]
    fn test_rounded_table() {
        struct TestCase {
            radius: f32,
        }

        let test_cases = [
            TestCase { radius: 0.0 },
            TestCase { radius: 4.0 },
            TestCase { radius: 8.0 },
            TestCase { radius: 16.0 },
            TestCase { radius: 100.0 },
        ];

        for tc in test_cases {
            let inp = Input::new().rounded(tc.radius);
            assert_eq!(inp.style.border.radius, Corners::all(tc.radius));
        }
    }

    // ==================== Edge Cases Tests ====================

    #[test]
    fn test_empty_placeholder() {
        let inp = Input::new().placeholder("");
        assert_eq!(inp.placeholder, "");
    }

    #[test]
    fn test_long_value() {
        let long_string = "a".repeat(10000);
        let inp = Input::new().value(long_string.clone());
        assert_eq!(inp.state.value, long_string);
        assert_eq!(inp.state.cursor_position, 10000);
    }

    #[test]
    fn test_unicode_value() {
        let inp = Input::new().value("Unicode test string");
        assert_eq!(inp.state.value, "Unicode test string");
    }

    #[test]
    fn test_special_characters() {
        let inp = Input::new().value("!@#$%^&*()_+-=[]{}|;':\",./<>?");
        assert_eq!(inp.state.value, "!@#$%^&*()_+-=[]{}|;':\",./<>?");
    }

    // ==================== Callback Not Panicking Tests ====================

    #[test]
    fn test_on_change_not_set_no_panic() {
        let inp = Input::new().value("test");
        // This should not panic
        if let Some(handler) = &inp.on_change {
            handler(&inp.state.value);
        }
    }

    #[test]
    fn test_on_submit_not_set_no_panic() {
        let inp = Input::new().value("test");
        // This should not panic
        if let Some(handler) = &inp.on_submit {
            handler(&inp.state.value);
        }
    }

    #[test]
    fn test_on_focus_not_set_no_panic() {
        let inp = Input::new();
        // This should not panic
        if let Some(handler) = &inp.on_focus {
            handler();
        }
    }

    #[test]
    fn test_on_blur_not_set_no_panic() {
        let inp = Input::new();
        // This should not panic
        if let Some(handler) = &inp.on_blur {
            handler();
        }
    }
}
