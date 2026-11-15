//! Platform-specific implementations

#[cfg(any(all(target_os = "macos", feature = "metal"), feature = "winit"))]
mod frame_driver;
pub mod window;

#[cfg(all(target_os = "macos", feature = "metal"))]
pub mod mac;
#[cfg(feature = "winit")]
pub mod winit;

// The runner `App` starts: AppKit when the Metal backend is built for macOS,
// winit and wgpu everywhere else.
#[cfg(all(feature = "winit", not(all(target_os = "macos", feature = "metal"))))]
pub(crate) use self::winit::run_app_with_options as run_native_app;
#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) use mac::run_app_with_options as run_native_app;

pub use window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererAttachment, PlatformRendererTarget, PlatformWindow, PlatformWindowError,
    PlatformWindowEvent, PlatformWindowFeature, PlatformWindowFeatures, PlatformWindowState,
    UnsupportedPlatformWindow, validate_window_options,
};
