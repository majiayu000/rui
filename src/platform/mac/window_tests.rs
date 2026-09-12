use super::*;
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityAction;
use crate::core::geometry::Bounds;
use crate::core::text_editing::{TextInputCommand, Utf16TextRange};
use crate::platform::mac::{
    MacAccessibilityActionRequest, MacAccessibilityRequest, MacPlatformEvent,
};

fn append_test_ime_events(
    platform_events: &mut Vec<PlatformWindowEvent>,
    commands: Vec<TextInputCommand>,
) -> (bool, Vec<MacWindowEvent>) {
    let consumed = suppress_key_down_for_ime(platform_events, &commands);
    let mut events = Vec::new();
    append_platform_events(&mut events, std::mem::take(platform_events));
    events.extend(commands.into_iter().map(MacWindowEvent::Text));
    (consumed, events)
}

#[test]
fn clipboard_text_or_error_rejects_missing_text() {
    assert_eq!(
        clipboard_text_or_error(None),
        Err(PlatformWindowError::backend(
            "macos",
            "general pasteboard does not contain text",
        ))
    );
    assert_eq!(
        clipboard_text_or_error(Some(String::new())),
        Ok(String::new())
    );
    assert_eq!(
        clipboard_text_or_error(Some("copied".to_string())),
        Ok("copied".to_string())
    );
}

#[test]
fn target_window_event_ends_poll_batch_before_the_next_native_callback() {
    assert!(event_ends_poll_batch(17, 17));
    assert!(!event_ends_poll_batch(23, 17));
}

#[test]
fn public_mac_poll_event_preserves_rich_text_commands() {
    let replacement_range = match Utf16TextRange::new(2, 3) {
        Ok(range) => range,
        Err(err) => panic!("valid range rejected: {err}"),
    };
    let command = TextInputCommand::InsertTextReplacing {
        text: "replacement".to_string(),
        replacement_range,
    };

    assert!(matches!(
        MacWindowEvent::Text(command.clone()).into_public_event(),
        MacPlatformEvent::Text(actual) if actual == command
    ));
    assert!(
        MacWindowEvent::Text(command)
            .try_into_platform_event()
            .is_err()
    );

    let selection = TextInputCommand::SetCompositionSelection(replacement_range);
    assert!(matches!(
        MacWindowEvent::Text(selection.clone()).into_public_event(),
        MacPlatformEvent::Text(actual) if actual == selection
    ));
    assert!(
        MacWindowEvent::Text(selection)
            .try_into_platform_event()
            .is_err()
    );
}

#[test]
fn public_mac_poll_event_preserves_accessibility_actions() {
    let request = MacAccessibilityActionRequest {
        id: ElementId::new(),
        request: MacAccessibilityRequest::Action {
            action: AccessibilityAction::Activate,
            value: None,
        },
        bounds: Bounds::from_xywh(10.0, 20.0, 30.0, 40.0),
    };

    assert!(matches!(
        MacWindowEvent::Accessibility(request.clone()).into_public_event(),
        MacPlatformEvent::Accessibility(actual) if actual == request
    ));
    assert!(
        MacWindowEvent::Accessibility(request)
            .try_into_platform_event()
            .is_err()
    );

    let focus = MacAccessibilityActionRequest {
        id: ElementId::new(),
        request: MacAccessibilityRequest::Focus(true),
        bounds: Bounds::from_xywh(50.0, 60.0, 70.0, 80.0),
    };
    assert!(matches!(
        MacWindowEvent::Accessibility(focus.clone()).into_public_event(),
        MacPlatformEvent::Accessibility(actual) if actual == focus
    ));
    assert!(
        MacWindowEvent::Accessibility(focus)
            .try_into_platform_event()
            .is_err()
    );
}

#[test]
fn native_ime_callbacks_suppress_consumed_key_down_without_duplication() {
    let mut events = vec![PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a'),
    ))];

    let (consumed, events) = append_test_ime_events(
        &mut events,
        vec![TextInputCommand::InsertText("a".to_string())],
    );

    assert!(consumed);
    assert!(matches!(
        &events[..],
        [MacWindowEvent::Text(TextInputCommand::InsertText(text))] if text == "a"
    ));
}

#[test]
fn native_ime_commit_suppresses_the_raw_confirmation_key() {
    let mut events = vec![PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        KeyEvent::new(KeyCode::Enter, Modifiers::none()),
    ))];

    let (consumed, events) = append_test_ime_events(
        &mut events,
        vec![TextInputCommand::CommitComposition("你".to_string())],
    );

    assert!(consumed);
    assert!(matches!(
        &events[..],
        [MacWindowEvent::Text(TextInputCommand::CommitComposition(text))] if text == "你"
    ));
}

#[test]
fn native_ime_callbacks_preserve_full_composition_order() {
    let mut events = Vec::new();

    let (consumed, events) = append_test_ime_events(
        &mut events,
        vec![
            TextInputCommand::BeginComposition("你".to_string()),
            TextInputCommand::UpdateComposition("你好".to_string()),
            TextInputCommand::CancelComposition,
        ],
    );

    assert!(!consumed);
    assert!(matches!(
        &events[..],
        [
            MacWindowEvent::Text(TextInputCommand::BeginComposition(begin)),
            MacWindowEvent::Text(TextInputCommand::UpdateComposition(update)),
            MacWindowEvent::Text(TextInputCommand::CancelComposition),
        ] if begin == "你" && update == "你好"
    ));
}

#[test]
fn native_composition_cancel_suppresses_the_raw_escape_key() {
    let mut events = vec![PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        KeyEvent::new(KeyCode::Escape, Modifiers::none()),
    ))];

    let (consumed, events) =
        append_test_ime_events(&mut events, vec![TextInputCommand::CancelComposition]);

    assert!(consumed);
    assert!(matches!(
        &events[..],
        [MacWindowEvent::Text(TextInputCommand::CancelComposition)]
    ));
}
