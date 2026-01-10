//! Mouse hook for terminal mouse event handling
//!
//! This module provides a React-like hook for handling terminal mouse events
//! in a declarative way, supporting various mouse actions like clicks, drags,
//! and scrolling.

use crate::core::event::Modifiers;
use std::collections::HashMap;

/// Mouse button types for terminal events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerminalMouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (scroll wheel click)
    Middle,
    /// No button (for move events)
    #[default]
    None,
}

impl TerminalMouseButton {
    /// Returns true if this is the left button
    pub fn is_left(&self) -> bool {
        matches!(self, TerminalMouseButton::Left)
    }

    /// Returns true if this is the right button
    pub fn is_right(&self) -> bool {
        matches!(self, TerminalMouseButton::Right)
    }

    /// Returns true if this is the middle button
    pub fn is_middle(&self) -> bool {
        matches!(self, TerminalMouseButton::Middle)
    }

    /// Returns true if no button is pressed
    pub fn is_none(&self) -> bool {
        matches!(self, TerminalMouseButton::None)
    }
}

/// Kind of terminal mouse event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalMouseEventKind {
    /// Mouse button pressed down
    Press,
    /// Mouse button released
    Release,
    /// Mouse dragged while button held
    Drag,
    /// Mouse moved (without button pressed)
    Move,
    /// Scroll wheel up
    ScrollUp,
    /// Scroll wheel down
    ScrollDown,
}

impl TerminalMouseEventKind {
    /// Returns true if this is a press event
    pub fn is_press(&self) -> bool {
        matches!(self, TerminalMouseEventKind::Press)
    }

    /// Returns true if this is a release event
    pub fn is_release(&self) -> bool {
        matches!(self, TerminalMouseEventKind::Release)
    }

    /// Returns true if this is a drag event
    pub fn is_drag(&self) -> bool {
        matches!(self, TerminalMouseEventKind::Drag)
    }

    /// Returns true if this is a move event
    pub fn is_move(&self) -> bool {
        matches!(self, TerminalMouseEventKind::Move)
    }

    /// Returns true if this is a scroll up event
    pub fn is_scroll_up(&self) -> bool {
        matches!(self, TerminalMouseEventKind::ScrollUp)
    }

    /// Returns true if this is a scroll down event
    pub fn is_scroll_down(&self) -> bool {
        matches!(self, TerminalMouseEventKind::ScrollDown)
    }

    /// Returns true if this is any scroll event
    pub fn is_scroll(&self) -> bool {
        matches!(
            self,
            TerminalMouseEventKind::ScrollUp | TerminalMouseEventKind::ScrollDown
        )
    }
}

/// A terminal mouse event with position, button, and modifier information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMouseEvent {
    /// The kind of mouse event
    pub kind: TerminalMouseEventKind,
    /// The mouse button involved (if any)
    pub button: TerminalMouseButton,
    /// X coordinate (column) in terminal cells
    pub x: u16,
    /// Y coordinate (row) in terminal cells
    pub y: u16,
    /// Keyboard modifiers held during the event
    pub modifiers: Modifiers,
}

impl TerminalMouseEvent {
    /// Create a new terminal mouse event
    pub fn new(
        kind: TerminalMouseEventKind,
        button: TerminalMouseButton,
        x: u16,
        y: u16,
    ) -> Self {
        Self {
            kind,
            button,
            x,
            y,
            modifiers: Modifiers::default(),
        }
    }

    /// Create a new terminal mouse event with modifiers
    pub fn with_modifiers(
        kind: TerminalMouseEventKind,
        button: TerminalMouseButton,
        x: u16,
        y: u16,
        modifiers: Modifiers,
    ) -> Self {
        Self {
            kind,
            button,
            x,
            y,
            modifiers,
        }
    }

    /// Create a press event
    pub fn press(button: TerminalMouseButton, x: u16, y: u16) -> Self {
        Self::new(TerminalMouseEventKind::Press, button, x, y)
    }

    /// Create a release event
    pub fn release(button: TerminalMouseButton, x: u16, y: u16) -> Self {
        Self::new(TerminalMouseEventKind::Release, button, x, y)
    }

    /// Create a drag event
    pub fn drag(button: TerminalMouseButton, x: u16, y: u16) -> Self {
        Self::new(TerminalMouseEventKind::Drag, button, x, y)
    }

    /// Create a move event
    pub fn move_event(x: u16, y: u16) -> Self {
        Self::new(TerminalMouseEventKind::Move, TerminalMouseButton::None, x, y)
    }

