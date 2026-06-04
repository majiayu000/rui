//! Rendering subsystem

mod primitives;
mod resources;
mod runtime;
mod scene;
mod telemetry;
pub mod text;
mod text_cluster_position;
mod text_shaping;

#[cfg(target_os = "macos")]
pub mod metal;

pub use primitives::{PathVertex, Primitive, PrimitiveKind};
pub use resources::{
    GlyphResourceKey, ImageResourceEntry, ImageResourceKey, RendererDeviceDiagnostics,
    RendererDiagnostics, RendererImageCache, RendererResourceAllocation, RendererResourceCache,
    RendererResourceError, RendererResourceHandle, RendererResourceId, RendererResourceKind,
    RendererResourceLimits, RendererResourceStats, RendererUnsupportedPrimitive,
};
pub use runtime::{
    RecordedScene, RecordingRenderer, Renderer, RendererError, RendererPrimitiveSupport,
};
pub use scene::{HitRegion, Layer, LayerId, Scene, ZIndex};
pub use telemetry::{
    RUI_PROFILE_ENV, RendererBatchDiagnostics, RendererFramePhaseDurations, RendererFrameTelemetry,
    RendererTelemetryRecorder,
};
