use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole, AccessibilityScrollPosition,
};
use crate::core::color::Color;
use crate::core::geometry::Size;
use crate::core::style::Style;
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::{ScrollDirection, ScrollView};
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::NodeId;

pub struct Scrollable {
    id: ElementId,
    inner: ScrollView,
    accessibility_label: Option<String>,
    state: InteractionState,
}

impl Scrollable {
    pub fn new(child: impl Into<AnyElement>) -> Self {
        let id = ElementId::new();
        Self {
            id,
            inner: ScrollView::new().id(id).child(child),
            accessibility_label: None,
            state: InteractionState::default(),
        }
    }

    pub fn empty() -> Self {
        let id = ElementId::new();
        Self {
            id,
            inner: ScrollView::new().id(id),
            accessibility_label: None,
            state: InteractionState::default(),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self.inner = self.inner.id(id);
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "scrollable accessibility label must not be empty");
        self.accessibility_label = Some(label);
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

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.inner = self.inner.direction(direction);
        self
    }

    pub fn vertical(self) -> Self {
        self.direction(ScrollDirection::Vertical)
    }

    pub fn horizontal(self) -> Self {
        self.direction(ScrollDirection::Horizontal)
    }

    pub fn both(self) -> Self {
        self.direction(ScrollDirection::Both)
    }

