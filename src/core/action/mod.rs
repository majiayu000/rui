//! Typed actions and keymap routing.

mod dispatch;
mod focus;
mod keymap;
mod router;
mod types;

pub(crate) use dispatch::{route_action, route_key_event};
pub(crate) use focus::{can_refocus, focus_order, scope_contains, sync_focus};
pub(crate) const ACCESSIBILITY_DECREMENT_ACTION: &str = "__rui_accessibility_decrement";
pub(crate) const ACCESSIBILITY_INCREMENT_ACTION: &str = "__rui_accessibility_increment";
pub(crate) const ACCESSIBILITY_SCROLL_BACKWARD_ACTION: &str = "__rui_accessibility_scroll_backward";
pub(crate) const ACCESSIBILITY_SCROLL_FORWARD_ACTION: &str = "__rui_accessibility_scroll_forward";
pub(crate) const SYNC_ACCESSIBILITY_FOCUS_ACTION: &str = "__rui_sync_accessibility_focus";
const ACCESSIBILITY_SET_VALUE_ACTION: &str = "__rui_accessibility_set_value:";

/// A screen reader setting a range control (slider, splitter) to `value`.
pub(crate) fn accessibility_set_value_action(value: f64) -> ActionId {
    ActionId::custom(format!("{ACCESSIBILITY_SET_VALUE_ACTION}{value}"))
}

/// The value an [`accessibility_set_value_action`] carries.
pub(crate) fn accessibility_set_value(action: &ActionId) -> Option<f64> {
    match action {
        ActionId::Custom(name) => name
            .strip_prefix(ACCESSIBILITY_SET_VALUE_ACTION)?
            .parse()
            .ok(),
        ActionId::Standard(_) => None,
    }
}
pub use keymap::{KeyChord, Keymap};
pub use router::{ActionHandler, ActionRouter};
pub use types::{ActionError, ActionId, ActionOutcome, StandardAction};
