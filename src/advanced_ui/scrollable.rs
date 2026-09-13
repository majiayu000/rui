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
    use crate::advanced_ui::DataList;
    use crate::advanced_ui::container;
    use crate::advanced_ui::text_field::TextField;
    use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
    use crate::core::geometry::{Bounds, Point};
    use crate::core::presenter::Presenter;
    use crate::core::text_editing::TextInputEvent;
    use crate::elements::element::PointerEventKind;
    use crate::renderer::Scene;
    use std::cell::{Cell, RefCell};
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
    fn advanced_ui_scrollable_pointer_up_outside_viewport_does_not_select_data_list() {
        let selected = Rc::new(RefCell::new(None::<String>));
        let selected_ref = Rc::clone(&selected);
        // Four 40px rows = 160px tall list inside an 80px viewport.
        let mut scrollable = Scrollable::new(
            DataList::new([
                ("a", "Alpha"),
                ("b", "Beta"),
                ("c", "Gamma"),
                ("d", "Delta"),
            ])
            .row_height(40.0)
            .w(120.0)
            .on_select(move |value| *selected_ref.borrow_mut() = Some(value.to_string())),
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

        // Press begins on the first visible row.
        assert!(
            scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Down,
                    position: Point::new(8.0, 20.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "pointer-down inside viewport should press a data list row"
        );

        // Release outside the clipped viewport but still within the list's
        // translated layout bounds must not select via index_at(cx.bounds()).
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 150.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            selected.borrow().is_none(),
            "pointer-up outside viewport must not select clipped data list rows"
        );

        // Outside down/up with no scene target must also stay inert.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 150.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 150.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            selected.borrow().is_none(),
            "outside press/release must not select clipped data list rows"
        );

        // Inside activation still works after the outside attempts.
        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 20.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 20.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert_eq!(selected.borrow().as_deref(), Some("a"));
    }

    #[test]
    fn advanced_ui_scrollable_preserves_nested_hit_origin_after_scroll() {
        let field_id = ElementId::new();
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(crate::elements::div().w(120.0).h(100.0))
                .child(
                    crate::elements::Input::new()
                        .id(field_id)
                        .value("nested")
                        .w(120.0)
                        .h(40.0),
                ),
        )
        .w(140.0)
        .h(80.0);

        let viewport = Size::new(140.0, 80.0);
        paint_for_accessibility_state(&mut scrollable, viewport);

        let mut taffy = TaffyTree::<ElementId>::new();
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

        // Scroll so the tall container origin moves above the viewport while the
        // nested input remains visibly painted around y=50.
        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(8.0, 8.0),
                delta_x: 0.0,
                delta_y: 50.0,
                modifiers: Modifiers::default(),
            },
        ));

        assert!(
            scrollable.handle_pointer_event(
                &mut event_cx,
                &PointerEvent {
                    kind: PointerEventKind::Down,
                    position: Point::new(8.0, 50.0),
                    button: Some(MouseButton::Left),
                },
            ),
            "nested input at painted scrolled position must remain hittable"
        );
        assert_eq!(
            event_cx.focused_id(),
            Some(field_id),
            "scrolled nested origin must not shift hit-testing to the viewport origin"
        );
    }

    #[test]
    fn advanced_ui_scrollable_fully_clipped_child_does_not_hit_at_origin() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(
                    crate::elements::div()
                        .w(120.0)
                        .h(40.0)
                        .on_click(move || clicked_ref.set(true)),
                )
                .child(crate::elements::div().w(120.0).h(160.0)),
        )
        .w(140.0)
        .h(80.0);

        let viewport = Size::new(140.0, 80.0);
        paint_for_accessibility_state(&mut scrollable, viewport);

        let mut taffy = TaffyTree::<ElementId>::new();
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

        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(8.0, 8.0),
                delta_x: 0.0,
                delta_y: 80.0,
                modifiers: Modifiers::default(),
            },
        ));

        // Top clickable child is fully above the viewport. An empty-intersection
        // Bounds::ZERO fallback would wrongly treat (0,0) as inside.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(0.0, 0.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(0.0, 0.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !clicked.get(),
            "fully clipped child must not activate from a Bounds::ZERO origin hit"
        );
    }

    /// Legacy/custom Element that hit-tests with `cx.bounds().contains` only.
    struct LegacyBoundsHitProbe {
        inner: crate::elements::Div,
        activated: Rc<Cell<bool>>,
    }

    impl Element for LegacyBoundsHitProbe {
        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            // Intentionally ignores contains_pointer / hit_clip.
            if !cx.bounds().contains(event.position) {
                return false;
            }
            if matches!(event.kind, PointerEventKind::Up) {
                self.activated.set(true);
            }
            true
        }
    }

    #[test]
    fn advanced_ui_scrollable_enforces_hit_clip_for_bounds_contains_elements() {
        let activated = Rc::new(Cell::new(false));
        let activated_ref = Rc::clone(&activated);
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(crate::elements::div().w(120.0).h(80.0))
                .child(LegacyBoundsHitProbe {
                    inner: crate::elements::div().w(120.0).h(80.0),
                    activated: activated_ref,
                }),
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

        // Second row starts at the viewport max edge (y=80). Edge-only contact
        // must not activate a legacy bounds().contains Element.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 80.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 80.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !activated.get(),
            "legacy bounds().contains element must not activate on clipped edge-only hit"
        );

        // Deep clipped-away point within the second row's layout bounds.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !activated.get(),
            "legacy bounds().contains element must not activate outside the viewport"
        );
    }

    /// Parent with small layout bounds that still paints an overflowing child
    /// inside the viewport (models Div→Button overflow without scene hits).
    struct OverflowHost {
        inner: crate::elements::Div,
        child: AnyElement,
        overflow_height: f32,
    }

    impl Element for OverflowHost {
        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            let layout = cx.layout_bounds();
            let overflow_bounds = Bounds::from_xywh(
                layout.x(),
                layout.y(),
                layout.width(),
                self.overflow_height,
            );
            let mut child_cx = cx.with_bounds(overflow_bounds);
            self.child.handle_pointer_event(&mut child_cx, event)
        }
    }

    #[test]
    fn advanced_ui_scrollable_forwards_pointer_to_visible_overflow_descendant() {
        let activated = Rc::new(Cell::new(false));
        let activated_ref = Rc::clone(&activated);
        let mut scrollable = Scrollable::new(OverflowHost {
            inner: crate::elements::div().w(120.0).h(40.0),
            overflow_height: 80.0,
            child: AnyElement::new(LegacyBoundsHitProbe {
                inner: crate::elements::div().w(120.0).h(80.0),
                activated: activated_ref,
            }),
        })
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

        // Click in the overflow region: outside the host's 40px layout bounds,
        // but still inside the viewport and the descendant's painted area.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 60.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 60.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            activated.get(),
            "visible overflow descendant inside hit_clip must remain clickable"
        );
    }

    #[test]
    fn advanced_ui_scrollable_forwards_through_fully_clipped_overflow_host() {
        let activated = Rc::new(Cell::new(false));
        let activated_ref = Rc::clone(&activated);

        /// Host whose layout is fully below the viewport, but forwards to a
        /// child that overflows upward back into the visible clip.
        struct UpwardOverflowHost {
            inner: crate::elements::Div,
            child: AnyElement,
            child_y_offset: f32,
            child_height: f32,
        }

        impl Element for UpwardOverflowHost {
            fn style(&self) -> &crate::core::style::Style {
                self.inner.style()
            }

            fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
                self.inner.layout(cx)
            }

            fn paint(&mut self, cx: &mut PaintContext) {
                self.inner.paint(cx);
            }

            fn handle_pointer_event(
                &mut self,
                cx: &mut EventContext,
                event: &PointerEvent,
            ) -> bool {
                let layout = cx.layout_bounds();
                let overflow_bounds = Bounds::from_xywh(
                    layout.x(),
                    layout.y() + self.child_y_offset,
                    layout.width(),
                    self.child_height,
                );
                let mut child_cx = cx.with_bounds(overflow_bounds);
                self.child.handle_pointer_event(&mut child_cx, event)
            }
        }

        // Spacer pushes the host to y=80 (fully clipped); child overflows up.
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(crate::elements::div().w(120.0).h(80.0))
                .child(UpwardOverflowHost {
                    inner: crate::elements::div().w(120.0).h(40.0),
                    child_y_offset: -40.0,
                    child_height: 80.0,
                    child: AnyElement::new(LegacyBoundsHitProbe {
                        inner: crate::elements::div().w(120.0).h(80.0),
                        activated: activated_ref,
                    }),
                }),
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

        // Click inside the viewport over the upward-overflow paint of a fully
        // clipped host (host layout starts at y=80).
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 50.0),
                button: Some(MouseButton::Left),
            },
        );
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(8.0, 50.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            activated.get(),
            "fully clipped overflow host must still forward in-clip descendant hits"
        );
    }

    /// Captured legacy probe that activates on Up via bounds().contains only.
    struct CapturedLegacyBoundsProbe {
        id: ElementId,
        inner: crate::elements::Div,
        activated: Rc<Cell<bool>>,
        pressed: Rc<Cell<bool>>,
    }

    impl Element for CapturedLegacyBoundsProbe {
        fn id(&self) -> Option<ElementId> {
            Some(self.id)
        }

        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            // Intentionally ignores contains_pointer / hit_clip.
            if !cx.bounds().contains(event.position) {
                if matches!(event.kind, PointerEventKind::Move | PointerEventKind::Up)
                    && self.pressed.get()
                {
                    self.pressed.set(false);
                    return true;
                }
                return false;
            }
            match event.kind {
                PointerEventKind::Down => {
                    self.pressed.set(true);
                    true
                }
                PointerEventKind::Up => {
                    if self.pressed.get() {
                        self.pressed.set(false);
                        self.activated.set(true);
                    }
                    true
                }
                PointerEventKind::Move => true,
            }
        }
    }

    #[test]
    fn advanced_ui_scrollable_captured_legacy_outside_release_is_not_activatable() {
        let id = ElementId::new();
        let activated = Rc::new(Cell::new(false));
        let pressed = Rc::new(Cell::new(false));
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(CapturedLegacyBoundsProbe {
                    id,
                    inner: crate::elements::div().w(120.0).h(160.0),
                    activated: Rc::clone(&activated),
                    pressed: Rc::clone(&pressed),
                }),
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
        event_cx.set_hit_target(Some(id));

        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(20.0, 40.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(pressed.get(), "press inside the visible region should stick");

        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(20.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !activated.get(),
            "captured legacy bounds().contains must not activate outside the viewport clip"
        );
        assert!(
            !pressed.get(),
            "outside Up cleanup must still clear pressed state"
        );
    }

    #[test]
    fn advanced_ui_scrollable_unregistered_button_clears_pressed_outside_viewport() {
        let pressed = Rc::new(Cell::new(false));
        let clicked = Rc::new(Cell::new(false));
        let pressed_ref = Rc::clone(&pressed);
        let clicked_ref = Rc::clone(&clicked);

        struct UnregisteredPressProbe {
            inner: crate::elements::Div,
            pressed: Rc<Cell<bool>>,
            clicked: Rc<Cell<bool>>,
        }

        impl Element for UnregisteredPressProbe {
            fn style(&self) -> &crate::core::style::Style {
                self.inner.style()
            }

            fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
                self.inner.layout(cx)
            }

            fn paint(&mut self, cx: &mut PaintContext) {
                self.inner.paint(cx);
            }

            fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
                let inside = cx.contains_pointer(event.position);
                match event.kind {
                    PointerEventKind::Down => {
                        if inside {
                            self.pressed.set(true);
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        let was_pressed = self.pressed.get();
                        self.pressed.set(false);
                        if inside && was_pressed {
                            self.clicked.set(true);
                            true
                        } else {
                            was_pressed
                        }
                    }
                    PointerEventKind::Move => {
                        if self.pressed.get() && !inside {
                            self.pressed.set(false);
                            true
                        } else {
                            false
                        }
                    }
                }
            }
        }

        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(UnregisteredPressProbe {
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: pressed_ref,
                    clicked: clicked_ref,
                }),
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
        // No hit_target / previous_hit_target: unregistered control.

        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(20.0, 40.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(pressed.get(), "unregistered press inside viewport should stick");

        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(20.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !pressed.get(),
            "outside Up must reach unregistered controls to clear pressed"
        );
        assert!(
            !clicked.get(),
            "outside Up must not fire click for unregistered controls"
        );
    }

    #[test]
    fn advanced_ui_scrollable_unregistered_button_clears_pressed_on_unrelated_hit_target_up() {
        let pressed = Rc::new(Cell::new(false));
        let clicked = Rc::new(Cell::new(false));
        let pressed_ref = Rc::clone(&pressed);
        let clicked_ref = Rc::clone(&clicked);
        let unrelated_hit = ElementId::new();

        struct UnregisteredPressProbe {
            inner: crate::elements::Div,
            pressed: Rc<Cell<bool>>,
            clicked: Rc<Cell<bool>>,
        }

        impl Element for UnregisteredPressProbe {
            fn style(&self) -> &crate::core::style::Style {
                self.inner.style()
            }

            fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
                self.inner.layout(cx)
            }

            fn paint(&mut self, cx: &mut PaintContext) {
                self.inner.paint(cx);
            }

            fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
                let inside = cx.contains_pointer(event.position);
                match event.kind {
                    PointerEventKind::Down => {
                        if inside {
                            self.pressed.set(true);
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Up => {
                        let was_pressed = self.pressed.get();
                        self.pressed.set(false);
                        if inside && was_pressed {
                            self.clicked.set(true);
                            true
                        } else {
                            was_pressed
                        }
                    }
                    PointerEventKind::Move => {
                        if self.pressed.get() && !inside {
                            self.pressed.set(false);
                            true
                        } else {
                            false
                        }
                    }
                }
            }
        }

        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(UnregisteredPressProbe {
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: pressed_ref,
                    clicked: clicked_ref,
                }),
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
        // No hit filter on Down: unregistered press.
        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(20.0, 40.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(pressed.get(), "unregistered press inside viewport should stick");

        // Up lands outside the viewport on an unrelated registered scene target.
        // Presenter supplies that hit_target with no previous_hit_target for Up.
        event_cx.set_hit_target(Some(unrelated_hit));
        event_cx.set_previous_hit_target(None);
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(20.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !pressed.get(),
            "cleanup Up must reach unregistered controls despite unrelated hit_target"
        );
        assert!(
            !clicked.get(),
            "outside Up must not fire click for unregistered controls"
        );
    }

    #[test]
    fn any_element_preserves_hit_filter_under_overlay_without_hit_clip() {
        // Registered overlay hit_target must keep filtering unrelated background
        // controls: Move/Up cleanup bypass applies only outside a hit clip, not
        // for every Move/Up under an overlay.
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let hovered = Rc::new(Cell::new(false));
        let hovered_ref = Rc::clone(&hovered);
        let overlay_hit = ElementId::new();

        let mut background = AnyElement::new(
            crate::elements::div()
                .w(200.0)
                .h(200.0)
                .on_click(move || clicked_ref.set(true))
                .on_hover(move |inside| hovered_ref.set(inside)),
        );

        let mut taffy = TaffyTree::<ElementId>::new();
        let viewport = Size::new(200.0, 200.0);
        let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
        let node = background.layout(&mut layout_cx);
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
        event_cx.set_hit_target(Some(overlay_hit));
        event_cx.set_previous_hit_target(None);

        let handled_move = background.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Move,
                position: Point::new(40.0, 20.0),
                button: None,
            },
        );
        let handled_up = background.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(40.0, 20.0),
                button: Some(MouseButton::Left),
            },
        );

        assert!(
            !handled_move && !handled_up,
            "unrelated background must not handle overlay-filtered pointer events"
        );
        assert!(
            !clicked.get(),
            "Div::on_click must not fire under a registered overlay hit_target"
        );
        assert!(
            !hovered.get(),
            "background hover must not update under a registered overlay hit_target"
        );
    }

    /// Custom element that starts a press inside the viewport and relies on
    /// `cx.layout_bounds()` for local coordinates while clearing pressed state on
    /// outside Move/Up (capture cleanup).
    struct CapturedBoundsCleanupProbe {
        id: ElementId,
        inner: crate::elements::Div,
        pressed: Rc<Cell<bool>>,
        observed_bounds: Rc<RefCell<Vec<Bounds>>>,
        local_deltas: Rc<RefCell<Vec<Point>>>,
    }

    impl Element for CapturedBoundsCleanupProbe {
        fn id(&self) -> Option<ElementId> {
            Some(self.id)
        }

        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            let bounds = cx.layout_bounds();
            self.observed_bounds.borrow_mut().push(bounds);
            self.local_deltas.borrow_mut().push(Point::new(
                event.position.x - bounds.x(),
                event.position.y - bounds.y(),
            ));
            // Containment uses the separate hit-test; layout_bounds stay layout space.
            let inside = cx.contains_pointer(event.position);
            match event.kind {
                PointerEventKind::Down => {
                    if inside {
                        self.pressed.set(true);
                        true
                    } else {
                        false
                    }
                }
                PointerEventKind::Move | PointerEventKind::Up => {
                    if self.pressed.get() {
                        if !inside {
                            self.pressed.set(false);
                        }
                        true
                    } else {
                        false
                    }
                }
            }
        }
    }

    #[test]
    fn advanced_ui_scrollable_keeps_bounds_stable_for_captured_outside_cleanup() {
        let id = ElementId::new();
        let pressed = Rc::new(Cell::new(false));
        let observed_bounds = Rc::new(RefCell::new(Vec::new()));
        let local_deltas = Rc::new(RefCell::new(Vec::new()));
        let mut scrollable = Scrollable::new(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(CapturedBoundsCleanupProbe {
                    id,
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: Rc::clone(&pressed),
                    observed_bounds: Rc::clone(&observed_bounds),
                    local_deltas: Rc::clone(&local_deltas),
                }),
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
        // Simulate presenter capture: Down begins inside, then Move/Up outside
        // the viewport are still routed to this element.
        event_cx.set_hit_target(Some(id));

        assert!(scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(20.0, 40.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(pressed.get(), "press inside the visible region should stick");

        // Move does not stop propagation, but the captured target must still
        // receive it with stable bounds for cleanup.
        let _ = scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Move,
                position: Point::new(20.0, 120.0),
                button: Some(MouseButton::Left),
            },
        );
        assert!(
            !pressed.get(),
            "outside Move must clear pressed using stable bounds"
        );

        let bounds_log = observed_bounds.borrow();
        assert!(
            bounds_log.len() >= 2,
            "Down and outside Move should both reach the probe"
        );
        for bounds in bounds_log.iter() {
            assert!(
                bounds.width() > 0.0 && bounds.height() > 0.0,
                "layout_bounds must stay positive during clipped cleanup: {bounds:?}"
            );
        }
        let down_bounds = bounds_log[0];
        let move_bounds = bounds_log[1];
        assert_eq!(
            down_bounds, move_bounds,
            "layout_bounds() must stay stable between inside Down and outside Move"
        );

        let deltas = local_deltas.borrow();
        assert_eq!(
            deltas[0],
            Point::new(20.0 - down_bounds.x(), 40.0 - down_bounds.y())
        );
        assert_eq!(
            deltas[1],
            Point::new(20.0 - move_bounds.x(), 120.0 - move_bounds.y()),
            "outside Move local coordinates must derive from real geometry"
        );
    }

    #[test]
    fn event_context_contains_pointer_rejects_edge_only_hit_clip_intersection() {
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut root = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
            &taffy,
            &mut focused,
        );
        // Child starts exactly at the viewport's max y edge: inclusive contains
        // on both rects would accept y=80, but the intersection has zero area.
        {
            let cx = root.with_bounds_and_hit_clip(
                Bounds::from_xywh(0.0, 80.0, 120.0, 80.0),
                Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
            );
            assert!(
                !cx.contains_pointer(Point::new(8.0, 80.0)),
                "edge-only bounds∩hit_clip must not count as a pointer hit"
            );
            assert!(
                !cx.contains_pointer(Point::new(8.0, 100.0)),
                "fully clipped layout area must not count as a pointer hit"
            );
        }

        let visible = root.with_bounds_and_hit_clip(
            Bounds::from_xywh(0.0, 0.0, 120.0, 160.0),
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        );
        assert!(
            visible.contains_pointer(Point::new(8.0, 40.0)),
            "positive-area visible intersection must still accept interior points"
        );
    }

    #[test]
    fn event_context_pointer_outside_hit_region_allows_overflow_inside_clip() {
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut root = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
            &taffy,
            &mut focused,
        );
        let small_ancestor = root.with_bounds_and_hit_clip(
            Bounds::from_xywh(0.0, 0.0, 120.0, 40.0),
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        );
        assert!(
            !small_ancestor.pointer_outside_hit_region(Point::new(8.0, 60.0)),
            "point outside ancestor bounds but inside hit_clip must still dispatch"
        );
        assert!(
            small_ancestor.pointer_outside_hit_region(Point::new(8.0, 100.0)),
            "point outside hit_clip must remain suppressed"
        );

        let fully_clipped = root.with_bounds_and_hit_clip(
            Bounds::from_xywh(0.0, 80.0, 120.0, 80.0),
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        );
        assert!(
            !fully_clipped.pointer_outside_hit_region(Point::new(8.0, 40.0)),
            "fully clipped ancestors must still forward points inside the viewport clip"
        );
        assert!(
            !fully_clipped.pointer_outside_hit_region(Point::new(8.0, 80.0)),
            "edge points inside the inclusive clip must still reach overflow hosts"
        );
        assert!(
            fully_clipped.pointer_outside_hit_region(Point::new(8.0, 100.0)),
            "points outside the viewport clip must remain suppressed"
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
