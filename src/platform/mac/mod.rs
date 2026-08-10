//! macOS platform implementation using AppKit

mod accessibility;
mod app;
mod events;
mod frame;
mod ime_state;
mod lifecycle;
mod text_input;
mod window;

pub use accessibility::MacAccessibilityBridge;
pub use app::{run_app, run_app_with_options};
pub use window::{MacWindow, MacWindowBackend};
