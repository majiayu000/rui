use super::clipboard::Clipboard;
use super::error::TextEditError;
use super::types::{
    TextComposition, TextEditOutcome, TextInputEvent, TextRange, TextSelection, Utf16TextRange,
};
use crate::core::event::{KeyCode, KeyEvent};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditBuffer {
    text: String,
    selection: TextSelection,
    composition: Option<TextComposition>,
    multiline: bool,
}

impl TextEditBuffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            selection: TextSelection::collapsed(0),
            composition: None,
            multiline: false,
        }
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            text,
            selection: TextSelection::collapsed(cursor),
            composition: None,
            multiline: false,
        }
    }

    pub fn multiline() -> Self {
        Self {
            multiline: true,
            ..Self::new()
        }
    }

    pub fn multiline_with_text(text: impl Into<String>) -> Self {
        let mut buffer = Self::with_text(text);
        buffer.multiline = true;
        buffer
    }

    pub fn allow_multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.selection.head()
    }

    pub fn selection(&self) -> TextSelection {
        self.selection
    }

    pub fn selected_range(&self) -> TextRange {
        self.selection.normalized_range()
    }

    pub fn selected_text(&self) -> &str {
        let range = self.selected_range();
        &self.text[range.start()..range.end()]
    }

    pub fn composition(&self) -> Option<&TextComposition> {
        self.composition.as_ref()
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    pub fn set_cursor(&mut self, index: usize) -> Result<(), TextEditError> {
        self.ensure_offset_boundary(index)?;
        self.selection = TextSelection::collapsed(index);
        Ok(())
    }

    pub fn set_selection(&mut self, selection: TextSelection) -> Result<(), TextEditError> {
        self.ensure_valid_range(selection.normalized_range())?;
        self.selection = selection;
        Ok(())
    }

    pub fn insert_text(&mut self, text: &str) -> Result<TextEditOutcome, TextEditError> {
        self.ensure_text_allowed(text)?;
        if self.composition.is_some() {
            self.commit_current_composition()?;
        }
        let range = self.selected_range();
        self.replace_range_internal(range, text)?;
        self.selection = TextSelection::collapsed(range.start() + text.len());
        Ok(TextEditOutcome {
            changed: !text.is_empty() || !range.is_empty(),
            submitted: false,
        })
    }

    pub fn insert_text_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<TextEditOutcome, TextEditError> {
        self.ensure_text_allowed(text)?;
        let range = replacement_range.to_text_range(&self.text)?;
        if self.composition.is_some() {
            self.commit_current_composition()?;
        }
        self.replace_range_internal(range, text)?;
        self.selection = TextSelection::collapsed(range.start() + text.len());
        Ok(TextEditOutcome {
            changed: !text.is_empty() || !range.is_empty(),
            submitted: false,
        })
    }

    pub fn delete_backward(&mut self) -> Result<TextEditOutcome, TextEditError> {
        if !self.selection.is_collapsed() {
            return self.insert_text("");
        }
        let cursor = self.cursor();
        if cursor == 0 {
            return Ok(TextEditOutcome::default());
        }
        let start = self.previous_boundary(cursor);
        self.replace_range_internal(TextRange::ordered(start, cursor), "")?;
        self.selection = TextSelection::collapsed(start);
        Ok(TextEditOutcome {
            changed: true,
            submitted: false,
        })
    }

    pub fn delete_forward(&mut self) -> Result<TextEditOutcome, TextEditError> {
        if !self.selection.is_collapsed() {
            return self.insert_text("");
        }
        let cursor = self.cursor();
        if cursor == self.text.len() {
            return Ok(TextEditOutcome::default());
        }
        let end = self.next_boundary(cursor);
        self.replace_range_internal(TextRange::ordered(cursor, end), "")?;
        self.selection = TextSelection::collapsed(cursor);
        Ok(TextEditOutcome {
            changed: true,
            submitted: false,
        })
    }

    pub fn move_left(&mut self, extend: bool) -> Result<(), TextEditError> {
        let target = if !extend && !self.selection.is_collapsed() {
            self.selected_range().start()
        } else {
            self.previous_boundary(self.cursor())
        };
        self.move_to(target, extend)
    }

    pub fn move_right(&mut self, extend: bool) -> Result<(), TextEditError> {
        let target = if !extend && !self.selection.is_collapsed() {
            self.selected_range().end()
        } else {
            self.next_boundary(self.cursor())
        };
        self.move_to(target, extend)
    }

    pub fn move_word_left(&mut self, extend: bool) -> Result<(), TextEditError> {
        self.move_to(self.word_left_boundary(self.cursor()), extend)
    }

    pub fn move_word_right(&mut self, extend: bool) -> Result<(), TextEditError> {
        self.move_to(self.word_right_boundary(self.cursor()), extend)
    }

    pub fn move_line_start(&mut self, extend: bool) -> Result<(), TextEditError> {
        self.move_to(self.line_start(self.cursor()), extend)
    }

    pub fn move_line_end(&mut self, extend: bool) -> Result<(), TextEditError> {
        self.move_to(self.line_end(self.cursor()), extend)
    }

    pub fn move_up(&mut self, extend: bool) -> Result<(), TextEditError> {
        let cursor = self.cursor();
        let current_start = self.line_start(cursor);
        if current_start == 0 {
            return self.move_to(0, extend);
        }
        let column = self.char_column(current_start, cursor);
        let previous_end = current_start - 1;
        let previous_start = self.line_start(previous_end);
        let target = self.index_at_char_column(previous_start, previous_end, column);
        self.move_to(target, extend)
    }

    pub fn move_down(&mut self, extend: bool) -> Result<(), TextEditError> {
        let cursor = self.cursor();
        let current_start = self.line_start(cursor);
        let current_end = self.line_end(cursor);
        if current_end == self.text.len() {
            return self.move_to(self.text.len(), extend);
        }
        let column = self.char_column(current_start, cursor);
        let next_start = current_end + 1;
        let next_end = self.line_end(next_start);
        let target = self.index_at_char_column(next_start, next_end, column);
        self.move_to(target, extend)
    }

    pub fn begin_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        if self.composition.is_some() {
            return Err(TextEditError::CompositionActive);
        }
        self.ensure_text_allowed(text)?;
        self.begin_composition_in_range(text, self.selected_range())
    }

    pub fn begin_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        if self.composition.is_some() {
            return Err(TextEditError::CompositionActive);
        }
        self.ensure_text_allowed(text)?;
        let replacement_range = replacement_range.to_text_range(&self.text)?;
        self.begin_composition_in_range(text, replacement_range)
    }

    fn begin_composition_in_range(
        &mut self,
        text: &str,
        original_range: TextRange,
    ) -> Result<(), TextEditError> {
        let original_text = self.text[original_range.start()..original_range.end()].to_string();
        self.replace_range_internal(original_range, text)?;
        let range = TextRange::ordered(original_range.start(), original_range.start() + text.len());
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            original_range,
            original_text,
        ));
        Ok(())
    }

    pub fn update_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        let composition = self
            .composition
            .clone()
            .ok_or(TextEditError::CompositionMissing)?;
        self.replace_range_internal(composition.replacement_range(), text)?;
        let range = TextRange::ordered(
            composition.replacement_range().start(),
            composition.replacement_range().start() + text.len(),
        );
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            composition.original_replacement_range(),
            composition.original_text().to_string(),
        ));
        Ok(())
    }

    pub fn update_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        let replacement_range = replacement_range.to_text_range(&self.text)?;
        let composition = self
            .composition
            .clone()
            .ok_or(TextEditError::CompositionMissing)?;
        let (original_range, original_text) =
            if replacement_range == composition.replacement_range() {
                (
                    composition.original_replacement_range(),
                    composition.original_text().to_string(),
                )
            } else {
                (
                    replacement_range,
                    self.text[replacement_range.start()..replacement_range.end()].to_string(),
                )
            };
        self.replace_range_internal(replacement_range, text)?;
        let range = TextRange::ordered(
            replacement_range.start(),
            replacement_range.start() + text.len(),
        );
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            original_range,
            original_text,
        ));
        Ok(())
    }

    pub fn commit_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        if let Some(composition) = self.composition.take() {
            self.replace_range_internal(composition.replacement_range(), text)?;
            self.selection =
                TextSelection::collapsed(composition.replacement_range().start() + text.len());
        } else {
            let range = self.selected_range();
            self.replace_range_internal(range, text)?;
            self.selection = TextSelection::collapsed(range.start() + text.len());
        }
        Ok(())
    }

    pub fn commit_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        let replacement_range = replacement_range.to_text_range(&self.text)?;
        self.composition = None;
        self.replace_range_internal(replacement_range, text)?;
        self.selection = TextSelection::collapsed(replacement_range.start() + text.len());
        Ok(())
    }

    pub fn commit_current_composition(&mut self) -> Result<(), TextEditError> {
        let composition = self
            .composition
            .take()
            .ok_or(TextEditError::CompositionMissing)?;
        self.selection = TextSelection::collapsed(composition.replacement_range().end());
        Ok(())
    }

    pub fn cancel_composition(&mut self) -> Result<(), TextEditError> {
        let composition = self
            .composition
            .take()
            .ok_or(TextEditError::CompositionMissing)?;
        self.replace_range_internal(composition.replacement_range(), composition.original_text())?;
        let cursor =
            composition.original_replacement_range().start() + composition.original_text().len();
        self.selection = TextSelection::collapsed(cursor);
        Ok(())
    }

    pub fn apply_text_input_event(
        &mut self,
        event: TextInputEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        match event {
            TextInputEvent::InsertText(text) => self.insert_text(&text),
            TextInputEvent::InsertTextReplacing {
                text,
                replacement_range,
            } => self.insert_text_replacing_utf16(&text, replacement_range),
            TextInputEvent::BeginComposition(text) => {
                self.begin_composition(&text)?;
                Ok(changed())
            }
            TextInputEvent::BeginCompositionReplacing {
                text,
                replacement_range,
            } => {
                self.begin_composition_replacing_utf16(&text, replacement_range)?;
                Ok(changed())
            }
            TextInputEvent::UpdateComposition(text) => {
                self.update_composition(&text)?;
                Ok(changed())
            }
            TextInputEvent::UpdateCompositionReplacing {
                text,
                replacement_range,
            } => {
                self.update_composition_replacing_utf16(&text, replacement_range)?;
                Ok(changed())
            }
            TextInputEvent::CommitComposition(text) => {
                self.commit_composition(&text)?;
                Ok(changed())
            }
            TextInputEvent::CommitCompositionReplacing {
                text,
                replacement_range,
            } => {
                self.commit_composition_replacing_utf16(&text, replacement_range)?;
                Ok(changed())
            }
            TextInputEvent::CancelComposition => {
                self.cancel_composition()?;
                Ok(changed())
            }
        }
    }

    pub fn apply_key_event(&mut self, event: &KeyEvent) -> Result<TextEditOutcome, TextEditError> {
        match event.key {
            KeyCode::Backspace => self.delete_backward(),
            KeyCode::Delete => self.delete_forward(),
            KeyCode::ArrowLeft => {
                if event.modifiers.alt || event.modifiers.ctrl {
                    self.move_word_left(event.modifiers.shift)?;
                } else {
                    self.move_left(event.modifiers.shift)?;
                }
                Ok(TextEditOutcome::default())
            }
            KeyCode::ArrowRight => {
                if event.modifiers.alt || event.modifiers.ctrl {
                    self.move_word_right(event.modifiers.shift)?;
                } else {
                    self.move_right(event.modifiers.shift)?;
                }
                Ok(TextEditOutcome::default())
            }
            KeyCode::ArrowUp => {
                self.move_up(event.modifiers.shift)?;
                Ok(TextEditOutcome::default())
            }
            KeyCode::ArrowDown => {
                self.move_down(event.modifiers.shift)?;
                Ok(TextEditOutcome::default())
            }
            KeyCode::Home => {
                self.move_line_start(event.modifiers.shift)?;
                Ok(TextEditOutcome::default())
            }
            KeyCode::End => {
                self.move_line_end(event.modifiers.shift)?;
                Ok(TextEditOutcome::default())
            }
            KeyCode::Enter => {
                if self.multiline {
                    self.insert_text("\n")
                } else {
                    Ok(TextEditOutcome {
                        changed: false,
                        submitted: true,
                    })
                }
            }
            _ => self.insert_char_from_event(event),
        }
    }

    pub fn copy_selection_to<C: Clipboard>(
        &self,
        clipboard: &mut C,
    ) -> Result<bool, TextEditError> {
        if self.selection.is_collapsed() {
            return Ok(false);
        }
        clipboard.write_text(self.selected_text())?;
        Ok(true)
    }

    pub fn cut_selection_to<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<TextEditOutcome, TextEditError> {
        if !self.copy_selection_to(clipboard)? {
            return Ok(TextEditOutcome::default());
        }
        self.insert_text("")
    }

    pub fn paste_from<C: Clipboard>(
        &mut self,
        clipboard: &mut C,
    ) -> Result<TextEditOutcome, TextEditError> {
        let text = clipboard.read_text()?;
        self.insert_text(&text)
    }

    fn insert_char_from_event(
        &mut self,
        event: &KeyEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        if event.modifiers.ctrl || event.modifiers.meta {
            return Ok(TextEditOutcome::default());
        }
        if let Some(ch) = event.char
            && !ch.is_control()
        {
            return self.insert_text(&ch.to_string());
        }
        Ok(TextEditOutcome::default())
    }

    fn move_to(&mut self, index: usize, extend: bool) -> Result<(), TextEditError> {
        self.ensure_offset_boundary(index)?;
        self.selection = if extend {
            TextSelection::new(self.selection.anchor(), index)
        } else {
            TextSelection::collapsed(index)
        };
        Ok(())
    }

    fn replace_range_internal(
        &mut self,
        range: TextRange,
        text: &str,
    ) -> Result<(), TextEditError> {
        self.ensure_valid_range(range)?;
        self.ensure_text_allowed(text)?;
        self.text.replace_range(range.start()..range.end(), text);
        Ok(())
    }

    fn ensure_text_allowed(&self, text: &str) -> Result<(), TextEditError> {
        if !self.multiline && text.contains('\n') {
            return Err(TextEditError::MultilineDisabled);
        }
        Ok(())
    }

    fn ensure_valid_range(&self, range: TextRange) -> Result<(), TextEditError> {
        if range.end() > self.text.len() {
            return Err(TextEditError::InvalidRange {
                start: range.start(),
                end: range.end(),
            });
        }
        self.ensure_offset_boundary(range.start())?;
        self.ensure_offset_boundary(range.end())
    }

    fn ensure_offset_boundary(&self, index: usize) -> Result<(), TextEditError> {
        if index <= self.text.len()
            && self.text.is_char_boundary(index)
            && self.is_grapheme_boundary(index)
        {
            Ok(())
        } else {
            Err(TextEditError::InvalidBoundary { index })
        }
    }

    fn is_grapheme_boundary(&self, index: usize) -> bool {
        index == 0
            || index == self.text.len()
            || self
                .text
                .grapheme_indices(true)
                .any(|(byte_index, _)| byte_index == index)
    }

    fn previous_boundary(&self, index: usize) -> usize {
        if index == 0 {
            return 0;
        }
        self.text
            .grapheme_indices(true)
            .take_while(|(byte_index, _)| *byte_index < index)
            .last()
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(0)
    }

    fn next_boundary(&self, index: usize) -> usize {
        if index >= self.text.len() {
            return self.text.len();
        }
        self.text
            .grapheme_indices(true)
            .find(|(byte_index, _)| *byte_index > index)
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(self.text.len())
    }

    fn char_before(&self, index: usize) -> Option<char> {
        if index == 0 {
            return None;
        }
        self.text[..index].chars().next_back()
    }

    fn char_at(&self, index: usize) -> Option<char> {
        self.text[index..].chars().next()
    }

    fn word_left_boundary(&self, mut index: usize) -> usize {
        while let Some(ch) = self.char_before(index) {
            if !ch.is_whitespace() {
                break;
            }
            index = self.previous_boundary(index);
        }
        let Some(ch) = self.char_before(index) else {
            return 0;
        };
        let class = CharClass::of(ch);
        while let Some(ch) = self.char_before(index) {
            if CharClass::of(ch) != class || ch.is_whitespace() {
                break;
            }
            index = self.previous_boundary(index);
        }
        index
    }

    fn word_right_boundary(&self, mut index: usize) -> usize {
        while let Some(ch) = self.char_at(index) {
            if !ch.is_whitespace() {
                break;
            }
            index = self.next_boundary(index);
        }
        let Some(ch) = self.char_at(index) else {
            return self.text.len();
        };
        let class = CharClass::of(ch);
        while let Some(ch) = self.char_at(index) {
            if CharClass::of(ch) != class || ch.is_whitespace() {
                break;
            }
            index = self.next_boundary(index);
        }
        index
    }

    fn line_start(&self, index: usize) -> usize {
        self.text[..index]
            .rfind('\n')
            .map(|line| line + 1)
            .unwrap_or(0)
    }

    fn line_end(&self, index: usize) -> usize {
        self.text[index..]
            .find('\n')
            .map(|line| index + line)
            .unwrap_or(self.text.len())
    }

    fn char_column(&self, start: usize, index: usize) -> usize {
        self.text[start..index].graphemes(true).count()
    }

    fn index_at_char_column(&self, start: usize, end: usize, column: usize) -> usize {
        self.text[start..end]
            .grapheme_indices(true)
            .nth(column)
            .map(|(offset, _)| start + offset)
            .unwrap_or(end)
    }
}

impl Default for TextEditBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn changed() -> TextEditOutcome {
    TextEditOutcome {
        changed: true,
        submitted: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Whitespace,
    Punctuation,
}

impl CharClass {
    fn of(ch: char) -> Self {
        if ch.is_whitespace() {
            Self::Whitespace
        } else if ch.is_alphanumeric() || ch == '_' {
            Self::Word
        } else {
            Self::Punctuation
        }
    }
}
