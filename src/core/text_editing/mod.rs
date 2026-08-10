//! Core text editing state for input controls.

mod buffer;
mod clipboard;
mod error;
mod layout;
mod types;

pub use buffer::TextEditBuffer;
pub use clipboard::{Clipboard, MemoryClipboard};
pub use error::{ClipboardError, TextEditError, Utf16TextRangeError};
pub(crate) use layout::VisualCaret;
pub use layout::{
    CaretGeometry, SelectionRect, TextEditLayout, TextEditPaintStyle, TextInputGeometry, TextLine,
};
pub use types::{
    TextComposition, TextEditOutcome, TextInputCommand, TextInputEvent, TextInputSnapshot,
    TextRange, TextSelection, Utf16TextRange,
};
