//! Text input element

mod presentation;

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole, AccessibilityTextRange,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::Color;
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::text_editing::{
    Clipboard, TextEditBuffer, TextEditError, TextEditOutcome, TextInputCommand, TextInputEvent,
    TextInputSnapshot, TextRange, TextSelection,
};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::*;

const INPUT_HORIZONTAL_PADDING: f32 = 12.0;
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
    pub disabled: bool,
    pub read_only: bool,
    pub invalid: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct InputPaintTokens {
    pub background: Color,
    pub text: Color,
    pub placeholder: Color,
    pub border: Color,
    pub focus_ring: Color,
    pub font_size: f32,
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
    on_cancel: Option<Box<dyn Fn()>>,
    on_focus: Option<Box<dyn Fn()>>,
    on_blur: Option<Box<dyn Fn()>>,
    paint_tokens: Option<InputPaintTokens>,
    caret_bounds: Option<Bounds>,
    text_layout: Option<crate::core::text_editing::TextEditLayout>,
}

impl Input {
    pub fn new() -> Self {
        let mut style = Style::new();
        style.border.radius = Corners::all(6.0);
        style.border.color = Color::hex(0xd1d5db);
        style.border.width = Edges::all(1.0);

        Self {
            id: Some(ElementId::new()),
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
            on_cancel: None,
            on_focus: None,
            on_blur: None,
            paint_tokens: None,
            caret_bounds: None,
            text_layout: None,
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        if disabled {
            self.state.focused = false;
            self.state.hovered = false;
        }
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.read_only = read_only;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.invalid = invalid;
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

    pub(crate) fn set_interaction_flags(&mut self, disabled: bool, read_only: bool, invalid: bool) {
        self.state.disabled = disabled;
        self.state.read_only = read_only;
        self.state.invalid = invalid;
        if disabled {
            self.state.focused = false;
            self.state.hovered = false;
        }
    }

    pub(crate) fn set_paint_tokens(&mut self, tokens: InputPaintTokens) {
        self.paint_tokens = Some(tokens);
    }

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    pub fn on_cancel(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Box::new(handler));
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

    pub fn get_value(&self) -> &str {
        &self.state.value
    }

    pub fn apply_text_input_event(
        &mut self,
        event: TextInputEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        self.apply_text_input_command(event.into())
    }

    pub fn apply_key_event(&mut self, event: &KeyEvent) -> Result<TextEditOutcome, TextEditError> {
        if self.state.disabled || (self.state.read_only && Self::key_event_would_mutate(event)) {
            return Ok(TextEditOutcome::default());
        }
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = if event.key == KeyCode::Escape {
            TextEditOutcome {
                changed: false,
                submitted: false,
            }
        } else if matches!(event.char, Some('\n' | '\r')) {
            TextEditOutcome {
                changed: false,
                submitted: true,
            }
        } else {
            self.editor.apply_key_event(event)?
        };
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        if event.key == KeyCode::Escape {
            self.emit_cancel();
        }
        if outcome.submitted {
            self.emit_submit();
        }
        Ok(outcome)
    }

    pub fn copy_selection_to<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<bool, TextEditError> {
        if self.state.disabled {
            return Ok(false);
        }
        self.sync_editor_from_public_state_if_needed()?;
        self.editor.copy_selection_to(clipboard)
    }

    pub fn cut_selection_to<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<TextEditOutcome, TextEditError> {
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
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
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
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

    fn can_edit(&self) -> bool {
        !self.state.disabled && !self.state.read_only
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

    fn emit_cancel(&self) {
        if let Some(handler) = &self.on_cancel {
            handler();
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
                | KeyCode::Escape
        ) || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
    }

    pub(crate) fn key_event_would_mutate(event: &KeyEvent) -> bool {
        matches!(
            event.key,
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter
        ) || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
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

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.native_text_input_snapshot(focused)
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.update_text_layout(cx.text_measurer(), self.height.unwrap_or(40.0));
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
        node
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        self.refresh_text_layout_if_stale(text_measurer);
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        if let Some(id) = self.id {
            cx.register_accessibility_region(id, bounds);
        }
        let (bg, text_color, border_color) = self.colors();
        cx.paint(Primitive::Quad {
            bounds,
            background: bg.to_rgba(),
            border_color: border_color.to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: self.style.border.radius,
        });

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
                border_color: self
                    .paint_tokens
                    .map(|tokens| tokens.focus_ring.with_alpha(0.3))
                    .unwrap_or_else(|| Color::hex(0x6366f1).with_alpha(0.3))
                    .to_rgba(),
                border_widths: Edges::all(2.0),
                corner_radii: Corners::all(8.0),
            });
        }

        cx.scene.push_layer(bounds);
        self.paint_selection_and_marked_text(cx, bounds);

        // Paint text or placeholder
        let display = if self.state.value.is_empty() {
            &self.placeholder
        } else {
            &self.display_text()
        };

        if !display.is_empty() {
            let text_x = bounds.x() + INPUT_HORIZONTAL_PADDING;
            let font_size = self.font_size(bounds);
            let text_y = bounds.y() + (bounds.height() - font_size).max(0.0) / 2.0;
            let text_width = self.text_width(bounds);

            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(text_x, text_y, text_width, font_size),
                content: display.to_string(),
                color: text_color.to_rgba(),
                font_size,
                font_weight: 400,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Left,
            });
        }

