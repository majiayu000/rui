use super::error::TextEditError;
use super::layout::TextInputGeometry;
use crate::core::geometry::Bounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    start: usize,
    end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Result<Self, TextEditError> {
        if start > end {
            return Err(TextEditError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn collapsed(index: usize) -> Self {
        Self {
            start: index,
            end: index,
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub(crate) fn ordered(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utf16TextRange {
    location: usize,
    length: usize,
}

impl Utf16TextRange {
    pub fn new(location: usize, length: usize) -> Result<Self, TextEditError> {
        location
            .checked_add(length)
            .ok_or(TextEditError::InvalidUtf16Range { location, length })?;
        Ok(Self { location, length })
    }

    pub fn location(self) -> usize {
        self.location
    }

    pub fn length(self) -> usize {
        self.length
    }

    pub fn to_text_range(self, text: &str) -> Result<TextRange, TextEditError> {
        let end =
            self.location
                .checked_add(self.length)
                .ok_or(TextEditError::InvalidUtf16Range {
                    location: self.location,
                    length: self.length,
                })?;
        let start_byte =
            utf16_offset_to_byte(text, self.location).ok_or(TextEditError::InvalidUtf16Range {
                location: self.location,
                length: self.length,
            })?;
        let end_byte = utf16_offset_to_byte(text, end).ok_or(TextEditError::InvalidUtf16Range {
            location: self.location,
            length: self.length,
        })?;
        Ok(TextRange::ordered(start_byte, end_byte))
    }

    pub fn from_text_range(text: &str, range: TextRange) -> Result<Self, TextEditError> {
        if range.end() > text.len() {
            return Err(TextEditError::InvalidRange {
                start: range.start(),
                end: range.end(),
            });
        }
        if !text.is_char_boundary(range.start()) {
            return Err(TextEditError::InvalidBoundary {
                index: range.start(),
            });
        }
        if !text.is_char_boundary(range.end()) {
            return Err(TextEditError::InvalidBoundary { index: range.end() });
        }
        let location = text[..range.start()].encode_utf16().count();
        let length = text[range.start()..range.end()].encode_utf16().count();
        Self::new(location, length)
    }
}

fn utf16_offset_to_byte(text: &str, utf16_offset: usize) -> Option<usize> {
    if utf16_offset == 0 {
        return Some(0);
    }
    let mut utf16_position = 0;
    for (byte_index, ch) in text.char_indices() {
        if utf16_position == utf16_offset {
            return Some(byte_index);
        }
        utf16_position += ch.len_utf16();
        if utf16_position > utf16_offset {
            return None;
        }
    }
    (utf16_position == utf16_offset).then_some(text.len())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelection {
    anchor: usize,
    head: usize,
}

impl TextSelection {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn collapsed(index: usize) -> Self {
        Self {
            anchor: index,
            head: index,
        }
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn head(&self) -> usize {
        self.head
    }

    pub fn normalized_range(&self) -> TextRange {
        if self.anchor <= self.head {
            TextRange::ordered(self.anchor, self.head)
        } else {
            TextRange::ordered(self.head, self.anchor)
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextComposition {
    range: TextRange,
    text: String,
    original_range: TextRange,
    original_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextInputSnapshot {
    text: String,
    selection: TextSelection,
    composition: Option<TextRange>,
    caret_bounds: Option<Bounds>,
    geometry: Option<TextInputGeometry>,
}

impl TextInputSnapshot {
    pub fn new(
        text: impl Into<String>,
        selection: TextSelection,
        composition: Option<TextRange>,
    ) -> Self {
        Self {
            text: text.into(),
            selection,
            composition,
            caret_bounds: None,
            geometry: None,
        }
    }

    pub fn with_caret_bounds(mut self, caret_bounds: Option<Bounds>) -> Self {
        self.caret_bounds = caret_bounds;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn selection(&self) -> TextSelection {
        self.selection
    }

    pub fn composition(&self) -> Option<TextRange> {
        self.composition
    }

    pub fn caret_bounds(&self) -> Option<Bounds> {
        self.caret_bounds
    }

    pub fn with_geometry(mut self, geometry: Option<TextInputGeometry>) -> Self {
        self.geometry = geometry;
        self
    }

    pub fn geometry(&self) -> Option<&TextInputGeometry> {
        self.geometry.as_ref()
    }
}

impl TextComposition {
    pub fn replacement_range(&self) -> TextRange {
        self.range
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn new(
        range: TextRange,
        text: String,
        original_range: TextRange,
        original_text: String,
    ) -> Self {
        Self {
            range,
            text,
            original_range,
            original_text,
        }
    }

    pub(crate) fn original_replacement_range(&self) -> TextRange {
        self.original_range
    }

    pub(crate) fn original_text(&self) -> &str {
        &self.original_text
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextEditOutcome {
    pub changed: bool,
    pub submitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputEvent {
    InsertText(String),
    BeginComposition(String),
    UpdateComposition(String),
    CommitComposition(String),
    CancelComposition,
}

/// Internal-compatible command channel for platform text input extensions.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputCommand {
    InsertText(String),
    InsertTextReplacing {
        text: String,
        replacement_range: Utf16TextRange,
    },
    BeginComposition(String),
    BeginCompositionReplacing {
        text: String,
        replacement_range: Utf16TextRange,
    },
    UpdateComposition(String),
    UpdateCompositionReplacing {
        text: String,
        replacement_range: Utf16TextRange,
    },
    CommitComposition(String),
    CommitCompositionReplacing {
        text: String,
        replacement_range: Utf16TextRange,
    },
    SetCompositionSelection(Utf16TextRange),
    CancelComposition,
}

impl TextInputCommand {
    pub fn into_legacy_event(self) -> Option<TextInputEvent> {
        match self {
            Self::InsertText(text) | Self::InsertTextReplacing { text, .. } => {
                Some(TextInputEvent::InsertText(text))
            }
            Self::BeginComposition(text) | Self::BeginCompositionReplacing { text, .. } => {
                Some(TextInputEvent::BeginComposition(text))
            }
            Self::UpdateComposition(text) | Self::UpdateCompositionReplacing { text, .. } => {
                Some(TextInputEvent::UpdateComposition(text))
            }
            Self::CommitComposition(text) | Self::CommitCompositionReplacing { text, .. } => {
                Some(TextInputEvent::CommitComposition(text))
            }
            Self::SetCompositionSelection(_) => None,
            Self::CancelComposition => Some(TextInputEvent::CancelComposition),
        }
    }
}

impl From<TextInputEvent> for TextInputCommand {
    fn from(event: TextInputEvent) -> Self {
        match event {
            TextInputEvent::InsertText(text) => Self::InsertText(text),
            TextInputEvent::BeginComposition(text) => Self::BeginComposition(text),
            TextInputEvent::UpdateComposition(text) => Self::UpdateComposition(text),
            TextInputEvent::CommitComposition(text) => Self::CommitComposition(text),
            TextInputEvent::CancelComposition => Self::CancelComposition,
        }
    }
}
