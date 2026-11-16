//! Event system for handling user interactions

use crate::core::geometry::Point;
use std::collections::HashMap;

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Keyboard modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Command on macOS, Windows key on Windows
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Default::default()
        }
    }

    pub fn meta() -> Self {
        Self {
            meta: true,
            ..Default::default()
        }
    }
}

/// Mouse event data
#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub position: Point,
    pub button: MouseButton,
    pub modifiers: Modifiers,
    pub click_count: u32,
}

impl MouseEvent {
    pub fn new(position: Point, button: MouseButton) -> Self {
        Self {
            position,
            button,
            modifiers: Modifiers::default(),
            click_count: 1,
        }
    }
}

/// Scroll event data
#[derive(Debug, Clone)]
pub struct ScrollEvent {
    pub position: Point,
    pub delta_x: f32,
    pub delta_y: f32,
    pub modifiers: Modifiers,
}

/// Key codes for keyboard events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Numbers
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,

    // Function keys (F1-F20)
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,

    // Special keys
    Escape,
    Tab,
    CapsLock,
    Shift,
    Control,
    Alt,
    Meta,
    Space,
    Enter,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,

    // Punctuation
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backquote,

    // Other
    Unknown(u32),
}

impl KeyCode {
    /// Returns true if this is a function key (F1-F20)
    pub fn is_function_key(&self) -> bool {
        matches!(
            self,
            KeyCode::F1
                | KeyCode::F2
                | KeyCode::F3
                | KeyCode::F4
                | KeyCode::F5
                | KeyCode::F6
                | KeyCode::F7
                | KeyCode::F8
                | KeyCode::F9
                | KeyCode::F10
                | KeyCode::F11
                | KeyCode::F12
                | KeyCode::F13
                | KeyCode::F14
                | KeyCode::F15
                | KeyCode::F16
                | KeyCode::F17
                | KeyCode::F18
                | KeyCode::F19
                | KeyCode::F20
        )
    }

    /// Returns the function key number (1-20) if this is a function key, None otherwise
    pub fn function_key_number(&self) -> Option<u8> {
        match self {
            KeyCode::F1 => Some(1),
            KeyCode::F2 => Some(2),
            KeyCode::F3 => Some(3),
            KeyCode::F4 => Some(4),
            KeyCode::F5 => Some(5),
            KeyCode::F6 => Some(6),
            KeyCode::F7 => Some(7),
            KeyCode::F8 => Some(8),
            KeyCode::F9 => Some(9),
            KeyCode::F10 => Some(10),
            KeyCode::F11 => Some(11),
            KeyCode::F12 => Some(12),
            KeyCode::F13 => Some(13),
            KeyCode::F14 => Some(14),
            KeyCode::F15 => Some(15),
            KeyCode::F16 => Some(16),
            KeyCode::F17 => Some(17),
            KeyCode::F18 => Some(18),
            KeyCode::F19 => Some(19),
            KeyCode::F20 => Some(20),
            _ => None,
        }
    }

    /// Creates a function key KeyCode from a number (1-20)
    /// Returns None if the number is out of range
    pub fn from_function_key_number(n: u8) -> Option<KeyCode> {
        match n {
            1 => Some(KeyCode::F1),
            2 => Some(KeyCode::F2),
            3 => Some(KeyCode::F3),
            4 => Some(KeyCode::F4),
            5 => Some(KeyCode::F5),
            6 => Some(KeyCode::F6),
            7 => Some(KeyCode::F7),
            8 => Some(KeyCode::F8),
            9 => Some(KeyCode::F9),
            10 => Some(KeyCode::F10),
            11 => Some(KeyCode::F11),
            12 => Some(KeyCode::F12),
            13 => Some(KeyCode::F13),
            14 => Some(KeyCode::F14),
            15 => Some(KeyCode::F15),
            16 => Some(KeyCode::F16),
            17 => Some(KeyCode::F17),
            18 => Some(KeyCode::F18),
            19 => Some(KeyCode::F19),
            20 => Some(KeyCode::F20),
            _ => None,
        }
    }

    /// Returns true if this is an arrow key
    pub fn is_arrow_key(&self) -> bool {
        matches!(
            self,
            KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight
        )
    }

    /// Returns true if this is a modifier key
    pub fn is_modifier_key(&self) -> bool {
        matches!(
            self,
            KeyCode::Shift | KeyCode::Control | KeyCode::Alt | KeyCode::Meta
        )
    }

