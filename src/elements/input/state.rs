//! What an input is and the state it edits.

use crate::core::text_editing::TextRange;

/// Input type variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    #[default]
    Text,
    Password,
    Email,
    Number,
    Search,
}

/// Input state
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub value: String,
    pub cursor_position: usize,
    pub selection_start: Option<usize>,
    pub selection_end: Option<usize>,
    pub composition_range: Option<TextRange>,
    pub marked_text: Option<String>,
    pub focused: bool,
    pub hovered: bool,
    pub disabled: bool,
    pub read_only: bool,
    pub invalid: bool,
}
