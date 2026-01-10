//! Window focus/blur event hook for terminal applications
//!
//! This module provides functionality to detect when a terminal window gains
//! or loses focus. It uses ANSI escape sequences for focus reporting mode.
//!
//! # ANSI Sequences
//! - Enable focus reporting: `\x1b[?1004h`
//! - Disable focus reporting: `\x1b[?1004l`
//! - Focus gained event: `\x1b[I`
//! - Focus lost event: `\x1b[O`
//!
//! # Example
//! ```ignore
//! use rui::hooks::use_window_focus::{WindowFocusEvent, UseWindowFocus, FocusReporting};
//!
//! let mut focus_hook = UseWindowFocus::new();
//! focus_hook.on_focus_change(|event| {
//!     match event {
//!         WindowFocusEvent::Focus => println!("Window focused"),
//!         WindowFocusEvent::Blur => println!("Window blurred"),
//!     }
//! });
//!
//! // Enable focus reporting
//! print!("{}", FocusReporting::enable_sequence());
//!
//! // Later, disable focus reporting
//! print!("{}", FocusReporting::disable_sequence());
//! ```

use std::io::{self, Write};

/// Window focus event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowFocusEvent {
    /// Window has gained focus
    Focus,
    /// Window has lost focus (blurred)
    Blur,
}

impl WindowFocusEvent {
    /// Returns true if this is a focus event
    pub fn is_focus(&self) -> bool {
        matches!(self, WindowFocusEvent::Focus)
    }

    /// Returns true if this is a blur event
    pub fn is_blur(&self) -> bool {
        matches!(self, WindowFocusEvent::Blur)
    }
}

/// ANSI escape sequences for focus reporting
pub struct FocusReporting;

impl FocusReporting {
    /// ANSI sequence to enable focus reporting mode
    pub const ENABLE: &'static str = "\x1b[?1004h";

    /// ANSI sequence to disable focus reporting mode
    pub const DISABLE: &'static str = "\x1b[?1004l";

    /// ANSI sequence sent by terminal when window gains focus
    pub const FOCUS_GAINED: &'static str = "\x1b[I";

    /// ANSI sequence sent by terminal when window loses focus
    pub const FOCUS_LOST: &'static str = "\x1b[O";

    /// Returns the ANSI sequence to enable focus reporting
    pub fn enable_sequence() -> &'static str {
        Self::ENABLE
    }

    /// Returns the ANSI sequence to disable focus reporting
    pub fn disable_sequence() -> &'static str {
        Self::DISABLE
    }

    /// Writes the enable sequence to stdout
    pub fn enable() -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(Self::ENABLE.as_bytes())?;
        stdout.flush()
    }

    /// Writes the disable sequence to stdout
    pub fn disable() -> io::Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(Self::DISABLE.as_bytes())?;
        stdout.flush()
    }

    /// Writes the enable sequence to a writer
    pub fn enable_to<W: Write>(writer: &mut W) -> io::Result<()> {
        writer.write_all(Self::ENABLE.as_bytes())?;
        writer.flush()
    }

    /// Writes the disable sequence to a writer
    pub fn disable_to<W: Write>(writer: &mut W) -> io::Result<()> {
        writer.write_all(Self::DISABLE.as_bytes())?;
        writer.flush()
    }
}

/// Focus event detector that parses terminal input
pub struct FocusDetector {
    buffer: Vec<u8>,
}

impl FocusDetector {
    /// Creates a new focus detector
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Clears the internal buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Processes input bytes and returns any detected focus events
    ///
    /// This method accumulates bytes and attempts to parse focus/blur sequences.
    /// It returns a vector of detected events and removes consumed bytes from the buffer.
    pub fn process(&mut self, input: &[u8]) -> Vec<WindowFocusEvent> {
        self.buffer.extend_from_slice(input);
        let mut events = Vec::new();

        loop {
            // First, skip any leading garbage (non-ESC bytes)
            while !self.buffer.is_empty() && self.buffer[0] != 0x1b {
                self.buffer.remove(0);
            }

            if let Some((event, consumed)) = self.try_parse() {
                events.push(event);
                self.buffer.drain(..consumed);
            } else {
                break;
            }
        }

        // Clean up incomplete sequences that can't possibly match
        self.cleanup_buffer();

        events
    }

