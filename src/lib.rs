//! RUI - A GPU-accelerated UI framework for Rust
//!
//! RUI is a high-performance UI framework inspired by GPUI and Warp's architecture.
//! It renders directly to the GPU using Metal (macOS) for smooth 120fps rendering.
//!
//! # Architecture
//!
//! RUI follows a three-layer architecture:
//!
//! 1. **Core Layer** - Basic types (geometry, colors, styles)
//! 2. **Element Layer** - UI primitives (Div, Text, Image)
//! 3. **Renderer Layer** - GPU rendering (Metal/Vulkan)
//!
//! # Example
//!
//! ```no_run
//! use rui::prelude::*;
//!
//! fn main() {
//!     App::new().run(|cx| {
//!         div()
//!             .size(Size::new(200.0, 100.0))
//!             .bg(Color::rgb(0.2, 0.4, 0.8))
//!             .child(text("Hello, RUI!"))
//!     });
//! }
//! ```

pub mod core;
pub mod elements;
pub mod hooks;
pub mod renderer;

#[cfg(target_os = "macos")]
pub mod platform;

pub mod prelude;

// Re-exports
pub use core::*;
pub use elements::*;
pub use prelude::*;
