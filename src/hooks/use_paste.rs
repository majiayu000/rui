//! Bracketed paste detection and handling for terminal applications.
//!
//! This module provides functionality for detecting and handling paste events
//! in terminal applications using the bracketed paste mode protocol.
//!
//! # Bracketed Paste Mode
//!
//! Bracketed paste mode is a terminal feature that wraps pasted text with special
//! escape sequences, allowing applications to distinguish between typed and pasted input.
//!
//! - Enable sequence: `\x1b[?2004h`
//! - Disable sequence: `\x1b[?2004l`
//! - Paste start: `\x1b[200~`
//! - Paste end: `\x1b[201~`
//!
//! # Example
//!
//! ```
//! use rui::hooks::{PasteDetector, PasteEvent, BracketedPasteMode};
//!
//! let mut detector = PasteDetector::new();
//!
//! // Simulate receiving bracketed paste data
//! let input = "\x1b[200~Hello, World!\x1b[201~";
//! if let Some(event) = detector.feed(input) {
//!     assert_eq!(event.content(), "Hello, World!");
//! }
//! ```

use std::io::{self, Write};

/// ANSI escape sequence to enable bracketed paste mode.
pub const ENABLE_BRACKETED_PASTE: &str = "\x1b[?2004h";

/// ANSI escape sequence to disable bracketed paste mode.
pub const DISABLE_BRACKETED_PASTE: &str = "\x1b[?2004l";

/// ANSI escape sequence marking the start of pasted content.
pub const PASTE_START: &str = "\x1b[200~";

/// ANSI escape sequence marking the end of pasted content.
pub const PASTE_END: &str = "\x1b[201~";

/// Represents a paste event containing the pasted content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteEvent {
    content: String,
}

impl PasteEvent {
    /// Creates a new paste event with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }

    /// Returns the pasted content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Consumes the event and returns the pasted content.
    pub fn into_content(self) -> String {
        self.content
    }

    /// Returns true if the pasted content is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the length of the pasted content in bytes.
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

/// Controls bracketed paste mode for the terminal.
///
/// This struct provides methods to enable and disable bracketed paste mode
/// by writing the appropriate ANSI escape sequences to the terminal.
#[derive(Debug, Default)]
pub struct BracketedPasteMode {
    enabled: bool,
}

impl BracketedPasteMode {
    /// Creates a new `BracketedPasteMode` instance (disabled by default).
    pub fn new() -> Self {
        Self { enabled: false }
    }

    /// Returns whether bracketed paste mode is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables bracketed paste mode by writing the enable sequence to stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to stdout fails.
    pub fn enable(&mut self) -> io::Result<()> {
        self.enable_to(&mut io::stdout())
    }

    /// Enables bracketed paste mode by writing to the specified writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn enable_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(ENABLE_BRACKETED_PASTE.as_bytes())?;
        writer.flush()?;
        self.enabled = true;
        Ok(())
    }

    /// Disables bracketed paste mode by writing the disable sequence to stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to stdout fails.
    pub fn disable(&mut self) -> io::Result<()> {
        self.disable_to(&mut io::stdout())
    }

    /// Disables bracketed paste mode by writing to the specified writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn disable_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(DISABLE_BRACKETED_PASTE.as_bytes())?;
        writer.flush()?;
        self.enabled = false;
        Ok(())
    }

    /// Returns the ANSI sequence for enabling bracketed paste mode.
    pub fn enable_sequence() -> &'static str {
        ENABLE_BRACKETED_PASTE
    }

    /// Returns the ANSI sequence for disabling bracketed paste mode.
    pub fn disable_sequence() -> &'static str {
        DISABLE_BRACKETED_PASTE
    }
}

impl Drop for BracketedPasteMode {
    fn drop(&mut self) {
        if self.enabled {
            // Best effort to disable on drop, ignore errors
            let _ = self.disable();
        }
    }
}

