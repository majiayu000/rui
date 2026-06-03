use rui::core::accessibility::{AccessibilityAction, AccessibilityContext, AccessibilityTree};
use rui::core::action::{ActionId, StandardAction};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::{Bounds, Size};
use rui::core::text_editing::{MemoryClipboard, TextEditError, TextInputEvent, TextRange};
use rui::elements::element::{EventContext, LayoutContext, PaintContext};
use rui::elements::{Element, TextArea, div};
use rui::renderer::{Primitive, Scene};
use taffy::TaffyTree;
use taffy::prelude::AvailableSpace;

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

#[test]
fn text_area_select_all_action_and_accessibility_follow_editability() {
    let id = rui::core::ElementId::new();
    let mut area = TextArea::new()
        .id(id)
        .accessibility_label("Message")
        .value("hello");
    let taffy = TaffyTree::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 240.0, 120.0),
        &taffy,
        &mut focused,
    );

    assert!(
        area.handle_action(&mut cx, &ActionId::from(StandardAction::SelectAll))
            .is_handled()
    );
    assert_eq!(area.selection_range(), Some(range(0, 5)));

    let editable = area
        .accessibility(&AccessibilityContext::new(Some(id)))
        .expect("editable accessibility should build")
        .expect("editable area should expose a node");
    assert!(
        editable
            .a11y_actions()
            .contains(&AccessibilityAction::SetValue)
    );

    let read_only = TextArea::new()
        .id(id)
        .accessibility_label("Message")
        .read_only(true)
        .accessibility(&AccessibilityContext::new(Some(id)))
        .expect("read-only accessibility should build")
        .expect("read-only area should expose a node");
    assert!(
        !read_only
            .a11y_actions()
            .contains(&AccessibilityAction::SetValue)
    );
}

#[test]
fn text_area_paint_clips_multiline_content_to_bounds() {
    let mut area = TextArea::new()
        .accessibility_label("Message")
        .value("one\ntwo\nthree\nfour")
        .h(40.0)
        .w(160.0);
    let mut taffy = TaffyTree::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(160.0, 40.0));
    let node = area.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(160.0),
                height: AvailableSpace::Definite(40.0),
            },
        )
        .expect("text area layout should compute");

    let mut scene = Scene::new();
    let mut paint_cx =
        PaintContext::new(&mut scene, Bounds::from_xywh(0.0, 0.0, 160.0, 40.0), &taffy);
    area.paint(&mut paint_cx);

    assert!(matches!(scene.primitives()[1], Primitive::PushClip { .. }));
    assert!(
        scene
            .primitives()
            .iter()
            .any(|primitive| matches!(primitive, Primitive::PopClip))
    );
}
