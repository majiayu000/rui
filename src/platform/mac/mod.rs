//! macOS platform implementation using AppKit

mod accessibility;
mod app;
mod clipboard;
mod display_link;
mod events;
mod file_drop;
mod frame;
mod frame_ticker;
mod ime_state;
mod key_suppression;
mod lifecycle;
mod menus;
mod present_latency;
mod text_input;
mod window;

pub use accessibility::{
    MacAccessibilityActionRequest, MacAccessibilityBridge, MacAccessibilityRequest,
};
pub(crate) use app::{NativeWindowDriver, NativeWindowParts, native_window};
pub use app::{run_app, run_app_with_options};
pub use events::MacPlatformEvent;
pub use window::{MacWindow, MacWindowBackend};
