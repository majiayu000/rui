use super::*;
use crate::core::geometry::Bounds;
use objc2_foundation::{NSNotFound, NSRange};

fn utf16_range(location: usize, length: usize) -> Utf16TextRange {
    match Utf16TextRange::new(location, length) {
        Ok(range) => range,
        Err(err) => panic!("test range construction failed: {err}"),
    }
}

fn selection_event(location: usize, length: usize) -> TextInputCommand {
    TextInputCommand::SetCompositionSelection(utf16_range(location, length))
}

#[test]
fn ime_session_emits_begin_update_commit_and_cancel() {
    let mut session = MacImeSession::default();

    session
        .set_marked_text("你", NSRange::new(1, 0), not_found_range())
        .expect("composition should begin");
    session
        .set_marked_text("你好", NSRange::new(2, 0), not_found_range())
        .expect("composition should update");
    session
        .insert_text("您好", not_found_range())
        .expect("composition should commit");
    session
        .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
        .expect("composition should begin");
    session.cancel_composition();

    assert_eq!(
        session.drain_events(),
        vec![
            TextInputCommand::BeginComposition("你".to_string()),
            selection_event(1, 0),
            TextInputCommand::UpdateComposition("你好".to_string()),
            selection_event(2, 0),
            TextInputCommand::CommitComposition("您好".to_string()),
            TextInputCommand::BeginComposition("draft".to_string()),
            selection_event(5, 0),
            TextInputCommand::CancelComposition,
        ]
    );
    assert!(!session.has_marked_text());
}

#[test]
fn ime_session_uses_appkit_not_found_sentinel() {
    let session = MacImeSession::default();

    assert_eq!(session.selected_range().location, NSNotFound as NSUInteger);
}

#[test]
fn ime_session_reports_document_absolute_utf16_ranges() {
    let mut session = MacImeSession::default();
    session.update_text_input_state(Some(utf16_range(4, 0)), None, Some(utf16_range(4, 0)), None);

    session
        .set_marked_text("a😀", NSRange::new(3, 0), not_found_range())
        .expect("composition should begin");

    assert_eq!(session.marked_range(), NSRange::new(4, 3));
    assert_eq!(session.selected_range(), NSRange::new(7, 0));
}

#[test]
fn ime_session_plain_insert_does_not_fake_a_composition_commit() {
    let mut session = MacImeSession::default();

    session
        .insert_text("a", not_found_range())
        .expect("plain insert should succeed");

    assert_eq!(
        session.drain_events(),
        vec![TextInputCommand::InsertText("a".to_string())]
    );
}

#[test]
fn ime_session_preserves_concrete_replacement_ranges_for_all_text_callbacks() {
    let mut session = MacImeSession::default();
    session
        .insert_text("plain", NSRange::new(1, 2))
        .expect("plain replacement should be accepted");
    session
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(3, 4))
        .expect("marked replacement should begin");
    session
        .set_marked_text("updated", NSRange::new(7, 0), NSRange::new(3, 5))
        .expect("marked replacement should update");
    session
        .insert_text("committed", NSRange::new(3, 7))
        .expect("marked replacement should commit");

    assert_eq!(
        session.drain_events(),
        vec![
            TextInputCommand::InsertTextReplacing {
                text: "plain".to_string(),
                replacement_range: utf16_range(1, 2),
            },
            TextInputCommand::BeginCompositionReplacing {
                text: "draft".to_string(),
                replacement_range: utf16_range(3, 4),
            },
            selection_event(5, 0),
            TextInputCommand::UpdateCompositionReplacing {
                text: "updated".to_string(),
                replacement_range: utf16_range(3, 5),
            },
            selection_event(7, 0),
            TextInputCommand::CommitCompositionReplacing {
                text: "committed".to_string(),
                replacement_range: utf16_range(3, 7),
            },
        ]
    );
}

#[test]
fn ime_session_unmark_commits_current_marked_text() {
    let mut session = MacImeSession::default();
    session.update_text_input_state(
        Some(utf16_range(10, 0)),
        None,
        Some(utf16_range(10, 0)),
        None,
    );

    session
        .set_marked_text("pending", NSRange::new(7, 0), not_found_range())
        .expect("composition should begin");
    session.commit_marked_text();

    assert_eq!(
        session.drain_events(),
        vec![
            TextInputCommand::BeginComposition("pending".to_string()),
            selection_event(7, 0),
            TextInputCommand::CommitComposition("pending".to_string()),
        ]
    );
    assert!(!session.has_marked_text());
    assert_eq!(session.selected_range(), NSRange::new(17, 0));
}

#[test]
fn ime_session_keeps_empty_marked_text_until_commit_or_cancel() {
    let mut committed = MacImeSession::default();
    committed
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(0, 0))
        .expect("composition should begin");
    committed
        .set_marked_text("", NSRange::new(0, 0), not_found_range())
        .expect("composition should update");

    assert!(committed.has_marked_text());
    assert_eq!(committed.marked_range(), NSRange::new(0, 0));
    committed.commit_marked_text();
    assert_eq!(
        committed.drain_events(),
        vec![
            TextInputCommand::BeginCompositionReplacing {
                text: "draft".to_string(),
                replacement_range: utf16_range(0, 0),
            },
            selection_event(5, 0),
            TextInputCommand::UpdateComposition(String::new()),
            selection_event(0, 0),
            TextInputCommand::CommitComposition(String::new()),
        ]
    );

    let mut cancelled = MacImeSession::default();
    cancelled
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(0, 0))
        .expect("composition should begin");
    cancelled
        .set_marked_text("", NSRange::new(0, 0), not_found_range())
        .expect("composition should update");
    cancelled.cancel_composition();
    assert_eq!(
        cancelled.drain_events(),
        vec![
            TextInputCommand::BeginCompositionReplacing {
                text: "draft".to_string(),
                replacement_range: utf16_range(0, 0),
            },
            selection_event(5, 0),
            TextInputCommand::UpdateComposition(String::new()),
            selection_event(0, 0),
            TextInputCommand::CancelComposition,
        ]
    );
}

#[test]
fn ime_session_discards_marked_text_without_queuing_an_event() {
    let mut session = MacImeSession::default();
    session
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(0, 0))
        .expect("composition should begin");
    session.drain_events();

    assert!(session.discard_marked_text());
    assert!(!session.has_marked_text());
    assert_eq!(session.marked_range(), not_found_range());
    assert!(session.drain_events().is_empty());
}

#[test]
fn caret_rect_is_zero_width_and_flips_framework_y_into_appkit_view_space() {
    let rect = appkit_view_caret_rect(Bounds::from_xywh(12.0, 25.0, 1.5, 20.0), 100.0);

    assert_eq!(rect.origin, NSPoint::new(12.0, 55.0));
    assert_eq!(rect.size, NSSize::new(0.0, 20.0));
}

#[test]
fn ime_session_reports_the_actual_caret_range_and_geometry() {
    let mut session = MacImeSession::default();
    let bounds = Bounds::from_xywh(20.0, 30.0, 1.5, 18.0);

    assert!(session.update_text_input_state(
        Some(utf16_range(2, 3)),
        None,
        Some(utf16_range(5, 0)),
        Some(bounds),
    ));
    assert_eq!(session.caret_geometry(), (NSRange::new(5, 0), Some(bounds)));
    assert!(!session.update_text_input_state(
        Some(utf16_range(2, 3)),
        None,
        Some(utf16_range(5, 0)),
        Some(bounds),
    ));
}