    /// Returns true if this is a navigation key (arrows, home, end, page up/down)
    pub fn is_navigation_key(&self) -> bool {
        matches!(
            self,
            KeyCode::ArrowUp
                | KeyCode::ArrowDown
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
        )
    }
}

/// Keyboard event data
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub modifiers: Modifiers,
    pub is_repeat: bool,
    /// The character that would be typed, if any
    pub char: Option<char>,
}

impl KeyEvent {
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            key,
            modifiers,
            is_repeat: false,
            char: None,
        }
    }

    pub fn with_char(mut self, c: char) -> Self {
        self.char = Some(c);
        self
    }

    /// Convert to a Key struct for easy boolean-based key detection
    pub fn to_key(&self) -> Key {
        Key::from_key_event(self)
    }
}

/// A struct for easy boolean-based key detection
///
/// This provides a convenient way to check which key was pressed
/// using boolean fields, similar to ink/tink style input handling.
///
/// # Example
/// ```
/// use rui::core::event::{KeyEvent, KeyCode, Modifiers, Key};
///
/// let event = KeyEvent::new(KeyCode::Home, Modifiers::ctrl());
/// let key = event.to_key();
///
/// if key.home && key.ctrl {
///     // Handle Ctrl+Home
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Key {
    // Navigation keys
    pub up_arrow: bool,
    pub down_arrow: bool,
    pub left_arrow: bool,
    pub right_arrow: bool,
    pub page_up: bool,
    pub page_down: bool,
    pub home: bool,
    pub end: bool,

    // Editing keys
    pub insert: bool,
    pub delete: bool,
    pub backspace: bool,

    // Action keys
    pub enter: bool,
    pub escape: bool,
    pub tab: bool,
    pub space: bool,

    // Function keys (F1-F20)
    pub f1: bool,
    pub f2: bool,
    pub f3: bool,
    pub f4: bool,
    pub f5: bool,
    pub f6: bool,
    pub f7: bool,
    pub f8: bool,
    pub f9: bool,
    pub f10: bool,
    pub f11: bool,
    pub f12: bool,
    pub f13: bool,
    pub f14: bool,
    pub f15: bool,
    pub f16: bool,
    pub f17: bool,
    pub f18: bool,
    pub f19: bool,
    pub f20: bool,

    // Modifier states
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Key {
    /// Create a new Key with all fields set to false
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a Key from a KeyEvent
    pub fn from_key_event(event: &KeyEvent) -> Self {
        let mut key = Self::new();

        // Set modifier states
        key.shift = event.modifiers.shift;
        key.ctrl = event.modifiers.ctrl;
        key.alt = event.modifiers.alt;
        key.meta = event.modifiers.meta;

        // Set key states based on KeyCode
        match event.key {
            KeyCode::ArrowUp => key.up_arrow = true,
            KeyCode::ArrowDown => key.down_arrow = true,
            KeyCode::ArrowLeft => key.left_arrow = true,
            KeyCode::ArrowRight => key.right_arrow = true,
            KeyCode::PageUp => key.page_up = true,
            KeyCode::PageDown => key.page_down = true,
            KeyCode::Home => key.home = true,
            KeyCode::End => key.end = true,
            KeyCode::Insert => key.insert = true,
            KeyCode::Delete => key.delete = true,
            KeyCode::Backspace => key.backspace = true,
            KeyCode::Enter => key.enter = true,
            KeyCode::Escape => key.escape = true,
            KeyCode::Tab => key.tab = true,
            KeyCode::Space => key.space = true,
            // Function keys F1-F20
            KeyCode::F1 => key.f1 = true,
            KeyCode::F2 => key.f2 = true,
            KeyCode::F3 => key.f3 = true,
            KeyCode::F4 => key.f4 = true,
            KeyCode::F5 => key.f5 = true,
            KeyCode::F6 => key.f6 = true,
            KeyCode::F7 => key.f7 = true,
            KeyCode::F8 => key.f8 = true,
            KeyCode::F9 => key.f9 = true,
            KeyCode::F10 => key.f10 = true,
            KeyCode::F11 => key.f11 = true,
            KeyCode::F12 => key.f12 = true,
            KeyCode::F13 => key.f13 = true,
            KeyCode::F14 => key.f14 = true,
            KeyCode::F15 => key.f15 = true,
            KeyCode::F16 => key.f16 = true,
            KeyCode::F17 => key.f17 = true,
            KeyCode::F18 => key.f18 = true,
            KeyCode::F19 => key.f19 = true,
            KeyCode::F20 => key.f20 = true,
            _ => {}
        }

        key
    }

    /// Create a Key from a KeyCode with no modifiers
    pub fn from_key_code(code: KeyCode) -> Self {
        Self::from_key_event(&KeyEvent::new(code, Modifiers::none()))
    }

    /// Create a Key from a KeyCode with specified modifiers
    pub fn from_key_code_with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self::from_key_event(&KeyEvent::new(code, modifiers))
    }

    /// Check if this is a navigation key (arrows, home, end, page up/down)
    pub fn is_navigation(&self) -> bool {
        self.up_arrow
            || self.down_arrow
            || self.left_arrow
            || self.right_arrow
            || self.home
            || self.end
            || self.page_up
            || self.page_down
    }

    /// Check if this is an editing key (insert, delete, backspace)
    pub fn is_editing(&self) -> bool {
        self.insert || self.delete || self.backspace
    }

    /// Check if any modifier is pressed
    pub fn has_modifier(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    /// Check if this is a function key (F1-F20)
    pub fn is_function_key(&self) -> bool {
        self.f1
            || self.f2
            || self.f3
            || self.f4
            || self.f5
            || self.f6
            || self.f7
            || self.f8
            || self.f9
            || self.f10
            || self.f11
            || self.f12
            || self.f13
            || self.f14
            || self.f15
            || self.f16
            || self.f17
            || self.f18
            || self.f19
            || self.f20
    }

    /// Returns the function key number (1-20) if a function key is pressed, None otherwise
    pub fn function_key_number(&self) -> Option<u8> {
        if self.f1 {
            Some(1)
        } else if self.f2 {
            Some(2)
        } else if self.f3 {
            Some(3)
        } else if self.f4 {
            Some(4)
        } else if self.f5 {
            Some(5)
        } else if self.f6 {
            Some(6)
        } else if self.f7 {
            Some(7)
        } else if self.f8 {
            Some(8)
        } else if self.f9 {
            Some(9)
        } else if self.f10 {
            Some(10)
        } else if self.f11 {
            Some(11)
        } else if self.f12 {
            Some(12)
        } else if self.f13 {
            Some(13)
        } else if self.f14 {
            Some(14)
        } else if self.f15 {
            Some(15)
        } else if self.f16 {
            Some(16)
        } else if self.f17 {
            Some(17)
        } else if self.f18 {
            Some(18)
        } else if self.f19 {
            Some(19)
        } else if self.f20 {
            Some(20)
        } else {
            None
        }
    }
}

/// Focus event data
#[derive(Debug, Clone)]
pub struct FocusEvent {
    pub focused: bool,
}

/// All possible events
#[derive(Debug, Clone)]
pub enum Event {
    // Mouse events
    MouseDown(MouseEvent),
    MouseUp(MouseEvent),
    MouseMove(MouseEvent),
    MouseEnter(MouseEvent),
    MouseLeave(MouseEvent),
    Click(MouseEvent),
    DoubleClick(MouseEvent),
    RightClick(MouseEvent),

    // Scroll events
    Scroll(ScrollEvent),

    // Keyboard events
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),

    // Focus events
    Focus(FocusEvent),
    Blur(FocusEvent),

    // Window events
    WindowResize { width: f32, height: f32 },
    WindowClose,
}

