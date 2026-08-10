use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextEditError {
    InvalidRange { start: usize, end: usize },
    InvalidBoundary { index: usize },
    InvalidUtf16Range { location: usize, length: usize },
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
            Self::InvalidUtf16Range { location, length } => {
                write!(
                    f,
                    "invalid UTF-16 text range at {location} with length {length}"
                )
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
