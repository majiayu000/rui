use super::hit_clip::CapturedLegacyBoundsProbe;
use super::*;
use crate::core::event::MouseButton;
use crate::core::geometry::{Bounds, Point};
use crate::elements::element::PointerEventKind;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::TaffyTree;

#[test]
fn scroll_view_captured_legacy_outside_release_is_not_activatable() {
    let id = ElementId::new();
    let activated = Rc::new(Cell::new(false));
    let pressed = Rc::new(Cell::new(false));
    let mut scrollable = scroll_view()
        .child(crate::elements::div().flex_col().w(120.0).h(160.0).child(
            CapturedLegacyBoundsProbe {
                id,
                inner: crate::elements::div().w(120.0).h(160.0),
                activated: Rc::clone(&activated),
                pressed: Rc::clone(&pressed),
            },
        ))
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
    assert!(
        pressed.get(),
        "press inside the visible region should stick"
    );

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
fn scroll_view_unregistered_button_clears_pressed_outside_viewport() {
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

    let mut scrollable =
        scroll_view()
            .child(crate::elements::div().flex_col().w(120.0).h(160.0).child(
                UnregisteredPressProbe {
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: pressed_ref,
                    clicked: clicked_ref,
                },
            ))
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
    assert!(
        pressed.get(),
        "unregistered press inside viewport should stick"
    );

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
fn scroll_view_unregistered_button_clears_pressed_on_unrelated_hit_target_up() {
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

    let mut scrollable =
        scroll_view()
            .child(crate::elements::div().flex_col().w(120.0).h(160.0).child(
                UnregisteredPressProbe {
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: pressed_ref,
                    clicked: clicked_ref,
                },
            ))
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
    assert!(
        pressed.get(),
        "unregistered press inside viewport should stick"
    );

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
fn scroll_view_unregistered_button_clears_pressed_on_in_viewport_filtered_up() {
    // Press an unregistered control, then release still inside the viewport
    // over an unrelated registered scene hit target (no previous_hit_target).
    // Cleanup must reach the pressed control with suppressed containment so
    // pressed clears without a click.
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

    let mut scrollable =
        scroll_view()
            .child(crate::elements::div().flex_col().w(120.0).h(160.0).child(
                UnregisteredPressProbe {
                    inner: crate::elements::div().w(120.0).h(160.0),
                    pressed: pressed_ref,
                    clicked: clicked_ref,
                },
            ))
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
    assert!(scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(20.0, 40.0),
            button: Some(MouseButton::Left),
        },
    ));
    assert!(
        pressed.get(),
        "unregistered press inside viewport should stick"
    );

    // Up stays inside the viewport but Presenter filters to an unrelated
    // registered sibling / overlay with no previous_hit_target.
    event_cx.set_hit_target(Some(unrelated_hit));
    event_cx.set_previous_hit_target(None);
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(100.0, 40.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !pressed.get(),
        "in-viewport filtered Up must clear unregistered pressed state"
    );
    assert!(
        !clicked.get(),
        "in-viewport filtered Up must not fire click for unregistered controls"
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

#[test]
fn any_element_focus_blur_under_overlay_does_not_activate() {
    // Focused background Input behind a registered overlay must receive
    // Down for blur, but with suppressed hit containment so caret/activation
    // cannot run through the overlay.
    let field_id = ElementId::new();
    let blur_count = Rc::new(Cell::new(0));
    let blur_count_ref = Rc::clone(&blur_count);
    let overlay_hit = ElementId::new();

    let mut field = AnyElement::new(
        crate::elements::Input::new()
            .id(field_id)
            .value("focused")
            .w(200.0)
            .h(40.0)
            .on_blur(move || blur_count_ref.set(blur_count_ref.get() + 1)),
    );

    let mut taffy = TaffyTree::<ElementId>::new();
    let viewport = Size::new(200.0, 200.0);
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = field.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = Some(field_id);
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Focus the input via a normal Down first so its internal focused flag
    // matches the EventContext focus id.
    event_cx.set_hit_target(None);
    event_cx.set_previous_hit_target(None);
    assert!(field.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(20.0, 20.0),
            button: Some(MouseButton::Left),
        },
    ));
    assert_eq!(event_cx.focused_id(), Some(field_id));

    // Overlay owns the scene hit; Down must blur without re-activating.
    event_cx.set_hit_target(Some(overlay_hit));
    event_cx.set_previous_hit_target(None);
    let handled = field.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(20.0, 20.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !handled,
        "overlay Down delivered for blur must not treat the hit as inside"
    );
    assert_eq!(
        event_cx.focused_id(),
        None,
        "focused Input must blur when Down is filtered to an overlay"
    );
    assert_eq!(
        blur_count.get(),
        1,
        "focused Input must invoke on_blur under overlay hit filter"
    );

    // Move/Up under overlay must still be filtered (no activation path).
    assert!(
        !field.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Move,
                position: Point::new(20.0, 20.0),
                button: None,
            },
        ),
        "Move must not bypass overlay hit filter merely because Input was focused"
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
fn scroll_view_keeps_bounds_stable_for_captured_outside_cleanup() {
    let id = ElementId::new();
    let pressed = Rc::new(Cell::new(false));
    let observed_bounds = Rc::new(RefCell::new(Vec::new()));
    let local_deltas = Rc::new(RefCell::new(Vec::new()));
    let mut scrollable = scroll_view()
        .child(crate::elements::div().flex_col().w(120.0).h(160.0).child(
            CapturedBoundsCleanupProbe {
                id,
                inner: crate::elements::div().w(120.0).h(160.0),
                pressed: Rc::clone(&pressed),
                observed_bounds: Rc::clone(&observed_bounds),
                local_deltas: Rc::clone(&local_deltas),
            },
        ))
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
    assert!(
        pressed.get(),
        "press inside the visible region should stick"
    );

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
