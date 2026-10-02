use super::*;
use crate::core::geometry::Bounds;
use crate::core::text_editing::{
    TextEditLayout, TextInputGeometry, TextInputSnapshot, TextSelection,
};
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
    session.update_text_input_state(
        None,
        Some(utf16_range(4, 0)),
        None,
        Some(utf16_range(4, 0)),
        None,
    );

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
        None,
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
        None,
        Some(utf16_range(2, 3)),
        None,
        Some(utf16_range(5, 0)),
        Some(bounds),
    ));
    assert_eq!(session.caret_geometry(), (NSRange::new(5, 0), Some(bounds)));
    assert!(!session.update_text_input_state(
        None,
        Some(utf16_range(2, 3)),
        None,
        Some(utf16_range(5, 0)),
        Some(bounds),
    ));
}

#[test]
fn ime_session_answers_document_substring_range_and_point_queries() {
    let text = "a😀z";
    let geometry = TextInputGeometry::new(
        TextEditLayout::new(text, 10.0, 20.0),
        Point::new(20.0, 30.0),
    );
    let snapshot = TextInputSnapshot::new(text, TextSelection::collapsed(text.len()), None)
        .with_geometry(Some(geometry));
    let mut session = MacImeSession::default();
    session.update_text_input_state(
        Some(snapshot),
        Some(utf16_range(4, 0)),
        None,
        Some(utf16_range(4, 0)),
        Some(Bounds::from_xywh(50.0, 30.0, 1.5, 20.0)),
    );

    assert_eq!(
        session.attributed_substring(NSRange::new(1, 2)),
        Some((NSRange::new(1, 2), "😀".to_string()))
    );
    let (actual, bounds) = session
        .range_geometry(NSRange::new(1, 2))
        .expect("emoji geometry should be available");
    assert_eq!(actual, NSRange::new(1, 2));
    assert_eq!(bounds, Bounds::from_xywh(30.0, 30.0, 10.0, 20.0));
    assert_eq!(
        session.character_index_for_point(Point::new(20.0, 30.0)),
        Some(0)
    );
    assert_eq!(
        session.character_index_for_point(Point::new(31.0, 35.0)),
        Some(1)
    );
    assert_eq!(
        session.character_index_for_point(Point::new(41.0, 35.0)),
        Some(3)
    );
    for point in [
        Point::new(19.0, 35.0),
        Point::new(50.0, 35.0),
        Point::new(25.0, 29.0),
        Point::new(25.0, 50.0),
    ] {
        assert_eq!(session.character_index_for_point(point), None);
    }
}

fn dispatch_session_events(
    session: &mut MacImeSession,
    presenter: &mut crate::core::presenter::Presenter<crate::elements::Input>,
    ime_state: &mut crate::platform::mac::ime_state::NativeImeState,
) -> Vec<(bool, bool, bool)> {
    session
        .drain_events()
        .iter()
        .map(|event| {
            crate::platform::mac::ime_state::dispatch_text_input_event_for_owner(
                presenter, ime_state, event,
            )
        })
        .collect()
}

fn sync_session_snapshot(
    session: &mut MacImeSession,
    presenter: &crate::core::presenter::Presenter<crate::elements::Input>,
    owner: crate::core::ElementId,
) {
    use crate::elements::element::Element;
    let snapshot = presenter
        .root()
        .text_input_snapshot(owner)
        .expect("input snapshot");
    let selection = snapshot.selection();
    let selected = Utf16TextRange::from_text_range(snapshot.text(), selection.normalized_range())
        .expect("valid selection");
    let marked = snapshot.composition().map(|range| {
        Utf16TextRange::from_text_range(snapshot.text(), range).expect("valid composition")
    });
    let caret = Utf16TextRange::from_text_range(
        snapshot.text(),
        crate::core::text_editing::TextRange::collapsed(selection.head()),
    )
    .expect("valid caret");
    let bounds = snapshot.caret_bounds();
    session.update_text_input_state(Some(snapshot), Some(selected), marked, Some(caret), bounds);
}

