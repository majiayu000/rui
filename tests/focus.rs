use rui::advanced_ui::{Button, ControlSize, Theme, container, dialog, tooltip};
use rui::core::event::{Event, FocusEvent, KeyCode, KeyEvent, Modifiers};
use rui::core::focus::FocusHandle;
use rui::core::{AppContext, Bounds, ElementId, Point, Size, View, ViewContext};
use rui::elements::{Div, Element, div, input};
use rui::renderer::Primitive;
use rui::renderer::text::{TextMeasureCache, TextRequest};
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const NAME: ElementId = ElementId(9_501);
const LOCKED: ElementId = ElementId(9_502);
const EMAIL: ElementId = ElementId(9_503);
const SAVE: ElementId = ElementId(9_504);

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(240.0, 240.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless focus session should mount: {err}"),
    }
}

fn tab(shift: bool) -> KeyEvent {
    let mut modifiers = Modifiers::none();
    modifiers.shift = shift;
    KeyEvent::new(KeyCode::Tab, modifiers)
}

/// Name field, a disabled button, email field and a save button, with the
/// fields' focus and blur recorded.
fn form(log: &Rc<RefCell<Vec<String>>>) -> Div {
    let field = |id: ElementId, label: &'static str| {
        let focused = Rc::clone(log);
        let blurred = Rc::clone(log);
        input()
            .id(id)
            .accessibility_label(label)
            .on_focus(move || focused.borrow_mut().push(format!("focus {label}")))
            .on_blur(move || blurred.borrow_mut().push(format!("blur {label}")))
    };
    let saved = Rc::clone(log);
    div()
        .flex_col()
        .child(field(NAME, "Name"))
        .child(Button::new("Locked").id(LOCKED).disabled(true))
        .child(field(EMAIL, "Email"))
        .child(
            Button::new("Save")
                .id(SAVE)
                .on_click(move || saved.borrow_mut().push("click Save".into())),
        )
}

#[test]
fn tab_moves_focus_through_enabled_controls_in_order_and_wraps() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| form(&tree_log));

    let mut order = Vec::new();
    for _ in 0..4 {
        assert!(session.dispatch_key_event(&tab(false)));
        order.push(session.focused_element());
    }
    assert_eq!(
        order,
        [Some(NAME), Some(EMAIL), Some(SAVE), Some(NAME)],
        "Tab skips the disabled button and wraps"
    );

    assert!(session.dispatch_key_event(&tab(true)));
    assert_eq!(session.focused_element(), Some(SAVE), "Shift-Tab goes back");
    assert_eq!(
        log.borrow().as_slice(),
        [
            "focus Name",
            "blur Name",
            "focus Email",
            "blur Email",
            "focus Name",
            "blur Name",
        ]
    );
}

struct Search {
    field: FocusHandle,
    log: Rc<RefCell<Vec<String>>>,
    focused_after_click: Rc<RefCell<Vec<bool>>>,
}

impl View for Search {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let focused = Rc::clone(&self.log);
        let blurred = Rc::clone(&self.log);
        self.focused_after_click
            .borrow_mut()
            .push(self.field.is_focused(cx.app()));
        div()
            .flex_col()
            .child(
                div()
                    .w(100.0)
                    .h(30.0)
                    .on_click(cx.listener(|this, cx| this.field.focus(cx.app_mut()))),
            )
            .child(
                div()
                    .w(100.0)
                    .h(30.0)
                    .on_click(cx.listener(|_, cx| cx.blur())),
            )
            .child(
                input()
                    .id(self.field.id())
                    .accessibility_label("Search")
                    .on_focus(move || focused.borrow_mut().push("focus".into()))
                    .on_blur(move || blurred.borrow_mut().push("blur".into())),
            )
    }
}

const FIND: Point = Point { x: 10.0, y: 15.0 };
const DONE: Point = Point { x: 10.0, y: 45.0 };

fn click(session: &mut rui::testing::HeadlessViewSession<Search>, at: Point) {
    session.pointer_down(at);
    session.pointer_up(at);
    if let Err(err) = session.frame() {
        panic!("frame after the click failed: {err}");
    }
}

#[test]
fn listeners_focus_and_blur_an_element_through_its_handle() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let focused_after_click = Rc::new(RefCell::new(Vec::new()));
    let field = FocusHandle::new();
    let view = Search {
        field,
        log: Rc::clone(&log),
        focused_after_click: Rc::clone(&focused_after_click),
    };
    let mut session = match mount_view(Size::new(240.0, 200.0), view) {
        Ok(session) => session,
        Err(err) => panic!("search view should mount: {err}"),
    };
    assert_eq!(session.focused_element(), None);

    click(&mut session, FIND);
    assert_eq!(session.focused_element(), Some(field.id()));
    assert_eq!(session.app_context().focused(), Some(field.id()));
    assert!(field.is_focused(session.app_context()));
    assert_eq!(log.borrow().as_slice(), ["focus".to_owned()]);
    assert_eq!(
        focused_after_click.borrow().last(),
        Some(&true),
        "the render after the listener already sees the focus it asked for"
    );

    click(&mut session, DONE);
    assert_eq!(session.focused_element(), None);
    assert!(!field.is_focused(session.app_context()));
    assert_eq!(
        log.borrow().as_slice(),
        ["focus".to_owned(), "blur".to_owned()]
    );
}

