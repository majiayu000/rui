use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::Theme;
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::event::{KeyCode, KeyEvent, ScrollEvent};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{
    AlignItems, Corners, Display, FlexDirection, JustifyContent, Position, Style,
};
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::*;

type DismissHandler = Box<dyn Fn()>;

pub struct Popover {
    id: ElementId,
    label: String,
    open: bool,
    state: InteractionState,
    children: Vec<AnyElement>,
    child_nodes: Vec<NodeId>,
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
            id: ElementId::new(),
            label,
            open: false,
            state: InteractionState::default(),
            children: vec![anchor.into(), content.into()],
            child_nodes: Vec::new(),
            theme,
            style: overlay_stack_style(theme),
            on_dismiss: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
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

    fn dismiss(&mut self, cx: &EventContext) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }
        self.open = false;
        if let Some(handler) = &self.on_dismiss {
            handler();
        }
        cx.announce_accessibility_action(self.id, format!("{} dismissed", self.label));
        cx.request_redraw();
        true
    }
}

impl Element for Popover {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let visible = self.visible_child_count();
        let child_nodes: Vec<NodeId> = self.children[..visible]
            .iter_mut()
            .map(|child| child.layout(cx))
            .collect();
        let node = cx
            .taffy
            .new_with_children(style_to_taffy(&self.style), &child_nodes)
            .unwrap_or_else(|err| panic!("failed to create advanced popover layout node: {err}"));
        self.child_nodes = child_nodes;
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        cx.register_hit_region(self.id, cx.bounds());
        for index in 0..self.visible_child_count() {
            let node = self.child_nodes[index];
            let bounds = cx.child_bounds(node).unwrap_or(cx.bounds());
            if index == 1 {
                paint_panel(cx, bounds, self.state.invalid(), self.theme);
            }
            let mut child_cx = cx.with_bounds(bounds);
            self.children[index].paint(&mut child_cx);
        }
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
            AccessibilityNode::label_required(self.id, AccessibilityRole::Popover, &self.label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id)),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if !self.state.can_activate() {
            return false;
        }

        for index in (0..self.visible_child_count()).rev() {
            let node = self.child_nodes.get(index).copied();
            let bounds = node
                .and_then(|node| cx.child_bounds(node))
                .unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(bounds);
            if self.children[index].handle_pointer_event(&mut child_cx, event) {
                return true;
            }
            if index == 1 && bounds.contains(event.position) {
                return true;
            }
        }
        false
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }

        for index in (0..self.visible_child_count()).rev() {
            let node = self.child_nodes.get(index).copied();
            let bounds = node
                .and_then(|node| cx.child_bounds(node))
                .unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(bounds);
            if self.children[index].handle_scroll_event(&mut child_cx, event) {
                return true;
            }
        }
        false
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if matches!(event.key, KeyCode::Escape) && self.dismiss(cx) {
            return true;
        }

        if !self.state.can_activate() {
            return self
                .open
                .then_some(())
                .and(cx.focused_id())
                .is_some_and(|focused| self.contains_id(focused));
        }

        for index in (0..self.visible_child_count()).rev() {
            if self.children[index].handle_key_event(cx, event) {
                return true;
            }
        }
        false
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if !self.state.can_activate() {
            return false;
        }
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
        if !self.state.can_activate() {
            return false;
        }
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
        &self.children[..self.visible_child_count()]
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id
            || self.children[..self.visible_child_count()]
                .iter()
                .any(|child| child.contains_id(id))
    }
}

pub struct Dialog {
    id: ElementId,
    label: String,
    open: bool,
    modal: bool,
    state: InteractionState,
    content: Vec<AnyElement>,
    content_node: Option<NodeId>,
    theme: Theme,
    style: Style,
    on_dismiss: Option<DismissHandler>,
}

impl Dialog {
    pub fn new(label: impl Into<String>, content: impl Into<AnyElement>) -> Self {
        let label = label.into();
        require_non_empty(&label, "dialog accessibility label must not be empty");

        let theme = Theme::default();
        Self {
            id: ElementId::new(),
            label,
            open: true,
            modal: true,
            state: InteractionState::default(),
            content: vec![content.into()],
            content_node: None,
            theme,
            style: dialog_root_style(),
            on_dismiss: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
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

    fn dismiss(&mut self, cx: &EventContext) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }
        self.open = false;
        if let Some(handler) = &self.on_dismiss {
            handler();
        }
        cx.announce_accessibility_action(self.id, format!("{} dismissed", self.label));
        cx.request_redraw();
        true
    }
}