#[test]
fn ime_session_failed_commit_retains_owner_for_update() {
    use crate::core::presenter::Presenter;
    use crate::core::{ElementId, Size};
    use crate::elements::{element::Element, input};
    use crate::platform::mac::ime_state::NativeImeState;

    let owner = ElementId::new();
    let mut presenter =
        Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
    presenter.set_focused_element(Some(owner));
    let mut ime_state = NativeImeState::default();
    let mut session = MacImeSession::default();
    sync_session_snapshot(&mut session, &presenter, owner);
    session
        .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
        .expect("begin");
    assert!(
        dispatch_session_events(&mut session, &mut presenter, &mut ime_state)
            .iter()
            .all(|result| result.0)
    );
    sync_session_snapshot(&mut session, &presenter, owner);
    let before = presenter.root().text_input_snapshot(owner);
    session
        .insert_text("rejected", NSRange::new(100, 1))
        .expect("queue stale replacement");
    assert_eq!(
        dispatch_session_events(&mut session, &mut presenter, &mut ime_state),
        vec![(false, false, false)]
    );
    assert_eq!(presenter.root().text_input_snapshot(owner), before);
    assert_eq!(
        ime_state.target_for_event(
            &TextInputCommand::UpdateComposition("updated".into()),
            Some(owner)
        ),
        Some(owner)
    );
}

#[test]
fn ime_session_failed_commit_restores_marked_text_from_editor() {
    use crate::core::presenter::Presenter;
    use crate::core::{ElementId, Size};
    use crate::elements::{element::Element, input};
    use crate::platform::mac::ime_state::NativeImeState;

    for (marked_text, committed_text, replacement) in [
        ("draft", "rejected", NSRange::new(100, 1)),
        ("😀draft", "rejected", NSRange::new(6, 1)),
        ("e\u{301}", "rejected", NSRange::new(5, 1)),
        ("draft", "\n", not_found_range()),
    ] {
        let owner = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        let mut session = MacImeSession::default();
        sync_session_snapshot(&mut session, &presenter, owner);
        session
            .set_marked_text(marked_text, NSRange::new(0, 0), not_found_range())
            .expect("begin");
        assert!(
            dispatch_session_events(&mut session, &mut presenter, &mut ime_state)
                .iter()
                .all(|result| result.0)
        );
        sync_session_snapshot(&mut session, &presenter, owner);
        let before = presenter.root().text_input_snapshot(owner);
        let selected = session.selected_range();
        let marked = session.marked_range();
        session
            .insert_text(committed_text, replacement)
            .expect("queue replacement");
        assert_eq!(
            dispatch_session_events(&mut session, &mut presenter, &mut ime_state),
            vec![(false, false, false)]
        );
        assert_eq!(presenter.root().text_input_snapshot(owner), before);
        sync_session_snapshot(&mut session, &presenter, owner);
        session.restore_marked_text_from_snapshot();
        assert!(
            session.has_marked_text(),
            "rejection must retain native composition mode"
        );
        assert_eq!(session.selected_range(), selected);
        assert_eq!(session.marked_range(), marked);
        session
            .set_marked_text("updated", NSRange::new(7, 0), not_found_range())
            .expect("next callback");
        let events = session.drain_events();
        assert_eq!(
            events,
            vec![
                TextInputCommand::UpdateComposition("updated".into()),
                selection_event(7, 0)
            ]
        );
        for event in events {
            assert!(
                crate::platform::mac::ime_state::dispatch_text_input_event_for_owner(
                    &mut presenter,
                    &mut ime_state,
                    &event,
                )
                .0
            );
        }
        sync_session_snapshot(&mut session, &presenter, owner);
        assert_eq!(
            presenter
                .root()
                .text_input_snapshot(owner)
                .expect("snapshot")
                .text(),
            "helloupdated"
        );
        session.cancel_composition();
        assert_eq!(
            dispatch_session_events(&mut session, &mut presenter, &mut ime_state),
            vec![(true, true, false)]
        );
        sync_session_snapshot(&mut session, &presenter, owner);
        assert!(!session.has_marked_text());
        assert_eq!(session.marked_range(), not_found_range());
        let restored = presenter
            .root()
            .text_input_snapshot(owner)
            .expect("snapshot");
        assert_eq!(restored.text(), "hello");
        assert!(restored.composition().is_none());
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("stale".into()),
                Some(owner)
            ),
            None
        );
    }
}

