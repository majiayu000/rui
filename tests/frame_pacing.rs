use rui::core::{Point, Size, TaskPriority, View, ViewContext};
use rui::elements::{Div, div};
use rui::testing::mount_view;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

struct ClickTarget {
    log: Log,
}

impl View for ClickTarget {
    type Element = Div;

    fn render(&mut self, _cx: &mut ViewContext<Self>) -> Self::Element {
        let log = Rc::clone(&self.log);
        div()
            .w(200.0)
            .h(100.0)
            .on_click(move || log.borrow_mut().push("input".to_string()))
    }
}

fn defer_idle(log: &Log, label: &'static str) -> impl FnOnce(&mut rui::core::AppContext) + use<> {
    let log = Rc::clone(log);
    move |_| log.borrow_mut().push(label.to_string())
}

#[test]
fn input_is_handled_before_queued_idle_work() {
    let log = Log::default();
    let mut session = mount_view(
        Size::new(200.0, 100.0),
        ClickTarget {
            log: Rc::clone(&log),
        },
    )
    .expect("mount view");

    let cx = session.app_context_mut();
    cx.defer(TaskPriority::Idle, defer_idle(&log, "idle 1"));
    cx.defer(TaskPriority::Idle, defer_idle(&log, "idle 2"));
    let layout_log = Rc::clone(&log);
    cx.defer(TaskPriority::Layout, move |_| {
        layout_log.borrow_mut().push("layout".to_string())
    });

    // The click's frame runs layout work before it, but leaves idle work for
    // after input and frames are done.
    session.pointer_down(Point::new(50.0, 50.0));
    session.pointer_up(Point::new(50.0, 50.0));
    assert_eq!(*log.borrow(), ["layout", "input"]);
    assert!(session.app_context().has_idle_work());

    assert_eq!(session.run_idle_work(), 2);
    assert_eq!(*log.borrow(), ["layout", "input", "idle 1", "idle 2"]);
    assert!(!session.app_context().has_pending_work());
}

struct Pulse {
    frames_left: u32,
    renders: Rc<Cell<u32>>,
}

impl View for Pulse {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        self.renders.set(self.renders.get() + 1);
        if self.frames_left > 0 {
            self.frames_left -= 1;
            cx.request_animation_frame();
        }
        div().w(10.0 * self.frames_left as f32).h(10.0)
    }
}

#[test]
fn animation_frames_stop_once_nothing_animates() {
    let renders = Rc::new(Cell::new(0));
    let mut session = mount_view(
        Size::new(100.0, 40.0),
        Pulse {
            frames_left: 3,
            renders: Rc::clone(&renders),
        },
    )
    .expect("mount view");
    assert_eq!(renders.get(), 1);

    // Asking for a frame alone does not draw one; the display drives it.
    assert!(session.app_context().animation_frame_requested());
    assert!(!session.app_context().has_pending_work());

    let mut animated = 0;
    while session
        .animation_frame()
        .expect("animation frame")
        .is_some()
    {
        animated += 1;
    }

    assert_eq!(animated, 3);
    assert_eq!(renders.get(), 4);
    assert!(!session.app_context().animation_frame_requested());
    assert!(!session.app_context().has_pending_work());
}