impl Element for Dialog {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        if !self.open {
            let mut closed = style_to_taffy(&self.style);
            closed.size.width = Dimension::Length(0.0);
            closed.size.height = Dimension::Length(0.0);
            self.content_node = None;
            return cx
                .taffy
                .new_leaf(closed)
                .unwrap_or_else(|err| panic!("failed to create closed dialog layout node: {err}"));
        }

        let content_node = self.content[0].layout(cx);
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(cx.available_space.width);
        style.size.height = Dimension::Length(cx.available_space.height);
        let node = cx
            .taffy
            .new_with_children(style, &[content_node])
            .unwrap_or_else(|err| panic!("failed to create advanced dialog layout node: {err}"));
        self.content_node = Some(content_node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if !self.open {
            return;
        }

        if self.modal {
            cx.register_hit_region(self.id, cx.bounds());
            cx.paint(Primitive::Quad {
                bounds: cx.bounds(),
                background: Color::rgba(0.0, 0.0, 0.0, 0.32).to_rgba(),
                border_color: Color::TRANSPARENT.to_rgba(),
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            });
        }

        let content_bounds = self
            .content_node
            .and_then(|node| cx.child_bounds(node))
            .unwrap_or(cx.bounds());
        cx.register_hit_region(self.id, content_bounds);
        paint_panel(cx, content_bounds, self.state.invalid(), self.theme);
        let mut content_cx = cx.with_bounds(content_bounds);
        self.content[0].paint(&mut content_cx);
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
            AccessibilityNode::label_required(self.id, AccessibilityRole::Dialog, &self.label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id)),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if !self.open {
            return false;
        }

        let content_bounds = self
            .content_node
            .and_then(|node| cx.child_bounds(node))
            .unwrap_or(cx.bounds());
        let inside_content = content_bounds.contains(event.position);
        let inside_modal_region = self.modal && cx.bounds().contains(event.position);

        if self.state.can_activate() {
            let mut content_cx = cx.with_bounds(content_bounds);
            if self.content[0].handle_pointer_event(&mut content_cx, event) {
                return true;
            }
        }

        inside_content || inside_modal_region
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if !self.open || !self.state.can_activate() {
            return false;
        }

        let content_bounds = self
            .content_node
            .and_then(|node| cx.child_bounds(node))
            .unwrap_or(cx.bounds());
        let mut content_cx = cx.with_bounds(content_bounds);
        self.content[0].handle_scroll_event(&mut content_cx, event)
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.open {
            return false;
        }

        if matches!(event.key, KeyCode::Escape) && self.dismiss(cx) {
            return true;
        }

        if !self.state.can_activate() {
            return self.modal
                || cx
                    .focused_id()
                    .is_some_and(|focused| self.content[0].contains_id(focused));
        }

        if self.content[0].handle_key_event(cx, event) {
            return true;
        }

        self.modal
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.open && self.state.can_activate() && self.content[0].handle_text_input_event(cx, event)
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.open
            && self.state.can_activate()
            && self.content[0].handle_text_input_command(cx, command)
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.open
            .then(|| self.content[0].text_input_snapshot(focused))
            .flatten()
    }

    fn children(&self) -> &[AnyElement] {
        if self.open { &self.content } else { &[] }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || (self.open && self.content[0].contains_id(id))
    }
}

fn overlay_stack_style(theme: Theme) -> Style {
    let mut style = Style::new();
    style.display = Display::Flex;
    style.flex_direction = FlexDirection::Column;
    style.align_items = AlignItems::FlexStart;
    style.gap = theme.control_gap() * 0.75;
    style.position = Position::Relative;
    style
}

