//! macOS platform implementation using AppKit

mod app;
mod accessibility;
mod window;

pub use accessibility::MacAccessibilityBridge;
pub use app::{run_app, run_app_with_options};
