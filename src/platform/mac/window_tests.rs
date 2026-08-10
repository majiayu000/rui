use super::*;
use crate::platform::PlatformImeEvent;

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

    append_ime_events_after_native_dispatch(
        &mut events,
        vec![PlatformImeEvent::InsertText("a".to_string())],
    );

    assert!(matches!(
        &events[..],
        [
        PlatformWindowEvent::Input(PlatformInputEvent::Ime(PlatformImeEvent::InsertText(text)))
        ] if text == "a"
    ));
}

#[test]
fn native_ime_commit_suppresses_the_raw_confirmation_key() {
    let mut events = vec![PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        KeyEvent::new(KeyCode::Enter, Modifiers::none()),
    ))];

    append_ime_events_after_native_dispatch(
        &mut events,
        vec![PlatformImeEvent::Commit("你".to_string())],
    );

    assert!(matches!(
        &events[..],
        [PlatformWindowEvent::Input(PlatformInputEvent::Ime(
            PlatformImeEvent::Commit(text)
        ))] if text == "你"
    ));
}

#[test]
fn native_ime_callbacks_preserve_full_composition_order() {
    let mut events = Vec::new();

    append_ime_events_after_native_dispatch(
        &mut events,
        vec![
            PlatformImeEvent::BeginComposition("你".to_string()),
            PlatformImeEvent::UpdateComposition("你好".to_string()),
            PlatformImeEvent::CancelComposition,
        ],
    );

    assert!(matches!(
        &events[..],
        [
            PlatformWindowEvent::Input(PlatformInputEvent::Ime(
                PlatformImeEvent::BeginComposition(begin)
            )),
            PlatformWindowEvent::Input(PlatformInputEvent::Ime(
                PlatformImeEvent::UpdateComposition(update)
            )),
            PlatformWindowEvent::Input(PlatformInputEvent::Ime(
                PlatformImeEvent::CancelComposition
            )),
        ] if begin == "你" && update == "你好"
    ));
}

#[test]
fn native_composition_cancel_suppresses_the_raw_escape_key() {
    let mut events = vec![PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        KeyEvent::new(KeyCode::Escape, Modifiers::none()),
    ))];

    append_ime_events_after_native_dispatch(&mut events, vec![PlatformImeEvent::CancelComposition]);

    assert!(matches!(
        &events[..],
        [PlatformWindowEvent::Input(PlatformInputEvent::Ime(
            PlatformImeEvent::CancelComposition
        ))]
    ));
}
