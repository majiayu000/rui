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
use crate::core::style::{AlignItems, Corners, Display, FlexDirection, Position, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::element::StableId;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    dispatch_action_to_children, style_to_taffy,
};
use crate::elements::overlay::{Align, OverlaySlot, Placement, Side, place_anchored};
use crate::renderer::text::TextMeasureCache;
use crate::renderer::{Overlay, Primitive};
use taffy::prelude::*;

mod dialog;
pub use dialog::Dialog;

type DismissHandler = Box<dyn Fn()>;

/// Content shown next to an anchor, in an overlay over the page: it takes
/// no room in the layout, flips and shifts to stay in the window, and closes
/// on a press outside it or on Escape.
pub struct Popover {
    id: StableId,
    label: String,
    open: bool,
    state: InteractionState,
    /// The anchor, then the content.
    children: Vec<AnyElement>,
    anchor_node: Option<NodeId>,
    slot: OverlaySlot,
    side: Side,
    align: Align,
    theme: Theme,
    style: Style,
    on_dismiss: Option<DismissHandler>,
}

impl Popover {
    pub fn new(
        label: impl Into<String>,
        anchor: impl Into<AnyElement>,
        content: impl Into<AnyElement>,
    ) -> Self {
        let label = label.into();
        require_non_empty(&label, "popover accessibility label must not be empty");

        let theme = Theme::default();
        Self {
            id: StableId::new(),
            label,
            open: false,
            state: InteractionState::default(),
            children: vec![anchor.into(), content.into()],
            anchor_node: None,
            slot: OverlaySlot::default(),
            side: Side::Bottom,
            align: Align::Start,
            theme,
            style: overlay_stack_style(theme),
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

    /// The side of the anchor the content opens on; below by default.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// How the content lines up with the anchor; at its start by default.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
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
        self.style = overlay_stack_style(theme);
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(handler));
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn visible_child_count(&self) -> usize {
        if self.open { 2 } else { 1 }
    }

    /// Where the anchor is, given the popover's own `bounds`.
    fn anchor_bounds(&self, taffy: &TaffyTree<ElementId>, bounds: Bounds) -> Bounds {
        self.anchor_node
            .and_then(|node| child_bounds(taffy, bounds, node))
            .unwrap_or(bounds)
    }

    /// Where the open content goes, given the popover's own `bounds`.
    pub(crate) fn content_bounds(
        &self,
        taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        window: Bounds,
    ) -> Bounds {
        place_anchored(
            self.anchor_bounds(taffy, bounds),
            self.slot.size(taffy).unwrap_or_default(),
            window,
            Placement::new(self.side, self.align, self.style.gap),
        )
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

impl Element for Popover {
    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let mut nodes = Vec::new();
        for child in &self.children[..self.visible_child_count()] {
            nodes.extend(child.accessibility_nodes(cx)?);
        }
        Ok(match self.accessibility(cx)? {
            Some(node) => vec![node.with_children(nodes)],
            None => nodes,
        })
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
        // The anchor first, so it keeps its identity as the content opens.
        let anchor = self.children[0].layout(cx);
        self.anchor_node = Some(anchor);
        if self.open {
            self.slot.layout(cx, &mut self.children[1]);
        } else {
            self.slot.clear();
        }
        cx.new_with_children(style_to_taffy(&self.style), &[anchor])
            .unwrap_or_else(|err| panic!("failed to create advanced popover layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        cx.register_hit_region(self.id.get(), cx.bounds());
        let anchor = self
            .anchor_node
            .and_then(|node| cx.child_bounds(node))
            .unwrap_or(cx.bounds());
        self.children[0].paint(&mut cx.with_bounds(anchor));
        if !self.open {
            return;
        }
        let content_bounds = self.content_bounds(cx.taffy, cx.bounds(), cx.window());
        let (id, invalid, theme) = (self.id.get(), self.state.invalid(), self.theme);
        let content = &mut self.children[1];
        cx.paint_overlay(Overlay::popup(id, content_bounds), content_bounds, |cx| {
            cx.register_hit_region(id, content_bounds);
            paint_panel(cx, content_bounds, invalid, theme);
            content.paint(cx);
        });
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for child in self.children.iter_mut().take(usize::from(self.open) + 1) {
            child.refresh_text_geometry(text_measurer);
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
                AccessibilityRole::Popover,
                &self.label,
            )?
            .with_enabled(!self.state.disabled())
            .with_read_only(self.state.read_only())
            .with_invalid(self.state.invalid())
            .with_focused(cx.a11y_has_focus(self.id.get())),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        // The content is over everything, so it goes first.
        let mut in_content = false;
        if self.open {
            let content_bounds = self.content_bounds(cx.taffy, cx.layout_bounds(), cx.window());
            let mut content_cx = cx.for_overlay(content_bounds);
            if self.children[1].handle_pointer_event(&mut content_cx, event) {
                return true;
            }
            in_content = content_cx.contains_pointer(event.position);
            if in_content {
                return true;
            }
        }

        let anchor = self.anchor_bounds(cx.taffy, cx.layout_bounds());
        let mut anchor_cx = cx.with_bounds(anchor);
        let in_anchor = anchor_cx.contains_pointer(event.position);
        if self.children[0].handle_pointer_event(&mut anchor_cx, event) {
            return true;
        }

        // A press outside the anchor and the content (and the overlays it
        // opened, which the hit target is in) closes it.
        if self.open && matches!(event.kind, PointerEventKind::Down) {
            let outside = match cx.hit_target() {
                Some(target) => !self.contains_id(target),
                None => !in_anchor && !in_content,
            };
            if outside {
                self.dismiss(cx);
            }
        }
        false
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }

        let content_bounds = self.content_bounds(cx.taffy, cx.layout_bounds(), cx.window());
        if self.children[1].handle_scroll_event(&mut cx.for_overlay(content_bounds), event) {
            return true;
        }
        let anchor = self.anchor_bounds(cx.taffy, cx.layout_bounds());
        self.children[0].handle_scroll_event(&mut cx.with_bounds(anchor), event)
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if matches!(event.key, KeyCode::Escape)
            && cx.is_escape_target(self.id.get())
            && self.dismiss(cx)
        {
            return true;
        }

        for index in (0..self.visible_child_count()).rev() {
            if self.children[index].handle_key_event(cx, event) {
                return true;
            }
        }

        !self.state.can_activate()
            && self
                .open
                .then_some(())
                .and(cx.focused_id())
                .is_some_and(|focused| self.contains_id(focused))
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        // Escape reaches `handle_key_event` while this is the overlay it closes.
        if matches!(action, ActionId::Standard(StandardAction::Cancel))
            && cx.is_escape_target(self.id.get())
        {
            return ActionOutcome::Ignored;
        }
        let visible = self.visible_child_count();
        dispatch_action_to_children(&mut self.children[..visible], cx, action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.children
            .iter_mut()
            .take(usize::from(self.open) + 1)
            .rev()
            .any(|child| child.handle_text_input_event(cx, event))
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        let visible = self.visible_child_count();
        if let Some(focused) = cx.focused_id()
            && let Some(child) = self.children[..visible]
                .iter_mut()
                .rev()
                .find(|child| child.contains_id(focused))
        {
            return child.handle_text_input_command(cx, command);
        }
        self.children[..visible]
            .iter_mut()
            .rev()
            .any(|child| child.handle_text_input_command(cx, command))
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.children[..self.visible_child_count()]
            .iter()
            .find_map(|child| child.text_input_snapshot(focused))
    }

    fn children(&self) -> &[AnyElement] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        // Hidden ones too: a view among them must have rendered when it shows.
        &mut self.children
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id.get() == id
            || self.children[..self.visible_child_count()]
                .iter()
                .any(|child| child.contains_id(id))
    }
}

fn child_bounds(taffy: &TaffyTree<ElementId>, bounds: Bounds, node: NodeId) -> Option<Bounds> {
    let layout = taffy.layout(node).ok()?;
    Some(Bounds::from_xywh(
        bounds.x() + layout.location.x,
        bounds.y() + layout.location.y,
        layout.size.width,
        layout.size.height,
    ))
}

fn overlay_stack_style(theme: Theme) -> Style {
    let mut style = Style::new();
    style.display = Display::Flex;
    style.flex_direction = FlexDirection::Column;
    style.align_items = AlignItems::FlexStart;
    // The gap between the anchor and the open content.
    style.gap = theme.control_gap() * 0.75;
    style.position = Position::Relative;
    style
}

fn paint_panel(cx: &mut PaintContext, bounds: Bounds, invalid: bool, theme: Theme) {
    cx.paint(Primitive::Shadow {
        bounds,
        corner_radii: Corners::all(theme.control_radius()),
        blur_radius: 12.0,
        color: Color::rgba(0.0, 0.0, 0.0, 0.18).to_rgba(),
    });
    cx.paint(Primitive::Quad {
        bounds,
        background: theme.colors.surface.to_rgba(),
        border_color: theme
            .validation_border_color(invalid, theme.colors.border)
            .to_rgba(),
        border_widths: Edges::all(1.0),
        corner_radii: Corners::all(theme.control_radius()),
    });
}

pub fn popover(
    label: impl Into<String>,
    anchor: impl Into<AnyElement>,
    content: impl Into<AnyElement>,
) -> Popover {
    Popover::new(label, anchor, content)
}

pub fn dialog(label: impl Into<String>, content: impl Into<AnyElement>) -> Dialog {
    Dialog::new(label, content)
}

#[cfg(test)]
mod tests;