    /// Create a scroll up event
    pub fn scroll_up(x: u16, y: u16) -> Self {
        Self::new(
            TerminalMouseEventKind::ScrollUp,
            TerminalMouseButton::None,
            x,
            y,
        )
    }

    /// Create a scroll down event
    pub fn scroll_down(x: u16, y: u16) -> Self {
        Self::new(
            TerminalMouseEventKind::ScrollDown,
            TerminalMouseButton::None,
            x,
            y,
        )
    }

    /// Add modifiers to this event
    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Add ctrl modifier to this event
    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Add alt modifier to this event
    pub fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Add meta modifier to this event
    pub fn with_meta(mut self) -> Self {
        self.modifiers.meta = true;
        self
    }

    /// Returns true if shift is held
    pub fn has_shift(&self) -> bool {
        self.modifiers.shift
    }

    /// Returns true if ctrl is held
    pub fn has_ctrl(&self) -> bool {
        self.modifiers.ctrl
    }

    /// Returns true if alt is held
    pub fn has_alt(&self) -> bool {
        self.modifiers.alt
    }

    /// Returns true if meta is held
    pub fn has_meta(&self) -> bool {
        self.modifiers.meta
    }

    /// Returns true if any modifier is held
    pub fn has_any_modifier(&self) -> bool {
        self.modifiers.shift || self.modifiers.ctrl || self.modifiers.alt || self.modifiers.meta
    }

    /// Get the position as a tuple
    pub fn position(&self) -> (u16, u16) {
        (self.x, self.y)
    }
}

/// Callback ID for tracking registered callbacks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseCallbackId(u64);

/// Mouse event callback type
pub type MouseCallback = Box<dyn Fn(&TerminalMouseEvent) + 'static>;

/// A hook for managing terminal mouse events
///
/// This provides a React-like hook pattern for handling mouse events
/// in terminal applications.
///
/// # Example
/// ```ignore
/// let mut mouse_hook = UseMouse::new();
///
/// // Register a callback for all mouse events
/// let id = mouse_hook.on_mouse(|event| {
///     println!("Mouse event: {:?}", event);
/// });
///
/// // Register a callback for specific event kinds
/// mouse_hook.on_click(|event| {
///     println!("Click at ({}, {})", event.x, event.y);
/// });
///
/// // Process an event
/// let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 5);
/// mouse_hook.handle_event(&event);
/// ```
pub struct UseMouse {
    /// All registered callbacks
    callbacks: HashMap<MouseCallbackId, MouseCallback>,
    /// Next callback ID
    next_id: u64,
    /// Callbacks for specific event kinds
    kind_callbacks: HashMap<TerminalMouseEventKind, Vec<MouseCallbackId>>,
    /// Callbacks for specific buttons
    button_callbacks: HashMap<TerminalMouseButton, Vec<MouseCallbackId>>,
    /// Whether mouse tracking is enabled
    enabled: bool,
}