    /// Attempts to parse a focus event from the buffer
    fn try_parse(&self) -> Option<(WindowFocusEvent, usize)> {
        // Focus sequence: ESC [ I (0x1b 0x5b 0x49)
        // Blur sequence: ESC [ O (0x1b 0x5b 0x4f)

        if self.buffer.len() >= 3 {
            if self.buffer[0] == 0x1b && self.buffer[1] == 0x5b {
                match self.buffer[2] {
                    0x49 => return Some((WindowFocusEvent::Focus, 3)), // 'I'
                    0x4f => return Some((WindowFocusEvent::Blur, 3)),  // 'O'
                    _ => {
                        // Invalid sequence starting with ESC [
                        // Skip this ESC and try again
                        return None;
                    }
                }
            }
        }

        None
    }

    /// Cleans up the buffer by removing bytes that can't form valid sequences
    fn cleanup_buffer(&mut self) {
        // Find the first ESC character, discard everything before it
        if let Some(pos) = self.buffer.iter().position(|&b| b == 0x1b) {
            if pos > 0 {
                self.buffer.drain(..pos);
            }
        } else if !self.buffer.is_empty() {
            // No ESC found, clear the buffer
            self.buffer.clear();
        }
    }

    /// Returns the current buffer contents (for debugging)
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl Default for FocusDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Callback type for focus events
pub type FocusCallback = Box<dyn Fn(WindowFocusEvent) + 'static>;

/// Hook for managing window focus events
///
/// This hook provides a React-like interface for handling window focus events
/// in terminal applications.
pub struct UseWindowFocus {
    detector: FocusDetector,
    callbacks: Vec<FocusCallback>,
    is_focused: bool,
    reporting_enabled: bool,
}

impl UseWindowFocus {
    /// Creates a new window focus hook
    pub fn new() -> Self {
        Self {
            detector: FocusDetector::new(),
            callbacks: Vec::new(),
            is_focused: true, // Assume focused initially
            reporting_enabled: false,
        }
    }

    /// Returns true if the window is currently focused
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Returns true if focus reporting is enabled
    pub fn is_reporting_enabled(&self) -> bool {
        self.reporting_enabled
    }

    /// Enables focus reporting mode
    ///
    /// This sends the ANSI sequence to enable focus reporting to stdout.
    pub fn enable_reporting(&mut self) -> io::Result<()> {
        FocusReporting::enable()?;
        self.reporting_enabled = true;
        Ok(())
    }

    /// Disables focus reporting mode
    ///
    /// This sends the ANSI sequence to disable focus reporting to stdout.
    pub fn disable_reporting(&mut self) -> io::Result<()> {
        FocusReporting::disable()?;
        self.reporting_enabled = false;
        Ok(())
    }

    /// Enables focus reporting to a custom writer
    pub fn enable_reporting_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        FocusReporting::enable_to(writer)?;
        self.reporting_enabled = true;
        Ok(())
    }

    /// Disables focus reporting to a custom writer
    pub fn disable_reporting_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        FocusReporting::disable_to(writer)?;
        self.reporting_enabled = false;
        Ok(())
    }

    /// Registers a callback for focus change events
    ///
    /// The callback will be invoked whenever a focus or blur event is detected.
    pub fn on_focus_change<F>(&mut self, callback: F)
    where
        F: Fn(WindowFocusEvent) + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }

    /// Processes input and triggers callbacks for any detected focus events
    ///
    /// Returns the list of detected events.
    pub fn process_input(&mut self, input: &[u8]) -> Vec<WindowFocusEvent> {
        let events = self.detector.process(input);

        for event in &events {
            // Update focus state
            match event {
                WindowFocusEvent::Focus => self.is_focused = true,
                WindowFocusEvent::Blur => self.is_focused = false,
            }

            // Invoke callbacks
            for callback in &self.callbacks {
                callback(*event);
            }
        }

        events
    }

    /// Clears all registered callbacks
    pub fn clear_callbacks(&mut self) {
        self.callbacks.clear();
    }

    /// Returns the number of registered callbacks
    pub fn callback_count(&self) -> usize {
        self.callbacks.len()
    }
}

