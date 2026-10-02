//! Logical ownership and targeted dispatch for macOS text input sessions.

use crate::core::ElementId;
use crate::core::presenter::Presenter;
use crate::core::text_editing::{
    TextEditError, TextInputCommand, TextInputSnapshot, Utf16TextRange,
};
use crate::elements::element::Element;
use crate::platform::mac::window::MacWindow;

#[derive(Debug, Default)]
pub(crate) struct NativeImeState {
    composition_owner: Option<ElementId>,
}

impl NativeImeState {
    pub(crate) fn target_for_event(
        &mut self,
        event: &TextInputCommand,
        focused: Option<ElementId>,
    ) -> Option<ElementId> {
        match event {
            TextInputCommand::InsertText(_) | TextInputCommand::InsertTextReplacing { .. } => {
                focused
            }
            TextInputCommand::BeginComposition(_)
            | TextInputCommand::BeginCompositionReplacing { .. } => {
                self.composition_owner = focused;
                focused
            }
            TextInputCommand::UpdateComposition(_)
            | TextInputCommand::UpdateCompositionReplacing { .. }
            | TextInputCommand::SetCompositionSelection(_) => self.composition_owner,
            TextInputCommand::CommitComposition(_)
            | TextInputCommand::CommitCompositionReplacing { .. }
            | TextInputCommand::CancelComposition => self.composition_owner.take(),
        }
    }

    pub(crate) fn cancel_owner_after_focus_change(
        &mut self,
        focused: Option<ElementId>,
        owner_exists: bool,
    ) -> Option<ElementId> {
        if self.composition_owner.is_some() && (self.composition_owner != focused || !owner_exists)
        {
            self.composition_owner.take()
        } else {
            None
        }
    }
}

pub(crate) fn dispatch_text_input_event<E>(
    presenter: &mut Presenter<E>,
    window: &MacWindow,
    ime_state: &mut NativeImeState,
    event: &TextInputCommand,
) -> (bool, bool)
where
    E: Element,
{
    let (handled, mut redraw_requested, discard_marked_text) =
        dispatch_text_input_event_for_owner(presenter, ime_state, event);
    if discard_marked_text {
        window.discard_marked_text();
    }
    if !handled
        && matches!(
            event,
            TextInputCommand::CommitComposition(_)
                | TextInputCommand::CommitCompositionReplacing { .. }
                | TextInputCommand::CancelComposition
        )
    {
        // AppKit callbacks optimistically end marked text. Restore the accepted editor
        // state before another callback, even when the rejected event requested no frame.
        let (sync_result, sync_redraw) = sync_text_input_snapshot(presenter, window, ime_state);
        redraw_requested |= sync_redraw;
        if let Err(err) = sync_result {
            log::error!("failed to restore macOS composition after a rejected command: {err}");
        } else if ime_state.composition_owner.is_some() {
            window.content_view.restore_marked_text_from_snapshot();
        }
    }
    (handled, redraw_requested)
}

/// Dispatches to the logical owner, returning `(handled, redraw_requested, discard_marked_text)`.
/// Native session synchronization stays in the window wrapper.
pub(crate) fn dispatch_text_input_event_for_owner<E>(
    presenter: &mut Presenter<E>,
    ime_state: &mut NativeImeState,
    event: &TextInputCommand,
) -> (bool, bool, bool)
where
    E: Element,
{
    let Some(target) = ime_state.target_for_event(event, presenter.focused_element()) else {
        log::error!("discarded macOS text input event without a focused composition owner");
        return (false, false, false);
    };
    if !presenter.root().contains_id(target) {
        ime_state.cancel_owner_after_focus_change(presenter.focused_element(), false);
        log::error!("discarded macOS text input event for a removed element");
        return (false, false, false);
    }
    let (handled, mut redraw_requested) = dispatch_text_input_event_to(presenter, target, event);
    if !handled
        && matches!(
            event,
            TextInputCommand::CommitComposition(_)
                | TextInputCommand::CommitCompositionReplacing { .. }
                | TextInputCommand::CancelComposition
        )
    {
        // target_for_event took the owner for this terminal command. Rejection must
        // leave later updates and focus-loss cancellation routed to the same editor.
        ime_state.composition_owner = Some(target);
    }
    // Begin claims ownership before dispatch. If the target rejected the command, clear the
    // logical owner *and* discard AppKit marked text: set_marked_text already armed the native
    // session, so later callbacks arrive as UpdateComposition. Leaving marked text while
    // clearing the owner makes cancel_composition_if_owner_lost unable to discard it, and
    // updates are dropped until AppKit independently ends the composition.
    let discard_marked_text = clear_owner_after_rejected_begin(ime_state, handled, event, target);
    if discard_marked_text
        && presenter
            .root()
            .text_input_snapshot(target)
            .is_some_and(|snapshot| snapshot.composition().is_some())
    {
        // A begin queued after a rejected commit can encounter the old composition.
        // Cancel that editor state as well as discarding the native marked text.
        let (cancelled, cancel_redraw) =
            dispatch_text_input_event_to(presenter, target, &TextInputCommand::CancelComposition);
        redraw_requested |= cancelled || cancel_redraw;
        if !cancelled {
            log::error!("failed to cancel macOS composition after a rejected begin");
        }
    }
    (handled, redraw_requested, discard_marked_text)
}

