//! Renderer runtime traits and test utilities.

use crate::core::geometry::Size;
use crate::renderer::resources::{RendererDiagnostics, RendererResourceError};
use crate::renderer::{Primitive, Scene};
use std::error::Error;
use std::fmt;

/// Explicit renderer failure surfaced to the platform layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererError {
    BackendUnavailable(String),
    RenderFailed(String),
    Resource(RendererResourceError),
}

impl RendererError {
    pub fn backend_unavailable(message: impl Into<String>) -> Self {
        Self::BackendUnavailable(message.into())
    }

    pub fn render_failed(message: impl Into<String>) -> Self {
        Self::RenderFailed(message.into())
    }
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable(message) => {
                write!(f, "renderer backend unavailable: {}", message)
            }
            Self::RenderFailed(message) => write!(f, "renderer failed: {}", message),
            Self::Resource(err) => write!(f, "{err}"),
        }
    }
}

impl Error for RendererError {}

impl From<RendererResourceError> for RendererError {
    fn from(value: RendererResourceError) -> Self {
        Self::Resource(value)
    }
}

/// Backend-independent rendering contract.
pub trait Renderer {
    type Target: ?Sized;

    fn render(
        &mut self,
        scene: &Scene,
        target: &Self::Target,
        viewport_size: Size,
    ) -> Result<(), RendererError>;

    fn diagnostics(&self) -> RendererDiagnostics {
        RendererDiagnostics::headless("renderer")
    }
}

/// Captured frame from `RecordingRenderer`.
#[derive(Debug, Clone)]
pub struct RecordedScene {
    pub viewport_size: Size,
    pub primitives: Vec<Primitive>,
}

/// Renderer implementation for tests that does not allocate platform resources.
#[derive(Debug, Default)]
pub struct RecordingRenderer {
    frames: Vec<RecordedScene>,
    next_error: Option<RendererError>,
}

impl RecordingRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_next(&mut self, error: RendererError) {
        self.next_error = Some(error);
    }

    pub fn frames(&self) -> &[RecordedScene] {
        &self.frames
    }
}

impl Renderer for RecordingRenderer {
    type Target = ();

    fn render(
        &mut self,
        scene: &Scene,
        _target: &Self::Target,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        if let Some(error) = self.next_error.take() {
            return Err(error);
        }

        self.frames.push(RecordedScene {
            viewport_size,
            primitives: scene.primitives().to_vec(),
        });
        Ok(())
    }

    fn diagnostics(&self) -> RendererDiagnostics {
        RendererDiagnostics::headless("recording")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::color::Rgba;
    use crate::core::geometry::{Bounds, Edges};
    use crate::core::style::Corners;

    fn sample_quad() -> Primitive {
        Primitive::Quad {
            bounds: Bounds::from_xywh(0.0, 0.0, 10.0, 10.0),
            background: Rgba::new(1.0, 0.0, 0.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        }
    }

    #[test]
    fn renderer_records_scene_without_backend_resources() {
        let mut scene = Scene::new();
        scene.insert(sample_quad());
        let mut renderer = RecordingRenderer::new();
        let viewport_size = Size::new(120.0, 80.0);

        let result = renderer.render(&scene, &(), viewport_size);

        assert_eq!(result, Ok(()));
        assert_eq!(renderer.frames().len(), 1);
        assert_eq!(renderer.frames()[0].viewport_size, viewport_size);
        assert_eq!(renderer.frames()[0].primitives.len(), 1);
    }

    #[test]
    fn renderer_returns_explicit_errors_without_recording_frame() {
        let scene = Scene::new();
        let mut renderer = RecordingRenderer::new();
        let error = RendererError::render_failed("synthetic failure");

        renderer.fail_next(error.clone());
        let result = renderer.render(&scene, &(), Size::new(10.0, 10.0));

        assert_eq!(result, Err(error));
        assert!(renderer.frames().is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_renderer_implements_renderer_trait() {
        fn assert_renderer<R: Renderer<Target = metal::MetalDrawableRef>>() {}

        assert_renderer::<crate::renderer::metal::MetalRenderer>();
    }
}
