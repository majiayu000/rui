use super::*;
use crate::core::text_editing::TextInputCommand;

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
