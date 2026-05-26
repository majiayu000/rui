//! Core text editing state for input controls.

mod buffer;
mod clipboard;
mod error;
mod layout;
mod types;

pub use buffer::TextEditBuffer;
pub use clipboard::{Clipboard, MemoryClipboard};
pub use error::{ClipboardError, TextEditError};
pub use layout::{CaretGeometry, SelectionRect, TextEditLayout, TextLine};
pub use types::{TextComposition, TextEditOutcome, TextInputEvent, TextRange, TextSelection};