/// Clears `composition_owner` after a rejected begin. Returns true when the caller must also
/// discard native marked text so AppKit leaves update-composition mode.
pub(crate) fn clear_owner_after_rejected_begin(
    ime_state: &mut NativeImeState,
    handled: bool,
    event: &TextInputCommand,
    target: ElementId,
) -> bool {
    if handled
        || !matches!(
            event,
            TextInputCommand::BeginComposition(_)
                | TextInputCommand::BeginCompositionReplacing { .. }
        )
        || ime_state.composition_owner != Some(target)
    {
        return false;
    }
    ime_state.composition_owner = None;
    true
}

fn dispatch_text_input_event_to<E>(
    presenter: &mut Presenter<E>,
    target: ElementId,
    event: &TextInputCommand,
) -> (bool, bool)
where
    E: Element,
{
    let focused = presenter.focused_element();
    *presenter.focused_element_mut() = Some(target);
    let result = presenter
        .with_event_context(|root, event_cx| root.handle_text_input_command(event_cx, event));
    *presenter.focused_element_mut() = focused;
    result
}

pub(crate) fn cancel_composition_if_owner_lost<E>(
    presenter: &mut Presenter<E>,
    window: &MacWindow,
    ime_state: &mut NativeImeState,
) -> bool
where
    E: Element,
{
    let Some(owner) = take_lost_composition_owner(presenter, ime_state) else {
        return false;
    };
    let (handled, redraw_requested) =
        dispatch_text_input_event_to(presenter, owner, &TextInputCommand::CancelComposition);
    window.discard_marked_text();
    if !handled {
        log::error!("failed to cancel macOS composition for its previous focused owner");
    }
    handled || redraw_requested
}

fn take_lost_composition_owner<E>(
    presenter: &mut Presenter<E>,
    ime_state: &mut NativeImeState,
) -> Option<ElementId>
where
    E: Element,
{
    if presenter
        .focused_element()
        .is_some_and(|focused| !presenter.root().contains_id(focused))
    {
        presenter.set_focused_element(None);
    }
    let owner_exists = ime_state
        .composition_owner
        .is_none_or(|owner| presenter.root().contains_id(owner));
    ime_state.cancel_owner_after_focus_change(presenter.focused_element(), owner_exists)
}

/// Syncs the focused text-input snapshot into AppKit.
///
/// Returns `(sync_result, redraw_requested)`. `redraw_requested` is true when an invalid
/// snapshot forced a post-frame `CancelComposition` that mutated the input; the run loop must
/// schedule a platform redraw so marked text does not linger until an unrelated event.
pub(crate) fn sync_text_input_snapshot<E>(
    presenter: &mut Presenter<E>,
    window: &MacWindow,
    ime_state: &mut NativeImeState,
) -> (Result<(), TextEditError>, bool)
where
    E: Element,
{
    let Some(focused) = presenter.focused_element() else {
        window
            .content_view
            .update_text_input_state(None, None, None, None, None);
        return (Ok(()), false);
    };
    let Some(snapshot) = presenter.root().text_input_snapshot(focused) else {
        window
            .content_view
            .update_text_input_state(None, None, None, None, None);
        return (Ok(()), false);
    };
    let (selected_range, marked_range, caret_range) = match text_input_ranges(&snapshot) {
        Ok(ranges) => ranges,
        Err(err) => {
            // Invalid selection/composition ranges must not leave AppKit reporting a
            // previous snapshot (possibly from another input) for later IME callbacks.
            // Also cancel logical composition ownership so the next setMarkedText can
            // BeginComposition instead of failing with CompositionActive.
            window
                .content_view
                .update_text_input_state(None, None, None, None, None);
            window.discard_marked_text();
            let redraw_requested =
                cancel_logical_composition_after_native_discard(presenter, ime_state);
            return (Err(err), redraw_requested);
        }
    };
    window.content_view.update_text_input_state(
        Some(snapshot.clone()),
        Some(selected_range),
        marked_range,
        Some(caret_range),
        snapshot.caret_bounds(),
    );
    (Ok(()), false)
}

