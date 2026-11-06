use super::error::ClipboardError;

/// A system or in-memory clipboard holding plain text.
///
/// `read_text` returns `Ok(None)` when the clipboard holds no text, which is an
/// ordinary state (it may be empty or hold an image), not a failure.
pub trait Clipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// What [`crate::core::app::AppContext::write_to_clipboard`] writes and
/// [`crate::core::app::AppContext::read_from_clipboard`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardItem {
    text: String,
}

impl ClipboardItem {
    pub fn new_string(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
    text: Option<String>,
    read_error: Option<ClipboardError>,
    write_error: Option<ClipboardError>,
}

impl MemoryClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            read_error: None,
            write_error: None,
        }
    }

    pub fn with_read_error(message: impl Into<String>) -> Self {
        Self {
            text: None,
            read_error: Some(ClipboardError::ReadFailed {
                message: message.into(),
            }),
            write_error: None,
        }
    }

    pub fn with_write_error(message: impl Into<String>) -> Self {
        Self {
            text: None,
            read_error: None,
            write_error: Some(ClipboardError::WriteFailed {
                message: message.into(),
            }),
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

impl Clipboard for MemoryClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        if let Some(err) = &self.read_error {
            return Err(err.clone());
        }
        Ok(self.text.clone())
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if let Some(err) = &self.write_error {
            return Err(err.clone());
        }
        self.text = Some(text.to_owned());
        Ok(())
    }
}
