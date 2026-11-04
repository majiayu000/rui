//! Metal rendering backend for macOS

mod batch;
mod capture;
mod glyph;
mod image_primitive;
mod path;
mod renderer;
mod shaders;

pub use renderer::MetalRenderer;
