use rui::core::action::{ActionHandler, ActionId, ActionOutcome, KeyChord, StandardAction};
use rui::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton};
use rui::core::menu::{Menu, MenuAction};
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::elements::{Div, Element, div, input};
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::RefCell;
use std::rc::Rc;

const FIELD: ElementId = ElementId(9_401);

fn new_file() -> ActionId {
    ActionId::custom("file.new")
}

fn delete_card() -> ActionId {
    ActionId::custom("card.delete")
}

struct Recorder {
    handles: Vec<ActionId>,
    ran: Rc<RefCell<Vec<ActionId>>>,
}

impl ActionHandler for Recorder {
    fn action_handler_name(&self) -> &str {
        "recorder"
    }

    fn run_action(&mut self, action: &ActionId) -> ActionOutcome {
        if !self.handles.contains(action) {
            return ActionOutcome::Ignored;
        }
        self.ran.borrow_mut().push(action.clone());
        ActionOutcome::handled("recorder")
    }
}

fn record(context: &mut AppContext, handles: Vec<ActionId>) -> Rc<RefCell<Vec<ActionId>>> {
    let ran = Rc::new(RefCell::new(Vec::new()));
    context.add_action_handler(Recorder {
        handles,
        ran: Rc::clone(&ran),
    });
    ran
}

fn menu_bar(dark: bool) -> Vec<Menu> {
    vec![
        Menu::new("App").item(MenuAction::new("Quit", ActionId::custom("app.quit"))),
        Menu::new("File")
            .item(MenuAction::new("New", new_file()))
            .item(MenuAction::new("Revert", ActionId::custom("file.revert")).enabled(false))
            .separator()
            .submenu(
                Menu::new("View")
                    .item(MenuAction::new("Dark", ActionId::custom("view.dark")).checked(dark)),
            ),
        Menu::new("Edit")
            .item(MenuAction::new("Copy", StandardAction::Copy))
            .item(MenuAction::new("Paste", StandardAction::Paste)),
    ]
}

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(240.0, 120.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless menu session should mount: {err}"),
    }
}

#[test]
fn menu_items_run_their_actions_and_disabled_ones_do_not() {
    let mut session = mount_or_panic(|_| div());
    let ran = record(
        session.app_context_mut(),
        vec![new_file(), ActionId::custom("file.revert")],
    );
    session.app_context_mut().set_menus(menu_bar(false));
    let file = session.app_context().menus()[1].clone();

    assert!(session.choose_menu_item(&file, "New"));
    assert_eq!(ran.borrow().as_slice(), [new_file()]);

    assert!(
        !session.choose_menu_item(&file, "Revert"),
        "Revert is disabled"
    );
    assert!(!session.choose_menu_item(&file, "Missing"));
    assert_eq!(ran.borrow().as_slice(), [new_file()]);
}

#[test]
fn edit_menu_items_act_on_the_focused_field_like_their_shortcuts() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    let mut session = mount_or_panic(move |_| {
        let recorded = Rc::clone(&recorded);
        input()
            .id(FIELD)
            .accessibility_label("Field")
            .value("menu text")
            .on_change(move |value| recorded.borrow_mut().push(value.to_owned()))
    });
    session.app_context_mut().set_menus(menu_bar(false));
    let edit = session.app_context().menus()[2].clone();
    session.request_focus(Some(FIELD));

    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta())));
    assert!(session.choose_menu_item(&edit, "Copy"));
    let copied = session.app_context().read_from_clipboard().ok().flatten();
    assert_eq!(copied.as_ref().map(|item| item.text()), Some("menu text"));

    // The selection is still all of the text, so pasting it back changes nothing
    // visible but goes through the field's paste.
    assert!(session.choose_menu_item(&edit, "Paste"));
    assert_eq!(changes.borrow().as_slice(), ["menu text".to_owned()]);
}

