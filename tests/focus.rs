use rui::advanced_ui::{Button, ControlSize, Theme, container, tooltip};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::focus::FocusHandle;
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::{Div, Element, div, input};
use rui::renderer::Primitive;
use rui::renderer::text::{TextMeasureCache, TextRequest};
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::RefCell;
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
    div()
        .flex_col()
        .child(field(NAME, "Name"))
        .child(Button::new("Locked").id(LOCKED).disabled(true))
        .child(field(EMAIL, "Email"))
        .child(Button::new("Save").id(SAVE))
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
