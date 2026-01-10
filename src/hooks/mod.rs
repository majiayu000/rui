//! Hooks module - React-like hooks for RUI
//!
//! This module provides hooks for handling various terminal events
//! and state management in a declarative way.

pub mod use_mouse;
pub mod use_paste;
pub mod use_window_focus;

pub use use_mouse::{
    MouseCallback, MouseCallbackId, TerminalMouseButton, TerminalMouseEvent,
    TerminalMouseEventKind, UseMouse,
};
pub use use_paste::{
    BracketedPasteMode, PasteDetector, PasteEvent, PasteHandler, UsePaste,
};
pub use use_window_focus::{
    FocusCallback, FocusDetector, FocusReporting, UseWindowFocus, WindowFocusEvent,
};
