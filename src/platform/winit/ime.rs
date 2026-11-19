//! A winit IME session belongs to the editor for which the window enabled it.

use crate::core::ElementId;
use crate::core::presenter::Presenter;
use crate::core::text_editing::{TextInputCommand, TextRange, Utf16TextRange};
use crate::elements::element::Element;
use crate::platform::frame_driver::dispatch_text_input_event_to;
use winit::event::Ime;

#[derive(Clone, Copy)]
pub(super) enum ImeSession {
    Inactive,
    Waiting(ElementId),
    Active(ElementId),
}

impl ImeSession {
    pub(super) fn release_if_focus_changed<E: Element>(
        &mut self,
        presenter: &mut Presenter<E>,
        focused: Option<ElementId>,
    ) -> (bool, bool) {
        let owner = match *self {
            Self::Inactive => return (false, false),
            Self::Waiting(owner) | Self::Active(owner) => owner,
        };
        if Some(owner) == focused {
            return (false, false);
        }
        let (handled, redraw) = cancel_composition(presenter, owner);
        *self = Self::Inactive;
        (true, handled || redraw)
    }

    pub(super) fn dispatch<E: Element>(
        &mut self,
        presenter: &mut Presenter<E>,
        event: &Ime,
    ) -> (bool, bool) {
        if let Self::Waiting(owner) = *self {
            if matches!(event, Ime::Enabled) && presenter.focused_element() == Some(owner) {
                *self = Self::Active(owner);
            }
            return (false, false);
        }
        let Self::Active(owner) = *self else {
            return (false, false);
        };
        if presenter.focused_element() != Some(owner) {
            return (false, false);
        }
        let Some(snapshot) = presenter.root().text_input_snapshot(owner) else {
            return (false, false);
        };
        match event {
            Ime::Enabled => (false, false),
            Ime::Disabled => {
                *self = Self::Waiting(owner);
                cancel_composition(presenter, owner)
            }
            Ime::Preedit(text, _) if text.is_empty() => cancel_composition(presenter, owner),
            Ime::Preedit(text, selection) => {
                let command = if snapshot.composition().is_some() {
                    TextInputCommand::UpdateComposition(text.clone())
                } else {
                    TextInputCommand::BeginComposition(text.clone())
                };
                let (handled, mut redraw) =
                    dispatch_text_input_event_to(presenter, owner, &command);
                if handled
                    && let Some((start, end)) = *selection
                    && let Ok(range) = TextRange::new(start.min(end), start.max(end))
                    && let Ok(range) = Utf16TextRange::from_text_range(text, range)
                {
                    let (_, selection_redraw) = dispatch_text_input_event_to(
                        presenter,
                        owner,
                        &TextInputCommand::SetCompositionSelection(range),
                    );
                    redraw |= selection_redraw;
                }
                (handled, redraw)
            }
            Ime::Commit(text) => dispatch_text_input_event_to(
                presenter,
                owner,
                &TextInputCommand::CommitComposition(text.clone()),
            ),
        }
    }
}

fn cancel_composition<E: Element>(presenter: &mut Presenter<E>, owner: ElementId) -> (bool, bool) {
    if presenter
        .root()
        .text_input_snapshot(owner)
        .is_some_and(|snapshot| snapshot.composition().is_some())
    {
        dispatch_text_input_event_to(presenter, owner, &TextInputCommand::CancelComposition)
    } else {
        (false, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::core::text_editing::TextSelection;
    use crate::elements::{div, input};

    #[test]
    fn leaving_a_field_cancels_its_preedit_and_rejects_late_events() {
        let first = ElementId::new();
        let second = ElementId::new();
        let root = div()
            .child(input().id(first).value("first"))
            .child(input().id(second).value("second"));
        let mut presenter = Presenter::with_root(Size::new(200.0, 100.0), root);
        presenter.set_focused_element(Some(first));
        let mut session = ImeSession::Waiting(first);
        session.dispatch(&mut presenter, &Ime::Enabled);
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit("拼".into(), None))
                .0
        );
        assert_eq!(
            presenter.root().text_input_snapshot(first).unwrap().text(),
            "first拼"
        );

        presenter.set_focused_element(Some(second));
        assert!(
            !session
                .dispatch(&mut presenter, &Ime::Commit("迟".into()))
                .0,
            "a commit queued before a programmatic focus change cannot follow the new focus"
        );
        assert_eq!(
            session.release_if_focus_changed(&mut presenter, Some(second)),
            (true, true)
        );
        let old = presenter.root().text_input_snapshot(first).unwrap();
        assert_eq!(old.text(), "first");
        assert!(old.composition().is_none());
        assert_eq!(presenter.focused_element(), Some(second));

        session = ImeSession::Waiting(second);
        assert!(
            !session
                .dispatch(&mut presenter, &Ime::Preedit("音".into(), None))
                .0
        );
        assert!(
            !session
                .dispatch(&mut presenter, &Ime::Commit("拼音".into()))
                .0
        );
        session.dispatch(&mut presenter, &Ime::Disabled);
        assert_eq!(
            presenter.root().text_input_snapshot(second).unwrap().text(),
            "second"
        );

        session.dispatch(&mut presenter, &Ime::Enabled);
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Commit("新".into()))
                .0
        );
        assert_eq!(
            presenter.root().text_input_snapshot(second).unwrap().text(),
            "second新"
        );
    }

    #[test]
    fn losing_window_focus_cancels_its_preedit_before_another_session() {
        let id = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 100.0), input().id(id).value("例"));
        presenter.set_focused_element(Some(id));
        let mut session = ImeSession::Waiting(id);
        session.dispatch(&mut presenter, &Ime::Enabled);
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit("拼".into(), None))
                .0
        );

        presenter.handle_window_focus(false);
        assert_eq!(presenter.focused_element(), None);
        assert_eq!(
            session.release_if_focus_changed(&mut presenter, None),
            (true, true)
        );
        assert!(
            !session
                .dispatch(&mut presenter, &Ime::Commit("旧".into()))
                .0
        );
        let text = presenter.root().text_input_snapshot(id).unwrap();
        assert_eq!(text.text(), "例");
        assert!(text.composition().is_none());
    }

    #[test]
    fn preedit_byte_selection_is_relative_to_the_composed_text() {
        let id = ElementId::new();
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 100.0), input().id(id).value("例"));
        presenter.set_focused_element(Some(id));
        let mut session = ImeSession::Waiting(id);
        session.dispatch(&mut presenter, &Ime::Enabled);
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit("😀音".into(), Some((4, 4))))
                .0
        );
        let text = presenter.root().text_input_snapshot(id).unwrap();
        assert_eq!(text.text(), "例😀音");
        assert_eq!(text.selection(), TextSelection::collapsed(7));

        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit("😀音".into(), Some((0, 4))))
                .0
        );
        let text = presenter.root().text_input_snapshot(id).unwrap();
        assert_eq!(text.selection(), TextSelection::new(3, 7));
    }
}
