use super::types::{ActionError, ActionId, StandardAction};
use crate::core::event::{KeyCode, KeyEvent, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    key: KeyCode,
    modifiers: Modifiers,
}

impl KeyChord {
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn key(&self) -> KeyCode {
        self.key
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl From<&KeyEvent> for KeyChord {
    fn from(value: &KeyEvent) -> Self {
        Self::new(value.key, value.modifiers)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: Vec<(KeyChord, ActionId)>,
}

impl Keymap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_standard_bindings() -> Result<Self, ActionError> {
        let mut keymap = Self::new();
        keymap.bind(
            KeyCode::ArrowLeft,
            Modifiers::none(),
            StandardAction::MoveLeft,
        )?;
        keymap.bind(
            KeyCode::ArrowRight,
            Modifiers::none(),
            StandardAction::MoveRight,
        )?;
        keymap.bind(KeyCode::ArrowUp, Modifiers::none(), StandardAction::MoveUp)?;
        keymap.bind(
            KeyCode::ArrowDown,
            Modifiers::none(),
            StandardAction::MoveDown,
        )?;
        keymap.bind(
            KeyCode::ArrowLeft,
            Modifiers::shift(),
            StandardAction::SelectLeft,
        )?;
        keymap.bind(
            KeyCode::ArrowRight,
            Modifiers::shift(),
            StandardAction::SelectRight,
        )?;
        keymap.bind(
            KeyCode::ArrowLeft,
            Modifiers::alt(),
            StandardAction::MoveWordLeft,
        )?;
        keymap.bind(
            KeyCode::ArrowRight,
            Modifiers::alt(),
            StandardAction::MoveWordRight,
        )?;
        keymap.bind(
            KeyCode::Backspace,
            Modifiers::none(),
            StandardAction::DeleteBackward,
        )?;
        keymap.bind(
            KeyCode::Delete,
            Modifiers::none(),
            StandardAction::DeleteForward,
        )?;
        keymap.bind(KeyCode::Enter, Modifiers::none(), StandardAction::Activate)?;
        keymap.bind(KeyCode::Escape, Modifiers::none(), StandardAction::Cancel)?;
        keymap.bind(KeyCode::A, Modifiers::meta(), StandardAction::SelectAll)?;
        keymap.bind(
            KeyCode::P,
            Modifiers::meta(),
            StandardAction::CommandPalette,
        )?;
        Ok(keymap)
    }

    pub fn bind(
        &mut self,
        key: KeyCode,
        modifiers: Modifiers,
        action: impl Into<ActionId>,
    ) -> Result<(), ActionError> {
        self.bind_chord(KeyChord::new(key, modifiers), action)
    }

    pub fn bind_chord(
        &mut self,
        chord: KeyChord,
        action: impl Into<ActionId>,
    ) -> Result<(), ActionError> {
        let action = validate_action(action.into())?;
        if let Some((_, existing)) = self.bindings.iter().find(|(bound, _)| *bound == chord) {
            return Err(ActionError::KeyChordConflict {
                key: chord.key,
                existing: existing.clone(),
                attempted: action,
            });
        }
        self.bindings.push((chord, action));
        Ok(())
    }

    pub fn action_for_chord(&self, chord: KeyChord) -> Option<&ActionId> {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == chord)
            .map(|(_, action)| action)
    }

    pub fn action_for_event(&self, event: &KeyEvent) -> Option<&ActionId> {
        self.action_for_chord(KeyChord::from(event))
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

fn validate_action(action: ActionId) -> Result<ActionId, ActionError> {
    match &action {
        ActionId::Custom(name) if name.trim().is_empty() => Err(ActionError::EmptyActionName),
        _ => Ok(action),
    }
}