/// Parser state for detecting paste events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    /// Normal state, looking for paste start sequence.
    Normal,
    /// Currently inside a paste (after seeing start sequence).
    InPaste,
}

/// Detects paste events from a stream of input data.
///
/// The detector maintains internal state to handle paste sequences that may
/// span multiple input chunks.
#[derive(Debug)]
pub struct PasteDetector {
    state: ParserState,
    buffer: String,
    paste_buffer: String,
}

impl Default for PasteDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteDetector {
    /// Creates a new paste detector.
    pub fn new() -> Self {
        Self {
            state: ParserState::Normal,
            buffer: String::new(),
            paste_buffer: String::new(),
        }
    }

    /// Resets the detector to its initial state.
    pub fn reset(&mut self) {
        self.state = ParserState::Normal;
        self.buffer.clear();
        self.paste_buffer.clear();
    }

    /// Returns true if currently inside a paste sequence.
    pub fn is_in_paste(&self) -> bool {
        self.state == ParserState::InPaste
    }

    /// Feeds input data to the detector and returns a paste event if one is complete.
    ///
    /// This method can be called multiple times with chunks of input data.
    /// It will return `Some(PasteEvent)` when a complete paste sequence is detected.
    pub fn feed(&mut self, input: &str) -> Option<PasteEvent> {
        self.buffer.push_str(input);
        self.process_buffer()
    }

    /// Processes the internal buffer and extracts paste events.
    fn process_buffer(&mut self) -> Option<PasteEvent> {
        loop {
            match self.state {
                ParserState::Normal => {
                    // Look for paste start sequence
                    if let Some(start_pos) = self.buffer.find(PASTE_START) {
                        // Remove everything before and including the start sequence
                        let after_start = start_pos + PASTE_START.len();
                        self.buffer = self.buffer[after_start..].to_string();
                        self.state = ParserState::InPaste;
                        self.paste_buffer.clear();
                    } else {
                        // Check if buffer might contain partial start sequence
                        // Keep only potential partial matches at the end
                        let max_partial = PASTE_START.len() - 1;
                        if self.buffer.len() > max_partial {
                            let keep_from = self.buffer.len() - max_partial;
                            // Only keep if it could be start of PASTE_START
                            let potential = &self.buffer[keep_from..];
                            if PASTE_START.starts_with(potential) {
                                self.buffer = self.buffer[keep_from..].to_string();
                            } else {
                                self.buffer.clear();
                            }
                        }
                        return None;
                    }
                }
                ParserState::InPaste => {
                    // Look for paste end sequence
                    if let Some(end_pos) = self.buffer.find(PASTE_END) {
                        // Extract paste content
                        self.paste_buffer.push_str(&self.buffer[..end_pos]);
                        let content = std::mem::take(&mut self.paste_buffer);

                        // Remove processed data from buffer
                        let after_end = end_pos + PASTE_END.len();
                        self.buffer = self.buffer[after_end..].to_string();
                        self.state = ParserState::Normal;

                        return Some(PasteEvent::new(content));
                    } else {
                        // Check if buffer might contain partial end sequence
                        let max_partial = PASTE_END.len() - 1;
                        if self.buffer.len() > max_partial {
                            let keep_from = self.buffer.len() - max_partial;
                            // Move safe content to paste buffer
                            self.paste_buffer.push_str(&self.buffer[..keep_from]);
                            self.buffer = self.buffer[keep_from..].to_string();
                        }
                        return None;
                    }
                }
            }
        }
    }

    /// Parses a complete input string and extracts all paste events.
    ///
    /// Unlike `feed`, this method processes the entire input at once and
    /// returns all paste events found.
    pub fn parse_all(&mut self, input: &str) -> Vec<PasteEvent> {
        let mut events = Vec::new();
        self.buffer.push_str(input);

        while let Some(event) = self.process_buffer() {
            events.push(event);
        }

        events
    }

