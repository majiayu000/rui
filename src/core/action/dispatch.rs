use super::focus::move_focus;
use crate::core::action::{ActionId, StandardAction};
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
        if route_action(root, app, cx, &action) {
            return true;
        }
        return root.handle_key_event(cx, event);
    }

    if key_event_has_text_editing_details(event) {
        return root.handle_key_event(cx, event);
    }

    false
}

/// Run `action` on the focused element, then the app's action handlers, as a
/// key binding or a menu item does. Returns whether one of them handled it.
pub(crate) fn route_action<E: Element>(
    root: &mut E,
    app: &mut AppContext,
    cx: &mut EventContext,
    action: &ActionId,
) -> bool {
    cx.set_clipboard(app.clipboard());
    if root.dispatch_action(cx, action).is_handled() || app.dispatch_app_action(action).is_handled()
    {
        return true;
    }
    match action {
        ActionId::Standard(StandardAction::FocusNext) => move_focus(root, cx, true),
        ActionId::Standard(StandardAction::FocusPrevious) => move_focus(root, cx, false),
        _ => false,
    }
}

fn key_event_has_text_editing_details(event: &KeyEvent) -> bool {
    event.key.is_navigation_key()
        || matches!(event.key, KeyCode::Backspace | KeyCode::Delete)
        || matches!(event.key, KeyCode::Enter)
        || event
            .char
            .is_some_and(|ch| !ch.is_control() || matches!(ch, '\n' | '\r'))
}
