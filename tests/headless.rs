use rui::advanced_ui::{Button, Checkbox, Flex, column};
use rui::core::accessibility::AccessibilityRole;
use rui::core::event::Event;
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::element::{EventContext, LayoutContext, PaintContext, PointerEvent};
use rui::elements::{Div, Element, div};
use rui::testing::{HeadlessError, mount, mount_view};
use std::cell::Cell;
use std::rc::Rc;
use taffy::prelude::NodeId;

struct HeadlessCounterView {
    count: Rc<Cell<i32>>,
    button_id: ElementId,
}

struct StateContractView {
    external_checked: Rc<Cell<bool>>,
    local_checked: Rc<Cell<bool>>,
    blocked_changes: Rc<Cell<u32>>,
    external_id: ElementId,
    local_id: ElementId,
    disabled_id: ElementId,
    read_only_id: ElementId,
    refresh_id: ElementId,
}

struct ResizeAwareElement {
    button: Button,
    resized: bool,
    resize_deliveries: Rc<Cell<u32>>,
    painted_resized_state: Rc<Cell<bool>>,
}

struct DispatchBoundsProbe {
    inner: Div,
    observed_width: Rc<Cell<u32>>,
}

impl Element for DispatchBoundsProbe {
    fn style(&self) -> &rui::core::style::Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, _event: &PointerEvent) -> bool {
        self.observed_width.set(cx.bounds().width() as u32);
        true
    }
}

impl Element for ResizeAwareElement {
    fn style(&self) -> &rui::core::style::Style {
        self.button.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.button.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.painted_resized_state.set(self.resized);
        self.button.paint(cx);
    }

    fn handle_window_event(&mut self, event: &Event) -> bool {
        if matches!(event, Event::WindowResize { .. }) {
            self.resized = true;
            self.resize_deliveries.set(self.resize_deliveries.get() + 1);
            true
        } else {
            false
        }
    }
}

impl View for HeadlessCounterView {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let count = Rc::clone(&self.count);
        let notifier = cx.notifier();

        Button::new(format!("Count {}", self.count.get()))
            .id(self.button_id)
            .on_click(move || {
                count.set(count.get() + 1);
                notifier.notify();
            })
    }
}

impl View for StateContractView {
    type Element = Flex;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let external_checked = Rc::clone(&self.external_checked);
        let external_notifier = cx.notifier();
        let local_checked = Rc::clone(&self.local_checked);
        let local_notifier = cx.notifier();
        let disabled_changes = Rc::clone(&self.blocked_changes);
        let read_only_changes = Rc::clone(&self.blocked_changes);
        let refresh_notifier = cx.notifier();

        column()
            .spacing(8.0)
            .child(
                Checkbox::new("External state")
                    .id(self.external_id)
                    .checked(self.external_checked.get())
                    .on_change(move |checked| {
                        external_checked.set(checked);
                        external_notifier.notify();
                    }),
            )
            .child(
                Checkbox::new("View state")
                    .id(self.local_id)
                    .checked(self.local_checked.get())
                    .on_change(move |checked| {
                        local_checked.set(checked);
                        local_notifier.notify();
                    }),
            )
            .child(
                Checkbox::new("Disabled state")
                    .id(self.disabled_id)
                    .disabled(true)
                    .on_change(move |_| disabled_changes.set(disabled_changes.get() + 1)),
            )
            .child(
                Checkbox::new("Read only state")
                    .id(self.read_only_id)
                    .checked(true)
                    .read_only(true)
                    .on_change(move |_| read_only_changes.set(read_only_changes.get() + 1)),
            )
            .child(
                Button::new("Refresh")
                    .id(self.refresh_id)
                    .on_click(move || {
                        refresh_notifier.notify();
                    }),
            )
    }
}

fn mount_or_panic<F, E>(viewport: Size, build_root: F) -> rui::testing::HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: rui::elements::Element,
{
    match mount(viewport, build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless mount should succeed: {err}"),
    }
}

