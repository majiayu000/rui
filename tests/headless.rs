use rui::advanced_ui::Button;
use rui::core::accessibility::AccessibilityRole;
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::testing::{HeadlessError, mount, mount_view};
use std::cell::Cell;
use std::rc::Rc;

struct HeadlessCounterView {
    count: Rc<Cell<i32>>,
    button_id: ElementId,
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
