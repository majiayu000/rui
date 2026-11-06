use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::{ElementId, Point, Size, View, ViewContext, WindowOptions};
use rui::elements::{Div, div};
use rui::testing::headless_app;

/// A window showing a shared counter and its own click count.
struct Panel {
    shared: Entity<u32>,
    clicks: u32,
    label_id: ElementId,
}

impl View for Panel {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let shared = *cx.app().get(self.shared).expect("shared entity");
        let size = cx.app().viewport_size();
        Button::new(format!(
            "shared {shared} clicks {} size {}x{}",
            self.clicks, size.width, size.height
        ))
        .id(self.label_id)
        .on_click(cx.listener(|this, _| this.clicks += 1))
    }
}

fn label(app: &rui::testing::HeadlessApp, window: rui::core::WindowId, id: ElementId) -> String {
    let tree = app.accessibility_tree(window).expect("accessibility tree");
    let node = tree.find(id).expect("label node");
    node.a11y_label().expect("label text").to_string()
}

const LEFT_LABEL: ElementId = ElementId(41_001);
const RIGHT_LABEL: ElementId = ElementId(41_002);

fn two_panels() -> (
    rui::testing::HeadlessApp,
    Entity<u32>,
    rui::core::WindowId,
    rui::core::WindowId,
) {
    let mut app = headless_app();
    let shared = app.app_context_mut().create(0_u32);
    let left = app
        .open_view_window(
            WindowOptions::new().size(300.0, 200.0),
            Panel {
                shared,
                clicks: 0,
                label_id: LEFT_LABEL,
            },
        )
        .expect("open left window");
    let right = app
        .open_view_window(
            WindowOptions::new().size(500.0, 400.0),
            Panel {
                shared,
                clicks: 0,
                label_id: RIGHT_LABEL,
            },
        )
        .expect("open right window");
    (app, shared, left, right)
}

#[test]
fn two_windows_keep_their_own_viewport_and_tree() {
    let (app, _, left, right) = two_panels();

    assert_eq!(app.window_ids(), [left, right]);
    assert_eq!(
        label(&app, left, LEFT_LABEL),
        "shared 0 clicks 0 size 300x200"
    );
    assert_eq!(
        label(&app, right, RIGHT_LABEL),
        "shared 0 clicks 0 size 500x400"
    );
    assert_eq!(app.app_context().windows().len(), 2);
}

#[test]
fn input_in_one_window_redraws_only_that_window() {
    let (mut app, _, left, right) = two_panels();
    let right_frames = app.frames_drawn(right).expect("right window");

    app.click(left, Point::new(5.0, 5.0)).expect("click left");
    assert_eq!(app.frame().expect("frame"), [left]);

    assert_eq!(
        label(&app, left, LEFT_LABEL),
        "shared 0 clicks 1 size 300x200"
    );
    assert_eq!(
        label(&app, right, RIGHT_LABEL),
        "shared 0 clicks 0 size 500x400"
    );
    assert_eq!(app.frames_drawn(right).expect("right window"), right_frames);
    assert!(app.frame().expect("idle frame").is_empty());
}

#[test]
fn shared_entity_updates_redraw_every_window() {
    let (mut app, shared, left, right) = two_panels();

    let handle = app.app_context().app_handle();
    std::thread::spawn(move || {
        handle
            .update(move |cx| {
                *cx.get_mut(shared).expect("shared entity") = 7;
                cx.notify(shared.id());
            })
            .expect("app is open");
    })
    .join()
    .expect("background thread");

    assert_eq!(app.frame().expect("frame"), [left, right]);
    assert_eq!(
        label(&app, left, LEFT_LABEL),
        "shared 7 clicks 0 size 300x200"
    );
    assert_eq!(
        label(&app, right, RIGHT_LABEL),
        "shared 7 clicks 0 size 500x400"
    );
}

#[test]
fn resizing_one_window_rebuilds_only_that_window() {
    let (mut app, _, left, right) = two_panels();

    app.resize(right, Size::new(640.0, 480.0)).expect("resize");

    assert_eq!(app.frame().expect("frame"), [right]);
    assert_eq!(
        label(&app, right, RIGHT_LABEL),
        "shared 0 clicks 0 size 640x480"
    );
    assert_eq!(
        label(&app, left, LEFT_LABEL),
        "shared 0 clicks 0 size 300x200"
    );
}