    /// Extracts paste content from a bracketed paste sequence.
    ///
    /// This is a stateless helper function for simple cases where the entire
    /// paste sequence is available at once.
    pub fn extract_paste(input: &str) -> Option<String> {
        let start = input.find(PASTE_START)?;
        let content_start = start + PASTE_START.len();
        let end = input[content_start..].find(PASTE_END)?;
        Some(input[content_start..content_start + end].to_string())
    }
}

/// Type alias for paste event callback functions.
pub type PasteCallback = Box<dyn FnMut(PasteEvent) + Send + 'static>;

/// Handler for paste events with callback support.
///
/// This struct combines paste detection with callback-based event handling,
/// providing a hook-like interface for paste events.
pub struct PasteHandler {
    detector: PasteDetector,
    callback: Option<PasteCallback>,
}

impl std::fmt::Debug for PasteHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PasteHandler")
            .field("detector", &self.detector)
            .field("has_callback", &self.callback.is_some())
            .finish()
    }
}

impl Default for PasteHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteHandler {
    /// Creates a new paste handler without a callback.
    pub fn new() -> Self {
        Self {
            detector: PasteDetector::new(),
            callback: None,
        }
    }

    /// Creates a new paste handler with the specified callback.
    pub fn with_callback<F>(callback: F) -> Self
    where
        F: FnMut(PasteEvent) + Send + 'static,
    {
        Self {
            detector: PasteDetector::new(),
            callback: Some(Box::new(callback)),
        }
    }

    /// Sets the callback for paste events.
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: FnMut(PasteEvent) + Send + 'static,
    {
        self.callback = Some(Box::new(callback));
    }

    /// Removes the current callback.
    pub fn clear_callback(&mut self) {
        self.callback = None;
    }

    /// Returns true if a callback is set.
    pub fn has_callback(&self) -> bool {
        self.callback.is_some()
    }

    /// Feeds input data and invokes the callback if a paste event is detected.
    ///
    /// Returns the paste event if one was detected (regardless of callback).
    pub fn feed(&mut self, input: &str) -> Option<PasteEvent> {
        if let Some(event) = self.detector.feed(input) {
            if let Some(ref mut callback) = self.callback {
                callback(event.clone());
            }
            Some(event)
        } else {
            None
        }
    }

    /// Resets the internal detector state.
    pub fn reset(&mut self) {
        self.detector.reset();
    }

    /// Returns a reference to the internal detector.
    pub fn detector(&self) -> &PasteDetector {
        &self.detector
    }

    /// Returns a mutable reference to the internal detector.
    pub fn detector_mut(&mut self) -> &mut PasteDetector {
        &mut self.detector
    }
}

/// Hook for managing paste events in a React-like manner.
///
/// This struct provides a higher-level interface for paste handling,
/// combining bracketed paste mode control with event detection.
pub struct UsePaste {
    mode: BracketedPasteMode,
    handler: PasteHandler,
}

impl std::fmt::Debug for UsePaste {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsePaste")
            .field("mode", &self.mode)
            .field("handler", &self.handler)
            .finish()
    }
}

impl Default for UsePaste {
    fn default() -> Self {
        Self::new()
    }
}

impl UsePaste {
    /// Creates a new paste hook (bracketed paste mode disabled by default).
    pub fn new() -> Self {
        Self {
            mode: BracketedPasteMode::new(),
            handler: PasteHandler::new(),
        }
    }

    /// Creates a new paste hook with the specified callback.
    pub fn with_callback<F>(callback: F) -> Self
    where
        F: FnMut(PasteEvent) + Send + 'static,
    {
        Self {
            mode: BracketedPasteMode::new(),
            handler: PasteHandler::with_callback(callback),
        }
    }

    /// Enables bracketed paste mode.
    pub fn enable(&mut self) -> io::Result<()> {
        self.mode.enable()
    }

