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
        Self { shift: true, ..Default::default() }
    }

    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Default::default() }
    }

    pub fn alt() -> Self {
        Self { alt: true, ..Default::default() }
    }

    pub fn meta() -> Self {
        Self { meta: true, ..Default::default() }
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
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // Numbers
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // Special keys
    Escape, Tab, CapsLock, Shift, Control, Alt, Meta,
    Space, Enter, Backspace, Delete,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Home, End, PageUp, PageDown,
    Insert,

    // Punctuation
    Minus, Equal, BracketLeft, BracketRight, Backslash,
    Semicolon, Quote, Comma, Period, Slash, Backquote,

    // Other
    Unknown(u32),
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
