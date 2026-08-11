use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextEditError {
    InvalidRange { start: usize, end: usize },
    InvalidBoundary { index: usize },
    CompositionMissing,
    CompositionActive,
    MultilineDisabled,
    Clipboard(ClipboardError),
}

impl fmt::Display for TextEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { start, end } => {
                write!(f, "invalid text range {start}..{end}")
            }
            Self::InvalidBoundary { index } => {
                write!(f, "index {index} is not a UTF-8 character boundary")
            }
            Self::CompositionMissing => write!(f, "no active text composition"),
            Self::CompositionActive => write!(f, "text composition is already active"),
            Self::MultilineDisabled => write!(f, "multiline editing is disabled"),
            Self::Clipboard(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TextEditError {}

impl From<ClipboardError> for TextEditError {
    fn from(value: ClipboardError) -> Self {
        Self::Clipboard(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utf16TextRangeError {
    location: usize,
    length: usize,
}

impl Utf16TextRangeError {
    pub(crate) fn new(location: usize, length: usize) -> Self {
        Self { location, length }
    }

    pub fn location(self) -> usize {
        self.location
    }

    pub fn length(self) -> usize {
        self.length
    }
}

impl fmt::Display for Utf16TextRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid UTF-16 text range at {} with length {}",
            self.location, self.length
        )
    }
}

impl std::error::Error for Utf16TextRangeError {}

impl From<Utf16TextRangeError> for TextEditError {
    fn from(value: Utf16TextRangeError) -> Self {
        Self::InvalidRange {
            start: value.location,
            end: value.location.saturating_add(value.length),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    Unavailable { message: String },
    ReadFailed { message: String },
    WriteFailed { message: String },
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { message } => write!(f, "clipboard unavailable: {message}"),
            Self::ReadFailed { message } => write!(f, "clipboard read failed: {message}"),
            Self::WriteFailed { message } => write!(f, "clipboard write failed: {message}"),
        }
    }
}

impl std::error::Error for ClipboardError {}