    pub fn w(mut self, width: f32) -> Self {
        self.inner = self.inner.w(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.inner = self.inner.bg(color);
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

    pub fn scrollbar_always(mut self) -> Self {
        self.inner = self.inner.scrollbar_always();
        self
    }

    pub fn scrollbar_never(mut self) -> Self {
        self.inner = self.inner.scrollbar_never();
        self
    }

    pub fn on_scroll(mut self, handler: impl Fn(f32, f32) + 'static) -> Self {
        self.inner = self.inner.on_scroll(handler);
        self
    }

    pub fn into_scroll_view(self) -> ScrollView {
        self.inner
    }
}

impl Element for Scrollable {
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
        cx.register_accessibility_region(self.id, cx.bounds());
        self.inner.paint(cx);
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        self.inner.refresh_text_geometry(text_measurer);
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node = AccessibilityNode::new(self.id, AccessibilityRole::ScrollArea)
            .with_enabled(!self.state.disabled())
            .with_read_only(self.state.read_only())
            .with_invalid(self.state.invalid())
            .with_focused(cx.a11y_has_focus(self.id));
        if self.state.can_activate() {
            let mut actions = Vec::new();
            if self.inner.can_scroll_forward() {
                actions.push(AccessibilityAction::ScrollForward);
            }
            if self.inner.can_scroll_backward() {
                actions.push(AccessibilityAction::ScrollBackward);
            }
            node = node.with_actions(actions);
        }
        if let Some(metrics) = self.inner.scroll_metrics() {
            node = node.with_scroll_position(AccessibilityScrollPosition::new(
                metrics.offset_x,
                metrics.offset_y,
                metrics.max_x,
                metrics.max_y,
            ));
        }
        if let Some(label) = &self.accessibility_label {
            node = node.with_label(label.clone());
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        // Always forward so nested editors can focus even when scrolling is frozen.
        self.inner.handle_pointer_event(cx, event)
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        if !self.state.can_activate() {
            return false;
        }
        self.inner.handle_scroll_event(cx, event)
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.inner.handle_key_event(cx, event)
    }

    fn dispatch_action(
        &mut self,
        cx: &mut EventContext,
        action: &crate::core::action::ActionId,
    ) -> crate::core::action::ActionOutcome {
        if cx.focused_id() == Some(self.id) {
            let forward = match action {
                crate::core::action::ActionId::Custom(name)
                    if name == crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION =>
                {
                    Some(true)
                }
                crate::core::action::ActionId::Custom(name)
                    if name == crate::core::action::ACCESSIBILITY_SCROLL_BACKWARD_ACTION =>
                {
                    Some(false)
                }
                _ => None,
            };
            if let Some(forward) = forward {
                if !self.state.can_activate() {
                    return crate::core::action::ActionOutcome::Ignored;
                }
                if self.inner.scroll_accessibility(forward) {
                    cx.request_redraw();
                    return crate::core::action::ActionOutcome::handled(
                        "advanced_ui.scrollable accessibility",
                    );
                }
                return crate::core::action::ActionOutcome::Ignored;
            }
        }
        // Non-scroll actions always reach nested children.
        self.inner.dispatch_action(cx, action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.inner.handle_text_input_event(cx, event)
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.inner.handle_text_input_command(cx, command)
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.inner.text_input_snapshot(focused)
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.inner.handle_window_event(event)
    }

    fn children(&self) -> &[AnyElement] {
        Element::children(&self.inner)
    }
}

pub fn scrollable(child: impl Into<AnyElement>) -> Scrollable {
    Scrollable::new(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::container;
    use crate::advanced_ui::text_field::TextField;
    use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
    use crate::core::geometry::{Bounds, Point};
    use crate::core::presenter::Presenter;
    use crate::core::text_editing::TextInputEvent;
    use crate::elements::element::PointerEventKind;
    use crate::renderer::Scene;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn paint_for_accessibility_state(scrollable: &mut Scrollable, size: Size) {
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, size);
        let node = scrollable.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(size.width),
                height: taffy::prelude::AvailableSpace::Definite(size.height),
            },
        ) {
            panic!("layout should compute: {}", err);
        }

        let mut scene = Scene::new();
        let mut paint_cx = PaintContext::new(
            &mut scene,
            Bounds::from_xywh(0.0, 0.0, size.width, size.height),
            &taffy,
        );
        scrollable.paint(&mut paint_cx);
    }

    fn accessibility_node(scrollable: &Scrollable) -> AccessibilityNode {
        match scrollable.accessibility(&AccessibilityContext::default()) {
            Ok(Some(node)) => node,
            Ok(None) => panic!("scrollable should expose an accessibility node"),
            Err(err) => panic!("accessibility failed: {}", err),
        }
    }

    #[test]
    fn advanced_ui_scrollable_forwards_scroll_events() {
        let did_scroll = Rc::new(Cell::new(false));
        let did_scroll_ref = Rc::clone(&did_scroll);
        let mut scrollable = Scrollable::new(container().w(100.0).h(300.0))
            .h(100.0)
            .on_scroll(move |_, _| did_scroll_ref.set(true));
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(100.0, 100.0));
        let node = scrollable.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(100.0),
                height: taffy::prelude::AvailableSpace::Definite(100.0),
            },
        ) {
            panic!("layout should compute: {}", err);
        }

        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            &taffy,
            &mut focused,
        );
        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(4.0, 4.0),
                delta_x: 0.0,
                delta_y: 24.0,
                modifiers: Modifiers::default(),
            },
        ));
        assert!(did_scroll.get());
    }

    #[test]
    fn advanced_ui_scrollable_disabled_or_read_only_does_not_dispatch_scroll_callback() {
        for mut scrollable in [
            Scrollable::new(container().w(100.0).h(300.0))
                .h(100.0)
                .disabled(true),
            Scrollable::new(container().w(100.0).h(300.0))
                .h(100.0)
                .read_only(true),
        ] {
            let did_scroll = Rc::new(Cell::new(false));
            let did_scroll_ref = Rc::clone(&did_scroll);
            scrollable = scrollable.on_scroll(move |_, _| did_scroll_ref.set(true));
            let mut taffy = TaffyTree::<ElementId>::new();
            let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(100.0, 100.0));
            let node = scrollable.layout(&mut layout_cx);
            if let Err(err) = taffy.compute_layout(
                node,
                taffy::Size {
                    width: taffy::prelude::AvailableSpace::Definite(100.0),
                    height: taffy::prelude::AvailableSpace::Definite(100.0),
                },
            ) {
                panic!("layout should compute: {}", err);
            }

            let mut focused = None;
            let mut event_cx = EventContext::new(
                Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
                &taffy,
                &mut focused,
            );
            assert!(!scrollable.handle_scroll_event(
                &mut event_cx,
                &ScrollEvent {
                    position: Point::new(4.0, 4.0),
                    delta_x: 0.0,
                    delta_y: 24.0,
                    modifiers: Modifiers::default(),
                },
            ));
            assert!(!did_scroll.get());
            paint_for_accessibility_state(&mut scrollable, Size::new(100.0, 100.0));
            assert!(
                accessibility_node(&scrollable).a11y_actions().is_empty(),
                "frozen scrollable must not expose scroll accessibility actions"
            );
        }
    }

    #[test]
    fn advanced_ui_scrollable_forwards_nested_editor_when_scroll_frozen() {
        let field_id_disabled = ElementId::new();
        let field_id_read_only = ElementId::new();
        for (label, mut scrollable, field_id) in [
            (
                "disabled",
                Scrollable::new(TextField::new("Name").id(field_id_disabled).value("").w(120.0))
                    .w(140.0)
                    .h(80.0)
                    .disabled(true),
                field_id_disabled,
            ),
            (
                "read_only",
                Scrollable::new(
                    TextField::new("Name")
                        .id(field_id_read_only)
                        .value("")
                        .w(120.0),
                )
                .w(140.0)
                .h(80.0)
                .read_only(true),
                field_id_read_only,
            ),
        ] {
            let mut taffy = TaffyTree::<ElementId>::new();
            let viewport = Size::new(140.0, 80.0);
            let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
            let node = scrollable.layout(&mut layout_cx);
            if let Err(err) = taffy.compute_layout(
                node,
                taffy::Size {
                    width: taffy::prelude::AvailableSpace::Definite(viewport.width),
                    height: taffy::prelude::AvailableSpace::Definite(viewport.height),
                },
            ) {
                panic!("{label}: layout should compute: {err}");
            }

            let mut focused = None;
            let mut event_cx = EventContext::new(
                Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
                &taffy,
                &mut focused,
            );

            assert!(
                scrollable.handle_pointer_event(
                    &mut event_cx,
                    &PointerEvent {
                        kind: PointerEventKind::Down,
                        position: Point::new(8.0, 8.0),
                        button: Some(MouseButton::Left),
                    },
                ),
                "{label}: nested TextField should receive pointer focus"
            );
            assert_eq!(
                event_cx.focused_id(),
                Some(field_id),
                "{label}: pointer should focus nested editor"
            );

            assert!(
                scrollable.handle_text_input_event(
                    &mut event_cx,
                    &TextInputEvent::InsertText("xy".into()),
                ),
                "{label}: nested TextField should receive text input"
            );
            assert!(
                scrollable.handle_key_event(
                    &mut event_cx,
                    &KeyEvent::new(KeyCode::ArrowLeft, Modifiers::none()),
                ),
                "{label}: nested TextField should receive key events"
            );
            assert!(
                scrollable.handle_text_input_command(
                    &mut event_cx,
                    &TextInputCommand::InsertText("z".into()),
                ),
                "{label}: nested TextField should receive text input commands"
            );

            let snapshot = scrollable
                .text_input_snapshot(field_id)
                .unwrap_or_else(|| panic!("{label}: text_input_snapshot should still forward"));
            assert_eq!(snapshot.text(), "xzy");

            assert!(
                !scrollable.handle_scroll_event(
                    &mut event_cx,
                    &ScrollEvent {
                        position: Point::new(8.0, 8.0),
                        delta_x: 0.0,
                        delta_y: 24.0,
                        modifiers: Modifiers::default(),
                    },
                ),
                "{label}: scroll events must stay blocked"
            );
        }
    }

    #[test]
    fn advanced_ui_scrollable_ignores_pointer_down_outside_viewport_when_frozen() {
        let field_id = ElementId::new();
        let blur_count = Rc::new(Cell::new(0));
        let blur_count_ref = Rc::clone(&blur_count);
        let mut scrollable = Scrollable::new(
            crate::elements::Input::new()
                .id(field_id)
                .value("tall")
                .w(120.0)
                .h(240.0)
                .on_blur(move || blur_count_ref.set(blur_count_ref.get() + 1)),
        )
        .w(140.0)
        .h(80.0)
        .disabled(true);

        let mut taffy = TaffyTree::<ElementId>::new();
        let viewport = Size::new(140.0, 80.0);
        let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
        let node = scrollable.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(viewport.width),
                height: taffy::prelude::AvailableSpace::Definite(viewport.height),
            },
        ) {
            panic!("layout should compute: {err}");
        }

        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
            &taffy,
            &mut focused,
        );

        // Outside the clipped viewport, but still within the oversized editor's
        // translated layout bounds (y=150 > viewport height 80).
        assert!(
            !scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Down,
                    position: Point::new(8.0, 150.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "pointer-down outside viewport must not focus clipped-away nested editor"
        );
        assert_eq!(
            event_cx.focused_id(),
            None,
            "oversized nested editor must not steal focus from outside viewport"
        );

        // Moves outside the viewport are still delivered so hover can clear.
        assert!(
            !scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Move,
                    position: Point::new(8.0, 150.0),
                    button: None,
                },
            ),
            "move outside viewport should still be forwarded (and return false if unhandled)"
        );

        assert!(
            scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Down,
                    position: Point::new(8.0, 8.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "pointer-down inside viewport should still focus nested editor when frozen"
        );
        assert_eq!(event_cx.focused_id(), Some(field_id));

        // Outside Down must still reach the focused editor so it can blur,
        // even though activation stays clipped to the viewport.
        assert!(
            !scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Down,
                    position: Point::new(8.0, 150.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "outside pointer-down should not re-activate after focus"
        );
        assert_eq!(
            event_cx.focused_id(),
            None,
            "outside pointer-down must clear nested editor focus for blur delivery"
        );
        assert_eq!(
            blur_count.get(),
            1,
            "outside pointer-down must invoke on_blur when nested editor was focused"
        );
    }

    #[test]
    fn advanced_ui_scrollable_pointer_up_outside_viewport_does_not_activate_child() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .w(120.0)
                .h(240.0)
                .on_click(move || clicked_ref.set(true)),
        )
        .w(140.0)
        .h(80.0)
        .disabled(true);

        let mut taffy = TaffyTree::<ElementId>::new();
        let viewport = Size::new(140.0, 80.0);
        let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
        let node = scrollable.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(viewport.width),
                height: taffy::prelude::AvailableSpace::Definite(viewport.height),
            },
        ) {
            panic!("layout should compute: {err}");
        }

        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
            &taffy,
            &mut focused,
        );

        // Press begins inside the visible viewport.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 8.0),
                button: Some(MouseButton::Left),
            },
        );

        // Release outside the clipped viewport but still within the child's
        // translated layout bounds must not fire on_click.
        assert!(
            !scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Up,
                    position: Point::new(8.0, 150.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "pointer-up outside viewport must not activate oversized child"
        );
        assert!(
            !clicked.get(),
            "outside pointer-up must not invoke on_click for clipped-away child area"
        );
    }

    #[test]
    fn advanced_ui_scrollable_accessibility_actions_follow_scroll_range() {
        let mut overflowing = Scrollable::new(container().w(100.0).h(300.0)).h(100.0);
        let before_paint = match overflowing.accessibility(&AccessibilityContext::default()) {
            Ok(Some(node)) => node,
            Ok(None) => panic!("scrollable should expose an accessibility node"),
            Err(err) => panic!("accessibility failed: {}", err),
        };
        assert!(before_paint.a11y_actions().is_empty());

        paint_for_accessibility_state(&mut overflowing, Size::new(100.0, 100.0));
        let after_paint = match overflowing.accessibility(&AccessibilityContext::default()) {
            Ok(Some(node)) => node,
            Ok(None) => panic!("scrollable should expose an accessibility node"),
            Err(err) => panic!("accessibility failed: {}", err),
        };
        assert_eq!(
            after_paint.a11y_actions(),
            [AccessibilityAction::ScrollForward]
        );

        let mut empty = Scrollable::empty().h(100.0);
        paint_for_accessibility_state(&mut empty, Size::new(100.0, 100.0));
        let empty_node = match empty.accessibility(&AccessibilityContext::default()) {
            Ok(Some(node)) => node,
            Ok(None) => panic!("scrollable should expose an accessibility node"),
            Err(err) => panic!("accessibility failed: {}", err),
        };
        assert!(empty_node.a11y_actions().is_empty());
    }

    #[test]
    fn advanced_ui_scrollable_accessibility_scroll_position_tracks_scroll_state() {
        let mut scrollable = Scrollable::new(container().w(100.0).h(300.0)).h(100.0);
        assert_eq!(accessibility_node(&scrollable).a11y_scroll_position(), None);

        paint_for_accessibility_state(&mut scrollable, Size::new(100.0, 100.0));
        let before_scroll = accessibility_node(&scrollable);
        assert_eq!(
            before_scroll.a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(0.0, 0.0, 0.0, 200.0))
        );
        assert_eq!(
            before_scroll.a11y_actions(),
            [AccessibilityAction::ScrollForward]
        );

        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            &taffy,
            &mut focused,
        );
        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(4.0, 4.0),
                delta_x: 0.0,
                delta_y: 24.0,
                modifiers: Modifiers::default(),
            },
        ));

        let after_scroll = accessibility_node(&scrollable);
        assert_eq!(
            after_scroll.a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
        );
        assert_eq!(
            after_scroll.a11y_actions(),
            [
                AccessibilityAction::ScrollForward,
                AccessibilityAction::ScrollBackward
            ]
        );

        paint_for_accessibility_state(&mut scrollable, Size::new(100.0, 100.0));
        assert_eq!(
            accessibility_node(&scrollable).a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
        );
    }

    #[test]
    fn accessibility_scroll_action_uses_the_horizontal_axis() {
        let id = ElementId::new();
        let scrollable = Scrollable::new(crate::elements::div().w(300.0).h(40.0).flex_shrink(0.0))
            .id(id)
            .horizontal()
            .w(100.0)
            .h(40.0)
            .accessibility_label("Columns");
        let mut presenter = Presenter::with_root(Size::new(100.0, 40.0), scrollable);
        presenter
            .layout(Size::new(100.0, 40.0))
            .expect("horizontal scrollable should lay out");
        presenter.paint();
        presenter.set_focused_element(Some(id));

        let (outcome, redraw) = presenter.with_event_context(|root, cx| {
            root.dispatch_action(
                cx,
                &crate::core::action::ActionId::custom(
                    crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION,
                ),
            )
        });

        assert!(outcome.is_handled());
        assert!(redraw);
        let tree = presenter
            .accessibility_tree()
            .expect("horizontal accessibility tree should build");
        assert_eq!(
            tree.roots()[0].a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(40.0, 0.0, 200.0, 0.0))
        );
    }

    #[test]
    fn accessibility_scroll_action_routes_to_the_nested_target() {
        let outer_id = ElementId::new();
        let inner_id = ElementId::new();
        let inner = Scrollable::new(container().w(100.0).h(300.0))
            .id(inner_id)
            .vertical()
            .w(100.0)
            .h(150.0)
            .accessibility_label("Inner");
        let outer = Scrollable::new(inner)
            .id(outer_id)
            .vertical()
            .w(100.0)
            .h(100.0)
            .accessibility_label("Outer");
        let mut presenter = Presenter::with_root(Size::new(100.0, 100.0), outer);
        presenter
            .layout(Size::new(100.0, 100.0))
            .expect("nested scrollables should lay out");
        presenter.paint();
        presenter.set_focused_element(Some(inner_id));

        let (outcome, redraw) = presenter.with_event_context(|root, cx| {
            root.dispatch_action(
                cx,
                &crate::core::action::ActionId::custom(
                    crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION,
                ),
            )
        });

        assert!(outcome.is_handled());
        assert!(redraw);
        let tree = presenter
            .accessibility_tree()
            .expect("nested accessibility tree should build");
        let outer = &tree.roots()[0];
        let inner = &outer.a11y_children()[0];
        assert_eq!(
            outer.a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(0.0, 0.0, 0.0, 50.0))
        );
        assert_eq!(
            inner.a11y_scroll_position(),
            Some(AccessibilityScrollPosition::new(0.0, 40.0, 0.0, 150.0))
        );
    }

    #[test]
    #[should_panic(expected = "scrollable accessibility label must not be empty")]
    fn advanced_ui_scrollable_rejects_empty_accessibility_label() {
        drop(Scrollable::empty().accessibility_label(" "));
    }
}
