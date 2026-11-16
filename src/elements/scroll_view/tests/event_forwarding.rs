use super::accessibility::{accessibility_node, paint_for_accessibility_state};
use super::*;
use crate::advanced_ui::DataList;
use crate::advanced_ui::container;
use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point};
use crate::core::text_editing::TextInputEvent;
use crate::elements::Input;
use crate::elements::element::PointerEventKind;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::TaffyTree;

#[test]
fn scroll_view_forwards_scroll_events() {
    let did_scroll = Rc::new(Cell::new(false));
    let did_scroll_ref = Rc::clone(&did_scroll);
    let mut scrollable = scroll_view()
        .child(container().w(100.0).h(300.0))
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
fn scroll_view_disabled_or_read_only_does_not_dispatch_scroll_callback() {
    for mut scrollable in [
        scroll_view()
            .child(container().w(100.0).h(300.0))
            .h(100.0)
            .disabled(true),
        scroll_view()
            .child(container().w(100.0).h(300.0))
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
fn scroll_view_forwards_nested_editor_when_scroll_frozen() {
    let field_id_disabled = ElementId::new();
    let field_id_read_only = ElementId::new();
    for (label, mut scrollable, field_id) in [
        (
            "disabled",
            scroll_view()
                .child(
                    Input::new()
                        .accessibility_label("Name")
                        .id(field_id_disabled)
                        .value("")
                        .w(120.0),
                )
                .w(140.0)
                .h(80.0)
                .disabled(true),
            field_id_disabled,
        ),
        (
            "read_only",
            scroll_view()
                .child(
                    Input::new()
                        .accessibility_label("Name")
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
            scrollable
                .handle_text_input_event(&mut event_cx, &TextInputEvent::InsertText("xy".into()),),
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
fn scroll_view_ignores_pointer_down_outside_viewport_when_frozen() {
    let field_id = ElementId::new();
    let blur_count = Rc::new(Cell::new(0));
    let blur_count_ref = Rc::clone(&blur_count);
    let mut scrollable = scroll_view()
        .child(
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
fn scroll_view_pointer_up_outside_viewport_does_not_activate_child() {
    let clicked = Rc::new(Cell::new(false));
    let clicked_ref = Rc::clone(&clicked);
    let mut scrollable = scroll_view()
        .child(
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
fn scroll_view_pointer_up_outside_viewport_does_not_select_data_list() {
    let selected = Rc::new(RefCell::new(None::<String>));
    let selected_ref = Rc::clone(&selected);
    // Four 40px rows = 160px tall list inside an 80px viewport.
    let mut scrollable = scroll_view()
        .child(
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