#[test]
fn focusing_an_element_that_is_not_in_the_window_changes_nothing() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| form(&tree_log));
    session.request_focus(Some(NAME));
    if let Err(err) = session.frame() {
        panic!("frame failed: {err}");
    }

    session.app_context_mut().focus(ElementId(1));
    if let Err(err) = session.frame() {
        panic!("frame failed: {err}");
    }
    assert_eq!(session.focused_element(), Some(NAME));
    assert_eq!(session.app_context().focused(), Some(NAME));
}

/// The window losing key status, as `windowDidResignKey:` or winit's
/// `Focused(false)` reports it to the runner.
fn window_blur() -> Event {
    Event::Blur(FocusEvent { focused: false })
}

/// The window becoming key again.
fn window_focus() -> Event {
    Event::Focus(FocusEvent { focused: true })
}

/// A typed `x` as a raw key, which an editor that thinks it has focus takes.
fn x_key() -> KeyEvent {
    KeyEvent::new(KeyCode::X, Modifiers::none()).with_char('x')
}

fn frame_or_panic<F, E>(session: &mut HeadlessSession<F, E>)
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    if let Err(err) = session.frame() {
        panic!("frame failed: {err}");
    }
}

fn rebuild<F, E>(session: &mut HeadlessSession<F, E>)
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    session.app_context_mut().request_rebuild();
    frame_or_panic(session);
}

fn value_of<F, E>(session: &HeadlessSession<F, E>, id: ElementId) -> String
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("accessibility tree failed: {err}"),
    };
    tree.find(id)
        .and_then(|node| node.a11y_value())
        .unwrap_or_default()
        .to_owned()
}

/// Whether the frame just drawn paints the ring an input draws around itself
/// while it has focus: a 2 px border 2 px outside its bounds.
fn input_ring<F, E>(session: &HeadlessSession<F, E>, id: ElementId) -> bool
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let Some(bounds) = session
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.hit_regions())
        .find(|region| region.id == id)
        .map(|region| region.bounds)
    else {
        panic!("no hit region for {id:?}");
    };
    let ring = Bounds::from_xywh(
        bounds.x() - 2.0,
        bounds.y() - 2.0,
        bounds.width() + 4.0,
        bounds.height() + 4.0,
    );
    session.primitives().iter().any(|primitive| {
        matches!(primitive, Primitive::Quad { bounds, border_widths, .. }
            if *bounds == ring && border_widths.top == 2.0)
    })
}

#[test]
fn window_blur_hides_and_return_restores_an_inputs_focus() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| form(&tree_log));
    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(NAME));
    assert!(input_ring(&session, NAME));

    session.dispatch_window_event(&window_blur());
    assert_eq!(session.focused_element(), None);
    assert_eq!(session.app_context().focused(), None);
    assert!(
        !input_ring(&session, NAME),
        "an inactive window shows no ring"
    );
    assert!(
        !session.dispatch_key_event(&x_key()),
        "the input no longer takes raw keys"
    );
    assert_eq!(value_of(&session, NAME), "");
    // A second report of the same loss forgets nothing.
    session.dispatch_window_event(&window_blur());

    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), Some(NAME));
    assert_eq!(session.app_context().focused(), Some(NAME));
    assert!(
        input_ring(&session, NAME),
        "the ring comes back with the window"
    );
    assert!(session.dispatch_key_event(&x_key()));
    assert_eq!(value_of(&session, NAME), "x");
    assert_eq!(
        log.borrow().as_slice(),
        ["focus Name", "blur Name", "focus Name"]
    );
}

#[test]
fn a_button_focused_before_blur_activates_with_enter_after_return() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| form(&tree_log));
    for _ in 0..3 {
        assert!(session.dispatch_key_event(&tab(false)));
    }
    assert_eq!(session.focused_element(), Some(SAVE));

    session.dispatch_window_event(&window_blur());
    assert_eq!(session.focused_element(), None);
    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), Some(SAVE));

    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::Enter, Modifiers::none())));
    let clicks = log
        .borrow()
        .iter()
        .filter(|entry| *entry == "click Save")
        .count();
    assert_eq!(clicks, 1);
}