impl UseMouse {
    /// Create a new mouse hook
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
            next_id: 0,
            kind_callbacks: HashMap::new(),
            button_callbacks: HashMap::new(),
            enabled: true,
        }
    }

    /// Enable mouse tracking
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable mouse tracking
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if mouse tracking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Register a callback for all mouse events
    pub fn on_mouse(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        let id = MouseCallbackId(self.next_id);
        self.next_id += 1;
        self.callbacks.insert(id, Box::new(callback));
        id
    }

    /// Register a callback for a specific event kind
    pub fn on_kind(
        &mut self,
        kind: TerminalMouseEventKind,
        callback: impl Fn(&TerminalMouseEvent) + 'static,
    ) -> MouseCallbackId {
        let id = self.on_mouse(callback);
        self.kind_callbacks.entry(kind).or_default().push(id);
        id
    }

    /// Register a callback for a specific button
    pub fn on_button(
        &mut self,
        button: TerminalMouseButton,
        callback: impl Fn(&TerminalMouseEvent) + 'static,
    ) -> MouseCallbackId {
        let id = self.on_mouse(callback);
        self.button_callbacks.entry(button).or_default().push(id);
        id
    }

    /// Register a callback for press events
    pub fn on_press(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::Press, callback)
    }

    /// Register a callback for release events
    pub fn on_release(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::Release, callback)
    }

    /// Register a callback for click events (press then release at same position)
    pub fn on_click(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::Press, callback)
    }

    /// Register a callback for drag events
    pub fn on_drag(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::Drag, callback)
    }

    /// Register a callback for move events
    pub fn on_move(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::Move, callback)
    }

    /// Register a callback for scroll events (both up and down)
    pub fn on_scroll(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        // We need to register for both scroll up and scroll down
        let id = self.on_mouse(callback);
        self.kind_callbacks
            .entry(TerminalMouseEventKind::ScrollUp)
            .or_default()
            .push(id);
        self.kind_callbacks
            .entry(TerminalMouseEventKind::ScrollDown)
            .or_default()
            .push(id);
        id
    }

    /// Register a callback for scroll up events
    pub fn on_scroll_up(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::ScrollUp, callback)
    }

    /// Register a callback for scroll down events
    pub fn on_scroll_down(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_kind(TerminalMouseEventKind::ScrollDown, callback)
    }

    /// Register a callback for left button events
    pub fn on_left_button(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_button(TerminalMouseButton::Left, callback)
    }

    /// Register a callback for right button events
    pub fn on_right_button(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_button(TerminalMouseButton::Right, callback)
    }

    /// Register a callback for middle button events
    pub fn on_middle_button(&mut self, callback: impl Fn(&TerminalMouseEvent) + 'static) -> MouseCallbackId {
        self.on_button(TerminalMouseButton::Middle, callback)
    }

    /// Unregister a callback by ID
    pub fn off(&mut self, id: MouseCallbackId) {
        self.callbacks.remove(&id);

        // Remove from kind callbacks
        for callbacks in self.kind_callbacks.values_mut() {
            callbacks.retain(|&callback_id| callback_id != id);
        }

        // Remove from button callbacks
        for callbacks in self.button_callbacks.values_mut() {
            callbacks.retain(|&callback_id| callback_id != id);
        }
    }

    /// Clear all callbacks
    pub fn clear(&mut self) {
        self.callbacks.clear();
        self.kind_callbacks.clear();
        self.button_callbacks.clear();
    }

    /// Handle a mouse event, invoking all registered callbacks
    pub fn handle_event(&self, event: &TerminalMouseEvent) {
        if !self.enabled {
            return;
        }

        // Check if this event matches any kind-specific callbacks
        if let Some(kind_ids) = self.kind_callbacks.get(&event.kind) {
            for id in kind_ids {
                if let Some(callback) = self.callbacks.get(id) {
                    callback(event);
                }
            }
            return;
        }

        // Check if this event matches any button-specific callbacks
        if let Some(button_ids) = self.button_callbacks.get(&event.button) {
            for id in button_ids {
                if let Some(callback) = self.callbacks.get(id) {
                    callback(event);
                }
            }
            return;
        }

        // If no specific callbacks, invoke all general callbacks
        // But only if not already invoked via kind/button callbacks
        let kind_ids: Vec<_> = self
            .kind_callbacks
            .values()
            .flat_map(|v| v.iter())
            .collect();
        let button_ids: Vec<_> = self
            .button_callbacks
            .values()
            .flat_map(|v| v.iter())
            .collect();

        for (id, callback) in &self.callbacks {
            if !kind_ids.contains(&id) && !button_ids.contains(&id) {
                callback(event);
            }
        }
    }

    /// Get the number of registered callbacks
    pub fn callback_count(&self) -> usize {
        self.callbacks.len()
    }
}

impl Default for UseMouse {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ==================== TerminalMouseButton Tests ====================

    #[test]
    fn test_terminal_mouse_button_default() {
        let button = TerminalMouseButton::default();
        assert_eq!(button, TerminalMouseButton::None);
    }

    #[test]
    fn test_terminal_mouse_button_left() {
        let button = TerminalMouseButton::Left;
        assert!(button.is_left());
        assert!(!button.is_right());
        assert!(!button.is_middle());
        assert!(!button.is_none());
    }

    #[test]
    fn test_terminal_mouse_button_right() {
        let button = TerminalMouseButton::Right;
        assert!(!button.is_left());
        assert!(button.is_right());
        assert!(!button.is_middle());
        assert!(!button.is_none());
    }

    #[test]
    fn test_terminal_mouse_button_middle() {
        let button = TerminalMouseButton::Middle;
        assert!(!button.is_left());
        assert!(!button.is_right());
        assert!(button.is_middle());
        assert!(!button.is_none());
    }

    #[test]
    fn test_terminal_mouse_button_none() {
        let button = TerminalMouseButton::None;
        assert!(!button.is_left());
        assert!(!button.is_right());
        assert!(!button.is_middle());
        assert!(button.is_none());
    }

    #[test]
    fn test_terminal_mouse_button_equality() {
        assert_eq!(TerminalMouseButton::Left, TerminalMouseButton::Left);
        assert_eq!(TerminalMouseButton::Right, TerminalMouseButton::Right);
        assert_eq!(TerminalMouseButton::Middle, TerminalMouseButton::Middle);
        assert_eq!(TerminalMouseButton::None, TerminalMouseButton::None);
    }

