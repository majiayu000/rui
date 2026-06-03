use crate::core::app::AppContext;
use crate::core::event::{KeyCode, KeyEvent};
use crate::elements::Element;
use crate::elements::element::EventContext;

pub(crate) fn route_key_event<E: Element>(
    root: &mut E,
    app: &mut AppContext,
    cx: &mut EventContext,
    event: &KeyEvent,
) -> bool {
    let action = app.keymap().action_for_event(event).cloned();
    if let Some(action) = action {
        let outcome = root.dispatch_action(cx, &action);
        if outcome.is_handled() {
            return true;
        }

        let outcome = app.dispatch_app_action(&action);
        if outcome.is_handled() {
            return true;
        }

        return root.handle_key_event(cx, event);
    }

    if key_event_has_text_editing_details(event) {
        return root.handle_key_event(cx, event);
    }

    false
}

fn key_event_has_text_editing_details(event: &KeyEvent) -> bool {
    event.key.is_navigation_key()
        || matches!(
            event.key,
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter
        )
        || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
}
