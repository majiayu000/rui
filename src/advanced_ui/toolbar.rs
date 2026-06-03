use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::Theme;
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::style::Style;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::{Div, div};
use taffy::prelude::NodeId;

pub struct Toolbar {
    id: ElementId,
    label: String,
    theme: Theme,
    state: InteractionState,
    inner: Div,
}

impl Toolbar {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "toolbar accessibility label must not be empty");

        let id = ElementId::new();
        let theme = Theme::default();
        Self {
            id,
            label,
            theme,
            state: InteractionState::default(),
            inner: base_toolbar_div(id, InteractionState::default(), theme),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self.inner = self.inner.id(id);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        self.inner = apply_toolbar_theme(self.inner, self.state, self.theme);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        self.inner = apply_toolbar_theme(self.inner, self.state, self.theme);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.inner = apply_toolbar_theme(self.inner, self.state, self.theme);
        self
    }

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children<I, E>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        self.inner = self.inner.children(children);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }
}

impl Element for Toolbar {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        Element::style(&self.inner)
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        Ok(Some(
            AccessibilityNode::label_required(self.id, AccessibilityRole::Toolbar, &self.label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id)),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        self.state.can_activate() && self.inner.handle_pointer_event(cx, event)
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        self.state.can_activate() && self.inner.handle_scroll_event(cx, event)
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.state.can_activate() && self.inner.handle_key_event(cx, event)
    }

    fn dispatch_action(
        &mut self,
        cx: &mut EventContext,
        action: &crate::core::action::ActionId,
    ) -> crate::core::action::ActionOutcome {
        if !self.state.can_activate() {
            return crate::core::action::ActionOutcome::Ignored;
        }
        self.inner.dispatch_action(cx, action)
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.state.can_activate() && self.inner.handle_window_event(event)
    }

    fn children(&self) -> &[AnyElement] {
        Element::children(&self.inner)
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.inner.contains_id(id)
    }
}

fn base_toolbar_div(id: ElementId, state: InteractionState, theme: Theme) -> Div {
    apply_toolbar_theme(div().id(id).flex_row().items_center(), state, theme)
}

fn apply_toolbar_theme(inner: Div, state: InteractionState, theme: Theme) -> Div {
    let token_state = state.into();
    inner
        .gap(theme.control_gap())
        .p(theme.toolbar_padding())
        .bg(theme.surface_color_for_state(token_state))
        .border(
            1.0,
            theme.state_border_color(token_state, theme.colors.border),
        )
        .rounded(theme.control_radius())
}

pub fn toolbar(label: impl Into<String>) -> Toolbar {
    Toolbar::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::button;
    use crate::core::event::MouseButton;
    use crate::core::geometry::{Bounds, Point, Size};
    use crate::elements::element::PointerEventKind;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(12.0, 12.0),
            button: Some(MouseButton::Left),
        }
    }

    fn layout(toolbar: &mut Toolbar) -> (TaffyTree<ElementId>, taffy::prelude::NodeId) {
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(220.0, 56.0));
        let node = toolbar.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(220.0),
                height: taffy::prelude::AvailableSpace::Definite(56.0),
            },
        ) {
            panic!("toolbar layout should compute: {err}");
        }
        (taffy, node)
    }

    #[test]
    fn advanced_ui_toolbar_lays_out_children_in_row() {
        let mut toolbar = Toolbar::new("Primary actions")
            .child(button("Save"))
            .child(button("Cancel"));
        let (taffy, node) = layout(&mut toolbar);
        let root = match taffy.layout(node) {
            Ok(layout) => layout,
            Err(err) => panic!("toolbar layout should be available: {err}"),
        };

        assert_eq!(Element::children(&toolbar).len(), 2);
        assert!(root.size.width >= 120.0);
        assert!(root.size.height >= 36.0);
    }

    #[test]
    fn advanced_ui_toolbar_forwards_child_pointer_events() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut toolbar = Toolbar::new("Primary actions")
            .child(button("Save").on_click(move || clicked_ref.set(true)));
        let (taffy, _) = layout(&mut toolbar);
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 220.0, 56.0),
            &taffy,
            &mut focused,
        );

        assert!(toolbar.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(toolbar.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        assert!(clicked.get());
    }

    #[test]
    fn advanced_ui_toolbar_disabled_or_read_only_blocks_child_events() {
        for mut toolbar in [
            Toolbar::new("Primary actions")
                .disabled(true)
                .child(button("Save").on_click(|| {})),
            Toolbar::new("Primary actions")
                .read_only(true)
                .child(button("Save").on_click(|| {})),
        ] {
            let (taffy, _) = layout(&mut toolbar);
            let mut focused = None;
            let mut cx = EventContext::new(
                Bounds::from_xywh(0.0, 0.0, 220.0, 56.0),
                &taffy,
                &mut focused,
            );
            assert!(!toolbar.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
            assert!(!toolbar.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        }
    }

    #[test]
    fn advanced_ui_toolbar_exposes_accessibility_tree() {
        let id = ElementId::from(900);
        let toolbar = Toolbar::new("Formatting")
            .id(id)
            .child(button("Bold"))
            .child(button("Italic"));
        let nodes = match toolbar.accessibility_nodes(&AccessibilityContext::default()) {
            Ok(nodes) => nodes,
            Err(err) => panic!("toolbar accessibility should build: {err}"),
        };

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_id(), id);
        assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Toolbar);
        assert_eq!(nodes[0].a11y_label(), Some("Formatting"));
        assert!(nodes[0].a11y_enabled());
        assert_eq!(nodes[0].a11y_children().len(), 2);
        assert_eq!(
            nodes[0].a11y_children()[0].a11y_role(),
            AccessibilityRole::Button
        );
    }

    #[test]
    #[should_panic(expected = "toolbar accessibility label must not be empty")]
    fn advanced_ui_toolbar_rejects_empty_label() {
        drop(Toolbar::new(" "));
    }
}