#[test]
fn headless_mount_renders_layout_and_primitives() {
    let session = mount_or_panic(Size::new(160.0, 80.0), |_cx| Button::new("Render"));
    let frame = match session.last_frame() {
        Some(frame) => frame,
        None => panic!("headless session should have an initial frame"),
    };

    assert_eq!(frame.viewport_size, Size::new(160.0, 80.0));
    assert!(frame.root_bounds.width() > 0.0);
    assert!(frame.root_bounds.height() > 0.0);
    assert!(!session.primitives().is_empty());
}

#[test]
fn headless_view_dispatches_events_and_rebuilds_after_notification() {
    let count = Rc::new(Cell::new(0));
    let button_id = ElementId::from(42);
    let view = HeadlessCounterView {
        count: Rc::clone(&count),
        button_id,
    };
    let mut session = match mount_view(Size::new(160.0, 80.0), view) {
        Ok(session) => session,
        Err(err) => panic!("headless view mount should succeed: {err}"),
    };

    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
    assert_eq!(count.get(), 1);

    if let Err(err) = session.frame() {
        panic!("headless frame should rebuild after notifier: {err}");
    }

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("headless accessibility tree should build: {err}"),
    };
    let node = match tree.find(button_id) {
        Some(node) => node,
        None => panic!("button node should be present in accessibility tree"),
    };

    assert_eq!(node.a11y_role(), AccessibilityRole::Button);
    assert_eq!(node.a11y_label(), Some("Count 1"));
}

#[test]
fn headless_view_keeps_external_local_and_read_only_state_contracts() {
    let external_checked = Rc::new(Cell::new(false));
    let local_checked = Rc::new(Cell::new(false));
    let blocked_changes = Rc::new(Cell::new(0));
    let external_id = ElementId::from(10);
    let local_id = ElementId::from(11);
    let disabled_id = ElementId::from(12);
    let read_only_id = ElementId::from(13);
    let refresh_id = ElementId::from(14);

    let view = StateContractView {
        external_checked: Rc::clone(&external_checked),
        local_checked: Rc::clone(&local_checked),
        blocked_changes: Rc::clone(&blocked_changes),
        external_id,
        local_id,
        disabled_id,
        read_only_id,
        refresh_id,
    };
    let mut session = match mount_view(Size::new(240.0, 240.0), view) {
        Ok(session) => session,
        Err(err) => panic!("headless state-contract view should mount: {err}"),
    };

    external_checked.set(true);
    assert!(session.pointer_down(Point::new(4.0, 180.0)));
    assert!(session.pointer_up(Point::new(4.0, 180.0)));
    if let Err(err) = session.frame() {
        panic!("external state notification should rebuild: {err}");
    }

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("accessibility tree should build after external update: {err}"),
    };
    assert_eq!(
        tree.find(external_id).and_then(|node| node.a11y_checked()),
        Some(true)
    );
    assert_eq!(
        tree.find(local_id).and_then(|node| node.a11y_checked()),
        Some(false)
    );

    assert!(session.pointer_down(Point::new(4.0, 48.0)));
    assert!(session.pointer_up(Point::new(4.0, 48.0)));
    if let Err(err) = session.frame() {
        panic!("local state notification should rebuild: {err}");
    }

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("accessibility tree should build after local update: {err}"),
    };
    assert_eq!(
        tree.find(external_id).and_then(|node| node.a11y_checked()),
        Some(true)
    );
    assert_eq!(
        tree.find(local_id).and_then(|node| node.a11y_checked()),
        Some(true)
    );

    let _ = session.pointer_down(Point::new(4.0, 92.0));
    let _ = session.pointer_up(Point::new(4.0, 92.0));
    let _ = session.pointer_down(Point::new(4.0, 136.0));
    let _ = session.pointer_up(Point::new(4.0, 136.0));
    if let Err(err) = session.frame() {
        panic!("blocked controls should remain frameable: {err}");
    }

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("accessibility tree should build after blocked controls: {err}"),
    };
    assert_eq!(blocked_changes.get(), 0);
    assert_eq!(
        tree.find(disabled_id).and_then(|node| node.a11y_checked()),
        Some(false)
    );
    assert_eq!(
        tree.find(read_only_id).and_then(|node| node.a11y_checked()),
        Some(true)
    );
}

