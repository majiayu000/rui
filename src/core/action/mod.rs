//! Typed actions and keymap routing.

mod dispatch;
mod keymap;
mod router;
mod types;

pub(crate) use dispatch::route_key_event;
pub use keymap::{KeyChord, Keymap};
pub use router::{ActionHandler, ActionRouter};
pub use types::{ActionError, ActionId, ActionOutcome, StandardAction};