fn cancel_logical_composition_after_native_discard<E>(
    presenter: &mut Presenter<E>,
    ime_state: &mut NativeImeState,
) -> bool
where
    E: Element,
{
    let Some(owner) = ime_state.composition_owner.take() else {
        return false;
    };
    if !presenter.root().contains_id(owner) {
        return false;
    }
    let (handled, redraw_requested) =
        dispatch_text_input_event_to(presenter, owner, &TextInputCommand::CancelComposition);
    if !handled {
        log::error!("failed to cancel macOS composition after invalid IME snapshot discard");
    }
    // CancelComposition may set redraw_requested after Paint/Present; propagate it so the
    // run loop schedules another frame instead of sleeping on pre-cancellation marked text.
    handled || redraw_requested
}

fn text_input_ranges(
    snapshot: &TextInputSnapshot,
) -> Result<(Utf16TextRange, Option<Utf16TextRange>, Utf16TextRange), TextEditError> {
    let selected_range =
        Utf16TextRange::from_text_range(snapshot.text(), snapshot.selection().normalized_range())?;
    let marked_range = snapshot
        .composition()
        .map(|range| Utf16TextRange::from_text_range(snapshot.text(), range))
        .transpose()?;
    let caret_range = Utf16TextRange::from_text_range(
        snapshot.text(),
        crate::core::text_editing::TextRange::collapsed(snapshot.selection().head()),
    )?;
    Ok((selected_range, marked_range, caret_range))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::core::text_editing::{TextRange, TextSelection};
    use crate::elements::{div, input};

    #[test]
    fn snapshot_ranges_are_document_absolute_utf16_offsets() {
        let snapshot =
            TextInputSnapshot::new("a😀z", TextSelection::new(1, 5), TextRange::new(1, 6).ok());

        let (selected, marked, caret) =
            text_input_ranges(&snapshot).expect("snapshot ranges should convert");

        assert_eq!(selected, Utf16TextRange::new(1, 2).expect("valid range"));
        assert_eq!(marked, Utf16TextRange::new(1, 3).ok());
        assert_eq!(caret, Utf16TextRange::new(3, 0).expect("valid caret"));
    }

    #[test]
    fn rebuild_that_removes_the_owner_clears_stale_focus_and_composition() {
        let owner = ElementId::new();
        let viewport = Size::new(200.0, 80.0);
        let mut presenter = Presenter::with_root(viewport, div().child(input().id(owner)));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::BeginComposition("marked".to_string()),
                Some(owner)
            ),
            Some(owner)
        );

        *presenter.root_mut() = div();

        assert_eq!(
            take_lost_composition_owner(&mut presenter, &mut ime_state),
            Some(owner)
        );
        assert_eq!(presenter.focused_element(), None);
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("stale".to_string()),
                None
            ),
            None
        );
    }

    #[test]
    fn failed_begin_composition_clears_owner_and_requires_marked_text_discard() {
        let focused = ElementId::new();
        let viewport = Size::new(200.0, 80.0);
        // A focused Div rejects text-input commands, so begin returns handled=false.
        let mut presenter = Presenter::with_root(viewport, div().id(focused));
        presenter.set_focused_element(Some(focused));
        let mut ime_state = NativeImeState::default();
        let begin = TextInputCommand::BeginComposition("draft".to_string());

        let Some(target) = ime_state.target_for_event(&begin, presenter.focused_element()) else {
            panic!("begin should claim the focused element");
        };
        let (handled, redraw_requested) =
            dispatch_text_input_event_to(&mut presenter, target, &begin);
        assert!(!handled);
        assert!(!redraw_requested);
        assert!(
            clear_owner_after_rejected_begin(&mut ime_state, handled, &begin, target),
            "rejected begin must clear the owner and signal native marked-text discard"
        );
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("stale".to_string()),
                Some(focused),
            ),
            None
        );
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::CommitComposition("stale".to_string()),
                Some(focused),
            ),
            None
        );
        // Without an owner, focus-loss cancel cannot reach discard_marked_text; the failed-begin
        // path must have already requested that discard (asserted true above).
        assert_eq!(
            take_lost_composition_owner(&mut presenter, &mut ime_state),
            None
        );
    }

    #[test]
    fn failed_begin_composition_replacing_also_requires_marked_text_discard() {
        let focused = ElementId::new();
        let viewport = Size::new(200.0, 80.0);
        let mut presenter = Presenter::with_root(viewport, div().id(focused));
        presenter.set_focused_element(Some(focused));
        let mut ime_state = NativeImeState::default();
        let begin = TextInputCommand::BeginCompositionReplacing {
            text: "draft".to_string(),
            replacement_range: Utf16TextRange::new(0, 0).expect("valid range"),
        };

        let Some(target) = ime_state.target_for_event(&begin, presenter.focused_element()) else {
            panic!("begin-replacing should claim the focused element");
        };
        let (handled, _) = dispatch_text_input_event_to(&mut presenter, target, &begin);
        assert!(!handled);
        assert!(clear_owner_after_rejected_begin(
            &mut ime_state,
            handled,
            &begin,
            target
        ));
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("stale".to_string()),
                Some(focused),
            ),
            None
        );
    }

    #[test]
    fn successful_begin_composition_keeps_composition_owner_without_discard() {
        let owner = ElementId::new();
        let viewport = Size::new(200.0, 80.0);
        let mut presenter = Presenter::with_root(viewport, input().id(owner));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        let begin = TextInputCommand::BeginComposition("draft".to_string());

        let Some(target) = ime_state.target_for_event(&begin, presenter.focused_element()) else {
            panic!("begin should claim the focused input");
        };
        let (handled, _) = dispatch_text_input_event_to(&mut presenter, target, &begin);
        assert!(handled);
        assert!(!clear_owner_after_rejected_begin(
            &mut ime_state,
            handled,
            &begin,
            target
        ));
        assert_eq!(
            ime_state.target_for_event(
                &TextInputCommand::UpdateComposition("draft2".to_string()),
                Some(owner),
            ),
            Some(owner)
        );
    }

    #[test]
    fn failed_insert_replacing_preserves_owner_and_marked_text_for_recovery() {
        let owner = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        let begin = TextInputCommand::BeginComposition("e\u{301}".to_string());
        let target = ime_state
            .target_for_event(&begin, presenter.focused_element())
            .expect("begin should claim the focused input");
        assert!(dispatch_text_input_event_to(&mut presenter, target, &begin).0);
        let before = presenter.root().text_input_snapshot(owner);
        assert_eq!(
            before.as_ref().expect("input snapshot").text(),
            "helloe\u{301}"
        );
        assert!(
            before
                .as_ref()
                .expect("input snapshot")
                .composition()
                .is_some()
        );

        let insert = TextInputCommand::InsertTextReplacing {
            text: "x".to_string(),
            replacement_range: Utf16TextRange::new(5, 1).expect("valid UTF-16 range"),
        };
        let target = ime_state
            .target_for_event(&insert, presenter.focused_element())
            .expect("insert should target the focused input");
        assert_eq!(
            dispatch_text_input_event_to(&mut presenter, target, &insert),
            (false, false)
        );
        assert_eq!(ime_state.composition_owner, Some(owner));
        assert_eq!(presenter.root().text_input_snapshot(owner), before);

        let update = TextInputCommand::UpdateComposition("updated".to_string());
        let target = ime_state
            .target_for_event(&update, presenter.focused_element())
            .expect("update should retain its composition owner");
        assert!(dispatch_text_input_event_to(&mut presenter, target, &update).0);
        assert_eq!(
            presenter
                .root()
                .text_input_snapshot(owner)
                .expect("input snapshot")
                .text(),
            "helloupdated"
        );
        let cancel = TextInputCommand::CancelComposition;
        let target = ime_state
            .target_for_event(&cancel, presenter.focused_element())
            .expect("cancel should reach the composition owner");
        assert!(dispatch_text_input_event_to(&mut presenter, target, &cancel).0);
        assert_eq!(ime_state.composition_owner, None);
        let restored = presenter
            .root()
            .text_input_snapshot(owner)
            .expect("input snapshot");
        assert_eq!(restored.text(), "hello");
        assert!(restored.composition().is_none());
    }

    #[test]
    fn rejected_commit_keeps_focus_loss_cancellation_reachable() {
        let owner = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        assert!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::BeginComposition("draft".into())
            )
            .0
        );
        assert_eq!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::CommitCompositionReplacing {
                    text: "rejected".into(),
                    replacement_range: Utf16TextRange::new(100, 1).expect("range"),
                }
            ),
            (false, false, false)
        );
        presenter.set_focused_element(None);
        let lost_owner = take_lost_composition_owner(&mut presenter, &mut ime_state)
            .expect("focus loss must still find the composition owner");
        assert_eq!(lost_owner, owner);
        assert!(
            dispatch_text_input_event_to(
                &mut presenter,
                lost_owner,
                &TextInputCommand::CancelComposition
            )
            .0
        );
        assert_eq!(ime_state.composition_owner, None);
        let restored = presenter
            .root()
            .text_input_snapshot(owner)
            .expect("snapshot");
        assert_eq!(restored.text(), "hello");
        assert!(restored.composition().is_none());
    }

    #[test]
    fn rejected_begin_cancels_the_still_active_editor_composition() {
        let owner = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 80.0), input().id(owner).value("hello"));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        assert!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::BeginComposition("draft".into())
            )
            .0
        );
        assert_eq!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::CommitCompositionReplacing {
                    text: "rejected".into(),
                    replacement_range: Utf16TextRange::new(100, 1).expect("range"),
                }
            ),
            (false, false, false)
        );
        assert_eq!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::BeginComposition("next".into())
            ),
            (false, true, true)
        );
        assert_eq!(ime_state.composition_owner, None);
        let restored = presenter
            .root()
            .text_input_snapshot(owner)
            .expect("snapshot");
        assert_eq!(restored.text(), "hello");
        assert!(restored.composition().is_none());
        assert!(
            dispatch_text_input_event_for_owner(
                &mut presenter,
                &mut ime_state,
                &TextInputCommand::BeginComposition("resume".into())
            )
            .0
        );
    }

    #[test]
    fn invalid_snapshot_discard_cancels_logical_composition_owner() {
        let owner = ElementId::new();
        let viewport = Size::new(200.0, 80.0);
        let mut presenter = Presenter::with_root(viewport, div().child(input().id(owner)));
        presenter.set_focused_element(Some(owner));
        let mut ime_state = NativeImeState::default();
        let begin = TextInputCommand::BeginComposition("marked".to_string());

        let Some(target) = ime_state.target_for_event(&begin, presenter.focused_element()) else {
            panic!("begin should claim the focused input");
        };
        let (begin_handled, _) = dispatch_text_input_event_to(&mut presenter, target, &begin);
        assert!(begin_handled);
        assert_eq!(ime_state.composition_owner, Some(owner));
        assert!(
            presenter
                .root()
                .text_input_snapshot(owner)
                .and_then(|snapshot| snapshot.composition())
                .is_some()
        );

        let redraw_requested =
            cancel_logical_composition_after_native_discard(&mut presenter, &mut ime_state);

        assert!(
            redraw_requested,
            "CancelComposition after invalid snapshot must request a redraw"
        );
        assert_eq!(ime_state.composition_owner, None);
        assert!(
            presenter
                .root()
                .text_input_snapshot(owner)
                .and_then(|snapshot| snapshot.composition())
                .is_none()
        );
        let resume = TextInputCommand::BeginComposition("resume".to_string());
        let Some(resume_target) =
            ime_state.target_for_event(&resume, presenter.focused_element())
        else {
            panic!("resume begin should claim the focused input");
        };
        let (rebegin_handled, _) =
            dispatch_text_input_event_to(&mut presenter, resume_target, &resume);
        assert!(rebegin_handled);
        assert_eq!(ime_state.composition_owner, Some(owner));
    }
}