    #[test]
    fn test_terminal_mouse_button_inequality() {
        assert_ne!(TerminalMouseButton::Left, TerminalMouseButton::Right);
        assert_ne!(TerminalMouseButton::Left, TerminalMouseButton::Middle);
        assert_ne!(TerminalMouseButton::Left, TerminalMouseButton::None);
        assert_ne!(TerminalMouseButton::Right, TerminalMouseButton::Middle);
        assert_ne!(TerminalMouseButton::Right, TerminalMouseButton::None);
        assert_ne!(TerminalMouseButton::Middle, TerminalMouseButton::None);
    }

    #[test]
    fn test_terminal_mouse_button_clone() {
        let button = TerminalMouseButton::Left;
        let cloned = button.clone();
        assert_eq!(button, cloned);
    }

    #[test]
    fn test_terminal_mouse_button_copy() {
        let button = TerminalMouseButton::Right;
        let copied = button;
        assert_eq!(button, copied);
    }

    #[test]
    fn test_terminal_mouse_button_debug() {
        assert_eq!(format!("{:?}", TerminalMouseButton::Left), "Left");
        assert_eq!(format!("{:?}", TerminalMouseButton::Right), "Right");
        assert_eq!(format!("{:?}", TerminalMouseButton::Middle), "Middle");
        assert_eq!(format!("{:?}", TerminalMouseButton::None), "None");
    }