#[test]
fn closing_one_window_leaves_the_other_running() {
    let (mut app, shared, left, right) = two_panels();

    assert!(app.close_window(left));
    assert!(app.app_context().is_running());
    assert_eq!(app.window_ids(), [right]);

    *app.app_context_mut().get_mut(shared).expect("shared") = 3;
    app.app_context_mut().notify(shared.id());
    assert_eq!(app.frame().expect("frame"), [right]);
    assert_eq!(
        label(&app, right, RIGHT_LABEL),
        "shared 3 clicks 0 size 500x400"
    );

    assert!(app.close_window(right));
    assert!(
        !app.app_context().is_running(),
        "the last window quits by default"
    );
}

#[test]
fn the_app_can_outlive_its_windows_when_configured() {
    let (mut app, _, left, right) = two_panels();
    app.app_context_mut()
        .set_quit_when_last_window_closes(false);

    assert!(app.close_window(left));
    assert!(app.close_window(right));

    assert!(app.app_context().is_running());
    assert!(app.window_ids().is_empty());
    assert!(!app.close_window(right));
}

struct Opener {
    shared: Entity<u32>,
}

impl View for Opener {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let shared = self.shared;
        div().w_full().h_full().on_click(cx.listener(move |_, cx| {
            cx.app_mut().open_view_window(
                WindowOptions::new().size(200.0, 100.0),
                Panel {
                    shared,
                    clicks: 0,
                    label_id: RIGHT_LABEL,
                },
            );
        }))
    }
}

#[test]
fn a_handler_can_open_another_window() {
    let mut app = headless_app();
    let shared = app.app_context_mut().create(5_u32);
    let opener = app
        .open_view_window(WindowOptions::new().size(200.0, 100.0), Opener { shared })
        .expect("open opener");

    app.click(opener, Point::new(10.0, 10.0)).expect("click");
    app.frame().expect("frame that runs the listener");
    app.frame().expect("frame that creates the window");

    let ids = app.window_ids();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        label(&app, ids[1], RIGHT_LABEL),
        "shared 5 clicks 0 size 200x100"
    );
}

struct Notified {
    notifier: std::rc::Rc<std::cell::RefCell<Option<rui::core::ViewNotifier>>>,
}

impl View for Notified {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        *self.notifier.borrow_mut() = Some(cx.notifier());
        div().w_full().h_full()
    }
}

#[test]
fn a_view_notified_outside_input_redraws_only_its_window() {
    let (mut app, _, left, right) = two_panels();
    let notifier = std::rc::Rc::default();
    let notified = app
        .open_view_window(
            WindowOptions::new().size(100.0, 100.0),
            Notified {
                notifier: std::rc::Rc::clone(&notifier),
            },
        )
        .expect("open notified window");
    let before = [left, right].map(|window| app.frames_drawn(window).expect("open"));

    notifier.borrow().as_ref().expect("rendered once").notify();

    assert_eq!(app.frame().expect("frame"), [notified]);
    assert_eq!(
        [left, right].map(|window| app.frames_drawn(window).expect("open")),
        before
    );
}