impl Default for UseWindowFocus {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for UseWindowFocus {
    fn drop(&mut self) {
        // Attempt to disable focus reporting when dropped
        if self.reporting_enabled {
            let _ = self.disable_reporting();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ==================== WindowFocusEvent Enum Tests ====================

    #[test]
    fn test_window_focus_event_focus_variant() {
        let event = WindowFocusEvent::Focus;
        assert!(event.is_focus());
        assert!(!event.is_blur());
    }

    #[test]
    fn test_window_focus_event_blur_variant() {
        let event = WindowFocusEvent::Blur;
        assert!(event.is_blur());
        assert!(!event.is_focus());
    }

    #[test]
    fn test_window_focus_event_equality() {
        assert_eq!(WindowFocusEvent::Focus, WindowFocusEvent::Focus);
        assert_eq!(WindowFocusEvent::Blur, WindowFocusEvent::Blur);
        assert_ne!(WindowFocusEvent::Focus, WindowFocusEvent::Blur);
    }

    #[test]
    fn test_window_focus_event_clone() {
        let event = WindowFocusEvent::Focus;
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_window_focus_event_copy() {
        let event = WindowFocusEvent::Blur;
        let copied = event; // Copy
        assert_eq!(event, copied);
    }

    #[test]
    fn test_window_focus_event_debug() {
        let focus_debug = format!("{:?}", WindowFocusEvent::Focus);
        let blur_debug = format!("{:?}", WindowFocusEvent::Blur);

        assert_eq!(focus_debug, "Focus");
        assert_eq!(blur_debug, "Blur");
    }

    #[test]
    fn test_window_focus_event_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(WindowFocusEvent::Focus);
        set.insert(WindowFocusEvent::Blur);

        assert!(set.contains(&WindowFocusEvent::Focus));
        assert!(set.contains(&WindowFocusEvent::Blur));
        assert_eq!(set.len(), 2);
    }

    // ==================== ANSI Sequence Tests ====================

    #[test]
    fn test_enable_sequence_constant() {
        assert_eq!(FocusReporting::ENABLE, "\x1b[?1004h");
    }

    #[test]
    fn test_disable_sequence_constant() {
        assert_eq!(FocusReporting::DISABLE, "\x1b[?1004l");
    }

    #[test]
    fn test_focus_gained_sequence_constant() {
        assert_eq!(FocusReporting::FOCUS_GAINED, "\x1b[I");
    }

    #[test]
    fn test_focus_lost_sequence_constant() {
        assert_eq!(FocusReporting::FOCUS_LOST, "\x1b[O");
    }

    #[test]
    fn test_enable_sequence_method() {
        assert_eq!(FocusReporting::enable_sequence(), "\x1b[?1004h");
    }

    #[test]
    fn test_disable_sequence_method() {
        assert_eq!(FocusReporting::disable_sequence(), "\x1b[?1004l");
    }

    #[test]
    fn test_enable_sequence_bytes() {
        let bytes = FocusReporting::ENABLE.as_bytes();
        assert_eq!(bytes, &[0x1b, 0x5b, 0x3f, 0x31, 0x30, 0x30, 0x34, 0x68]);
    }

    #[test]
    fn test_disable_sequence_bytes() {
        let bytes = FocusReporting::DISABLE.as_bytes();
        assert_eq!(bytes, &[0x1b, 0x5b, 0x3f, 0x31, 0x30, 0x30, 0x34, 0x6c]);
    }

    #[test]
    fn test_focus_gained_sequence_bytes() {
        let bytes = FocusReporting::FOCUS_GAINED.as_bytes();
        assert_eq!(bytes, &[0x1b, 0x5b, 0x49]);
    }

    #[test]
    fn test_focus_lost_sequence_bytes() {
        let bytes = FocusReporting::FOCUS_LOST.as_bytes();
        assert_eq!(bytes, &[0x1b, 0x5b, 0x4f]);
    }

    #[test]
    fn test_enable_to_writer() {
        let mut buffer = Vec::new();
        FocusReporting::enable_to(&mut buffer).unwrap();
        assert_eq!(buffer, FocusReporting::ENABLE.as_bytes());
    }

    #[test]
    fn test_disable_to_writer() {
        let mut buffer = Vec::new();
        FocusReporting::disable_to(&mut buffer).unwrap();
        assert_eq!(buffer, FocusReporting::DISABLE.as_bytes());
    }

    // ==================== FocusDetector Tests ====================

    #[test]
    fn test_focus_detector_new() {
        let detector = FocusDetector::new();
        assert!(detector.buffer().is_empty());
    }

    #[test]
    fn test_focus_detector_default() {
        let detector = FocusDetector::default();
        assert!(detector.buffer().is_empty());
    }

    #[test]
    fn test_detect_focus_event() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[I";
        let events = detector.process(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_detect_blur_event() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[O";
        let events = detector.process(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Blur);
    }

    #[test]
    fn test_detect_multiple_events() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[I\x1b[O\x1b[I";
        let events = detector.process(input);

        assert_eq!(events.len(), 3);
        assert_eq!(events[0], WindowFocusEvent::Focus);
        assert_eq!(events[1], WindowFocusEvent::Blur);
        assert_eq!(events[2], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_detect_focus_blur_alternating() {
        let mut detector = FocusDetector::new();

        let events1 = detector.process(b"\x1b[I");
        assert_eq!(events1.len(), 1);
        assert_eq!(events1[0], WindowFocusEvent::Focus);

        let events2 = detector.process(b"\x1b[O");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0], WindowFocusEvent::Blur);
    }

    #[test]
    fn test_detect_partial_sequence() {
        let mut detector = FocusDetector::new();

        // Send partial sequence
        let events1 = detector.process(b"\x1b[");
        assert!(events1.is_empty());

        // Complete the sequence
        let events2 = detector.process(b"I");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_detect_split_sequence() {
        let mut detector = FocusDetector::new();

        // Send sequence byte by byte
        let events1 = detector.process(b"\x1b");
        assert!(events1.is_empty());

        let events2 = detector.process(b"[");
        assert!(events2.is_empty());

        let events3 = detector.process(b"O");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0], WindowFocusEvent::Blur);
    }

    #[test]
    fn test_detect_with_garbage_before() {
        let mut detector = FocusDetector::new();
        let input = b"garbage\x1b[I";
        let events = detector.process(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_detect_with_garbage_after() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[Igarbage";
        let events = detector.process(input);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_detect_with_garbage_between() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[Igarbage\x1b[O";
        let events = detector.process(input);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], WindowFocusEvent::Focus);
        assert_eq!(events[1], WindowFocusEvent::Blur);
    }

    #[test]
    fn test_detect_no_events() {
        let mut detector = FocusDetector::new();
        let input = b"no events here";
        let events = detector.process(input);

        assert!(events.is_empty());
    }

    #[test]
    fn test_detect_invalid_escape_sequence() {
        let mut detector = FocusDetector::new();
        let input = b"\x1b[X"; // Invalid sequence
        let events = detector.process(input);

        assert!(events.is_empty());
    }

    #[test]
    fn test_clear_buffer() {
        let mut detector = FocusDetector::new();
        detector.process(b"\x1b[");
        assert!(!detector.buffer().is_empty());

        detector.clear();
        assert!(detector.buffer().is_empty());
    }

    #[test]
    fn test_buffer_contents() {
        let mut detector = FocusDetector::new();
        detector.process(b"\x1b");

        assert_eq!(detector.buffer(), b"\x1b");
    }

    // ==================== UseWindowFocus Hook Tests ====================

    #[test]
    fn test_use_window_focus_new() {
        let hook = UseWindowFocus::new();
        assert!(hook.is_focused());
        assert!(!hook.is_reporting_enabled());
        assert_eq!(hook.callback_count(), 0);
    }

    #[test]
    fn test_use_window_focus_default() {
        let hook = UseWindowFocus::default();
        assert!(hook.is_focused());
        assert!(!hook.is_reporting_enabled());
    }

    #[test]
    fn test_use_window_focus_process_focus() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"\x1b[I");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Focus);
        assert!(hook.is_focused());
    }

    #[test]
    fn test_use_window_focus_process_blur() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"\x1b[O");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WindowFocusEvent::Blur);
        assert!(!hook.is_focused());
    }

    #[test]
    fn test_use_window_focus_state_tracking() {
        let mut hook = UseWindowFocus::new();

        // Initially focused
        assert!(hook.is_focused());

        // Blur event
        hook.process_input(b"\x1b[O");
        assert!(!hook.is_focused());

        // Focus event
        hook.process_input(b"\x1b[I");
        assert!(hook.is_focused());

        // Another blur
        hook.process_input(b"\x1b[O");
        assert!(!hook.is_focused());
    }

    #[test]
    fn test_callback_invocation() {
        let mut hook = UseWindowFocus::new();
        let callback_called = Rc::new(RefCell::new(false));
        let callback_called_clone = callback_called.clone();

        hook.on_focus_change(move |_| {
            *callback_called_clone.borrow_mut() = true;
        });

        hook.process_input(b"\x1b[I");
        assert!(*callback_called.borrow());
    }

    #[test]
    fn test_callback_receives_correct_event() {
        let mut hook = UseWindowFocus::new();
        let received_event = Rc::new(RefCell::new(None));
        let received_event_clone = received_event.clone();

        hook.on_focus_change(move |event| {
            *received_event_clone.borrow_mut() = Some(event);
        });

        hook.process_input(b"\x1b[O");
        assert_eq!(*received_event.borrow(), Some(WindowFocusEvent::Blur));
    }

    #[test]
    fn test_multiple_callbacks() {
        let mut hook = UseWindowFocus::new();
        let count = Rc::new(RefCell::new(0));
        let count1 = count.clone();
        let count2 = count.clone();

        hook.on_focus_change(move |_| {
            *count1.borrow_mut() += 1;
        });

        hook.on_focus_change(move |_| {
            *count2.borrow_mut() += 10;
        });

        hook.process_input(b"\x1b[I");
        assert_eq!(*count.borrow(), 11);
    }

    #[test]
    fn test_callback_count() {
        let mut hook = UseWindowFocus::new();
        assert_eq!(hook.callback_count(), 0);

        hook.on_focus_change(|_| {});
        assert_eq!(hook.callback_count(), 1);

        hook.on_focus_change(|_| {});
        assert_eq!(hook.callback_count(), 2);
    }

    #[test]
    fn test_clear_callbacks() {
        let mut hook = UseWindowFocus::new();

        hook.on_focus_change(|_| {});
        hook.on_focus_change(|_| {});
        assert_eq!(hook.callback_count(), 2);

        hook.clear_callbacks();
        assert_eq!(hook.callback_count(), 0);
    }

    #[test]
    fn test_callbacks_not_called_after_clear() {
        let mut hook = UseWindowFocus::new();
        let callback_called = Rc::new(RefCell::new(false));
        let callback_called_clone = callback_called.clone();

        hook.on_focus_change(move |_| {
            *callback_called_clone.borrow_mut() = true;
        });

        hook.clear_callbacks();
        hook.process_input(b"\x1b[I");

        assert!(!*callback_called.borrow());
    }

    #[test]
    fn test_enable_reporting_to_writer() {
        let mut hook = UseWindowFocus::new();
        let mut buffer = Vec::new();

        assert!(!hook.is_reporting_enabled());

        hook.enable_reporting_to(&mut buffer).unwrap();

        assert!(hook.is_reporting_enabled());
        assert_eq!(buffer, FocusReporting::ENABLE.as_bytes());
    }

    #[test]
    fn test_disable_reporting_to_writer() {
        let mut hook = UseWindowFocus::new();
        let mut buffer = Vec::new();

        // Enable first
        hook.enable_reporting_to(&mut buffer).unwrap();
        buffer.clear();

        // Now disable
        hook.disable_reporting_to(&mut buffer).unwrap();

        assert!(!hook.is_reporting_enabled());
        assert_eq!(buffer, FocusReporting::DISABLE.as_bytes());
    }

    #[test]
    fn test_multiple_events_callback() {
        let mut hook = UseWindowFocus::new();
        let events = Rc::new(RefCell::new(Vec::new()));
        let events_clone = events.clone();

        hook.on_focus_change(move |event| {
            events_clone.borrow_mut().push(event);
        });

        hook.process_input(b"\x1b[I\x1b[O\x1b[I");

        let collected = events.borrow();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], WindowFocusEvent::Focus);
        assert_eq!(collected[1], WindowFocusEvent::Blur);
        assert_eq!(collected[2], WindowFocusEvent::Focus);
    }

