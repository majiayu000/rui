use rui::core::accessibility::{AccessibilityContext, AccessibilityTree};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::text_editing::{MemoryClipboard, TextEditError, TextInputEvent, TextRange};
use rui::elements::element::EventContext;
use rui::elements::{Element, TextArea, div};
use taffy::TaffyTree;

fn must<T>(result: Result<T, TextEditError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("text area operation failed: {err}"),
    }
}

fn range(start: usize, end: usize) -> TextRange {
    match TextRange::new(start, end) {
        Ok(range) => range,
        Err(err) => panic!("range construction failed: {err}"),
    }
}

fn key_event(key: KeyCode) -> KeyEvent {
    KeyEvent::new(key, Modifiers::none())
}

fn shifted_key_event(key: KeyCode) -> KeyEvent {
    KeyEvent::new(key, Modifiers::shift())
}

fn typed_char_event(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Unknown(ch as u32), Modifiers::none()).with_char(ch)
}

#[test]
fn text_area_supports_multiline_editing_ime_and_clipboard() {
    let mut area = TextArea::new().value("ab");
    must(area.apply_key_event(&key_event(KeyCode::Enter)));
    assert_eq!(area.value_text(), "ab\n");

    must(area.apply_text_input_event(TextInputEvent::BeginComposition("你".into())));
    assert_eq!(area.value_text(), "ab\n你");
    assert_eq!(area.marked_text(), Some("你"));
    assert_eq!(area.composition_range(), Some(range(3, "ab\n你".len())));

    must(area.apply_text_input_event(TextInputEvent::UpdateComposition("你好".into())));
    assert_eq!(area.marked_text(), Some("你好"));

    must(area.apply_text_input_event(TextInputEvent::CommitComposition("您好".into())));
    assert_eq!(area.value_text(), "ab\n您好");
    assert_eq!(area.marked_text(), None);
    assert_eq!(area.composition_range(), None);

    must(area.apply_key_event(&shifted_key_event(KeyCode::ArrowLeft)));
    let mut clipboard = MemoryClipboard::new();
    assert!(must(area.copy_selection_to(&mut clipboard)));
    assert_eq!(clipboard.text(), "好");
}

#[test]
fn text_area_readonly_and_disabled_state_block_mutation() {
    let mut read_only = TextArea::new().value("abc").read_only(true);
    must(read_only.apply_key_event(&typed_char_event('x')));
    assert_eq!(read_only.value_text(), "abc");
    must(read_only.apply_key_event(&key_event(KeyCode::ArrowLeft)));
    assert_eq!(read_only.cursor_position(), 2);

    let mut disabled = TextArea::new().value("abc").disabled(true);
    must(disabled.apply_text_input_event(TextInputEvent::CommitComposition("x".into())));
    assert_eq!(disabled.value_text(), "abc");
    let mut clipboard = MemoryClipboard::new();
    assert!(!must(disabled.copy_selection_to(&mut clipboard)));
}

#[test]
fn text_area_text_input_events_dispatch_through_div_to_focused_child() {
    let id = rui::core::ElementId::new();
    let mut root = div().child(TextArea::new().id(id).accessibility_label("Message"));
    let taffy = TaffyTree::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 240.0, 120.0),
        &taffy,
        &mut focused,
    );

    assert!(
        root.handle_text_input_event(&mut cx, &TextInputEvent::CommitComposition("你好".into()))
    );

    let tree = AccessibilityTree::new(
        root.accessibility_nodes(&AccessibilityContext::default())
            .expect("accessibility should build"),
    );
    let node = tree.find(id).expect("text area node should exist");
    assert_eq!(node.a11y_value(), Some("你好"));
}