    /// Enables bracketed paste mode using the specified writer.
    pub fn enable_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.mode.enable_to(writer)
    }

    /// Disables bracketed paste mode.
    pub fn disable(&mut self) -> io::Result<()> {
        self.mode.disable()
    }

    /// Disables bracketed paste mode using the specified writer.
    pub fn disable_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.mode.disable_to(writer)
    }

    /// Returns whether bracketed paste mode is enabled.
    pub fn is_enabled(&self) -> bool {
        self.mode.is_enabled()
    }

    /// Sets the callback for paste events.
    pub fn on_paste<F>(&mut self, callback: F)
    where
        F: FnMut(PasteEvent) + Send + 'static,
    {
        self.handler.set_callback(callback);
    }

    /// Feeds input data and processes paste events.
    pub fn feed(&mut self, input: &str) -> Option<PasteEvent> {
        self.handler.feed(input)
    }

    /// Resets the internal state.
    pub fn reset(&mut self) {
        self.handler.reset();
    }

    /// Returns a reference to the bracketed paste mode controller.
    pub fn mode(&self) -> &BracketedPasteMode {
        &self.mode
    }

    /// Returns a reference to the paste handler.
    pub fn handler(&self) -> &PasteHandler {
        &self.handler
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // PasteEvent tests
    // ==========================================================================

    #[test]
    fn test_paste_event_new() {
        let event = PasteEvent::new("Hello, World!");
        assert_eq!(event.content(), "Hello, World!");
    }

    #[test]
    fn test_paste_event_from_string() {
        let event = PasteEvent::new(String::from("Test content"));
        assert_eq!(event.content(), "Test content");
    }

    #[test]
    fn test_paste_event_into_content() {
        let event = PasteEvent::new("Take ownership");
        let content = event.into_content();
        assert_eq!(content, "Take ownership");
    }

    #[test]
    fn test_paste_event_is_empty() {
        let empty = PasteEvent::new("");
        let non_empty = PasteEvent::new("content");

        assert!(empty.is_empty());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_paste_event_len() {
        let event = PasteEvent::new("Hello");
        assert_eq!(event.len(), 5);

        let unicode = PasteEvent::new("Hello, 世界!");
        assert_eq!(unicode.len(), 14); // UTF-8 bytes
    }

    #[test]
    fn test_paste_event_equality() {
        let event1 = PasteEvent::new("same");
        let event2 = PasteEvent::new("same");
        let event3 = PasteEvent::new("different");

        assert_eq!(event1, event2);
        assert_ne!(event1, event3);
    }

    #[test]
    fn test_paste_event_clone() {
        let original = PasteEvent::new("clone me");
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_paste_event_debug() {
        let event = PasteEvent::new("debug");
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("PasteEvent"));
        assert!(debug_str.contains("debug"));
    }

    // ==========================================================================
    // BracketedPasteMode tests
    // ==========================================================================

    #[test]
    fn test_bracketed_paste_mode_new() {
        let mode = BracketedPasteMode::new();
        assert!(!mode.is_enabled());
    }

    #[test]
    fn test_bracketed_paste_mode_default() {
        let mode = BracketedPasteMode::default();
        assert!(!mode.is_enabled());
    }

    #[test]
    fn test_bracketed_paste_mode_enable_to() {
        let mut buffer = Vec::new();
        let mut mode = BracketedPasteMode::new();

        mode.enable_to(&mut buffer).unwrap();

        assert!(mode.is_enabled());
        assert_eq!(buffer, ENABLE_BRACKETED_PASTE.as_bytes());
    }

    #[test]
    fn test_bracketed_paste_mode_disable_to() {
        let mut buffer = Vec::new();
        let mut mode = BracketedPasteMode::new();

        mode.enable_to(&mut buffer).unwrap();
        buffer.clear();
        mode.disable_to(&mut buffer).unwrap();

        assert!(!mode.is_enabled());
        assert_eq!(buffer, DISABLE_BRACKETED_PASTE.as_bytes());
    }

    #[test]
    fn test_bracketed_paste_mode_sequences() {
        assert_eq!(BracketedPasteMode::enable_sequence(), "\x1b[?2004h");
        assert_eq!(BracketedPasteMode::disable_sequence(), "\x1b[?2004l");
    }

    #[test]
    fn test_bracketed_paste_mode_toggle() {
        let mut buffer = Vec::new();
        let mut mode = BracketedPasteMode::new();

        assert!(!mode.is_enabled());

        mode.enable_to(&mut buffer).unwrap();
        assert!(mode.is_enabled());

        mode.disable_to(&mut buffer).unwrap();
        assert!(!mode.is_enabled());

        mode.enable_to(&mut buffer).unwrap();
        assert!(mode.is_enabled());
    }

    // ==========================================================================
    // PasteDetector tests
    // ==========================================================================

    #[test]
    fn test_detector_new() {
        let detector = PasteDetector::new();
        assert!(!detector.is_in_paste());
    }

    #[test]
    fn test_detector_default() {
        let detector = PasteDetector::default();
        assert!(!detector.is_in_paste());
    }

    #[test]
    fn test_detector_simple_paste() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~Hello, World!\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Hello, World!");
    }

    #[test]
    fn test_detector_empty_paste() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert!(event.unwrap().is_empty());
    }

    #[test]
    fn test_detector_multiline_paste() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~Line 1\nLine 2\nLine 3\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_detector_paste_with_special_chars() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~Tab:\t Newline:\n Special: @#$%^&*()\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(
            event.unwrap().content(),
            "Tab:\t Newline:\n Special: @#$%^&*()"
        );
    }

    #[test]
    fn test_detector_unicode_paste() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~Hello, 世界! 🌍\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Hello, 世界! 🌍");
    }

    #[test]
    fn test_detector_chunked_input() {
        let mut detector = PasteDetector::new();

        // Feed in chunks
        assert!(detector.feed("\x1b[200").is_none());
        assert!(detector.feed("~Hel").is_none());
        assert!(detector.is_in_paste());
        assert!(detector.feed("lo\x1b[20").is_none());

        let event = detector.feed("1~");

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Hello");
    }

    #[test]
    fn test_detector_multiple_pastes() {
        let mut detector = PasteDetector::new();

        let event1 = detector.feed("\x1b[200~First\x1b[201~");
        assert!(event1.is_some());
        assert_eq!(event1.unwrap().content(), "First");

        let event2 = detector.feed("\x1b[200~Second\x1b[201~");
        assert!(event2.is_some());
        assert_eq!(event2.unwrap().content(), "Second");
    }

    #[test]
    fn test_detector_paste_all() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~First\x1b[201~normal\x1b[200~Second\x1b[201~";

        let events = detector.parse_all(input);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].content(), "First");
        assert_eq!(events[1].content(), "Second");
    }

    #[test]
    fn test_detector_no_paste() {
        let mut detector = PasteDetector::new();
        let input = "Just normal text without paste sequences";

        let event = detector.feed(input);

        assert!(event.is_none());
        assert!(!detector.is_in_paste());
    }

    #[test]
    fn test_detector_incomplete_start() {
        let mut detector = PasteDetector::new();

        assert!(detector.feed("\x1b[200").is_none());
        assert!(!detector.is_in_paste()); // Not yet in paste, sequence incomplete
    }

    #[test]
    fn test_detector_reset() {
        let mut detector = PasteDetector::new();

        detector.feed("\x1b[200~partial");
        assert!(detector.is_in_paste());

        detector.reset();
        assert!(!detector.is_in_paste());
    }

    #[test]
    fn test_detector_extract_paste_static() {
        let input = "\x1b[200~Extract this\x1b[201~";
        let content = PasteDetector::extract_paste(input);

        assert!(content.is_some());
        assert_eq!(content.unwrap(), "Extract this");
    }

    #[test]
    fn test_detector_extract_paste_with_prefix() {
        let input = "prefix\x1b[200~Extract this\x1b[201~suffix";
        let content = PasteDetector::extract_paste(input);

        assert!(content.is_some());
        assert_eq!(content.unwrap(), "Extract this");
    }

    #[test]
    fn test_detector_extract_paste_none() {
        let input = "no paste here";
        let content = PasteDetector::extract_paste(input);

        assert!(content.is_none());
    }

    #[test]
    fn test_detector_extract_paste_incomplete() {
        let input = "\x1b[200~no end sequence";
        let content = PasteDetector::extract_paste(input);

        assert!(content.is_none());
    }

    #[test]
    fn test_detector_text_before_paste() {
        let mut detector = PasteDetector::new();
        let input = "some text before\x1b[200~paste content\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "paste content");
    }

    #[test]
    fn test_detector_text_after_paste() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~paste content\x1b[201~some text after";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "paste content");
    }

    // ==========================================================================
    // PasteHandler tests
    // ==========================================================================

    #[test]
    fn test_handler_new() {
        let handler = PasteHandler::new();
        assert!(!handler.has_callback());
    }

    #[test]
    fn test_handler_default() {
        let handler = PasteHandler::default();
        assert!(!handler.has_callback());
    }

    #[test]
    fn test_handler_with_callback() {
        let handler = PasteHandler::with_callback(|_| {});
        assert!(handler.has_callback());
    }

    #[test]
    fn test_handler_set_callback() {
        let mut handler = PasteHandler::new();
        assert!(!handler.has_callback());

        handler.set_callback(|_| {});
        assert!(handler.has_callback());
    }

    #[test]
    fn test_handler_clear_callback() {
        let mut handler = PasteHandler::with_callback(|_| {});
        assert!(handler.has_callback());

        handler.clear_callback();
        assert!(!handler.has_callback());
    }

    #[test]
    fn test_handler_callback_invoked() {
        use std::sync::{Arc, Mutex};

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let mut handler = PasteHandler::with_callback(move |event| {
            received_clone.lock().unwrap().push(event.into_content());
        });

        handler.feed("\x1b[200~First\x1b[201~");
        handler.feed("\x1b[200~Second\x1b[201~");

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0], "First");
        assert_eq!(received[1], "Second");
    }

    #[test]
    fn test_handler_returns_event_without_callback() {
        let mut handler = PasteHandler::new();

        let event = handler.feed("\x1b[200~content\x1b[201~");

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "content");
    }

    #[test]
    fn test_handler_reset() {
        let mut handler = PasteHandler::new();

        handler.feed("\x1b[200~partial");
        assert!(handler.detector().is_in_paste());

        handler.reset();
        assert!(!handler.detector().is_in_paste());
    }

    #[test]
    fn test_handler_detector_access() {
        let mut handler = PasteHandler::new();

        handler.feed("\x1b[200~in paste");
        assert!(handler.detector().is_in_paste());

        handler.detector_mut().reset();
        assert!(!handler.detector().is_in_paste());
    }

    // ==========================================================================
    // UsePaste tests
    // ==========================================================================

    #[test]
    fn test_use_paste_new() {
        let paste = UsePaste::new();
        assert!(!paste.is_enabled());
    }

    #[test]
    fn test_use_paste_default() {
        let paste = UsePaste::default();
        assert!(!paste.is_enabled());
    }

    #[test]
    fn test_use_paste_with_callback() {
        let paste = UsePaste::with_callback(|_| {});
        assert!(paste.handler().has_callback());
    }

    #[test]
    fn test_use_paste_enable_disable() {
        let mut buffer = Vec::new();
        let mut paste = UsePaste::new();

        paste.enable_to(&mut buffer).unwrap();
        assert!(paste.is_enabled());

        paste.disable_to(&mut buffer).unwrap();
        assert!(!paste.is_enabled());
    }

    #[test]
    fn test_use_paste_on_paste() {
        use std::sync::{Arc, Mutex};

        let received = Arc::new(Mutex::new(None));
        let received_clone = received.clone();

        let mut paste = UsePaste::new();
        paste.on_paste(move |event| {
            *received_clone.lock().unwrap() = Some(event.into_content());
        });

        paste.feed("\x1b[200~callback test\x1b[201~");

        let received = received.lock().unwrap();
        assert!(received.is_some());
        assert_eq!(received.as_ref().unwrap(), "callback test");
    }

    #[test]
    fn test_use_paste_feed() {
        let mut paste = UsePaste::new();

        let event = paste.feed("\x1b[200~feed test\x1b[201~");

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "feed test");
    }

    #[test]
    fn test_use_paste_reset() {
        let mut paste = UsePaste::new();

        paste.feed("\x1b[200~partial");
        paste.reset();

        // After reset, should not be in paste state
        assert!(!paste.handler().detector().is_in_paste());
    }

    #[test]
    fn test_use_paste_mode_access() {
        let mut buffer = Vec::new();
        let mut paste = UsePaste::new();

        paste.enable_to(&mut buffer).unwrap();
        assert!(paste.mode().is_enabled());
    }

    // ==========================================================================
    // Constants tests
    // ==========================================================================

    #[test]
    fn test_escape_sequences() {
        assert_eq!(ENABLE_BRACKETED_PASTE, "\x1b[?2004h");
        assert_eq!(DISABLE_BRACKETED_PASTE, "\x1b[?2004l");
        assert_eq!(PASTE_START, "\x1b[200~");
        assert_eq!(PASTE_END, "\x1b[201~");
    }

    #[test]
    fn test_escape_sequence_lengths() {
        // Verify expected lengths for the sequences
        assert_eq!(ENABLE_BRACKETED_PASTE.len(), 8);
        assert_eq!(DISABLE_BRACKETED_PASTE.len(), 8);
        assert_eq!(PASTE_START.len(), 6);
        assert_eq!(PASTE_END.len(), 6);
    }

    // ==========================================================================
    // Edge case tests
    // ==========================================================================

    #[test]
    fn test_nested_like_sequences() {
        // Test that sequences inside paste content don't cause issues
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~Text with \x1b[ other escapes\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Text with \x1b[ other escapes");
    }

    #[test]
    fn test_very_long_paste() {
        let mut detector = PasteDetector::new();
        let long_content = "x".repeat(100_000);
        let input = format!("\x1b[200~{}\x1b[201~", long_content);

        let event = detector.feed(&input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().len(), 100_000);
    }

    #[test]
    fn test_paste_with_escape_in_content() {
        let mut detector = PasteDetector::new();
        // Content contains \x1b but not the full end sequence
        let input = "\x1b[200~Contains \x1b but not end\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Contains \x1b but not end");
    }

    #[test]
    fn test_paste_with_partial_end_in_content() {
        let mut detector = PasteDetector::new();
        // Content contains partial end sequence
        let input = "\x1b[200~Contains \x1b[201 partial\x1b[201~";

        let event = detector.feed(input);

        assert!(event.is_some());
        assert_eq!(event.unwrap().content(), "Contains \x1b[201 partial");
    }

    #[test]
    fn test_byte_by_byte_feeding() {
        let mut detector = PasteDetector::new();
        let input = "\x1b[200~AB\x1b[201~";

        for (i, byte) in input.bytes().enumerate() {
            let s = String::from_utf8(vec![byte]).unwrap();
            let event = detector.feed(&s);

            if i == input.len() - 1 {
                // Last byte should complete the paste
                assert!(event.is_some());
                assert_eq!(event.unwrap().content(), "AB");
            } else {
                assert!(event.is_none());
            }
        }
    }

    #[test]
    fn test_handler_debug() {
        let handler = PasteHandler::new();
        let debug_str = format!("{:?}", handler);
        assert!(debug_str.contains("PasteHandler"));
        assert!(debug_str.contains("has_callback"));
    }

    #[test]
    fn test_use_paste_debug() {
        let paste = UsePaste::new();
        let debug_str = format!("{:?}", paste);
        assert!(debug_str.contains("UsePaste"));
    }
}
