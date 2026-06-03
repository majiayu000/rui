use rui::ElementId;
use rui::advanced_ui::{button, container, dialog, popover, scrollable, text};
use rui::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use rui::core::geometry::{Bounds, Point, Size};
use rui::elements::Element;
use rui::elements::element::{
    EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use rui::renderer::Scene;
use std::cell::Cell;
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, TaffyTree};

fn layout_root(root: &mut impl Element, viewport: Size) -> TaffyTree<ElementId> {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = root.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .expect("overlay layout should compute");
    taffy
}

fn overlay_event_context<'a>(
    taffy: &'a TaffyTree<ElementId>,
    focused: &'a mut Option<ElementId>,
    bounds: Bounds,
) -> EventContext<'a> {
    EventContext::new(bounds, taffy, focused)
}

#[test]
fn popover_and_dialog_forward_scroll_to_open_content() {
    let popover_scrolled = Rc::new(Cell::new(false));
    let popover_scrolled_ref = Rc::clone(&popover_scrolled);
    let mut popover = popover(
        "Details",
        button("Open"),
        scrollable(container().w(100.0).h(240.0))
            .h(60.0)
            .on_scroll(move |_, _| popover_scrolled_ref.set(true)),
    )
    .open(true);
    let viewport = Size::new(220.0, 160.0);
    let taffy = layout_root(&mut popover, viewport);
    let mut focused = None;
    let mut cx = overlay_event_context(
        &taffy,
        &mut focused,
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
    );
    assert!(popover.handle_scroll_event(
        &mut cx,
        &ScrollEvent {
            position: Point::new(10.0, 50.0),
            delta_x: 0.0,
            delta_y: 20.0,
            modifiers: Modifiers::none(),
        },
    ));
    assert!(popover_scrolled.get());

    let dialog_scrolled = Rc::new(Cell::new(false));
    let dialog_scrolled_ref = Rc::clone(&dialog_scrolled);
    let mut dialog = dialog(
        "Details",
        scrollable(container().w(100.0).h(240.0))
            .h(60.0)
            .on_scroll(move |_, _| dialog_scrolled_ref.set(true)),
    );
    let taffy = layout_root(&mut dialog, viewport);
    let mut focused = None;
    let mut cx = overlay_event_context(
        &taffy,
        &mut focused,
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
    );
    assert!(dialog.handle_scroll_event(
        &mut cx,
        &ScrollEvent {
            position: Point::new(110.0, 80.0),
            delta_x: 0.0,
            delta_y: 20.0,
            modifiers: Modifiers::none(),
        },
    ));
    assert!(dialog_scrolled.get());
}

#[test]
fn dialog_hit_regions_follow_modal_mode() {
    let id = ElementId::new();
    let mut dialog = dialog("Palette", container().w(80.0).h(40.0))
        .id(id)
        .modal(false);
    let viewport = Size::new(220.0, 160.0);
    let taffy = layout_root(&mut dialog, viewport);
    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(
        &mut scene,
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
    );
    dialog.paint(&mut paint_cx);

    assert_eq!(scene.hit_test(Point::new(110.0, 80.0)), Some(id));
    assert_eq!(scene.hit_test(Point::new(5.0, 5.0)), None);
}

#[test]
fn disabled_overlays_consume_without_activating_children() {
    let mut dialog = dialog("Locked", container().w(80.0).h(40.0)).read_only(true);
    let viewport = Size::new(220.0, 160.0);
    let taffy = layout_root(&mut dialog, viewport);
    let mut focused = None;
    let mut cx = overlay_event_context(
        &taffy,
        &mut focused,
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
    );

    assert!(dialog.handle_pointer_event(
        &mut cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(110.0, 80.0),
            button: Some(MouseButton::Left),
        },
    ));
    assert!(dialog.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowDown, Modifiers::none()),
    ));
    assert!(dialog.is_open());

    let content_id = ElementId::new();
    let mut popover = popover(
        "Locked",
        button("Open"),
        container().id(content_id).child(text("Details")),
    )
    .open(true)
    .read_only(true);
    let taffy = layout_root(&mut popover, viewport);
    let mut focused = Some(content_id);
    let mut cx = overlay_event_context(
        &taffy,
        &mut focused,
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
    );
    assert!(popover.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowDown, Modifiers::none()),
    ));
}
