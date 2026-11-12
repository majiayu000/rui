//! Rendering subsystem

#[cfg(any(all(target_os = "macos", feature = "metal"), feature = "wgpu"))]
mod plan;
mod primitives;
mod resources;
mod runtime;
mod scene;
mod system_fonts;
mod telemetry;
pub mod text;
mod text_ascii;
mod text_bidi;
mod text_classify;
mod text_cluster_position;
mod text_default_instance;
mod text_face_size;
mod text_fallback;
mod text_glyph_box;
mod text_plan;
mod text_shaping;

#[cfg(all(target_os = "macos", feature = "metal"))]
pub mod metal;
#[cfg(feature = "wgpu")]
pub mod wgpu;

pub use primitives::{PathVertex, Primitive, PrimitiveKind, Reach};
pub(crate) use primitives::{quad_reach, text_reach};
pub use resources::{
    GlyphResourceKey, ImageResourceEntry, ImageResourceKey, RendererDeviceDiagnostics,
    RendererDiagnostics, RendererImageCache, RendererResourceAllocation, RendererResourceCache,
    RendererResourceError, RendererResourceHandle, RendererResourceId, RendererResourceKind,
    RendererResourceLimits, RendererResourceStats, RendererUnsupportedPrimitive,
};
pub use runtime::{
    RecordedScene, RecordingRenderer, Renderer, RendererError, RendererPrimitiveSupport,
};
pub(crate) use scene::PaintRecord;
pub use scene::{HitRegion, Layer, LayerId, Scene, ZIndex};
pub use telemetry::{
    RUI_PROFILE_ENV, RendererBatchDiagnostics, RendererFramePhaseDurations, RendererFrameTelemetry,
    RendererTelemetryRecorder,
};
