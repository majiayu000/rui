use rui::core::action::{ActionId, Keymap, StandardAction};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::text_editing::{TextInputCommand as Command, TextSelection, Utf16TextRange};
use rui::core::{ElementId, Size};
use rui::elements::element::EventContext;
use rui::elements::{Element, Input, TextArea, div};
use rui::testing::mount;
use std::cell::RefCell;
use std::rc::Rc;
use taffy::TaffyTree;

fn command(e: &mut impl Element, command: Command) {
    let mut focus = e.id();
    let tree = TaffyTree::new();
    let mut cx = EventContext::new(Bounds::from_xywh(0., 0., 300., 100.), &tree, &mut focus);
    assert!(e.handle_text_input_command(&mut cx, &command));
}
fn action(e: &mut impl Element, action: StandardAction) {
    let mut focus = e.id();
    let tree = TaffyTree::new();
    let mut cx = EventContext::new(Bounds::from_xywh(0., 0., 300., 100.), &tree, &mut focus);
    assert!(e.handle_action(&mut cx, &action.into()).is_handled());
}
fn assert_text(e: &impl Element, text: &str, selection: TextSelection) {
    let s = e.text_input_snapshot(e.id().unwrap()).unwrap();
    assert_eq!(s.text(), text);
    assert_eq!(s.selection(), selection);
}
fn undo_key(redo: bool) -> KeyEvent {
    KeyEvent::new(
        KeyCode::Z,
        Modifiers {
            shift: redo,
            ..Modifiers::meta()
        },
    )
}

fn exercise(e: &mut impl Element, multiline: bool) {
    let text = if multiline {
        "中😀e\u{301}\n末"
    } else {
        "中😀e\u{301}"
    };
    command(e, Command::InsertText(text.into()));
    action(e, StandardAction::SelectAll);
    command(e, Command::InsertText("换".into()));
    action(e, StandardAction::Undo);
    assert_text(e, text, TextSelection::new(0, text.len()));
    action(e, StandardAction::Redo);
    assert_text(e, "换", TextSelection::collapsed(3));
    let mut focus = e.id();
    let tree = TaffyTree::new();
    let mut cx = EventContext::new(Bounds::from_xywh(0., 0., 300., 100.), &tree, &mut focus);
    assert!(e.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::Backspace, Modifiers::none())
    ));
    assert_text(e, "", TextSelection::collapsed(0));
    action(e, StandardAction::Undo);
    assert_text(e, "换", TextSelection::collapsed(3));
    command(e, Command::InsertText("新".into()));
    action(e, StandardAction::Redo);
    assert_text(e, "换新", TextSelection::collapsed(6));
}
#[test]
fn input_and_textarea_actions_restore_unicode_text_and_exact_selection() {
    exercise(&mut Input::new().accessibility_label("Input"), false);
    exercise(
        &mut TextArea::new()
            .accessibility_label("Text area")
            .id(ElementId(2800)),
        true,
    );
}

