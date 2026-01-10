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

    // Function keys (F1-F20)
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20,

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
        self.up_arrow || self.down_arrow || self.left_arrow || self.right_arrow
            || self.home || self.end || self.page_up || self.page_down
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
        self.f1 || self.f2 || self.f3 || self.f4 || self.f5
            || self.f6 || self.f7 || self.f8 || self.f9 || self.f10
            || self.f11 || self.f12 || self.f13 || self.f14 || self.f15
            || self.f16 || self.f17 || self.f18 || self.f19 || self.f20
    }

    /// Returns the function key number (1-20) if a function key is pressed, None otherwise
    pub fn function_key_number(&self) -> Option<u8> {
        if self.f1 { Some(1) }
        else if self.f2 { Some(2) }
        else if self.f3 { Some(3) }
        else if self.f4 { Some(4) }
        else if self.f5 { Some(5) }
        else if self.f6 { Some(6) }
        else if self.f7 { Some(7) }
        else if self.f8 { Some(8) }
        else if self.f9 { Some(9) }
        else if self.f10 { Some(10) }
        else if self.f11 { Some(11) }
        else if self.f12 { Some(12) }
        else if self.f13 { Some(13) }
        else if self.f14 { Some(14) }
        else if self.f15 { Some(15) }
        else if self.f16 { Some(16) }
        else if self.f17 { Some(17) }
        else if self.f18 { Some(18) }
        else if self.f19 { Some(19) }
        else if self.f20 { Some(20) }
        else { None }
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
mod tests {
    use super::*;

    // ==================== Page Up Key Detection Tests ====================

    #[test]
    fn test_page_up_key_detection() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true for PageUp key");
        assert!(!key.page_down, "page_down should be false");
        assert!(!key.up_arrow, "up_arrow should be false");
        assert!(!key.down_arrow, "down_arrow should be false");
    }

    #[test]
    fn test_page_up_with_ctrl() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(!key.shift, "shift should be false");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_up_with_shift() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::shift());
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.shift, "shift should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_up_with_alt() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::alt());
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.alt, "alt should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.shift, "shift should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_up_with_meta() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::meta());
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.meta, "meta should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.shift, "shift should be false");
        assert!(!key.alt, "alt should be false");
    }

    #[test]
    fn test_page_up_with_ctrl_shift() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: false,
            meta: false,
        };
        let event = KeyEvent::new(KeyCode::PageUp, modifiers);
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(key.shift, "shift should be true");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_up_with_all_modifiers() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: true,
            meta: true,
        };
        let event = KeyEvent::new(KeyCode::PageUp, modifiers);
        let key = Key::from_key_event(&event);

        assert!(key.page_up, "page_up should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(key.shift, "shift should be true");
        assert!(key.alt, "alt should be true");
        assert!(key.meta, "meta should be true");
    }

    // ==================== Page Down Key Detection Tests ====================

    #[test]
    fn test_page_down_key_detection() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true for PageDown key");
        assert!(!key.page_up, "page_up should be false");
        assert!(!key.up_arrow, "up_arrow should be false");
        assert!(!key.down_arrow, "down_arrow should be false");
    }

    #[test]
    fn test_page_down_with_ctrl() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::ctrl());
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(!key.shift, "shift should be false");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_down_with_shift() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.shift, "shift should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_down_with_alt() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::alt());
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.alt, "alt should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.shift, "shift should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_down_with_meta() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::meta());
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.meta, "meta should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.shift, "shift should be false");
        assert!(!key.alt, "alt should be false");
    }

    #[test]
    fn test_page_down_with_ctrl_shift() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: false,
            meta: false,
        };
        let event = KeyEvent::new(KeyCode::PageDown, modifiers);
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(key.shift, "shift should be true");
        assert!(!key.alt, "alt should be false");
        assert!(!key.meta, "meta should be false");
    }

    #[test]
    fn test_page_down_with_alt_meta() {
        let modifiers = Modifiers {
            shift: false,
            ctrl: false,
            alt: true,
            meta: true,
        };
        let event = KeyEvent::new(KeyCode::PageDown, modifiers);
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(!key.ctrl, "ctrl should be false");
        assert!(!key.shift, "shift should be false");
        assert!(key.alt, "alt should be true");
        assert!(key.meta, "meta should be true");
    }

    #[test]
    fn test_page_down_with_all_modifiers() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: true,
            meta: true,
        };
        let event = KeyEvent::new(KeyCode::PageDown, modifiers);
        let key = Key::from_key_event(&event);

        assert!(key.page_down, "page_down should be true");
        assert!(key.ctrl, "ctrl should be true");
        assert!(key.shift, "shift should be true");
        assert!(key.alt, "alt should be true");
        assert!(key.meta, "meta should be true");
    }

    // ==================== Navigation Helper Method Tests ====================

    #[test]
    fn test_page_up_is_navigation() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(key.is_navigation(), "PageUp should be a navigation key");
    }

    #[test]
    fn test_page_down_is_navigation() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(key.is_navigation(), "PageDown should be a navigation key");
    }

    #[test]
    fn test_page_up_has_modifier_false() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(!key.has_modifier(), "has_modifier should be false without modifiers");
    }

    #[test]
    fn test_page_up_has_modifier_true() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());
        let key = Key::from_key_event(&event);

        assert!(key.has_modifier(), "has_modifier should be true with ctrl");
    }

    #[test]
    fn test_page_down_has_modifier_false() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
        let key = Key::from_key_event(&event);

        assert!(!key.has_modifier(), "has_modifier should be false without modifiers");
    }

    #[test]
    fn test_page_down_has_modifier_true() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());
        let key = Key::from_key_event(&event);

        assert!(key.has_modifier(), "has_modifier should be true with shift");
    }

    // ==================== KeyCode Navigation Tests ====================

    #[test]
    fn test_keycode_page_up_is_navigation() {
        assert!(KeyCode::PageUp.is_navigation_key(), "PageUp should be a navigation key");
    }

    #[test]
    fn test_keycode_page_down_is_navigation() {
        assert!(KeyCode::PageDown.is_navigation_key(), "PageDown should be a navigation key");
    }

    #[test]
    fn test_keycode_page_up_not_arrow() {
        assert!(!KeyCode::PageUp.is_arrow_key(), "PageUp should not be an arrow key");
    }

    #[test]
    fn test_keycode_page_down_not_arrow() {
        assert!(!KeyCode::PageDown.is_arrow_key(), "PageDown should not be an arrow key");
    }

    #[test]
    fn test_keycode_page_up_not_modifier() {
        assert!(!KeyCode::PageUp.is_modifier_key(), "PageUp should not be a modifier key");
    }

    #[test]
    fn test_keycode_page_down_not_modifier() {
        assert!(!KeyCode::PageDown.is_modifier_key(), "PageDown should not be a modifier key");
    }

    #[test]
    fn test_keycode_page_up_not_function() {
        assert!(!KeyCode::PageUp.is_function_key(), "PageUp should not be a function key");
    }

    #[test]
    fn test_keycode_page_down_not_function() {
        assert!(!KeyCode::PageDown.is_function_key(), "PageDown should not be a function key");
    }

    // ==================== Key Construction Tests ====================

    #[test]
    fn test_key_from_key_code_page_up() {
        let key = Key::from_key_code(KeyCode::PageUp);

        assert!(key.page_up, "page_up should be true");
        assert!(!key.page_down, "page_down should be false");
        assert!(!key.has_modifier(), "should have no modifiers");
    }

    #[test]
    fn test_key_from_key_code_page_down() {
        let key = Key::from_key_code(KeyCode::PageDown);

        assert!(key.page_down, "page_down should be true");
        assert!(!key.page_up, "page_up should be false");
        assert!(!key.has_modifier(), "should have no modifiers");
    }

    #[test]
    fn test_key_from_key_code_with_modifiers_page_up() {
        let key = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());

        assert!(key.page_up, "page_up should be true");
        assert!(key.ctrl, "ctrl should be true");
    }

    #[test]
    fn test_key_from_key_code_with_modifiers_page_down() {
        let key = Key::from_key_code_with_modifiers(KeyCode::PageDown, Modifiers::shift());

        assert!(key.page_down, "page_down should be true");
        assert!(key.shift, "shift should be true");
    }

    #[test]
    fn test_key_event_to_key_page_up() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::alt());
        let key = event.to_key();

        assert!(key.page_up, "page_up should be true");
        assert!(key.alt, "alt should be true");
    }

    #[test]
    fn test_key_event_to_key_page_down() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::meta());
        let key = event.to_key();

        assert!(key.page_down, "page_down should be true");
        assert!(key.meta, "meta should be true");
    }

    // ==================== Key Default and New Tests ====================

    #[test]
    fn test_key_default_page_up_false() {
        let key = Key::default();
        assert!(!key.page_up, "page_up should be false by default");
    }

    #[test]
    fn test_key_default_page_down_false() {
        let key = Key::default();
        assert!(!key.page_down, "page_down should be false by default");
    }

    #[test]
    fn test_key_new_page_up_false() {
        let key = Key::new();
        assert!(!key.page_up, "page_up should be false for new Key");
    }

    #[test]
    fn test_key_new_page_down_false() {
        let key = Key::new();
        assert!(!key.page_down, "page_down should be false for new Key");
    }

    // ==================== Equality Tests ====================

    #[test]
    fn test_key_equality_page_up() {
        let key1 = Key::from_key_code(KeyCode::PageUp);
        let key2 = Key::from_key_code(KeyCode::PageUp);

        assert_eq!(key1, key2, "Two PageUp keys should be equal");
    }

    #[test]
    fn test_key_equality_page_down() {
        let key1 = Key::from_key_code(KeyCode::PageDown);
        let key2 = Key::from_key_code(KeyCode::PageDown);

        assert_eq!(key1, key2, "Two PageDown keys should be equal");
    }

    #[test]
    fn test_key_inequality_page_up_vs_page_down() {
        let key1 = Key::from_key_code(KeyCode::PageUp);
        let key2 = Key::from_key_code(KeyCode::PageDown);

        assert_ne!(key1, key2, "PageUp and PageDown keys should not be equal");
    }

    #[test]
    fn test_key_inequality_same_key_different_modifier() {
        let key1 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::none());
        let key2 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());

        assert_ne!(key1, key2, "Same key with different modifiers should not be equal");
    }

    #[test]
    fn test_keycode_equality_page_up() {
        assert_eq!(KeyCode::PageUp, KeyCode::PageUp);
    }

    #[test]
    fn test_keycode_equality_page_down() {
        assert_eq!(KeyCode::PageDown, KeyCode::PageDown);
    }

    #[test]
    fn test_keycode_inequality_page_up_vs_page_down() {
        assert_ne!(KeyCode::PageUp, KeyCode::PageDown);
    }

    // ==================== Clone and Copy Tests ====================

    #[test]
    fn test_key_clone_page_up() {
        let key1 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());
        let key2 = key1.clone();

        assert_eq!(key1, key2);
        assert!(key2.page_up);
        assert!(key2.ctrl);
    }

    #[test]
    fn test_key_clone_page_down() {
        let key1 = Key::from_key_code_with_modifiers(KeyCode::PageDown, Modifiers::shift());
        let key2 = key1.clone();

        assert_eq!(key1, key2);
        assert!(key2.page_down);
        assert!(key2.shift);
    }

    #[test]
    fn test_key_copy_page_up() {
        let key1 = Key::from_key_code(KeyCode::PageUp);
        let key2 = key1; // Copy

        assert!(key1.page_up);
        assert!(key2.page_up);
    }

    #[test]
    fn test_key_copy_page_down() {
        let key1 = Key::from_key_code(KeyCode::PageDown);
        let key2 = key1; // Copy

        assert!(key1.page_down);
        assert!(key2.page_down);
    }

    #[test]
    fn test_keycode_clone_page_up() {
        let code1 = KeyCode::PageUp;
        let code2 = code1.clone();

        assert_eq!(code1, code2);
    }

    #[test]
    fn test_keycode_clone_page_down() {
        let code1 = KeyCode::PageDown;
        let code2 = code1.clone();

        assert_eq!(code1, code2);
    }

    // ==================== Debug Trait Tests ====================

    #[test]
    fn test_key_debug_page_up() {
        let key = Key::from_key_code(KeyCode::PageUp);
        let debug_str = format!("{:?}", key);

        assert!(debug_str.contains("page_up: true"), "Debug should show page_up: true");
        assert!(debug_str.contains("page_down: false"), "Debug should show page_down: false");
    }

    #[test]
    fn test_key_debug_page_down() {
        let key = Key::from_key_code(KeyCode::PageDown);
        let debug_str = format!("{:?}", key);

        assert!(debug_str.contains("page_down: true"), "Debug should show page_down: true");
        assert!(debug_str.contains("page_up: false"), "Debug should show page_up: false");
    }

    #[test]
    fn test_keycode_debug_page_up() {
        let debug_str = format!("{:?}", KeyCode::PageUp);
        assert_eq!(debug_str, "PageUp");
    }

    #[test]
    fn test_keycode_debug_page_down() {
        let debug_str = format!("{:?}", KeyCode::PageDown);
        assert_eq!(debug_str, "PageDown");
    }

    // ==================== Hash Trait Tests for KeyCode ====================

    #[test]
    fn test_keycode_hash_page_up() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(KeyCode::PageUp);

        assert!(set.contains(&KeyCode::PageUp));
        assert!(!set.contains(&KeyCode::PageDown));
    }

    #[test]
    fn test_keycode_hash_page_down() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(KeyCode::PageDown);

        assert!(set.contains(&KeyCode::PageDown));
        assert!(!set.contains(&KeyCode::PageUp));
    }

    // ==================== Modifiers Tests ====================

    #[test]
    fn test_modifiers_none() {
        let m = Modifiers::none();
        assert!(!m.shift);
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn test_modifiers_shift() {
        let m = Modifiers::shift();
        assert!(m.shift);
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn test_modifiers_ctrl() {
        let m = Modifiers::ctrl();
        assert!(!m.shift);
        assert!(m.ctrl);
        assert!(!m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn test_modifiers_alt() {
        let m = Modifiers::alt();
        assert!(!m.shift);
        assert!(!m.ctrl);
        assert!(m.alt);
        assert!(!m.meta);
    }

    #[test]
    fn test_modifiers_meta() {
        let m = Modifiers::meta();
        assert!(!m.shift);
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(m.meta);
    }

    // ==================== Other Navigation Keys (for comparison) ====================

    #[test]
    fn test_arrow_keys_not_page_keys() {
        let up = Key::from_key_code(KeyCode::ArrowUp);
        let down = Key::from_key_code(KeyCode::ArrowDown);

        assert!(!up.page_up, "ArrowUp should not set page_up");
        assert!(!up.page_down, "ArrowUp should not set page_down");
        assert!(!down.page_up, "ArrowDown should not set page_up");
        assert!(!down.page_down, "ArrowDown should not set page_down");
    }

    #[test]
    fn test_home_end_not_page_keys() {
        let home = Key::from_key_code(KeyCode::Home);
        let end = Key::from_key_code(KeyCode::End);

        assert!(!home.page_up, "Home should not set page_up");
        assert!(!home.page_down, "Home should not set page_down");
        assert!(!end.page_up, "End should not set page_up");
        assert!(!end.page_down, "End should not set page_down");
    }

    #[test]
    fn test_page_keys_not_other_navigation() {
        let page_up = Key::from_key_code(KeyCode::PageUp);
        let page_down = Key::from_key_code(KeyCode::PageDown);

        assert!(!page_up.up_arrow, "PageUp should not set up_arrow");
        assert!(!page_up.down_arrow, "PageUp should not set down_arrow");
        assert!(!page_up.home, "PageUp should not set home");
        assert!(!page_up.end, "PageUp should not set end");

        assert!(!page_down.up_arrow, "PageDown should not set up_arrow");
        assert!(!page_down.down_arrow, "PageDown should not set down_arrow");
        assert!(!page_down.home, "PageDown should not set home");
        assert!(!page_down.end, "PageDown should not set end");
    }

    // ==================== Is Editing Tests ====================

    #[test]
    fn test_page_up_not_editing() {
        let key = Key::from_key_code(KeyCode::PageUp);
        assert!(!key.is_editing(), "PageUp should not be an editing key");
    }

    #[test]
    fn test_page_down_not_editing() {
        let key = Key::from_key_code(KeyCode::PageDown);
        assert!(!key.is_editing(), "PageDown should not be an editing key");
    }

    // ==================== Is Function Key Tests ====================

    #[test]
    fn test_page_up_not_function_key() {
        let key = Key::from_key_code(KeyCode::PageUp);
        assert!(!key.is_function_key(), "PageUp should not be a function key");
    }

    #[test]
    fn test_page_down_not_function_key() {
        let key = Key::from_key_code(KeyCode::PageDown);
        assert!(!key.is_function_key(), "PageDown should not be a function key");
    }

    // ==================== KeyEvent Tests ====================

    #[test]
    fn test_key_event_new_page_up() {
        let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());

        assert_eq!(event.key, KeyCode::PageUp);
        assert!(event.modifiers.ctrl);
        assert!(!event.is_repeat);
        assert!(event.char.is_none());
    }

    #[test]
    fn test_key_event_new_page_down() {
        let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());

        assert_eq!(event.key, KeyCode::PageDown);
        assert!(event.modifiers.shift);
        assert!(!event.is_repeat);
        assert!(event.char.is_none());
    }

    #[test]
    fn test_key_event_with_char() {
        let event = KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a');

        assert_eq!(event.key, KeyCode::A);
        assert_eq!(event.char, Some('a'));
    }
}
