//! Typed actions and keymap routing.

mod keymap;
mod router;
mod types;

pub use keymap::{KeyChord, Keymap};
pub use router::{ActionHandler, ActionRouter};
pub use types::{ActionError, ActionId, ActionOutcome, StandardAction};
