//! Multiline text area element.

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole, AccessibilityTextRange,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::{Color, Rgba};
use crate::core::event::{Cursor, KeyCode, KeyEvent};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::core::text_editing::{
    Clipboard, TextEditBuffer, TextEditError, TextEditLayout, TextEditOutcome, TextEditPaintStyle,
    TextInputEvent, TextInputSnapshot, TextRange, TextSelection,
};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

const TEXT_AREA_HORIZONTAL_PADDING: f32 = 12.0;
const TEXT_AREA_VERTICAL_PADDING: f32 = 10.0;
const TEXT_AREA_GRAPHEME_WIDTH: f32 = 7.0;
const TEXT_AREA_LINE_HEIGHT: f32 = 20.0;
const TEXT_AREA_CARET_WIDTH: f32 = 1.5;
const TEXT_AREA_MARKED_UNDERLINE_HEIGHT: f32 = 2.0;

#[derive(Debug, Clone, Default)]
pub struct TextAreaState {
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

pub struct TextArea {
    id: Option<ElementId>,
    placeholder: String,
    accessibility_label: Option<String>,
    style: Style,
    state: TextAreaState,
    editor: TextEditBuffer,
    width: Option<f32>,
    height: f32,
    on_change: Option<Box<dyn Fn(&str)>>,
    on_focus: Option<Box<dyn Fn()>>,
    on_blur: Option<Box<dyn Fn()>>,
    layout_node: Option<NodeId>,
}

impl TextArea {
    pub fn new() -> Self {
        let mut style = Style::new();
        style.border.radius = Corners::all(6.0);
        style.border.color = Color::hex(0xd1d5db);
        style.border.width = Edges::all(1.0);

        Self {
            id: None,
            placeholder: String::new(),
            accessibility_label: None,
            style,
            state: TextAreaState::default(),
            editor: TextEditBuffer::multiline(),
            width: None,
            height: 96.0,
            on_change: None,
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
        self.editor = TextEditBuffer::multiline_with_text(value.into());
        self.sync_state_from_editor();
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.height = height.max(40.0);
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

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
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

    pub fn value_text(&self) -> &str {
        &self.state.value
    }

    pub fn cursor_position(&self) -> usize {
        self.state.cursor_position
    }

    pub fn selection_range(&self) -> Option<TextRange> {
        match (self.state.selection_start, self.state.selection_end) {
            (Some(start), Some(end)) => TextRange::new(start, end).ok(),
            _ => None,
        }
    }

    pub fn composition_range(&self) -> Option<TextRange> {
        self.state.composition_range
    }

    pub fn marked_text(&self) -> Option<&str> {
        self.state.marked_text.as_deref()
    }

    pub fn cursor(&self) -> Cursor {
        if self.state.disabled {
            Cursor::NotAllowed
        } else if self.state.read_only {
            Cursor::Default
        } else {
            Cursor::Text
        }
    }

    fn sync_focus_from_context(&mut self, cx: &mut EventContext<'_>) -> bool {
        let focused = cx.is_focused(self.id);
        if self.state.focused == focused {
            return false;
        }
        self.state.focused = focused;
        if focused {
            if let Some(handler) = &self.on_focus {
                handler();
            }
        } else if let Some(handler) = &self.on_blur {
            handler();
        }
        cx.request_redraw();
        true
    }

    pub fn apply_text_input_event(
        &mut self,
        event: TextInputEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = self.editor.apply_text_input_event(event)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        Ok(outcome)
    }

    pub fn apply_key_event(&mut self, event: &KeyEvent) -> Result<TextEditOutcome, TextEditError> {
        if self.state.disabled || (self.state.read_only && Self::key_event_would_mutate(event)) {
            return Ok(TextEditOutcome::default());
        }
        self.sync_editor_from_public_state_if_needed()?;
        let outcome = self.editor.apply_key_event(event)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
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

    fn can_edit(&self) -> bool {
        !self.state.disabled && !self.state.read_only
    }

    fn colors(&self) -> (Color, Color, Color) {
        let bg = if self.state.disabled {
            Color::hex(0xf3f4f6)
        } else {
            Color::WHITE
        };
        let text = if self.state.value.is_empty() {
            Color::hex(0x9ca3af)
        } else if self.state.disabled {
            Color::hex(0x6b7280)
        } else {
            Color::hex(0x111827)
        };
        let border = if self.state.invalid {
            Color::hex(0xdc2626)
        } else if self.state.focused {
            Color::hex(0x6366f1)
        } else if self.state.hovered {
            Color::hex(0x9ca3af)
        } else {
            Color::hex(0xd1d5db)
        };
        (bg, text, border)
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
        let mut editor = TextEditBuffer::multiline_with_text(self.state.value.clone());
        editor.set_selection(self.state_selection())?;
        self.editor = editor;
        self.sync_state_from_editor();
        Ok(())
    }

    fn public_state_matches_editor(&self) -> bool {
        if self.state.value != self.editor.text() {
            return false;
        }
        if self.normalize_cursor_position() != self.editor.selection().head() {
            return false;
        }
        let range = self.editor.selection().normalized_range();
        let expected = if range.is_empty() {
            (None, None)
        } else {
            (Some(range.start()), Some(range.end()))
        };
        if (self.state.selection_start, self.state.selection_end) != expected {
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

    fn key_event_would_mutate(event: &KeyEvent) -> bool {
        matches!(
            event.key,
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter
        ) || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
    }

    fn text_layout(&self) -> TextEditLayout {
        TextEditLayout::new(
            self.state.value.clone(),
            TEXT_AREA_GRAPHEME_WIDTH,
            TEXT_AREA_LINE_HEIGHT,
        )
    }

    fn text_origin(&self, bounds: Bounds) -> Point {
        Point::new(
            bounds.x() + TEXT_AREA_HORIZONTAL_PADDING,
            bounds.y() + TEXT_AREA_VERTICAL_PADDING,
        )
    }

    fn text_width(&self, bounds: Bounds) -> f32 {
        bounds.width() - (TEXT_AREA_HORIZONTAL_PADDING * 2.0)
    }

    fn paint_selection_and_marked_text(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }
        let layout = self.text_layout();
        let style = TextEditPaintStyle::new(
            TEXT_AREA_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        let paint_origin = self.text_origin(bounds);

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end)
            && let Ok(range) = TextRange::new(start, end)
            && let Ok(primitives) = layout.selection_primitives(range, paint_origin, style)
        {
            for primitive in primitives {
                cx.paint(primitive);
            }
        }

        if let Some(range) = self.state.composition_range
            && let Ok(rects) = layout.selection_rects(range)
        {
            for rect in rects {
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(
                        paint_origin.x + rect.bounds.x(),
                        paint_origin.y + rect.bounds.y() + rect.bounds.height()
                            - TEXT_AREA_MARKED_UNDERLINE_HEIGHT,
                        rect.bounds.width(),
                        TEXT_AREA_MARKED_UNDERLINE_HEIGHT,
                    ),
                    background: Color::hex(0x6366f1).to_rgba(),
                    border_color: Rgba::TRANSPARENT,
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::ZERO,
                });
            }
        }
    }

    fn paint_cursor(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }
        let layout = self.text_layout();
        let style = TextEditPaintStyle::new(
            TEXT_AREA_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        match layout.caret_primitive(
            self.normalize_cursor_position(),
            self.text_origin(bounds),
            style,
        ) {
            Ok(primitive) => cx.paint(primitive),
            Err(err) => log::error!("text area caret paint failed: {err}"),
        }
    }
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for TextArea {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        (self.id == Some(focused)).then(|| {
            TextInputSnapshot::new(
                self.state.value.clone(),
                self.state_selection(),
                self.state.composition_range,
            )
        })
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.height = Dimension::Length(self.height);
        if let Some(w) = self.width {
            style.size.width = Dimension::Length(w);
        } else {
            style.flex_grow = 1.0;
        }
        style.padding = taffy::Rect {
            top: LengthPercentage::Length(TEXT_AREA_VERTICAL_PADDING),
            right: LengthPercentage::Length(TEXT_AREA_HORIZONTAL_PADDING),
            bottom: LengthPercentage::Length(TEXT_AREA_VERTICAL_PADDING),
            left: LengthPercentage::Length(TEXT_AREA_HORIZONTAL_PADDING),
        };

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create text area layout node");
        self.layout_node = Some(node);
        node
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

        cx.scene.push_layer(bounds);
        self.paint_selection_and_marked_text(cx, bounds);

        let origin = self.text_origin(bounds);
        let text_width = self.text_width(bounds);
        let display = if self.state.value.is_empty() {
            self.placeholder.as_str()
        } else {
            self.state.value.as_str()
        };
        for (line_index, line) in display.split('\n').enumerate() {
            if line.is_empty() {
                continue;
            }
            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(
                    origin.x,
                    origin.y + line_index as f32 * TEXT_AREA_LINE_HEIGHT,
                    text_width,
                    14.0,
                ),
                content: line.to_string(),
                color: text_color.to_rgba(),
                font_size: 14.0,
                font_weight: 400,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Left,
            });
        }

        self.paint_cursor(cx, bounds);
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

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
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
                log::error!("text area key event failed: {err}");
                false
            }
        }
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if !cx.is_focused(self.id) && !self.state.focused {
            return false;
        }
        match self.apply_text_input_event(event.clone()) {
            Ok(_) => {
                cx.request_redraw();
                true
            }
            Err(err) => {
                log::error!("text area text input event failed: {err}");
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
        if self.state.disabled {
            return ActionOutcome::Ignored;
        }

        let ActionId::Standard(action) = action else {
            return ActionOutcome::Ignored;
        };

        if *action != StandardAction::SelectAll {
            return ActionOutcome::Ignored;
        }

        let result = self
            .sync_editor_from_public_state_if_needed()
            .and_then(|_| {
                let end = self.editor.text().len();
                self.editor.set_selection(TextSelection::new(0, end))
            });
        match result {
            Ok(()) => {
                self.sync_state_from_editor();
                cx.request_redraw();
                ActionOutcome::handled("text_area")
            }
            Err(err) => {
                log::error!("text area select all action failed: {err}");
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
            .with_value(self.state.value.clone())
            .with_enabled(!self.state.disabled)
            .with_read_only(self.state.read_only)
            .with_invalid(self.state.invalid)
            .with_text_caret(self.normalize_cursor_position())
            .with_focused(cx.a11y_has_focus(id));
        if !self.state.disabled && !self.state.read_only {
            node = node.with_action(AccessibilityAction::SetValue);
        }
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

pub fn text_area() -> TextArea {
    TextArea::new()
}
