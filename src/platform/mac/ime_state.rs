//! Logical ownership and targeted dispatch for macOS text input sessions.

use crate::core::ElementId;
use crate::core::presenter::Presenter;
use crate::core::text_editing::{TextEditError, TextInputEvent, TextInputSnapshot, Utf16TextRange};
use crate::elements::element::Element;
use crate::platform::mac::window::MacWindow;

#[derive(Debug, Default)]
pub(crate) struct NativeImeState {
    composition_owner: Option<ElementId>,
}

impl NativeImeState {
    pub(crate) fn target_for_event(
        &mut self,
        event: &TextInputEvent,
        focused: Option<ElementId>,
    ) -> Option<ElementId> {
        match event {
            TextInputEvent::InsertText(_) | TextInputEvent::InsertTextReplacing { .. } => focused,
            TextInputEvent::BeginComposition(_)
            | TextInputEvent::BeginCompositionReplacing { .. } => {
                self.composition_owner = focused;
                focused
            }
            TextInputEvent::UpdateComposition(_)
            | TextInputEvent::UpdateCompositionReplacing { .. }
            | TextInputEvent::SetCompositionSelection(_) => self.composition_owner,
            TextInputEvent::CommitComposition(_)
            | TextInputEvent::CommitCompositionReplacing { .. }
            | TextInputEvent::CancelComposition => self.composition_owner.take(),
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
    ime_state: &mut NativeImeState,
    event: &TextInputEvent,
) -> (bool, bool)
where
    E: Element,
{
    let Some(target) = ime_state.target_for_event(event, presenter.focused_element()) else {
        log::error!("discarded macOS text input event without a focused composition owner");
        return (false, false);
    };
    if !presenter.root().contains_id(target) {
        ime_state.cancel_owner_after_focus_change(presenter.focused_element(), false);
        log::error!("discarded macOS text input event for a removed element");
        return (false, false);
    }
    dispatch_text_input_event_to(presenter, target, event)
}

fn dispatch_text_input_event_to<E>(
    presenter: &mut Presenter<E>,
    target: ElementId,
    event: &TextInputEvent,
) -> (bool, bool)
where
    E: Element,
{
    let focused = presenter.focused_element();
    *presenter.focused_element_mut() = Some(target);
    let result = presenter
        .with_event_context(|root, event_cx| root.handle_text_input_event(event_cx, event));
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
        dispatch_text_input_event_to(presenter, owner, &TextInputEvent::CancelComposition);
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

pub(crate) fn sync_text_input_snapshot<E>(
    presenter: &Presenter<E>,
    window: &MacWindow,
) -> Result<(), TextEditError>
where
    E: Element,
{
    let Some(focused) = presenter.focused_element() else {
        window
            .content_view
            .update_text_input_state(None, None, None, None);
        return Ok(());
    };
    let Some(snapshot) = presenter.root().text_input_snapshot(focused) else {
        window
            .content_view
            .update_text_input_state(None, None, None, None);
        return Ok(());
    };
    let (selected_range, marked_range, caret_range) = text_input_ranges(&snapshot)?;
    window.content_view.update_text_input_state(
        Some(selected_range),
        marked_range,
        Some(caret_range),
        snapshot.caret_bounds(),
    );
    Ok(())
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
                &TextInputEvent::BeginComposition("marked".to_string()),
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
                &TextInputEvent::UpdateComposition("stale".to_string()),
                None
            ),
            None
        );
    }
}