        self.caret_bounds = self.paint_cursor(cx, bounds);
        cx.scene.pop_layer();
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.state.disabled {
            self.state.hovered = false;
            return false;
        }

        self.sync_focus_from_context(cx);

        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.hovered = inside;
                false
            }
            PointerEventKind::Down => {
                if inside {
                    if matches!(
                        event.button,
                        None | Some(crate::core::event::MouseButton::Left)
                    ) {
                        match self.set_cursor_from_shaped_point(event.position, cx.bounds()) {
                            Ok(true) => cx.request_redraw(),
                            Ok(false) => {}
                            Err(err) => log::error!("input pointer text positioning failed: {err}"),
                        }
                    }
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

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.handle_text_input_command_impl(cx, &event.clone().into())
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.handle_text_input_command_impl(cx, command)
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

        let result = self
            .apply_shaped_navigation(event)
            .unwrap_or_else(|| self.apply_key_event(event));
        match result {
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

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if matches!(action, ActionId::Custom(name) if name == crate::core::action::SYNC_ACCESSIBILITY_FOCUS_ACTION)
        {
            self.sync_focus_from_context(cx);
            return ActionOutcome::Ignored;
        }
        if !cx.is_focused(self.id) && !self.state.focused {
            return ActionOutcome::Ignored;
        }

        let ActionId::Standard(action) = action else {
            return ActionOutcome::Ignored;
        };

        if *action == StandardAction::Cancel {
            self.emit_cancel();
            return ActionOutcome::handled("input");
        }

        if *action == StandardAction::SelectAll {
            let result = self
                .sync_editor_from_public_state_if_needed()
                .and_then(|_| {
                    let end = self.editor.text().len();
                    self.editor.set_selection(TextSelection::new(0, end))
                });
            return match result {
                Ok(()) => {
                    self.sync_state_from_editor();
                    cx.request_redraw();
                    ActionOutcome::handled("input")
                }
                Err(err) => {
                    log::error!("input select all action failed: {err}");
                    ActionOutcome::Ignored
                }
            };
        }

        let event = match action {
            StandardAction::MoveLeft => KeyEvent::new(KeyCode::ArrowLeft, Modifiers::none()),
            StandardAction::MoveRight => KeyEvent::new(KeyCode::ArrowRight, Modifiers::none()),
            StandardAction::MoveUp => KeyEvent::new(KeyCode::ArrowUp, Modifiers::none()),
            StandardAction::MoveDown => KeyEvent::new(KeyCode::ArrowDown, Modifiers::none()),
            StandardAction::MoveWordLeft => KeyEvent::new(KeyCode::ArrowLeft, Modifiers::alt()),
            StandardAction::MoveWordRight => KeyEvent::new(KeyCode::ArrowRight, Modifiers::alt()),
            StandardAction::SelectLeft => KeyEvent::new(KeyCode::ArrowLeft, Modifiers::shift()),
            StandardAction::SelectRight => KeyEvent::new(KeyCode::ArrowRight, Modifiers::shift()),
            StandardAction::DeleteBackward => KeyEvent::new(KeyCode::Backspace, Modifiers::none()),
            StandardAction::DeleteForward => KeyEvent::new(KeyCode::Delete, Modifiers::none()),
            StandardAction::Activate | StandardAction::Submit | StandardAction::InsertNewline => {
                KeyEvent::new(KeyCode::Enter, Modifiers::none())
            }
            _ => return ActionOutcome::Ignored,
        };

        match self.apply_key_event(&event) {
            Ok(_) => {
                cx.request_redraw();
                ActionOutcome::handled("input")
            }
            Err(err) => {
                log::error!("input action failed: {err}");
                ActionOutcome::Ignored
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
            .with_enabled(!self.state.disabled)
            .with_read_only(self.state.read_only)
            .with_invalid(self.state.invalid)
            .with_focused(cx.a11y_has_focus(id));
        if let Some(caret) = self.display_offset_for_value_offset(self.normalize_cursor_position())
        {
            node = node.with_text_caret(caret);
        }
        if !self.state.disabled && !self.state.read_only {
            node = node.with_action(AccessibilityAction::SetValue);
        }

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end) {
            let range =
                TextRange::new(start, end).map_err(|_| AccessibilityError::BridgeFailure {
                    message: "input accessibility selection is not a valid text range".to_string(),
                })?;
            if let Some(range) = self.display_range_for_value_range(range) {
                node = node
                    .with_text_selection(AccessibilityTextRange::new(range.start(), range.end()));
            }
        }
        if let Some(range) = self.state.composition_range {
            if let Some(range) = self.display_range_for_value_range(range) {
                node = node
                    .with_text_composition(AccessibilityTextRange::new(range.start(), range.end()));
            }
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
