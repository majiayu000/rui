//! Typed actions and keymap routing.

mod dispatch;
mod keymap;
mod router;
mod types;

pub(crate) use dispatch::route_key_event;
pub(crate) const ACCESSIBILITY_SCROLL_BACKWARD_ACTION: &str = "__rui_accessibility_scroll_backward";
pub(crate) const ACCESSIBILITY_SCROLL_FORWARD_ACTION: &str = "__rui_accessibility_scroll_forward";
pub(crate) const SYNC_ACCESSIBILITY_FOCUS_ACTION: &str = "__rui_sync_accessibility_focus";
pub use keymap::{KeyChord, Keymap};
pub use router::{ActionHandler, ActionRouter};
pub use types::{ActionError, ActionId, ActionOutcome, StandardAction};
