//! The general pasteboard as the app clipboard.

use crate::core::text_editing::{Clipboard, ClipboardError};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

/// Plain text on `NSPasteboard::generalPasteboard`, shared with other apps.
#[derive(Debug, Default)]
pub(crate) struct MacPasteboard;

impl Clipboard for MacPasteboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        Ok(pasteboard
            .stringForType(unsafe { NSPasteboardTypeString })
            .map(|value| value.to_string()))
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        let value = NSString::from_str(text);
        if pasteboard.setString_forType(&value, unsafe { NSPasteboardTypeString }) {
            Ok(())
        } else {
            Err(ClipboardError::WriteFailed {
                message: String::from("the general pasteboard rejected the text"),
            })
        }
    }
}
