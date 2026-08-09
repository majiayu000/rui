//! macOS platform implementation using AppKit

mod accessibility;
mod app;
mod frame;
mod lifecycle;
mod window;

pub use accessibility::MacAccessibilityBridge;
pub use app::{run_app, run_app_with_options};
pub use window::{MacWindow, MacWindowBackend};
