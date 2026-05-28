mod basics;
mod builders;
mod callbacks;
mod presentation;
mod scenarios;
mod state_and_edges;

mod support {
    pub(super) use super::super::*;
    pub(super) use crate::core::event::{KeyCode, KeyEvent, Modifiers};
    pub(super) use std::cell::RefCell;
    pub(super) use std::rc::Rc;
    pub(super) use taffy::prelude::TaffyTree;

    pub(super) fn focused_context<'a>(
        taffy: &'a TaffyTree<ElementId>,
        focused: &'a mut Option<ElementId>,
    ) -> EventContext<'a> {
        EventContext::new(Bounds::from_xywh(0.0, 0.0, 200.0, 40.0), taffy, focused)
    }

    pub(super) fn typed_char_event(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Unknown(ch as u32), Modifiers::none()).with_char(ch)
    }

    pub(super) fn key_event(key: KeyCode) -> KeyEvent {
        KeyEvent::new(key, Modifiers::none())
    }
}
