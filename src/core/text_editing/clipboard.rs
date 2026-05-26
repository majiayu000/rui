use super::error::ClipboardError;

pub trait Clipboard {
    fn read_text(&mut self) -> Result<String, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
    text: String,
    read_error: Option<ClipboardError>,
    write_error: Option<ClipboardError>,
}

impl MemoryClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            read_error: None,
            write_error: None,
        }
    }

    pub fn with_read_error(message: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            read_error: Some(ClipboardError::ReadFailed {
                message: message.into(),
            }),
            write_error: None,
        }
    }

    pub fn with_write_error(message: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            read_error: None,
            write_error: Some(ClipboardError::WriteFailed {
                message: message.into(),
            }),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Clipboard for MemoryClipboard {
    fn read_text(&mut self) -> Result<String, ClipboardError> {
        if let Some(err) = &self.read_error {
            return Err(err.clone());
        }
        Ok(self.text.clone())
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if let Some(err) = &self.write_error {
            return Err(err.clone());
        }
        self.text.clear();
        self.text.push_str(text);
        Ok(())
    }
}