    #[test]
    fn test_process_empty_input() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"");

        assert!(events.is_empty());
        assert!(hook.is_focused()); // State unchanged
    }

    #[test]
    fn test_process_garbage_input() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"random garbage data");

        assert!(events.is_empty());
        assert!(hook.is_focused()); // State unchanged
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_consecutive_focus_events() {
        let mut hook = UseWindowFocus::new();

        hook.process_input(b"\x1b[I");
        assert!(hook.is_focused());

        hook.process_input(b"\x1b[I");
        assert!(hook.is_focused()); // Still focused
    }

    #[test]
    fn test_consecutive_blur_events() {
        let mut hook = UseWindowFocus::new();

        hook.process_input(b"\x1b[O");
        assert!(!hook.is_focused());

        hook.process_input(b"\x1b[O");
        assert!(!hook.is_focused()); // Still blurred
    }

    #[test]
    fn test_rapid_focus_blur_toggle() {
        let mut hook = UseWindowFocus::new();
        let count = Rc::new(RefCell::new(0));
        let count_clone = count.clone();

        hook.on_focus_change(move |_| {
            *count_clone.borrow_mut() += 1;
        });

        // Simulate rapid toggling
        for _ in 0..10 {
            hook.process_input(b"\x1b[O\x1b[I");
        }

        assert_eq!(*count.borrow(), 20);
        assert!(hook.is_focused()); // Ends with focus
    }

    #[test]
    fn test_interleaved_data() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"abc\x1b[Idef\x1b[Oghi");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], WindowFocusEvent::Focus);
        assert_eq!(events[1], WindowFocusEvent::Blur);
    }

    #[test]
    fn test_single_escape_character() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"\x1b");

        assert!(events.is_empty());
        assert!(hook.is_focused());
    }

    #[test]
    fn test_escape_bracket_only() {
        let mut hook = UseWindowFocus::new();
        let events = hook.process_input(b"\x1b[");

        assert!(events.is_empty());
        assert!(hook.is_focused());
    }

    #[test]
    fn test_incomplete_then_complete() {
        let mut hook = UseWindowFocus::new();

        let events1 = hook.process_input(b"\x1b[");
        assert!(events1.is_empty());

        let events2 = hook.process_input(b"I");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0], WindowFocusEvent::Focus);
    }

    // ==================== Focus State Tests ====================

    #[test]
    fn test_initial_focus_state() {
        let hook = UseWindowFocus::new();
        // Window should be assumed focused initially
        assert!(hook.is_focused());
    }

    #[test]
    fn test_focus_state_after_blur() {
        let mut hook = UseWindowFocus::new();
        hook.process_input(b"\x1b[O");
        assert!(!hook.is_focused());
    }

    #[test]
    fn test_focus_state_after_refocus() {
        let mut hook = UseWindowFocus::new();
        hook.process_input(b"\x1b[O"); // Blur
        hook.process_input(b"\x1b[I"); // Focus
        assert!(hook.is_focused());
    }

    // ==================== Reporting State Tests ====================

    #[test]
    fn test_reporting_initially_disabled() {
        let hook = UseWindowFocus::new();
        assert!(!hook.is_reporting_enabled());
    }

    #[test]
    fn test_reporting_state_after_enable() {
        let mut hook = UseWindowFocus::new();
        let mut buffer = Vec::new();
        hook.enable_reporting_to(&mut buffer).unwrap();
        assert!(hook.is_reporting_enabled());
    }

    #[test]
    fn test_reporting_state_after_disable() {
        let mut hook = UseWindowFocus::new();
        let mut buffer = Vec::new();
        hook.enable_reporting_to(&mut buffer).unwrap();
        hook.disable_reporting_to(&mut buffer).unwrap();
        assert!(!hook.is_reporting_enabled());
    }
}