fn dialog_root_style() -> Style {
    let mut style = Style::new();
    style.display = Display::Flex;
    style.flex_direction = FlexDirection::Column;
    style.justify_content = JustifyContent::Center;
    style.align_items = AlignItems::Center;
    style.position = Position::Absolute;
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
mod tests {
    use super::*;
    use crate::advanced_ui::tokens::ThemeDensity;
    use crate::advanced_ui::{button, container, text};
    use crate::core::event::{Modifiers, MouseButton};
    use crate::core::geometry::{Point, Size};
    use crate::elements::element::PointerEventKind;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, y),
            button: Some(MouseButton::Left),
        }
    }

    fn layout(element: &mut impl Element) -> (TaffyTree<ElementId>, NodeId) {
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(320.0, 240.0));
        let node = element.layout(&mut layout_cx);
        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(320.0),
                    height: AvailableSpace::Definite(240.0),
                },
            )
            .expect("overlay layout should compute");
        (taffy, node)
    }

    #[test]
    fn advanced_ui_popover_hides_content_when_closed() {
        let popover = Popover::new("Inspector", button("Open"), text("Details"));
        let nodes = popover
            .accessibility_nodes(&AccessibilityContext::default())
            .expect("closed popover accessibility should build");

        assert!(!popover.is_open());
        assert_eq!(popover.children().len(), 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Button);
    }

    #[test]
    fn advanced_ui_popover_exposes_content_when_open() {
        let id = ElementId::from(800);
        let popover = Popover::new("Inspector", button("Open"), text("Details"))
            .id(id)
            .open(true);
        let nodes = popover
            .accessibility_nodes(&AccessibilityContext::new(Some(id)))
            .expect("open popover accessibility should build");

        assert_eq!(popover.children().len(), 2);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Popover);
        assert_eq!(nodes[0].a11y_label(), Some("Inspector"));
        assert!(nodes[0].a11y_focused());
        assert_eq!(nodes[0].a11y_children().len(), 2);
    }

    #[test]
    fn advanced_ui_popover_theme_density_changes_layout_gap() {
        let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
        let popover = Popover::new("Inspector", button("Open"), text("Details")).theme(theme);

        assert_eq!(popover.style().gap, 9.0);
    }

    #[test]
    fn advanced_ui_popover_escape_dismisses_and_announces() {
        let dismissed = Rc::new(Cell::new(false));
        let dismissed_ref = Rc::clone(&dismissed);
        let mut popover = Popover::new("Inspector", button("Open"), text("Details"))
            .open(true)
            .on_dismiss(move || dismissed_ref.set(true));
        let (taffy, _) = layout(&mut popover);
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
            &taffy,
            &mut focused,
        );

        assert!(
            popover.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none()))
        );
        assert!(!popover.is_open());
        assert!(dismissed.get());
        assert_eq!(
            cx.take_accessibility_announcements()[0].message(),
            "Inspector dismissed"
        );
    }

    #[test]
    fn advanced_ui_dialog_exposes_modal_accessibility_tree() {
        let id = ElementId::from(801);
        let dialog = Dialog::new(
            "Confirm delete",
            container().w(180.0).h(90.0).child(text("Delete item?")),
        )
        .id(id);
        let nodes = dialog
            .accessibility_nodes(&AccessibilityContext::new(Some(id)))
            .expect("dialog accessibility should build");

        assert!(dialog.is_modal());
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Dialog);
        assert_eq!(nodes[0].a11y_label(), Some("Confirm delete"));
        assert!(nodes[0].a11y_focused());
        assert_eq!(nodes[0].a11y_children().len(), 1);
    }

    #[test]
    fn advanced_ui_dialog_escape_dismisses_when_enabled() {
        let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0));
        let (taffy, _) = layout(&mut dialog);
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
            &taffy,
            &mut focused,
        );

        assert!(
            dialog.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none()))
        );
        assert!(!dialog.is_open());
        assert!(cx.redraw_requested());
    }

    #[test]
    fn advanced_ui_dialog_read_only_does_not_dismiss() {
        let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0)).read_only(true);
        let (taffy, _) = layout(&mut dialog);
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
            &taffy,
            &mut focused,
        );

        assert!(
            dialog.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none()),)
        );
        assert!(dialog.is_open());
    }

    #[test]
    fn advanced_ui_dialog_modal_consumes_inside_pointer_events() {
        let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0));
        let (taffy, _) = layout(&mut dialog);
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
            &taffy,
            &mut focused,
        );

        assert!(dialog.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 4.0, 4.0)));
    }

    #[test]
    #[should_panic(expected = "popover accessibility label must not be empty")]
    fn advanced_ui_popover_rejects_empty_label() {
        drop(Popover::new(" ", button("Open"), text("Details")));
    }

    #[test]
    #[should_panic(expected = "dialog accessibility label must not be empty")]
    fn advanced_ui_dialog_rejects_empty_label() {
        drop(Dialog::new(" ", text("Details")));
    }
}
