use rui::advanced_ui::Button;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::text_editing::{ClipboardItem, MemoryClipboard};
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::{Element, input, text_area};
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::RefCell;
use std::rc::Rc;

const FIELD: ElementId = ElementId(9_301);

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(240.0, 120.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless clipboard session should mount: {err}"),
    }
}

fn command(key: KeyCode, ch: char) -> KeyEvent {
    KeyEvent::new(key, Modifiers::meta()).with_char(ch)
}

fn clipboard_text<F, E>(session: &HeadlessSession<F, E>) -> Option<String>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match session.app_context().read_from_clipboard() {
        Ok(item) => item.map(|item| item.text().to_owned()),
        Err(err) => panic!("in-memory clipboard read failed: {err}"),
    }
}

fn recording_input(
    value: &'static str,
    read_only: bool,
) -> (
    impl FnMut(&mut AppContext) -> rui::elements::Input,
    Rc<RefCell<Vec<String>>>,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    let build = move |_: &mut AppContext| {
        let recorded = Rc::clone(&recorded);
        input()
            .id(FIELD)
            .accessibility_label("Field")
            .value(value)
            .read_only(read_only)
            .on_change(move |value| recorded.borrow_mut().push(value.to_owned()))
    };
    (build, changes)
}

#[test]
fn input_copy_and_cut_write_the_selection_to_the_app_clipboard() {
    let (build, changes) = recording_input("hello world", false);
    let mut session = mount_or_panic(build);
    session.request_focus(Some(FIELD));

    assert!(session.dispatch_key_event(&command(KeyCode::A, 'a')));
    assert!(session.dispatch_key_event(&command(KeyCode::C, 'c')));
    assert_eq!(clipboard_text(&session).as_deref(), Some("hello world"));
    assert!(changes.borrow().is_empty(), "copy must not edit the field");

    assert!(session.dispatch_key_event(&command(KeyCode::X, 'x')));
    assert_eq!(clipboard_text(&session).as_deref(), Some("hello world"));
    assert_eq!(changes.borrow().as_slice(), [String::new()]);
}

#[test]
fn input_paste_inserts_clipboard_text_written_through_the_app() {
    let (build, changes) = recording_input("", false);
    let mut session = mount_or_panic(build);
    session.request_focus(Some(FIELD));

    if let Err(err) = session
        .app_context()
        .write_to_clipboard(ClipboardItem::new_string("pasted"))
    {
        panic!("in-memory clipboard write failed: {err}");
    }
    assert!(session.dispatch_key_event(&command(KeyCode::V, 'v')));
    assert_eq!(changes.borrow().as_slice(), ["pasted".to_owned()]);
}

#[test]
fn read_only_input_copies_but_does_not_cut_or_paste() {
    let (build, changes) = recording_input("locked", true);
    let mut session = mount_or_panic(build);
    session.request_focus(Some(FIELD));

    assert!(session.dispatch_key_event(&command(KeyCode::A, 'a')));
    assert!(session.dispatch_key_event(&command(KeyCode::C, 'c')));
    assert_eq!(clipboard_text(&session).as_deref(), Some("locked"));

    // The ignored actions fall through to the field's raw key handling, which
    // does not insert command-modified characters either.
    session.dispatch_key_event(&command(KeyCode::X, 'x'));
    session.dispatch_key_event(&command(KeyCode::V, 'v'));
    assert!(changes.borrow().is_empty());
}

#[test]
fn paste_without_clipboard_text_or_with_a_failing_clipboard_changes_nothing() {
    let (build, changes) = recording_input("keep", false);
    let mut session = mount_or_panic(build);
    session.request_focus(Some(FIELD));

    assert_eq!(clipboard_text(&session), None);
    session.dispatch_key_event(&command(KeyCode::V, 'v'));

    session
        .app_context_mut()
        .set_clipboard(MemoryClipboard::with_read_error("denied"));
    assert!(session.app_context().read_from_clipboard().is_err());
    session.dispatch_key_event(&command(KeyCode::V, 'v'));
    assert!(changes.borrow().is_empty());
}

#[test]
fn text_area_copy_and_paste_use_the_app_clipboard() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    let mut session = mount_or_panic(move |_| {
        let recorded = Rc::clone(&recorded);
        text_area()
            .id(FIELD)
            .accessibility_label("Notes")
            .value("line one\nline two")
            .on_change(move |value| recorded.borrow_mut().push(value.to_owned()))
    });
    session.request_focus(Some(FIELD));

    assert!(session.dispatch_key_event(&command(KeyCode::A, 'a')));
    assert!(session.dispatch_key_event(&command(KeyCode::C, 'c')));
    assert_eq!(
        clipboard_text(&session).as_deref(),
        Some("line one\nline two")
    );

    if let Err(err) = session
        .app_context()
        .write_to_clipboard(ClipboardItem::new_string("replaced"))
    {
        panic!("in-memory clipboard write failed: {err}");
    }
    assert!(session.dispatch_key_event(&command(KeyCode::V, 'v')));
    assert_eq!(changes.borrow().as_slice(), ["replaced".to_owned()]);
}

struct CopyButton;

impl View for CopyButton {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        Button::new("Copy link")
            .id(FIELD)
            .on_click(cx.listener(|_, cx| {
                if let Err(err) = cx.write_to_clipboard(ClipboardItem::new_string("rui://link")) {
                    panic!("listener clipboard write failed: {err}");
                }
            }))
    }
}

#[test]
fn view_listeners_write_to_the_clipboard() {
    let mut session = match mount_view(Size::new(160.0, 80.0), CopyButton) {
        Ok(session) => session,
        Err(err) => panic!("copy button view should mount: {err}"),
    };
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
    if let Err(err) = session.frame() {
        panic!("frame after the listener failed: {err}");
    }

    let item = match session.app_context().read_from_clipboard() {
        Ok(item) => item,
        Err(err) => panic!("in-memory clipboard read failed: {err}"),
    };
    assert_eq!(item.as_ref().map(ClipboardItem::text), Some("rui://link"));
}
