use super::error::TextEditError;

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
