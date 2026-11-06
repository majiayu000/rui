//! The app clipboard: plain text shared with key actions and listeners.

use super::AppContext;
use crate::core::text_editing::{Clipboard, ClipboardError, ClipboardItem};
use std::cell::RefCell;
use std::rc::Rc;

/// The clipboard shared by the app and the event contexts that key actions run in.
pub(crate) type SharedClipboard = Rc<RefCell<dyn Clipboard>>;

impl AppContext {
    /// Replace the clipboard that copy, cut, paste and
    /// [`Self::write_to_clipboard`] use. The macOS runner installs the general
    /// pasteboard; headless sessions keep an in-memory clipboard.
    pub fn set_clipboard(&mut self, clipboard: impl Clipboard + 'static) {
        self.clipboard = Rc::new(RefCell::new(clipboard));
    }

    pub(crate) fn clipboard(&self) -> SharedClipboard {
        Rc::clone(&self.clipboard)
    }

    pub fn write_to_clipboard(&self, item: ClipboardItem) -> Result<(), ClipboardError> {
        self.clipboard.borrow_mut().write_text(item.text())
    }

    /// The clipboard's text, or `None` when it holds no text.
    pub fn read_from_clipboard(&self) -> Result<Option<ClipboardItem>, ClipboardError> {
        Ok(self
            .clipboard
            .borrow_mut()
            .read_text()?
            .map(ClipboardItem::new_string))
    }
}
