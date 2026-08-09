use super::*;

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
fn committed_text_only_matches_single_char_key_events() {
    let a_key = KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a');
    assert!(committed_text_matches_key_event("a", &a_key));
    assert!(!committed_text_matches_key_event("ab", &a_key));
    assert!(!committed_text_matches_key_event("", &a_key));

    let ime_key = KeyEvent::new(KeyCode::Unknown(0), Modifiers::none());
    assert!(!committed_text_matches_key_event("好", &ime_key));
}
