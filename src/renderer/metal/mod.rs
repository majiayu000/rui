//! Metal rendering backend for macOS

mod batch;
mod capture;
mod glyph;
mod image_primitive;
mod path;
mod renderer;
mod shaders;

pub use renderer::MetalRenderer;
pub(crate) use renderer::TARGET_PIXEL_FORMAT;