fn composition(e: &mut impl Element) {
    command(e, Command::InsertText("原😀".into()));
    action(e, StandardAction::SelectAll);
    command(e, Command::BeginComposition("n".into()));
    command(e, Command::UpdateComposition("ni".into()));
    command(e, Command::UpdateComposition("你".into()));
    action(e, StandardAction::Redo);
    command(e, Command::CommitComposition("你好".into()));
    action(e, StandardAction::Undo);
    assert_text(e, "原😀", TextSelection::new(0, 7));
    action(e, StandardAction::Redo);
    assert_text(e, "你好", TextSelection::collapsed(6));
    command(e, Command::BeginComposition("暂".into()));
    action(e, StandardAction::Undo);
    assert_text(e, "你好", TextSelection::collapsed(6));
    assert!(
        e.text_input_snapshot(e.id().unwrap())
            .unwrap()
            .composition()
            .is_none()
    );
    action(e, StandardAction::Undo);
    assert_text(e, "原😀", TextSelection::new(0, 7));
    command(e, Command::BeginComposition("x".into()));
    command(e, Command::CancelComposition);
    assert_text(e, "原😀", TextSelection::new(0, 7));
    action(e, StandardAction::Redo);
    assert_text(e, "你好", TextSelection::collapsed(6));
}
#[test]
fn both_components_group_composition_and_cancel_without_consuming_history() {
    composition(&mut Input::new().accessibility_label("Input"));
    composition(
        &mut TextArea::new()
            .accessibility_label("Text area")
            .id(ElementId(2800)),
    );
}
#[test]
fn utf16_replacement_and_composition_cancel_restore_original_document() {
    let mut input = Input::new().accessibility_label("Input").value("a😀中");
    command(
        &mut input,
        Command::InsertTextReplacing {
            text: "e\u{301}".into(),
            replacement_range: Utf16TextRange::new(1, 2).unwrap(),
        },
    );
    action(&mut input, StandardAction::Undo);
    assert_text(&input, "a😀中", TextSelection::collapsed(8));
    command(
        &mut input,
        Command::BeginCompositionReplacing {
            text: "ni".into(),
            replacement_range: Utf16TextRange::new(1, 2).unwrap(),
        },
    );
    command(
        &mut input,
        Command::UpdateCompositionReplacing {
            text: "换".into(),
            replacement_range: Utf16TextRange::new(0, 1).unwrap(),
        },
    );
    command(&mut input, Command::CancelComposition);
    assert_text(&input, "a😀中", TextSelection::collapsed(8));
}
#[test]
fn history_retains_only_last_hundred_edits_and_programmatic_value_resets() {
    let mut input = Input::new().accessibility_label("Input");
    for _ in 0..105 {
        command(&mut input, Command::InsertText("a".into()));
    }
    for _ in 0..105 {
        action(&mut input, StandardAction::Undo);
    }
    assert_text(&input, "aaaaa", TextSelection::collapsed(5));
    input = input.value("reset");
    action(&mut input, StandardAction::Redo);
    action(&mut input, StandardAction::Undo);
    assert_text(&input, "reset", TextSelection::collapsed(5));
}
#[test]
fn native_style_key_route_keeps_field_histories_separate_across_focus_and_rebuild() {
    let a = ElementId(2801);
    let b = ElementId(2802);
    let mut session = mount(Size::new(300., 100.), move |_| {
        div()
            .child(Input::new().accessibility_label("Input").id(a))
            .child(
                TextArea::new()
                    .accessibility_label("Text area")
                    .id(ElementId(2800))
                    .id(b),
            )
    })
    .unwrap();
    session.request_focus(Some(a));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('中'));
    session.request_focus(Some(b));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::B, Modifiers::none()).with_char('😀'));
    session.resize(Size::new(320., 100.));
    assert!(session.dispatch_key_event(&undo_key(false)));
    let tree = session.accessibility_tree().unwrap();
    assert_eq!(tree.find(a).unwrap().a11y_value(), Some("中"));
    assert_eq!(tree.find(b).unwrap().a11y_value(), Some(""));
    session.request_focus(Some(a));
    session.dispatch_key_event(&undo_key(false));
    session.dispatch_key_event(&undo_key(true));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(a)
            .unwrap()
            .a11y_value(),
        Some("中")
    );
}
#[test]
fn controlled_echo_keeps_history_but_parent_reset_to_initial_value_clears_it() {
    let id = ElementId(2803);
    let value = Rc::new(RefCell::new(String::new()));
    let model = value.clone();
    let mut session = mount(Size::new(300., 100.), move |_| {
        let current = model.borrow().clone();
        let model = model.clone();
        Input::new()
            .accessibility_label("Input")
            .id(id)
            .value(current)
            .on_change(move |s| *model.borrow_mut() = s.into())
    })
    .unwrap();
    session.request_focus(Some(id));
    for ch in "abc".chars() {
        session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::none()).with_char(ch));
    }
    session.resize(Size::new(320., 100.));
    session.dispatch_key_event(&undo_key(false));
    assert_eq!(&*value.borrow(), "ab");
    value.borrow_mut().clear();
    session.resize(Size::new(330., 100.));
    session.dispatch_key_event(&undo_key(false));
    session.dispatch_key_event(&undo_key(true));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("")
    );
}
#[test]
fn explicit_application_binding_takes_priority_over_default_undo() {
    let mut keymap = Keymap::with_standard_bindings().unwrap();
    assert_eq!(
        keymap.action_for_event(&undo_key(false)),
        Some(&ActionId::Standard(StandardAction::Undo))
    );
    keymap
        .bind(KeyCode::Z, Modifiers::meta(), StandardAction::SelectAll)
        .unwrap();
    let id = ElementId(2804);
    let mut session = mount(Size::new(300., 100.), move |_| {
        Input::new().accessibility_label("Input").id(id)
    })
    .unwrap();
    *session.app_context_mut().keymap_mut() = keymap;
    session.request_focus(Some(id));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a'));
    session.dispatch_key_event(&undo_key(false));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::B, Modifiers::none()).with_char('b'));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("b")
    );
}