#[test]
fn a_control_removed_or_disabled_while_inactive_is_not_refocused() {
    let disabled = Rc::new(Cell::new(false));
    let present = Rc::new(Cell::new(true));
    let log = Rc::new(RefCell::new(Vec::new()));
    let (tree_disabled, tree_present, tree_log) =
        (Rc::clone(&disabled), Rc::clone(&present), Rc::clone(&log));
    let mut session = mount_or_panic(move |_| {
        let focused = Rc::clone(&tree_log);
        let mut root = div().flex_col();
        if tree_present.get() {
            root = root.child(
                input()
                    .id(NAME)
                    .accessibility_label("Name")
                    .disabled(tree_disabled.get())
                    .on_focus(move || focused.borrow_mut().push("focus Name")),
            );
        }
        root.child(Button::new("Save").id(SAVE))
    });

    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(NAME));
    session.dispatch_window_event(&window_blur());
    disabled.set(true);
    rebuild(&mut session);
    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), None, "Name is disabled now");

    disabled.set(false);
    rebuild(&mut session);
    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(NAME));
    session.dispatch_window_event(&window_blur());
    present.set(false);
    rebuild(&mut session);
    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), None, "Name is gone");
    assert_eq!(log.borrow().as_slice(), ["focus Name", "focus Name"]);

    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(SAVE));
}

#[test]
fn focus_moved_while_the_window_is_inactive_is_kept() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| form(&tree_log));
    assert!(session.dispatch_key_event(&tab(false)));
    session.dispatch_window_event(&window_blur());

    session.app_context_mut().focus(EMAIL);
    frame_or_panic(&mut session);
    session.dispatch_window_event(&window_focus());
    assert_eq!(
        session.focused_element(),
        Some(EMAIL),
        "the return does not take focus back to Name"
    );

    // The platform sets focus directly, as an accessibility client does.
    session.dispatch_window_event(&window_blur());
    session.request_focus(Some(SAVE));
    frame_or_panic(&mut session);
    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), Some(SAVE), "nor back to Email");

    session.dispatch_window_event(&window_blur());
    session.app_context_mut().blur();
    frame_or_panic(&mut session);
    session.dispatch_window_event(&window_focus());
    assert_eq!(
        session.focused_element(),
        None,
        "nor undo a blur asked for meanwhile"
    );
    assert_eq!(
        log.borrow().as_slice(),
        ["focus Name", "blur Name", "focus Email", "blur Email"]
    );
}

#[test]
fn a_return_does_not_refocus_a_control_behind_a_dialog_that_opened_meanwhile() {
    const DIALOG: ElementId = ElementId(9_505);
    const FIRST: ElementId = ElementId(9_506);
    const SECOND: ElementId = ElementId(9_507);
    let open = Rc::new(Cell::new(false));
    let first = Rc::new(Cell::new(true));
    let (tree_open, tree_first) = (Rc::clone(&open), Rc::clone(&first));
    let mut session = mount_or_panic(move |_| {
        let mut controls = div().flex();
        if tree_first.get() {
            controls = controls.child(Button::new("First").id(FIRST));
        }
        div().flex_col().child(Button::new("Save").id(SAVE)).child(
            dialog("Confirm", controls.child(Button::new("Second").id(SECOND)))
                .id(DIALOG)
                .open(tree_open.get()),
        )
    });
    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(SAVE));
    session.dispatch_window_event(&window_blur());

    // While the window is inactive a dialog opens and takes focus, then the
    // control it gave focus to goes away.
    open.set(true);
    rebuild(&mut session);
    assert_eq!(session.focused_element(), Some(FIRST));
    first.set(false);
    rebuild(&mut session);
    assert_eq!(session.focused_element(), None);

    session.dispatch_window_event(&window_focus());
    assert_eq!(session.focused_element(), None, "Save is behind the dialog");
    assert!(session.dispatch_key_event(&tab(false)));
    assert_eq!(session.focused_element(), Some(SECOND));
}

#[test]
fn a_tooltip_is_as_wide_as_its_measured_text() {
    // Wide characters, which a per-character estimate made too narrow.
    let content = "提示文字很长的说明";
    let mut session = mount_or_panic(move |_| {
        div()
            .flex_col()
            .child(div().h(60.0))
            .child(tooltip(container().w(40.0).h(20.0), content))
    });
    session.pointer_move(Point::new(10.0, 70.0));
    if let Err(err) = session.frame() {
        panic!("frame failed: {err}");
    }

    let theme = Theme::default();
    let font_size = theme.text_size(ControlSize::Small);
    let measured = match TextMeasureCache::new().measure_single_line(TextRequest::new(
        content,
        font_size,
        theme.typography.label_weight,
        None,
        theme.typography.line_height,
    )) {
        Ok(metrics) => metrics.size.width,
        Err(err) => panic!("tooltip text should measure: {err:?}"),
    };
    let background = theme.colors.tooltip_background.to_rgba();
    let width = session
        .primitives()
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Quad {
                bounds,
                background: color,
                ..
            } if *color == background => Some(bounds.width()),
            _ => None,
        });
    assert_eq!(
        width,
        Some(measured.ceil() + theme.control_gap() * 2.0),
        "the tooltip fits its text"
    );
    assert!(measured > content.chars().count() as f32 * font_size * 0.58);
}