/// Event handler type
pub type EventHandler<T> = Box<dyn Fn(&T) + 'static>;

/// Event listener ID for removal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId(pub(crate) u64);

/// Manages event subscriptions
pub struct EventEmitter<T> {
    listeners: HashMap<ListenerId, EventHandler<T>>,
    next_id: u64,
}

impl<T> EventEmitter<T> {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn subscribe(&mut self, handler: impl Fn(&T) + 'static) -> ListenerId {
        let id = ListenerId(self.next_id);
        self.next_id += 1;
        self.listeners.insert(id, Box::new(handler));
        id
    }

    pub fn unsubscribe(&mut self, id: ListenerId) {
        self.listeners.remove(&id);
    }

    pub fn emit(&self, event: &T) {
        for handler in self.listeners.values() {
            handler(event);
        }
    }

    pub fn clear(&mut self) {
        self.listeners.clear();
    }
}

impl<T> Default for EventEmitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Hit testing result
#[derive(Debug, Clone)]
pub struct HitTestResult {
    pub element_id: crate::core::ElementId,
    pub local_position: Point,
}

/// Cursor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    NotAllowed,
    Grab,
    Grabbing,
    ResizeNS,
    ResizeEW,
    ResizeNESW,
    ResizeNWSE,
    Wait,
    Progress,
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