    #[test]
    fn test_terminal_mouse_button_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TerminalMouseButton::Left);
        set.insert(TerminalMouseButton::Right);
        set.insert(TerminalMouseButton::Middle);
        set.insert(TerminalMouseButton::None);

        assert!(set.contains(&TerminalMouseButton::Left));
        assert!(set.contains(&TerminalMouseButton::Right));
        assert!(set.contains(&TerminalMouseButton::Middle));
        assert!(set.contains(&TerminalMouseButton::None));
        assert_eq!(set.len(), 4);
    }

    // ==================== TerminalMouseEventKind Tests ====================

    #[test]
    fn test_terminal_mouse_event_kind_press() {
        let kind = TerminalMouseEventKind::Press;
        assert!(kind.is_press());
        assert!(!kind.is_release());
        assert!(!kind.is_drag());
        assert!(!kind.is_move());
        assert!(!kind.is_scroll_up());
        assert!(!kind.is_scroll_down());
        assert!(!kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_release() {
        let kind = TerminalMouseEventKind::Release;
        assert!(!kind.is_press());
        assert!(kind.is_release());
        assert!(!kind.is_drag());
        assert!(!kind.is_move());
        assert!(!kind.is_scroll_up());
        assert!(!kind.is_scroll_down());
        assert!(!kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_drag() {
        let kind = TerminalMouseEventKind::Drag;
        assert!(!kind.is_press());
        assert!(!kind.is_release());
        assert!(kind.is_drag());
        assert!(!kind.is_move());
        assert!(!kind.is_scroll_up());
        assert!(!kind.is_scroll_down());
        assert!(!kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_move() {
        let kind = TerminalMouseEventKind::Move;
        assert!(!kind.is_press());
        assert!(!kind.is_release());
        assert!(!kind.is_drag());
        assert!(kind.is_move());
        assert!(!kind.is_scroll_up());
        assert!(!kind.is_scroll_down());
        assert!(!kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_scroll_up() {
        let kind = TerminalMouseEventKind::ScrollUp;
        assert!(!kind.is_press());
        assert!(!kind.is_release());
        assert!(!kind.is_drag());
        assert!(!kind.is_move());
        assert!(kind.is_scroll_up());
        assert!(!kind.is_scroll_down());
        assert!(kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_scroll_down() {
        let kind = TerminalMouseEventKind::ScrollDown;
        assert!(!kind.is_press());
        assert!(!kind.is_release());
        assert!(!kind.is_drag());
        assert!(!kind.is_move());
        assert!(!kind.is_scroll_up());
        assert!(kind.is_scroll_down());
        assert!(kind.is_scroll());
    }

    #[test]
    fn test_terminal_mouse_event_kind_equality() {
        assert_eq!(TerminalMouseEventKind::Press, TerminalMouseEventKind::Press);
        assert_eq!(TerminalMouseEventKind::Release, TerminalMouseEventKind::Release);
        assert_eq!(TerminalMouseEventKind::Drag, TerminalMouseEventKind::Drag);
        assert_eq!(TerminalMouseEventKind::Move, TerminalMouseEventKind::Move);
        assert_eq!(TerminalMouseEventKind::ScrollUp, TerminalMouseEventKind::ScrollUp);
        assert_eq!(TerminalMouseEventKind::ScrollDown, TerminalMouseEventKind::ScrollDown);
    }

    #[test]
    fn test_terminal_mouse_event_kind_inequality() {
        assert_ne!(TerminalMouseEventKind::Press, TerminalMouseEventKind::Release);
        assert_ne!(TerminalMouseEventKind::Drag, TerminalMouseEventKind::Move);
        assert_ne!(TerminalMouseEventKind::ScrollUp, TerminalMouseEventKind::ScrollDown);
    }

    #[test]
    fn test_terminal_mouse_event_kind_debug() {
        assert_eq!(format!("{:?}", TerminalMouseEventKind::Press), "Press");
        assert_eq!(format!("{:?}", TerminalMouseEventKind::Release), "Release");
        assert_eq!(format!("{:?}", TerminalMouseEventKind::Drag), "Drag");
        assert_eq!(format!("{:?}", TerminalMouseEventKind::Move), "Move");
        assert_eq!(format!("{:?}", TerminalMouseEventKind::ScrollUp), "ScrollUp");
        assert_eq!(format!("{:?}", TerminalMouseEventKind::ScrollDown), "ScrollDown");
    }

    #[test]
    fn test_terminal_mouse_event_kind_clone() {
        let kind = TerminalMouseEventKind::Drag;
        let cloned = kind.clone();
        assert_eq!(kind, cloned);
    }

    #[test]
    fn test_terminal_mouse_event_kind_copy() {
        let kind = TerminalMouseEventKind::Move;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn test_terminal_mouse_event_kind_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TerminalMouseEventKind::Press);
        set.insert(TerminalMouseEventKind::Release);
        set.insert(TerminalMouseEventKind::Drag);
        set.insert(TerminalMouseEventKind::Move);
        set.insert(TerminalMouseEventKind::ScrollUp);
        set.insert(TerminalMouseEventKind::ScrollDown);

        assert_eq!(set.len(), 6);
        assert!(set.contains(&TerminalMouseEventKind::Press));
        assert!(set.contains(&TerminalMouseEventKind::ScrollDown));
    }

    // ==================== TerminalMouseEvent Creation Tests ====================

    #[test]
    fn test_terminal_mouse_event_new() {
        let event = TerminalMouseEvent::new(
            TerminalMouseEventKind::Press,
            TerminalMouseButton::Left,
            10,
            20,
        );

        assert_eq!(event.kind, TerminalMouseEventKind::Press);
        assert_eq!(event.button, TerminalMouseButton::Left);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 20);
        assert!(!event.modifiers.shift);
        assert!(!event.modifiers.ctrl);
        assert!(!event.modifiers.alt);
        assert!(!event.modifiers.meta);
    }

    #[test]
    fn test_terminal_mouse_event_with_modifiers() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: false,
            meta: false,
        };

        let event = TerminalMouseEvent::with_modifiers(
            TerminalMouseEventKind::Drag,
            TerminalMouseButton::Right,
            5,
            15,
            modifiers,
        );

        assert_eq!(event.kind, TerminalMouseEventKind::Drag);
        assert_eq!(event.button, TerminalMouseButton::Right);
        assert_eq!(event.x, 5);
        assert_eq!(event.y, 15);
        assert!(event.modifiers.shift);
        assert!(event.modifiers.ctrl);
        assert!(!event.modifiers.alt);
        assert!(!event.modifiers.meta);
    }

    #[test]
    fn test_terminal_mouse_event_press() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0);

        assert_eq!(event.kind, TerminalMouseEventKind::Press);
        assert_eq!(event.button, TerminalMouseButton::Left);
        assert_eq!(event.x, 0);
        assert_eq!(event.y, 0);
    }

    #[test]
    fn test_terminal_mouse_event_release() {
        let event = TerminalMouseEvent::release(TerminalMouseButton::Right, 100, 50);

        assert_eq!(event.kind, TerminalMouseEventKind::Release);
        assert_eq!(event.button, TerminalMouseButton::Right);
        assert_eq!(event.x, 100);
        assert_eq!(event.y, 50);
    }

    #[test]
    fn test_terminal_mouse_event_drag() {
        let event = TerminalMouseEvent::drag(TerminalMouseButton::Middle, 30, 40);

        assert_eq!(event.kind, TerminalMouseEventKind::Drag);
        assert_eq!(event.button, TerminalMouseButton::Middle);
        assert_eq!(event.x, 30);
        assert_eq!(event.y, 40);
    }

    #[test]
    fn test_terminal_mouse_event_move() {
        let event = TerminalMouseEvent::move_event(25, 35);

        assert_eq!(event.kind, TerminalMouseEventKind::Move);
        assert_eq!(event.button, TerminalMouseButton::None);
        assert_eq!(event.x, 25);
        assert_eq!(event.y, 35);
    }

    #[test]
    fn test_terminal_mouse_event_scroll_up() {
        let event = TerminalMouseEvent::scroll_up(10, 20);

        assert_eq!(event.kind, TerminalMouseEventKind::ScrollUp);
        assert_eq!(event.button, TerminalMouseButton::None);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 20);
    }

    #[test]
    fn test_terminal_mouse_event_scroll_down() {
        let event = TerminalMouseEvent::scroll_down(15, 25);

        assert_eq!(event.kind, TerminalMouseEventKind::ScrollDown);
        assert_eq!(event.button, TerminalMouseButton::None);
        assert_eq!(event.x, 15);
        assert_eq!(event.y, 25);
    }

    // ==================== TerminalMouseEvent Modifier Chain Tests ====================

    #[test]
    fn test_terminal_mouse_event_with_shift() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0).with_shift();

        assert!(event.has_shift());
        assert!(!event.has_ctrl());
        assert!(!event.has_alt());
        assert!(!event.has_meta());
        assert!(event.has_any_modifier());
    }

    #[test]
    fn test_terminal_mouse_event_with_ctrl() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0).with_ctrl();

        assert!(!event.has_shift());
        assert!(event.has_ctrl());
        assert!(!event.has_alt());
        assert!(!event.has_meta());
        assert!(event.has_any_modifier());
    }

    #[test]
    fn test_terminal_mouse_event_with_alt() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0).with_alt();

        assert!(!event.has_shift());
        assert!(!event.has_ctrl());
        assert!(event.has_alt());
        assert!(!event.has_meta());
        assert!(event.has_any_modifier());
    }

    #[test]
    fn test_terminal_mouse_event_with_meta() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0).with_meta();

        assert!(!event.has_shift());
        assert!(!event.has_ctrl());
        assert!(!event.has_alt());
        assert!(event.has_meta());
        assert!(event.has_any_modifier());
    }

    #[test]
    fn test_terminal_mouse_event_chained_modifiers() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0)
            .with_shift()
            .with_ctrl()
            .with_alt()
            .with_meta();

        assert!(event.has_shift());
        assert!(event.has_ctrl());
        assert!(event.has_alt());
        assert!(event.has_meta());
        assert!(event.has_any_modifier());
    }

    #[test]
    fn test_terminal_mouse_event_no_modifiers() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0);

        assert!(!event.has_shift());
        assert!(!event.has_ctrl());
        assert!(!event.has_alt());
        assert!(!event.has_meta());
        assert!(!event.has_any_modifier());
    }

    // ==================== TerminalMouseEvent Position Tests ====================

    #[test]
    fn test_terminal_mouse_event_position() {
        let event = TerminalMouseEvent::new(
            TerminalMouseEventKind::Move,
            TerminalMouseButton::None,
            42,
            73,
        );

        assert_eq!(event.position(), (42, 73));
        assert_eq!(event.x, 42);
        assert_eq!(event.y, 73);
    }

    #[test]
    fn test_terminal_mouse_event_position_zero() {
        let event = TerminalMouseEvent::move_event(0, 0);
        assert_eq!(event.position(), (0, 0));
    }

    #[test]
    fn test_terminal_mouse_event_position_max() {
        let event = TerminalMouseEvent::move_event(u16::MAX, u16::MAX);
        assert_eq!(event.position(), (u16::MAX, u16::MAX));
    }

    // ==================== TerminalMouseEvent Equality Tests ====================

    #[test]
    fn test_terminal_mouse_event_equality() {
        let event1 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let event2 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);

        assert_eq!(event1, event2);
    }

    #[test]
    fn test_terminal_mouse_event_inequality_kind() {
        let event1 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let event2 = TerminalMouseEvent::release(TerminalMouseButton::Left, 10, 20);

        assert_ne!(event1, event2);
    }

    #[test]
    fn test_terminal_mouse_event_inequality_button() {
        let event1 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let event2 = TerminalMouseEvent::press(TerminalMouseButton::Right, 10, 20);

        assert_ne!(event1, event2);
    }

    #[test]
    fn test_terminal_mouse_event_inequality_position() {
        let event1 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let event2 = TerminalMouseEvent::press(TerminalMouseButton::Left, 11, 20);

        assert_ne!(event1, event2);
    }

    #[test]
    fn test_terminal_mouse_event_inequality_modifiers() {
        let event1 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let event2 = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20).with_shift();

        assert_ne!(event1, event2);
    }

    // ==================== TerminalMouseEvent Clone Tests ====================

    #[test]
    fn test_terminal_mouse_event_clone() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20)
            .with_shift()
            .with_ctrl();
        let cloned = event.clone();

        assert_eq!(event, cloned);
        assert_eq!(cloned.kind, TerminalMouseEventKind::Press);
        assert_eq!(cloned.button, TerminalMouseButton::Left);
        assert_eq!(cloned.x, 10);
        assert_eq!(cloned.y, 20);
        assert!(cloned.has_shift());
        assert!(cloned.has_ctrl());
    }

    // ==================== TerminalMouseEvent Debug Tests ====================

    #[test]
    fn test_terminal_mouse_event_debug() {
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        let debug_str = format!("{:?}", event);

        assert!(debug_str.contains("Press"));
        assert!(debug_str.contains("Left"));
        assert!(debug_str.contains("10"));
        assert!(debug_str.contains("20"));
    }

    // ==================== MouseCallbackId Tests ====================

    #[test]
    fn test_mouse_callback_id_equality() {
        let id1 = MouseCallbackId(42);
        let id2 = MouseCallbackId(42);
        let id3 = MouseCallbackId(43);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_mouse_callback_id_debug() {
        let id = MouseCallbackId(123);
        assert_eq!(format!("{:?}", id), "MouseCallbackId(123)");
    }

    #[test]
    fn test_mouse_callback_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(MouseCallbackId(1));
        set.insert(MouseCallbackId(2));
        set.insert(MouseCallbackId(1)); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&MouseCallbackId(1)));
        assert!(set.contains(&MouseCallbackId(2)));
    }

    // ==================== UseMouse Creation Tests ====================

    #[test]
    fn test_use_mouse_new() {
        let hook = UseMouse::new();

        assert!(hook.is_enabled());
        assert_eq!(hook.callback_count(), 0);
    }

    #[test]
    fn test_use_mouse_default() {
        let hook = UseMouse::default();

        assert!(hook.is_enabled());
        assert_eq!(hook.callback_count(), 0);
    }

    // ==================== UseMouse Enable/Disable Tests ====================

    #[test]
    fn test_use_mouse_enable_disable() {
        let mut hook = UseMouse::new();

        assert!(hook.is_enabled());

        hook.disable();
        assert!(!hook.is_enabled());

        hook.enable();
        assert!(hook.is_enabled());
    }

    // ==================== UseMouse Callback Registration Tests ====================

    #[test]
    fn test_use_mouse_on_mouse() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let _id = hook.on_mouse(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        assert_eq!(hook.callback_count(), 1);

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_press() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_press(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_release() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_release(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::release(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_drag() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_drag(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::drag(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_move() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_move(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::move_event(10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_scroll_up() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_scroll_up(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::scroll_up(10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_scroll_down() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_scroll_down(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::scroll_down(10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_scroll_both() {
        let mut hook = UseMouse::new();
        let count = Rc::new(RefCell::new(0));
        let count_clone = count.clone();

        hook.on_scroll(move |_event| {
            *count_clone.borrow_mut() += 1;
        });

        hook.handle_event(&TerminalMouseEvent::scroll_up(10, 20));
        hook.handle_event(&TerminalMouseEvent::scroll_down(10, 20));

        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn test_use_mouse_on_left_button() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_left_button(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_right_button() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_right_button(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Right, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    #[test]
    fn test_use_mouse_on_middle_button() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_middle_button(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Middle, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    // ==================== UseMouse Callback Unregistration Tests ====================

    #[test]
    fn test_use_mouse_off() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        let id = hook.on_mouse(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        assert_eq!(hook.callback_count(), 1);

        hook.off(id);
        assert_eq!(hook.callback_count(), 0);

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(!*called.borrow());
    }

    #[test]
    fn test_use_mouse_clear() {
        let mut hook = UseMouse::new();

        hook.on_mouse(|_| {});
        hook.on_press(|_| {});
        hook.on_release(|_| {});

        assert_eq!(hook.callback_count(), 3);

        hook.clear();
        assert_eq!(hook.callback_count(), 0);
    }

    // ==================== UseMouse Event Handling Tests ====================

    #[test]
    fn test_use_mouse_disabled_no_callback() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_mouse(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        hook.disable();

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(!*called.borrow());
    }

    #[test]
    fn test_use_mouse_event_receives_correct_data() {
        let mut hook = UseMouse::new();
        let received_x = Rc::new(RefCell::new(0u16));
        let received_y = Rc::new(RefCell::new(0u16));
        let x_clone = received_x.clone();
        let y_clone = received_y.clone();

        hook.on_mouse(move |event| {
            *x_clone.borrow_mut() = event.x;
            *y_clone.borrow_mut() = event.y;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 42, 73);
        hook.handle_event(&event);

        assert_eq!(*received_x.borrow(), 42);
        assert_eq!(*received_y.borrow(), 73);
    }

    #[test]
    fn test_use_mouse_multiple_callbacks() {
        let mut hook = UseMouse::new();
        let count = Rc::new(RefCell::new(0));

        let count1 = count.clone();
        hook.on_mouse(move |_| {
            *count1.borrow_mut() += 1;
        });

        let count2 = count.clone();
        hook.on_mouse(move |_| {
            *count2.borrow_mut() += 1;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn test_use_mouse_kind_filter() {
        let mut hook = UseMouse::new();
        let press_called = Rc::new(RefCell::new(false));
        let release_called = Rc::new(RefCell::new(false));

        let press_clone = press_called.clone();
        hook.on_press(move |_| {
            *press_clone.borrow_mut() = true;
        });

        let release_clone = release_called.clone();
        hook.on_release(move |_| {
            *release_clone.borrow_mut() = true;
        });

        // Only trigger press
        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*press_called.borrow());
        assert!(!*release_called.borrow());
    }

    // ==================== UseMouse on_click Tests ====================

    #[test]
    fn test_use_mouse_on_click() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_click(move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::press(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    // ==================== UseMouse on_kind Tests ====================

    #[test]
    fn test_use_mouse_on_kind() {
        let mut hook = UseMouse::new();
        let called = Rc::new(RefCell::new(false));
        let called_clone = called.clone();

        hook.on_kind(TerminalMouseEventKind::Drag, move |_event| {
            *called_clone.borrow_mut() = true;
        });

        let event = TerminalMouseEvent::drag(TerminalMouseButton::Left, 10, 20);
        hook.handle_event(&event);

        assert!(*called.borrow());
    }

    // ==================== UseMouse callback_count Tests ====================

    #[test]
    fn test_use_mouse_callback_count() {
        let mut hook = UseMouse::new();

        assert_eq!(hook.callback_count(), 0);

        let id1 = hook.on_mouse(|_| {});
        assert_eq!(hook.callback_count(), 1);

        let _id2 = hook.on_press(|_| {});
        assert_eq!(hook.callback_count(), 2);

        hook.off(id1);
        assert_eq!(hook.callback_count(), 1);

        hook.clear();
        assert_eq!(hook.callback_count(), 0);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_integration_full_mouse_workflow() {
        let mut hook = UseMouse::new();

        let events_received = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events_received.clone();

        hook.on_mouse(move |event| {
            events_clone.borrow_mut().push(event.clone());
        });

        // Simulate a click-drag-release workflow
        hook.handle_event(&TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0));
        hook.handle_event(&TerminalMouseEvent::drag(TerminalMouseButton::Left, 5, 5));
        hook.handle_event(&TerminalMouseEvent::drag(TerminalMouseButton::Left, 10, 10));
        hook.handle_event(&TerminalMouseEvent::release(TerminalMouseButton::Left, 10, 10));

        let events = events_received.borrow();
        assert_eq!(events.len(), 4);
        assert!(events[0].kind.is_press());
        assert!(events[1].kind.is_drag());
        assert!(events[2].kind.is_drag());
        assert!(events[3].kind.is_release());
    }

    #[test]
    fn test_integration_scroll_events() {
        let mut hook = UseMouse::new();

        let scroll_count = Rc::new(RefCell::new(0i32));
        let scroll_clone = scroll_count.clone();

        hook.on_scroll(move |event| {
            if event.kind.is_scroll_up() {
                *scroll_clone.borrow_mut() += 1;
            } else if event.kind.is_scroll_down() {
                *scroll_clone.borrow_mut() -= 1;
            }
        });

        hook.handle_event(&TerminalMouseEvent::scroll_up(10, 10));
        hook.handle_event(&TerminalMouseEvent::scroll_up(10, 10));
        hook.handle_event(&TerminalMouseEvent::scroll_down(10, 10));

        assert_eq!(*scroll_count.borrow(), 1);
    }

    #[test]
    fn test_integration_modifier_detection() {
        let mut hook = UseMouse::new();

        let has_ctrl = Rc::new(RefCell::new(false));
        let ctrl_clone = has_ctrl.clone();

        hook.on_press(move |event| {
            *ctrl_clone.borrow_mut() = event.has_ctrl();
        });

        hook.handle_event(&TerminalMouseEvent::press(TerminalMouseButton::Left, 0, 0).with_ctrl());

        assert!(*has_ctrl.borrow());
    }
}