#[test]
fn menu_items_show_the_keymap_shortcut_of_their_action() {
    let session = mount_or_panic(|_| div());
    let keymap = session.app_context().keymap();
    assert_eq!(
        keymap.chord_for_action(&ActionId::from(StandardAction::Copy)),
        Some(KeyChord::new(KeyCode::C, Modifiers::meta()))
    );
    assert_eq!(keymap.chord_for_action(&new_file()), None);
}

struct Settings {
    dark: bool,
}

impl View for Settings {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        cx.set_menus(menu_bar(self.dark));
        div()
            .w(160.0)
            .h(80.0)
            .on_click(cx.listener(|this, _| this.dark = !this.dark))
    }
}

#[test]
fn views_set_menus_with_checked_state_from_their_own() {
    let mut session = match mount_view(Size::new(160.0, 80.0), Settings { dark: false }) {
        Ok(session) => session,
        Err(err) => panic!("settings view should mount: {err}"),
    };
    let dark = |session: &rui::testing::HeadlessViewSession<Settings>| {
        session.app_context().menus()[1]
            .find("Dark")
            .map(MenuAction::is_checked)
    };
    assert_eq!(dark(&session), Some(false));

    session.pointer_down(Point::new(4.0, 4.0));
    session.pointer_up(Point::new(4.0, 4.0));
    if let Err(err) = session.frame() {
        panic!("frame after the click failed: {err}");
    }
    assert_eq!(dark(&session), Some(true));
}

fn right_press(position: Point) -> PointerEvent {
    PointerEvent {
        kind: PointerEventKind::Down,
        position,
        button: Some(MouseButton::Right),
    }
}

struct Card {
    clicks: Rc<RefCell<u32>>,
}

impl View for Card {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let clicks = Rc::clone(&self.clicks);
        div()
            .w(160.0)
            .h(80.0)
            .on_click(move || *clicks.borrow_mut() += 1)
            .on_drag(|| "card", |_| div().w(10.0).h(10.0))
            .on_context_menu(cx.listener_with(|_, position, cx| {
                cx.show_context_menu(
                    Menu::new("Card").item(MenuAction::new("Delete", delete_card())),
                    position,
                );
            }))
    }
}

#[test]
fn a_right_press_asks_for_a_context_menu_at_the_pointer_without_clicking() {
    let clicks = Rc::new(RefCell::new(0));
    let view = Card {
        clicks: Rc::clone(&clicks),
    };
    let mut session = match mount_view(Size::new(160.0, 80.0), view) {
        Ok(session) => session,
        Err(err) => panic!("card view should mount: {err}"),
    };
    let ran = record(session.app_context_mut(), vec![delete_card()]);

    assert!(session.dispatch_pointer_event(right_press(Point::new(30.0, 20.0))));
    session.dispatch_pointer_event(PointerEvent {
        kind: PointerEventKind::Up,
        position: Point::new(30.0, 20.0),
        button: Some(MouseButton::Right),
    });
    if let Err(err) = session.frame() {
        panic!("frame after the right press failed: {err}");
    }
    assert_eq!(*clicks.borrow(), 0, "a right press does not click");
    assert!(!session.is_dragging());

    let request = match session.take_context_menu() {
        Some(request) => request,
        None => panic!("the right press should ask for a context menu"),
    };
    assert_eq!(request.position, Point::new(30.0, 20.0));
    assert!(session.choose_menu_item(&request.menu, "Delete"));
    assert_eq!(ran.borrow().as_slice(), [delete_card()]);
}

#[test]
fn setting_the_same_menus_again_asks_the_platform_for_nothing() {
    let mut session = mount_or_panic(|_| div());
    session.app_context_mut().set_menus(menu_bar(false));
    if let Err(err) = session.frame() {
        panic!("frame failed: {err}");
    }
    session.app_context_mut().set_menus(menu_bar(false));
    assert!(
        !session.app_context().has_frame_work(),
        "menus set from every render must not keep the window redrawing"
    );
}
