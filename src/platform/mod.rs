//! Platform-specific implementations

pub mod window;

#[cfg(target_os = "macos")]
pub mod mac;

pub use window::{
    validate_window_options, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererAttachment, PlatformRendererTarget, PlatformWindow, PlatformWindowError,
    PlatformWindowEvent, PlatformWindowFeature, PlatformWindowFeatures, PlatformWindowState,
    UnsupportedPlatformWindow,
};
