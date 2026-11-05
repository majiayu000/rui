//! Metal rendering backend for macOS

mod batch;
mod capture;
mod frame;
mod glyph;
mod image_primitive;
mod order;
mod path;
mod renderer;
mod retained;
mod shaders;
#[cfg(test)]
mod tests;

pub(crate) use renderer::TARGET_PIXEL_FORMAT;
pub use renderer::{MetalFrameStats, MetalRenderer, PendingMetalRenderer};
pub use retained::RetainedTarget;
