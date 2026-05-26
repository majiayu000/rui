use rui::advanced_ui::{container, hoverable};
use rui::core::event::{Cursor, MouseButton};
use rui::elements::element::{
    Element, EventContext, EventResult, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use rui::renderer::Scene;
use rui::{div, Bounds, Color, Div, ElementId, Point, Size, Style};
use std::cell::RefCell;
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, NodeId, TaffyTree};

type Log = Rc<RefCell<Vec<&'static str>>>;

struct Probe {
    id: ElementId,
    label: &'static str,
    log: Log,
    inner: Div,
    handled: bool,
    register_hit_region: bool,
}

impl Probe {
    fn new(
        id: ElementId,
        label: &'static str,
        log: Log,
        handled: bool,
        register_hit_region: bool,
    ) -> Self {
        Self {
            id,
            label,
            log,
            inner: div().w(20.0).h(20.0).absolute(),
            handled,
            register_hit_region,
        }
    }
}

impl Element for Probe {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if self.register_hit_region {
            cx.register_hit_region(self.id, cx.bounds());
        }
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, _cx: &mut EventContext, _event: &PointerEvent) -> bool {
        self.log.borrow_mut().push(self.label);
        self.handled
    }
}

fn layout_and_paint(root: &mut impl Element) -> (TaffyTree<ElementId>, Bounds, Scene) {
    let viewport = Size::new(40.0, 40.0);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let root_node = root.layout(&mut layout_cx);

    taffy
        .compute_layout(
            root_node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .expect("layout should compute");

    let layout = taffy.layout(root_node).expect("root layout should exist");
    let root_bounds = Bounds::from_xywh(
        layout.location.x,
        layout.location.y,
        layout.size.width,
        layout.size.height,
    );

    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(&mut scene, root_bounds, &taffy);
    root.paint(&mut paint_cx);
    (taffy, root_bounds, scene)
}

fn pointer_up(position: Point) -> PointerEvent {
    PointerEvent {
        kind: PointerEventKind::Up,
        position,
        button: Some(MouseButton::Left),
    }
}

fn pointer_move(position: Point) -> PointerEvent {
    PointerEvent {
        kind: PointerEventKind::Move,
        position,
        button: None,
    }
}

#[test]
fn event_result_maps_existing_handled_boolean_semantics() {
    assert_eq!(EventResult::from_handled(false), EventResult::Propagate);
    assert_eq!(EventResult::from_handled(true), EventResult::Stop);
    assert!(!EventResult::Propagate.is_stopped());
    assert!(EventResult::Stop.is_stopped());
}

#[test]
fn targeted_dispatch_sends_pointer_to_topmost_hit_region_first() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let bottom_id = ElementId::from(1);
    let top_id = ElementId::from(2);
    let mut root = div()
        .w(40.0)
        .h(40.0)
        .child(Probe::new(bottom_id, "bottom", Rc::clone(&log), true, true))
        .child(Probe::new(top_id, "top", Rc::clone(&log), true, true));

    let (taffy, root_bounds, scene) = layout_and_paint(&mut root);
    let position = Point::new(5.0, 5.0);
    let mut focused = None;
    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(position));

    let result = root.dispatch_pointer_event(&mut cx, &pointer_up(position));

    assert_eq!(result, EventResult::Stop);
    assert_eq!(scene.hit_test(position), Some(top_id));
    assert_eq!(&*log.borrow(), &["top"]);
}

#[test]
fn propagation_continues_to_parent_when_target_does_not_stop() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let parent_log = Rc::clone(&log);
    let child_id = ElementId::from(2);
    let mut root = div()
        .w(40.0)
        .h(40.0)
        .on_click(move || parent_log.borrow_mut().push("parent"))
        .child(Probe::new(child_id, "child", Rc::clone(&log), false, true));

    let (taffy, root_bounds, scene) = layout_and_paint(&mut root);
    let position = Point::new(5.0, 5.0);
    let mut focused = None;
    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(position));

    let result = root.dispatch_pointer_event(&mut cx, &pointer_up(position));

    assert_eq!(result, EventResult::Stop);
    assert_eq!(&*log.borrow(), &["child", "parent"]);
}

#[test]
fn no_hit_target_preserves_reverse_child_bound_forwarding() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut root = div()
        .w(40.0)
        .h(40.0)
        .child(Probe::new(
            ElementId::from(1),
            "bottom",
            Rc::clone(&log),
            true,
            false,
        ))
        .child(Probe::new(
            ElementId::from(2),
            "top",
            Rc::clone(&log),
            true,
            false,
        ));

    let (taffy, root_bounds, scene) = layout_and_paint(&mut root);
    let position = Point::new(5.0, 5.0);
    let mut focused = None;
    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(position));

    let result = root.dispatch_pointer_event(&mut cx, &pointer_up(position));

    assert_eq!(result, EventResult::Stop);
    assert_eq!(scene.hit_test(position), None);
    assert_eq!(&*log.borrow(), &["top"]);
}

#[test]
fn hoverable_tracks_enter_move_leave_and_cursor_intent() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let hover_id = ElementId::from(9);
    let enter_log = Rc::clone(&log);
    let move_log = Rc::clone(&log);
    let leave_log = Rc::clone(&log);
    let mut root = hoverable(container().w(20.0).h(20.0).background(Color::hex(0xff0000)))
        .id(hover_id)
        .cursor(Cursor::Pointer)
        .on_enter(move || enter_log.borrow_mut().push("enter"))
        .on_move(move |_| move_log.borrow_mut().push("move"))
        .on_leave(move || leave_log.borrow_mut().push("leave"));

    let (taffy, root_bounds, scene) = layout_and_paint(&mut root);
    let inside = Point::new(5.0, 5.0);
    let outside = Point::new(30.0, 30.0);
    let mut focused = None;

    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(inside));
    root.dispatch_pointer_event(&mut cx, &pointer_move(inside));
    assert_eq!(cx.cursor(), Some(Cursor::Pointer));
    assert!(cx.redraw_requested());

    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(inside));
    cx.set_previous_hit_target(Some(hover_id));
    root.dispatch_pointer_event(&mut cx, &pointer_move(inside));
    assert!(!cx.redraw_requested());

    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(outside));
    cx.set_previous_hit_target(Some(hover_id));
    root.dispatch_pointer_event(&mut cx, &pointer_move(outside));
    assert!(cx.redraw_requested());

    assert_eq!(&*log.borrow(), &["enter", "move", "move", "leave"]);
}

#[test]
fn hoverable_delegates_pointer_events_to_child() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let hover_id = ElementId::from(10);
    let child_id = ElementId::from(11);
    let mut root =
        hoverable(Probe::new(child_id, "child", Rc::clone(&log), true, false)).id(hover_id);

    let (taffy, root_bounds, scene) = layout_and_paint(&mut root);
    let position = Point::new(5.0, 5.0);
    let mut focused = None;
    let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
    cx.set_hit_target(scene.hit_test(position));

    let result = root.dispatch_pointer_event(&mut cx, &pointer_up(position));

    assert_eq!(result, EventResult::Stop);
    assert_eq!(scene.hit_test(position), Some(hover_id));
    assert_eq!(&*log.borrow(), &["child"]);
}
