//! The system clipboard through arboard: X11 or Wayland on Linux, Windows, and
//! the general pasteboard on macOS.

use super::window::{PLATFORM, backend_error};
use crate::core::app::{AppContext, SharedClipboard};
use crate::core::text_editing::{Clipboard, ClipboardError};
use crate::platform::window::{PlatformWindowError, PlatformWindowFeature};

/// Plain text on the system clipboard, shared with other apps.
struct SystemClipboard(arboard::Clipboard);

impl Clipboard for SystemClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        text_or_none(self.0.get_text())
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.0
            .set_text(text)
            .map_err(|err| ClipboardError::WriteFailed {
                message: err.to_string(),
            })
    }
}

/// arboard reports a clipboard that holds no text, being empty or holding an
/// image, as an error; [`Clipboard::read_text`] reports it as `Ok(None)`.
fn text_or_none(text: Result<String, arboard::Error>) -> Result<Option<String>, ClipboardError> {
    match text {
        Ok(text) => Ok(Some(text)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(err) => Err(ClipboardError::ReadFailed {
            message: err.to_string(),
        }),
    }
}

/// Make the system clipboard the one that copy, cut and paste use, and return
/// it for the windows. When it cannot be opened (on Linux: no X server and no
/// Wayland compositor with the data-control protocol), `context` keeps its
/// in-memory clipboard, which copies and pastes only inside the app.
pub(super) fn install_system_clipboard(context: &mut AppContext) -> WindowClipboard {
    match arboard::Clipboard::new() {
        Ok(clipboard) => {
            context.set_clipboard(SystemClipboard(clipboard));
            WindowClipboard(Some(context.clipboard()))
        }
        Err(err) => {
            log::error!(
                "the system clipboard is unavailable, so copy and paste work only inside this app: {err}"
            );
            WindowClipboard(None)
        }
    }
}

/// The system clipboard behind a window's `PlatformWindow` clipboard methods,
/// when the runner could open it.
#[derive(Clone)]
pub(super) struct WindowClipboard(Option<SharedClipboard>);

impl WindowClipboard {
    pub(super) fn is_available(&self) -> bool {
        self.0.is_some()
    }

    pub(super) fn read_text(&self) -> Result<String, PlatformWindowError> {
        self.with_clipboard(|clipboard| clipboard.read_text())?
            .ok_or_else(|| backend_error("the system clipboard holds no text"))
    }

    pub(super) fn write_text(&self, text: &str) -> Result<(), PlatformWindowError> {
        self.with_clipboard(|clipboard| clipboard.write_text(text))
    }

    fn with_clipboard<T>(
        &self,
        operation: impl FnOnce(&mut dyn Clipboard) -> Result<T, ClipboardError>,
    ) -> Result<T, PlatformWindowError> {
        let Some(clipboard) = &self.0 else {
            return Err(PlatformWindowError::unsupported(
                PLATFORM,
                PlatformWindowFeature::Clipboard,
            ));
        };
        let mut clipboard = clipboard
            .try_borrow_mut()
            .map_err(|_| backend_error("the clipboard is in use"))?;
        operation(&mut *clipboard).map_err(backend_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text_editing::MemoryClipboard;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn a_clipboard_without_text_reads_as_none() {
        assert_eq!(
            text_or_none(Err(arboard::Error::ContentNotAvailable)),
            Ok(None)
        );
        assert_eq!(
            text_or_none(Ok("copied".to_owned())),
            Ok(Some("copied".to_owned()))
        );
    }

    #[test]
    fn other_arboard_failures_are_read_errors() {
        for (error, detail) in [
            (arboard::Error::ClipboardOccupied, "held by another party"),
            (
                arboard::Error::Unknown {
                    description: "no reply".to_owned(),
                },
                "no reply",
            ),
        ] {
            match text_or_none(Err(error)) {
                Err(ClipboardError::ReadFailed { message }) => {
                    assert!(message.contains(detail), "{message}")
                }
                other => panic!("expected a read error with {detail:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_window_without_the_system_clipboard_reports_it_unsupported() {
        let clipboard = WindowClipboard(None);
        let unsupported =
            PlatformWindowError::unsupported("winit", PlatformWindowFeature::Clipboard);

        assert!(!clipboard.is_available());
        assert_eq!(clipboard.read_text(), Err(unsupported.clone()));
        assert_eq!(clipboard.write_text("copied"), Err(unsupported));
    }

    #[test]
    fn a_window_reads_and_writes_the_clipboard_it_was_given() {
        let shared: SharedClipboard = Rc::new(RefCell::new(MemoryClipboard::new()));
        let clipboard = WindowClipboard(Some(Rc::clone(&shared)));

        assert!(clipboard.is_available());
        assert_eq!(
            clipboard.read_text(),
            Err(PlatformWindowError::backend(
                "winit",
                "the system clipboard holds no text"
            ))
        );
        assert_eq!(clipboard.write_text("copied"), Ok(()));
        assert_eq!(clipboard.read_text(), Ok("copied".to_owned()));
        assert_eq!(
            shared.borrow_mut().read_text(),
            Ok(Some("copied".to_owned()))
        );
    }
}
