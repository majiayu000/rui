//! Text input element

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole, AccessibilityTextRange,
};
use crate::core::color::{Color, Rgba};
use crate::core::event::{Cursor, KeyCode, KeyEvent};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::core::text_editing::{
    Clipboard, TextEditBuffer, TextEditError, TextEditLayout, TextEditOutcome, TextEditPaintStyle,
    TextInputEvent, TextRange, TextSelection,
};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

const INPUT_HORIZONTAL_PADDING: f32 = 12.0;
const INPUT_CARET_TOP_PADDING: f32 = 10.0;
const INPUT_GRAPHEME_WIDTH: f32 = 7.0;
const INPUT_CARET_WIDTH: f32 = 1.5;
const INPUT_MARKED_UNDERLINE_HEIGHT: f32 = 2.0;
const PASSWORD_MASK: &str = "\u{2022}";

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
    pub composition_range: Option<TextRange>,
    pub marked_text: Option<String>,
    pub focused: bool,
    pub hovered: bool,
}

/// Text input component
pub struct Input {
    id: Option<ElementId>,
    placeholder: String,
    accessibility_label: Option<String>,
    input_type: InputType,
    style: Style,
    state: InputState,
    editor: TextEditBuffer,
    width: Option<f32>,
    height: Option<f32>,
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
            accessibility_label: None,
            input_type: InputType::default(),
            style,
            state: InputState::default(),
            editor: TextEditBuffer::new(),
            width: None,
            height: None,
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

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.editor = TextEditBuffer::with_text(value.into());
        self.sync_state_from_editor();
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

    pub fn h(mut self, height: f32) -> Self {
        self.height = Some(height);
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
            PASSWORD_MASK.repeat(self.state.value.graphemes(true).count())
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

    pub fn get_value(&self) -> &str {
        &self.state.value
    }

    pub fn apply_text_input_event(
        &mut self,
        event: TextInputEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = self.editor.apply_text_input_event(event)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        Ok(outcome)
    }

    pub fn apply_key_event(&mut self, event: &KeyEvent) -> Result<TextEditOutcome, TextEditError> {
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = if matches!(event.char, Some('\n' | '\r')) {
            TextEditOutcome {
                changed: false,
                submitted: true,
            }
        } else {
            self.editor.apply_key_event(event)?
        };
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        if outcome.submitted {
            self.emit_submit();
        }
        Ok(outcome)
    }

    pub fn copy_selection_to<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<bool, TextEditError> {
        self.sync_editor_from_public_state_if_needed()?;
        self.editor.copy_selection_to(clipboard)
    }

    pub fn cut_selection_to<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<TextEditOutcome, TextEditError> {
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = self.editor.cut_selection_to(clipboard)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        Ok(outcome)
    }

    pub fn paste_from<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<TextEditOutcome, TextEditError> {
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = self.editor.paste_from(clipboard)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        Ok(outcome)
    }

    fn normalize_cursor_position(&self) -> usize {
        let mut cursor = self.state.cursor_position.min(self.state.value.len());
        while cursor > 0 && !self.state.value.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }

    fn state_selection(&self) -> TextSelection {
        let cursor = self.normalize_cursor_position();
        match (self.state.selection_start, self.state.selection_end) {
            (Some(start), Some(end)) if start != end => {
                let head = if cursor == start || cursor == end {
                    cursor
                } else {
                    end
                };
                let anchor = if head == start { end } else { start };
                TextSelection::new(anchor, head)
            }
            _ => TextSelection::collapsed(cursor),
        }
    }

    fn sync_editor_from_public_state_if_needed(&mut self) -> Result<(), TextEditError> {
        if self.public_state_matches_editor() {
            return Ok(());
        }

        let mut editor = TextEditBuffer::with_text(self.state.value.clone());
        editor.set_selection(self.state_selection())?;
        self.editor = editor;
        self.sync_state_from_editor();
        Ok(())
    }

    fn public_state_matches_editor(&self) -> bool {
        if self.state.value != self.editor.text() {
            return false;
        }

        let selection = self.editor.selection();
        if self.normalize_cursor_position() != selection.head() {
            return false;
        }

        let range = selection.normalized_range();
        let (selection_start, selection_end) = if range.is_empty() {
            (None, None)
        } else {
            (Some(range.start()), Some(range.end()))
        };
        if self.state.selection_start != selection_start
            || self.state.selection_end != selection_end
        {
            return false;
        }

        match self.editor.composition() {
            Some(composition) => {
                self.state.composition_range == Some(composition.replacement_range())
                    && self.state.marked_text.as_deref() == Some(composition.text())
            }
            None => self.state.composition_range.is_none() && self.state.marked_text.is_none(),
        }
    }

    fn sync_state_from_editor(&mut self) {
        self.state.value = self.editor.text().to_string();
        self.state.cursor_position = self.editor.selection().head();

        let range = self.editor.selection().normalized_range();
        if range.is_empty() {
            self.state.selection_start = None;
            self.state.selection_end = None;
        } else {
            self.state.selection_start = Some(range.start());
            self.state.selection_end = Some(range.end());
        }

        if let Some(composition) = self.editor.composition() {
            self.state.composition_range = Some(composition.replacement_range());
            self.state.marked_text = Some(composition.text().to_string());
        } else {
            self.state.composition_range = None;
            self.state.marked_text = None;
        }
    }

    fn emit_change_if_needed(&self, changed: bool) {
        if changed && let Some(handler) = &self.on_change {
            handler(&self.state.value);
        }
    }

    fn emit_submit(&self) {
        if let Some(handler) = &self.on_submit {
            handler(&self.state.value);
        }
    }

    fn key_event_is_text_editing(event: &KeyEvent) -> bool {
        matches!(
            event.key,
            KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Enter
        ) || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
    }

    fn display_offset_for_value_offset(&self, offset: usize) -> Option<usize> {
        if offset > self.state.value.len() || !self.state.value.is_char_boundary(offset) {
            return None;
        }

        if self.input_type == InputType::Password {
            Some(self.state.value[..offset].graphemes(true).count() * PASSWORD_MASK.len())
        } else {
            Some(offset)
        }
    }

    fn display_range_for_value_range(&self, range: TextRange) -> Option<TextRange> {
        let start = self.display_offset_for_value_offset(range.start())?;
        let end = self.display_offset_for_value_offset(range.end())?;
        TextRange::new(start, end).ok()
    }

    fn input_layout_text(&self) -> String {
        if self.state.value.is_empty() {
            String::new()
        } else {
            self.display_text()
        }
    }

    fn text_layout(&self) -> TextEditLayout {
        TextEditLayout::new(
            self.input_layout_text(),
            INPUT_GRAPHEME_WIDTH,
            self.cursor_height(),
        )
    }

    fn text_origin(&self, bounds: Bounds) -> Point {
        Point::new(
            bounds.x() + INPUT_HORIZONTAL_PADDING,
            bounds.y() + INPUT_CARET_TOP_PADDING,
        )
    }

    fn text_width(&self, bounds: Bounds) -> f32 {
        bounds.width() - (INPUT_HORIZONTAL_PADDING * 2.0)
    }

    fn cursor_height(&self) -> f32 {
        20.0
    }

    fn paint_selection_and_marked_text(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }

        let layout = self.text_layout();
        let style = TextEditPaintStyle::new(
            INPUT_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        let paint_origin = self.text_origin(bounds);

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end)
            && let Ok(range) = TextRange::new(start, end)
            && let Some(display_range) = self.display_range_for_value_range(range)
        {
            match layout.selection_primitives(display_range, paint_origin, style) {
                Ok(primitives) => {
                    for primitive in primitives {
                        cx.paint(primitive);
                    }
                }
                Err(err) => log::error!("input selection paint failed: {err}"),
            }
        }

        if let Some(range) = self.state.composition_range
            && let Some(display_range) = self.display_range_for_value_range(range)
        {
            match layout.selection_rects(display_range) {
                Ok(rects) => {
                    for rect in rects {
                        cx.paint(Primitive::Quad {
                            bounds: Bounds::from_xywh(
                                paint_origin.x + rect.bounds.x(),
                                paint_origin.y + rect.bounds.y() + rect.bounds.height()
                                    - INPUT_MARKED_UNDERLINE_HEIGHT,
                                rect.bounds.width(),
                                INPUT_MARKED_UNDERLINE_HEIGHT,
                            ),
                            background: Color::hex(0x6366f1).to_rgba(),
                            border_color: Rgba::TRANSPARENT,
                            border_widths: Edges::ZERO,
                            corner_radii: Corners::ZERO,
                        });
                    }
                }
                Err(err) => log::error!("input marked text paint failed: {err}"),
            }
        }
    }