#[test]
fn keyboard_focus_text_and_activation_stay_in_the_target_window() {
    use rui::core::event::{KeyCode, KeyEvent, Modifiers};
    use rui::core::text_editing::TextInputEvent;
    use rui::elements::input;
    use std::{cell::RefCell, rc::Rc};

    // Deliberately reuse IDs: element identity is scoped to the window.
    const FIELD: ElementId = ElementId(42_001);
    const SAVE: ElementId = ElementId(42_002);
    let mut app = headless_app();
    let values = Rc::new(RefCell::new([String::new(), String::new()]));
    let clicks = Rc::new(RefCell::new([0, 0]));
    let windows = [0, 1].map(|index| {
        let values = Rc::clone(&values);
        let clicks = Rc::clone(&clicks);
        app.open_window(WindowOptions::new().size(300.0, 200.0), move |_| {
            let changed = Rc::clone(&values);
            let clicked = Rc::clone(&clicks);
            div()
                .flex_col()
                .child(
                    input()
                        .id(FIELD)
                        .accessibility_label("Draft")
                        .value(values.borrow()[index].clone())
                        .on_change(move |value| changed.borrow_mut()[index] = value.to_owned()),
                )
                .child(
                    Button::new("Save")
                        .id(SAVE)
                        .on_click(move || clicked.borrow_mut()[index] += 1),
                )
        })
        .expect("open editor")
    });
    let [left, right] = windows;
    let tab = KeyEvent::new(KeyCode::Tab, Modifiers::none());
    let backtab = KeyEvent::new(KeyCode::Tab, Modifiers::shift());
    let enter = KeyEvent::new(KeyCode::Enter, Modifiers::none());

    assert!(app.frame().expect("settle both windows").is_empty());
    let right_frames = app.frames_drawn(right).expect("right window");
    assert!(app.dispatch_key_event(left, &tab).expect("left Tab"));
    app.frame().expect("frame after left key");
    assert_eq!(
        app.frames_drawn(right).expect("right window"),
        right_frames,
        "a key in the left window must not redraw the idle right window"
    );
    assert_eq!(app.focused_element(left).unwrap(), Some(FIELD));
    assert_eq!(app.focused_element(right).unwrap(), None);
    assert!(
        app.dispatch_text_input_event(left, &TextInputEvent::InsertText("left".into()))
            .unwrap()
    );
    app.frame().expect("frame after left text");
    assert_eq!(
        app.frames_drawn(right).expect("right window"),
        right_frames,
        "text in the left window must not redraw the idle right window"
    );
    assert_eq!(*values.borrow(), ["left", ""]);

    assert!(
        app.dispatch_key_event(right, &backtab)
            .expect("right reverse wrap")
    );
    assert_eq!(app.focused_element(right).unwrap(), Some(SAVE));
    assert_eq!(app.focused_element(left).unwrap(), Some(FIELD));
    assert!(app.dispatch_key_event(right, &enter).unwrap());
    assert_eq!(*clicks.borrow(), [0, 1]);
    assert!(
        !app.dispatch_text_input_event(right, &TextInputEvent::InsertText("ignored".into()))
            .unwrap()
    );
    assert_eq!(*values.borrow(), ["left", ""]);

    assert!(app.dispatch_key_event(right, &backtab).unwrap());
    assert!(
        app.dispatch_text_input_event(right, &TextInputEvent::InsertText("右".into()))
            .unwrap()
    );
    assert_eq!(*values.borrow(), ["left", "右"]);
    assert!(app.dispatch_key_event(left, &tab).unwrap());
    assert_eq!(app.focused_element(left).unwrap(), Some(SAVE));
    assert_eq!(app.focused_element(right).unwrap(), Some(FIELD));
    assert!(app.dispatch_key_event(left, &enter).unwrap());
    assert_eq!(*clicks.borrow(), [1, 1]);
    assert!(app.dispatch_key_event(left, &tab).unwrap());
    assert_eq!(app.focused_element(left).unwrap(), Some(FIELD));

    // Closing a focused text owner must not steal or clear the survivor's owner.
    app.frame().expect("settle before closing");
    let right_frames = app.frames_drawn(right).expect("surviving window");
    assert!(app.app_context_mut().close_window(left));
    assert!(app.dispatch_key_event(left, &tab).is_err());
    assert!(
        app.dispatch_text_input_event(left, &TextInputEvent::InsertText("closed".into()))
            .is_err()
    );
    assert!(app.click(left, Point::new(5.0, 5.0)).is_err());
    assert!(app.resize(left, Size::new(1.0, 1.0)).is_err());
    app.frame()
        .expect("frame after rejected closed-window events");
    assert_eq!(
        app.frames_drawn(right).expect("surviving window"),
        right_frames
    );
    assert_eq!(*values.borrow(), ["left", "右"]);
    assert_eq!(app.window_ids(), [right]);
    assert!(app.app_context().is_running());
    assert_eq!(app.focused_element(right).unwrap(), Some(FIELD));
    assert!(
        app.dispatch_text_input_event(right, &TextInputEvent::InsertText("!".into()))
            .unwrap()
    );
    assert_eq!(*values.borrow(), ["left", "右!"]);
    assert!(app.dispatch_key_event(right, &tab).unwrap());
    assert!(app.dispatch_key_event(right, &enter).unwrap());
    assert_eq!(*clicks.borrow(), [1, 2]);
    assert!(app.dispatch_key_event(left, &tab).is_err());
    assert!(
        app.dispatch_text_input_event(left, &TextInputEvent::InsertText("closed".into()))
            .is_err()
    );
    assert_eq!(*values.borrow(), ["left", "右!"]);
}