#[test]
fn headless_resize_rebuilds_with_latest_viewport_size() {
    let render_width = Rc::new(Cell::new(0));
    let render_width_ref = Rc::clone(&render_width);
    let mut session = mount_or_panic(Size::new(160.0, 80.0), move |cx| {
        let width = cx.viewport_size().width as u32;
        render_width_ref.set(width);
        Button::new(format!("Width {width}"))
    });

    assert_eq!(render_width.get(), 160);

    session.resize(Size::new(240.0, 80.0));
    if let Err(err) = session.frame() {
        panic!("headless frame should rebuild after resize: {err}");
    }

    assert_eq!(render_width.get(), 240);
}

#[test]
fn headless_input_dispatch_uses_layout_for_the_latest_viewport() {
    let observed_width = Rc::new(Cell::new(0));
    let observed_width_ref = Rc::clone(&observed_width);
    let mut session = mount_or_panic(Size::new(160.0, 80.0), move |_cx| DispatchBoundsProbe {
        inner: div().w_full().h_full(),
        observed_width: Rc::clone(&observed_width_ref),
    });

    let _handled_resize = session.resize(Size::new(240.0, 120.0));
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert_eq!(observed_width.get(), 240);
}

#[test]
fn headless_resize_delivers_once_to_rebuilt_root_before_frame() {
    let build_count = Rc::new(Cell::new(0));
    let resize_deliveries = Rc::new(Cell::new(0));
    let painted_resized_state = Rc::new(Cell::new(false));
    let build_count_ref = Rc::clone(&build_count);
    let resize_deliveries_ref = Rc::clone(&resize_deliveries);
    let painted_resized_state_ref = Rc::clone(&painted_resized_state);
    let mut session = mount_or_panic(Size::new(160.0, 80.0), move |_cx| {
        build_count_ref.set(build_count_ref.get() + 1);
        ResizeAwareElement {
            button: Button::new("Resize aware"),
            resized: false,
            resize_deliveries: Rc::clone(&resize_deliveries_ref),
            painted_resized_state: Rc::clone(&painted_resized_state_ref),
        }
    });

    assert_eq!(build_count.get(), 1);
    assert!(session.resize(Size::new(240.0, 120.0)));
    assert_eq!(build_count.get(), 2);
    assert_eq!(resize_deliveries.get(), 1);

    if let Err(err) = session.frame() {
        panic!("headless frame should preserve current-root resize state: {err}");
    }

    assert_eq!(build_count.get(), 2);
    assert_eq!(resize_deliveries.get(), 1);
    assert!(painted_resized_state.get());
}

#[test]
fn headless_record_frame_and_capture_errors_are_explicit() {
    let session = mount_or_panic(Size::new(160.0, 80.0), |_cx| Button::new("Capture"));
    let recorded = match session.record_frame() {
        Ok(frame) => frame,
        Err(err) => panic!("recording renderer should capture primitives: {err}"),
    };

    assert_eq!(recorded.viewport_size, Size::new(160.0, 80.0));
    assert_eq!(recorded.primitives.len(), session.primitives().len());

    let error = match session.capture_current_frame() {
        Ok(_) => panic!("headless capture without a backend should fail"),
        Err(err) => err,
    };

    match error {
        HeadlessError::Renderer(err) => {
            assert!(err.to_string().contains("backend unavailable"));
        }
        other => panic!("expected renderer error, got {other}"),
    }
}

#[test]
fn headless_record_frame_keeps_the_prepared_scene_viewport_after_resize() {
    let initial_size = Size::new(160.0, 80.0);
    let resized = Size::new(240.0, 120.0);
    let mut session = mount_or_panic(initial_size, |_cx| Button::new("Capture"));

    assert!(
        !session.resize(resized),
        "the button does not handle window-resize events"
    );
    let recorded = match session.record_frame() {
        Ok(frame) => frame,
        Err(err) => panic!("recording the prepared scene should succeed: {err}"),
    };

    assert_eq!(recorded.viewport_size, initial_size);
    assert_eq!(recorded.primitives.len(), session.primitives().len());
}
