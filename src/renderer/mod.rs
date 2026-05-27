//! Rendering subsystem

mod primitives;
mod resources;
mod runtime;
mod scene;
pub mod text;
mod text_shaping;

#[cfg(target_os = "macos")]
pub mod metal;

pub use primitives::Primitive;
pub use resources::{
    GlyphResourceKey, ImageResourceEntry, ImageResourceKey, RendererDeviceDiagnostics,
    RendererDiagnostics, RendererImageCache, RendererResourceAllocation, RendererResourceCache,
    RendererResourceError, RendererResourceHandle, RendererResourceId, RendererResourceKind,
    RendererResourceStats,
};
pub use runtime::{RecordedScene, RecordingRenderer, Renderer, RendererError};
pub use scene::{HitRegion, Layer, LayerId, Scene, ZIndex};
