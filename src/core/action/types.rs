use crate::core::event::KeyCode;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionId {
    Standard(StandardAction),
    Custom(String),
}

impl ActionId {
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(name.into())
    }

    pub fn requires_enabled(&self) -> bool {
        matches!(
            self,
            Self::Standard(
                StandardAction::Activate
                    | StandardAction::Submit
                    | StandardAction::InsertNewline
                    | StandardAction::DeleteBackward
                    | StandardAction::DeleteForward
            )
        )
    }
}

impl From<StandardAction> for ActionId {
    fn from(value: StandardAction) -> Self {
        Self::Standard(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardAction {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    SelectLeft,
    SelectRight,
    SelectAll,
    DeleteBackward,
    DeleteForward,
    InsertNewline,
    Activate,
    Submit,
    Cancel,
    CommandPalette,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOutcome {
    Handled { handler: String },
    Ignored,
}

impl ActionOutcome {
    pub fn handled(handler: impl Into<String>) -> Self {
        Self::Handled {
            handler: handler.into(),
        }
    }

    pub fn is_handled(&self) -> bool {
        matches!(self, Self::Handled { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    EmptyActionName,
    KeyChordConflict {
        key: KeyCode,
        existing: ActionId,
        attempted: ActionId,
    },
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyActionName => write!(f, "custom action name cannot be empty"),
            Self::KeyChordConflict {
                key,
                existing,
                attempted,
            } => write!(
                f,
                "key chord for {key:?} is already bound to {existing:?}, cannot bind {attempted:?}"
            ),
        }
    }
}

impl std::error::Error for ActionError {}
