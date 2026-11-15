use super::*;
use taffy::TaffyTree;

const FRUIT: [(&str, &str); 4] = [
    ("apple", "Apple"),
    ("banana", "Banana"),
    ("blueberry", "Blueberry"),
    ("cherry", "Cherry"),
];

fn key(key: KeyCode, ch: Option<char>) -> KeyEvent {
    let mut event = KeyEvent::new(key, Modifiers::none());
    event.char = ch;
    event
}

fn changes() -> (Rc<RefCell<Vec<String>>>, impl Fn(&str) + 'static) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&changes);
    (changes, move |value: &str| {
        sink.borrow_mut().push(value.to_string())
    })
}

#[test]
fn advanced_ui_select_arrows_open_move_and_enter_picks() {
    let id = ElementId::new();
    let (changes, handler) = changes();
    let mut select = Select::new("Fruit", FRUIT, "apple")
        .id(id)
        .on_change(handler);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    let down = ActionId::Standard(StandardAction::MoveDown);
    assert!(select.handle_action(&mut cx, &down).is_handled());
    assert!(select.is_open());
    assert_eq!(select.highlighted_index(), Some(0));
    assert!(select.handle_action(&mut cx, &down).is_handled());
    assert!(select.handle_action(&mut cx, &down).is_handled());
    assert_eq!(select.highlighted_index(), Some(2));
    let enter = ActionId::Standard(StandardAction::Activate);
    assert!(select.handle_action(&mut cx, &enter).is_handled());
    assert!(!select.is_open());
    assert_eq!(select.selected_value(), "blueberry");
    assert_eq!(&*changes.borrow(), &["blueberry".to_string()]);
    assert_eq!(cx.focused_id(), Some(id));
}

#[test]
fn advanced_ui_select_escape_closes_without_changing() {
    let id = ElementId::new();
    let (changes, handler) = changes();
    let mut select = Select::new("Fruit", FRUIT, "apple")
        .id(id)
        .on_change(handler);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    assert!(select.handle_key_event(&mut cx, &key(KeyCode::Space, Some(' '))));
    assert!(select.is_open());
    assert!(select.handle_key_event(&mut cx, &key(KeyCode::End, None)));
    assert_eq!(select.highlighted_index(), Some(3));
    let cancel = ActionId::Standard(StandardAction::Cancel);
    assert!(select.handle_action(&mut cx, &cancel).is_handled());
    assert!(!select.is_open());
    assert!(!select.handle_action(&mut cx, &cancel).is_handled());
    assert_eq!(select.selected_value(), "apple");
    assert!(changes.borrow().is_empty());
}

#[test]
fn advanced_ui_select_type_ahead_matches_prefixes_and_cycles() {
    let id = ElementId::new();
    let mut select = Select::new("Fruit", FRUIT, "apple").id(id);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    assert!(select.handle_key_event(&mut cx, &key(KeyCode::B, Some('b'))));
    assert!(select.is_open());
    assert_eq!(select.highlighted_index(), Some(1));
    assert!(select.handle_key_event(&mut cx, &key(KeyCode::L, Some('l'))));
    assert_eq!(select.highlighted_index(), Some(2));

    select.typed_at = None;
    assert!(select.handle_key_event(&mut cx, &key(KeyCode::C, Some('c'))));
    assert_eq!(select.highlighted_index(), Some(3));
    let enter = ActionId::Standard(StandardAction::Activate);
    assert!(select.handle_action(&mut cx, &enter).is_handled());
    assert_eq!(select.selected_value(), "cherry");
}

#[test]
fn advanced_ui_select_disabled_does_not_open() {
    let id = ElementId::new();
    let mut select = Select::new("Fruit", FRUIT, "apple").id(id).disabled(true);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    let down = ActionId::Standard(StandardAction::MoveDown);
    assert!(!select.handle_action(&mut cx, &down).is_handled());
    assert!(!select.handle_key_event(&mut cx, &key(KeyCode::Space, Some(' '))));
    assert!(!select.is_open());
    let node = select
        .accessibility(&AccessibilityContext::default())
        .expect("select node")
        .expect("select exposes a node");
    assert!(!node.a11y_enabled());
    assert!(node.a11y_actions().is_empty());
}

#[test]
fn advanced_ui_select_accessibility_is_a_combo_box_with_its_open_list() {
    let id = ElementId::new();
    let mut select = Select::new("Fruit", FRUIT, "banana").id(id);
    let closed = select
        .accessibility(&AccessibilityContext::default())
        .expect("select node")
        .expect("select exposes a node");
    assert_eq!(closed.a11y_role(), AccessibilityRole::ComboBox);
    assert_eq!(closed.a11y_label(), Some("Fruit"));
    assert_eq!(closed.a11y_value(), Some("Banana"));
    assert_eq!(closed.a11y_actions(), &[AccessibilityAction::Activate]);
    assert!(closed.a11y_children().is_empty());

    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );
    let enter = ActionId::Standard(StandardAction::Activate);
    assert!(select.handle_action(&mut cx, &enter).is_handled());
    let open = select
        .accessibility(&AccessibilityContext::default())
        .expect("select node")
        .expect("select exposes a node");
    let list = &open.a11y_children()[0];
    assert_eq!(list.a11y_role(), AccessibilityRole::Menu);
    assert_eq!(list.a11y_children().len(), FRUIT.len());
    assert_eq!(list.a11y_children()[1].a11y_selected(), Some(true));
}

#[test]
fn advanced_ui_select_skips_disabled_items() {
    let id = ElementId::new();
    let items = [
        MenuItem::new("a", "Alpha"),
        MenuItem::new("b", "Beta").disabled(true),
        MenuItem::new("c", "Gamma"),
    ];
    let mut select = Select::new("Letter", items, "a").id(id);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    let down = ActionId::Standard(StandardAction::MoveDown);
    assert!(select.handle_action(&mut cx, &down).is_handled());
    assert!(select.handle_action(&mut cx, &down).is_handled());
    assert_eq!(select.highlighted_index(), Some(2));
}
