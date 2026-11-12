//! Metal rendering backend for macOS

mod batch;
mod capture;
mod glyph;
mod renderer;
mod retained;
mod shader_cache;
mod shader_hash;
mod shaders;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) use renderer::TARGET_PIXEL_FORMAT;
pub use renderer::{MetalFrameStats, MetalRenderer, PendingMetalRenderer};
pub use retained::RetainedTarget;