#[test]
fn ime_session_successful_commit_finishes_both_session_layers() {
    use crate::core::presenter::Presenter;
    use crate::core::{ElementId, Size};
    use crate::elements::{element::Element, input};
    use crate::platform::mac::ime_state::NativeImeState;

    for replacement in [not_found_range(), NSRange::new(5, 5)] {
        let owner = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        let mut session = MacImeSession::default();
        sync_session_snapshot(&mut session, &presenter, owner);
        session
            .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
            .expect("begin");
        assert!(
            dispatch_session_events(&mut session, &mut presenter, &mut ime_state)
                .iter()
                .all(|result| result.0)
        );
        sync_session_snapshot(&mut session, &presenter, owner);
        session
            .insert_text("done", replacement)
            .expect("queue commit");
        assert_eq!(
            dispatch_session_events(&mut session, &mut presenter, &mut ime_state),
            vec![(true, true, false)]
        );
        sync_session_snapshot(&mut session, &presenter, owner);
        assert!(!session.has_marked_text());
        assert_eq!(session.marked_range(), not_found_range());
        assert_eq!(session.selected_range(), NSRange::new(9, 0));
        let committed = presenter
            .root()
            .text_input_snapshot(owner)
            .expect("snapshot");
        assert_eq!(committed.text(), "hellodone");
        assert!(committed.composition().is_none());
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("stale".into()),
                Some(owner)
            ),
            None
        );
    }
}

#[test]
fn ime_session_invalid_commit_does_not_end_marked_text_before_dispatch() {
    let mut session = MacImeSession::default();
    session
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(0, 0))
        .expect("begin");
    session.drain_events();
    let selected = session.selected_range();
    let marked = session.marked_range();
    assert!(
        session
            .insert_text("x", NSRange::new(usize::MAX, 0))
            .is_err()
    );
    assert!(session.has_marked_text());
    assert_eq!(session.selected_range(), selected);
    assert_eq!(session.marked_range(), marked);
    assert!(session.drain_events().is_empty());
}

#[test]
fn ime_session_snapshot_does_not_create_marked_text_without_owner() {
    use crate::core::presenter::Presenter;
    use crate::core::{ElementId, Size};
    use crate::elements::input;
    use crate::platform::mac::ime_state::NativeImeState;

    let owner = ElementId::new();
    let mut presenter =
        Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
    presenter.set_focused_element(Some(owner));
    presenter
        .root_mut()
        .apply_text_input_command(TextInputCommand::BeginComposition("draft".into()))
        .expect("public editor API begins composition independently of native ownership");
    let mut session = MacImeSession::default();
    let mut ime_state = NativeImeState::default();
    sync_session_snapshot(&mut session, &presenter, owner);
    assert!(
        !session.has_marked_text(),
        "snapshot alone must not arm native composition"
    );
    session
        .insert_text("x", not_found_range())
        .expect("native insert");
    let events = session.drain_events();
    assert_eq!(events, vec![TextInputCommand::InsertText("x".into())]);
    assert!(
        crate::platform::mac::ime_state::dispatch_text_input_event_for_owner(
            &mut presenter,
            &mut ime_state,
            &events[0],
        )
        .0,
        "ordinary insert must reach the focused editor"
    );
}

#[test]
fn ime_session_snapshot_clear_preserves_marked_text_until_native_discard() {
    let mut session = MacImeSession::default();
    session
        .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(0, 0))
        .expect("begin");
    session.drain_events();
    session.update_text_input_state(None, None, None, None, None);
    assert!(
        session.discard_marked_text(),
        "invalid snapshot cleanup must still request AppKit discard"
    );
    assert!(!session.has_marked_text());
    assert_eq!(session.marked_range(), not_found_range());
}