#[test]
fn multiline_paste_and_cut_use_one_transaction_through_key_actions() {
    use rui::core::text_editing::MemoryClipboard;
    let id = ElementId(2805);
    let mut session = mount(Size::new(300., 100.), move |_| {
        TextArea::new()
            .accessibility_label("Text area")
            .id(ElementId(2800))
            .id(id)
    })
    .unwrap();
    session
        .app_context_mut()
        .set_clipboard(MemoryClipboard::with_text("中😀\ne\u{301}"));
    session.request_focus(Some(id));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::V, Modifiers::meta()));
    session.dispatch_key_event(&undo_key(false));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("")
    );
    session.dispatch_key_event(&undo_key(true));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta()));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::X, Modifiers::meta()));
    assert_eq!(
        session
            .app_context()
            .read_from_clipboard()
            .unwrap()
            .unwrap()
            .text(),
        "中😀\ne\u{301}"
    );
    session.dispatch_key_event(&undo_key(false));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("中😀\ne\u{301}")
    );
}
#[test]
fn replacing_root_field_identity_does_not_transfer_history() {
    let id = Rc::new(RefCell::new(ElementId(2806)));
    let model = id.clone();
    let mut session = mount(Size::new(300., 100.), move |_| {
        Input::new()
            .accessibility_label("Input")
            .id(*model.borrow())
    })
    .unwrap();
    session.request_focus(Some(*id.borrow()));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a'));
    let new_id = ElementId(2807);
    *id.borrow_mut() = new_id;
    session.resize(Size::new(320., 100.));
    session.request_focus(Some(new_id));
    session.dispatch_key_event(&undo_key(false));
    session.dispatch_key_event(&undo_key(true));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(new_id)
            .unwrap()
            .a11y_value(),
        Some("")
    );
}

#[test]
fn disabled_and_read_only_composition_cleanup_restores_text_and_selection() {
    for disabled in [false, true] {
        let mut input = Input::new().accessibility_label("Input").value("中😀");
        action(&mut input, StandardAction::SelectAll);
        command(&mut input, Command::BeginComposition("临时".into()));
        input = input.disabled(disabled).read_only(!disabled);
        command(&mut input, Command::CancelComposition);
        assert_text(&input, "中😀", TextSelection::new(0, 7));
        let mut area = TextArea::new()
            .accessibility_label("Text area")
            .id(ElementId(2800))
            .value("中\n😀");
        action(&mut area, StandardAction::SelectAll);
        command(&mut area, Command::BeginComposition("临时".into()));
        area = area.disabled(disabled).read_only(!disabled);
        command(&mut area, Command::CancelComposition);
        assert_text(&area, "中\n😀", TextSelection::new(0, 8));
    }
}

#[test]
fn derived_input_identity_preserves_history_across_rebuild() {
    let mut session = mount(Size::new(300., 100.), |_| {
        div().child(Input::new().accessibility_label("Input"))
    })
    .unwrap();
    session.dispatch_key_event(&KeyEvent::new(KeyCode::Tab, Modifiers::none()));
    let id = session.focused_element().unwrap();
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a'));
    session.resize(Size::new(320., 100.));
    session.dispatch_key_event(&undo_key(false));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("")
    );
    session.dispatch_key_event(&undo_key(true));
    assert_eq!(
        session
            .accessibility_tree()
            .unwrap()
            .find(id)
            .unwrap()
            .a11y_value(),
        Some("a")
    );
}

#[test]
fn reverse_selection_survives_undo_in_both_components() {
    fn check(e: &mut impl Element) {
        command(e, Command::InsertText("中😀".into()));
        let mut focus = e.id();
        let tree = TaffyTree::new();
        let mut cx = EventContext::new(Bounds::from_xywh(0., 0., 300., 100.), &tree, &mut focus);
        assert!(e.handle_key_event(
            &mut cx,
            &KeyEvent::new(KeyCode::ArrowLeft, Modifiers::shift())
        ));
        command(e, Command::InsertText("替".into()));
        action(e, StandardAction::Undo);
        assert_text(e, "中😀", TextSelection::new(7, 3));
        action(e, StandardAction::Redo);
        assert_text(e, "中替", TextSelection::collapsed(6));
    }
    check(&mut Input::new());
    check(&mut TextArea::new().id(ElementId(2901)));
}

#[test]
fn rejected_composition_start_cannot_poison_next_edits_selection_snapshot() {
    let mut input = Input::new().value("e\u{301}x");
    assert!(
        input
            .apply_text_input_command(Command::BeginCompositionReplacing {
                text: "临时".into(),
                replacement_range: Utf16TextRange::new(1, 0).unwrap(),
            })
            .is_err()
    );
    input
        .apply_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::none()))
        .unwrap();
    command(&mut input, Command::InsertText("a".into()));
    action(&mut input, StandardAction::Undo);
    assert_text(&input, "e\u{301}x", TextSelection::collapsed(3));
}
