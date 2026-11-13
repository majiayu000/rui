use super::{DismissHandler, paint_panel};
use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::Theme;
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::Color;
use crate::core::event::{KeyCode, KeyEvent, ScrollEvent};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Position, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::element::StableId;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    dispatch_action_to_children, style_to_taffy,
};
use crate::elements::overlay::{OverlaySlot, center_in};
use crate::renderer::text::TextMeasureCache;
use crate::renderer::{Overlay, Primitive};
use taffy::prelude::*;

/// A dialog centred in the window, in an overlay over the page. A modal one
/// dims the whole window behind it, takes the pointer from it, and keeps
/// focus inside itself until it closes; Escape closes it.
pub struct Dialog {
    id: StableId,
    label: String,
    open: bool,
    modal: bool,
    state: InteractionState,
    content: Vec<AnyElement>,
    slot: OverlaySlot,
    theme: Theme,
    style: Style,
    on_dismiss: Option<DismissHandler>,
}

impl Dialog {
    pub fn new(label: impl Into<String>, content: impl Into<AnyElement>) -> Self {
        let label = label.into();
        require_non_empty(&label, "dialog accessibility label must not be empty");

        let mut style = Style::new();
        // Takes no room where it is placed: its content is an overlay.
        style.position = Position::Absolute;
        Self {
            id: StableId::new(),
            label,
            open: true,
            modal: true,
            state: InteractionState::default(),
            content: vec![content.into()],
            slot: OverlaySlot::default(),
            theme: Theme::default(),
            style,
            on_dismiss: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.set_invalid(invalid);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(handler));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_modal(&self) -> bool {
        self.modal
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    /// Where the content goes in `window`.
    pub(crate) fn content_bounds(&self, taffy: &TaffyTree<ElementId>, window: Bounds) -> Bounds {
        center_in(window, self.slot.size(taffy).unwrap_or_default())
    }

    fn dismiss(&mut self, cx: &EventContext) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }
        self.open = false;
        if let Some(handler) = &self.on_dismiss {
            handler();
        }
        cx.announce_accessibility_action(self.id.get(), format!("{} dismissed", self.label));
        cx.request_redraw();
        true
    }
}

impl Element for Dialog {
    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        if !self.open {
            return Ok(Vec::new());
        }
        let nodes = self.content[0].accessibility_nodes(cx)?;
        Ok(self
            .accessibility(cx)?
            .map(|node| vec![node.with_children(nodes)])
            .unwrap_or_default())
    }

    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        if self.open {
            self.slot.layout(cx, &mut self.content[0]);
        } else {
            self.slot.clear();
        }
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(0.0);
        style.size.height = Dimension::Length(0.0);
        cx.new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create dialog layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if !self.open {
            return;
        }

        let window = cx.window();
        let content_bounds = self.content_bounds(cx.taffy, window);
        let (id, modal) = (self.id.get(), self.modal);
        let covers = if modal { window } else { content_bounds };
        let (invalid, theme) = (self.state.invalid(), self.theme);
        let content = &mut self.content[0];
        cx.paint_overlay(Overlay::dialog(id, covers, modal), content_bounds, |cx| {
            if modal {
                cx.register_hit_region(id, window);
                cx.paint(Primitive::Quad {
                    bounds: window,
                    background: Color::rgba(0.0, 0.0, 0.0, 0.32).to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::ZERO,
                });
            }
            cx.register_hit_region(id, content_bounds);
            paint_panel(cx, content_bounds, invalid, theme);
            content.paint(cx);
        });
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        if self.open {
            self.content[0].refresh_text_geometry(text_measurer);
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        if !self.open {
            return Ok(None);
        }

        Ok(Some(
            AccessibilityNode::label_required(
                self.id.get(),
                AccessibilityRole::Dialog,
                &self.label,
            )?
            .with_enabled(!self.state.disabled())
            .with_read_only(self.state.read_only())
            .with_invalid(self.state.invalid())
            .with_focused(cx.a11y_has_focus(self.id.get())),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if !self.open {
            return false;
        }

        let window = cx.window();
        let content_bounds = self.content_bounds(cx.taffy, window);
        let inside_modal_region =
            self.modal && cx.for_overlay(window).contains_pointer(event.position);
        let mut content_cx = cx.for_overlay(content_bounds);
        let inside_content = content_cx.contains_pointer(event.position);
        if self.content[0].handle_pointer_event(&mut content_cx, event) {
            return true;
        }

        inside_content || inside_modal_region
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }

        let content_bounds = self.content_bounds(cx.taffy, cx.window());
        let mut content_cx = cx.for_overlay(content_bounds);
        self.content[0].handle_scroll_event(&mut content_cx, event)
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.open {
            return false;
        }

        if matches!(event.key, KeyCode::Escape)
            && cx.is_escape_target(self.id.get())
            && self.dismiss(cx)
        {
            return true;
        }

        if self.content[0].handle_key_event(cx, event) {
            return true;
        }

        if self.state.can_activate() {
            return self.modal;
        }

        self.modal
            || cx
                .focused_id()
                .is_some_and(|focused| self.content[0].contains_id(focused))
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !self.open {
            return ActionOutcome::Ignored;
        }
        // Escape reaches `handle_key_event` while this is the overlay it closes.
        if matches!(action, ActionId::Standard(StandardAction::Cancel))
            && cx.is_escape_target(self.id.get())
        {
            return ActionOutcome::Ignored;
        }
        dispatch_action_to_children(&mut self.content, cx, action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.open && self.content[0].handle_text_input_event(cx, event)
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.open && self.content[0].handle_text_input_command(cx, command)
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.open
            .then(|| self.content[0].text_input_snapshot(focused))
            .flatten()
    }

    fn children(&self) -> &[AnyElement] {
        &self.content
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        // Closed content too: a view in it must have rendered when it opens.
        &mut self.content
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id.get() == id || (self.open && self.content[0].contains_id(id))
    }
}