    fn paint_cursor(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }

        let Some(cursor) = self.display_offset_for_value_offset(self.normalize_cursor_position())
        else {
            log::error!(
                "input cursor paint failed: cursor {} is not a valid display offset",
                self.state.cursor_position
            );
            return;
        };

        let layout = self.text_layout();
        let style = TextEditPaintStyle::new(
            INPUT_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        match layout.caret_primitive(cursor, self.text_origin(bounds), style) {
            Ok(primitive) => cx.paint(primitive),
            Err(err) => log::error!("input caret paint failed: {err}"),
        }
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
        style.size.height = Dimension::Length(self.height.unwrap_or(40.0));
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

        self.paint_selection_and_marked_text(cx, bounds);

        // Paint text or placeholder
        let display = if self.state.value.is_empty() {
            &self.placeholder
        } else {
            &self.display_text()
        };

        if !display.is_empty() {
            let text_x = bounds.x() + INPUT_HORIZONTAL_PADDING;
            let text_y = bounds.y() + (bounds.height() - 14.0) / 2.0;
            let text_width = self.text_width(bounds);

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

        self.paint_cursor(cx, bounds);
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

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        if !cx.is_focused(self.id) && !self.state.focused {
            return false;
        }

        if !Self::key_event_is_text_editing(event) {
            return false;
        }

        match self.apply_key_event(event) {
            Ok(_) => {
                cx.request_redraw();
                true
            }
            Err(err) => {
                log::error!("input key event failed: {err}");
                false
            }
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let Some(id) = self.id else {
            return Ok(None);
        };
        let label = self
            .accessibility_label
            .as_deref()
            .or_else(|| (!self.placeholder.trim().is_empty()).then_some(self.placeholder.as_str()))
            .ok_or(AccessibilityError::MissingLabel {
                role: AccessibilityRole::TextInput,
            })?;

        let mut node = AccessibilityNode::label_required(id, AccessibilityRole::TextInput, label)?
            .with_value(self.display_text())
            .with_text_caret(self.normalize_cursor_position())
            .with_focused(cx.a11y_has_focus(id))
            .with_action(AccessibilityAction::SetValue);

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end) {
            node = node.with_text_selection(AccessibilityTextRange::new(start, end));
        }
        if let Some(range) = self.state.composition_range {
            node =
                node.with_text_composition(AccessibilityTextRange::new(range.start(), range.end()));
        }

        Ok(Some(node))
    }
}

/// Create a new Input
pub fn input() -> Input {
    Input::new()
}

#[cfg(test)]
mod tests;
