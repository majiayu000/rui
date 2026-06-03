//! Renderer runtime traits and test utilities.

use crate::core::geometry::Size;
use crate::renderer::resources::{
    RendererDiagnostics, RendererResourceError, RendererUnsupportedPrimitive,
};
use crate::renderer::{Primitive, PrimitiveKind, RendererBatchDiagnostics, Scene};
use std::error::Error;
use std::fmt;

/// Explicit renderer failure surfaced to the platform layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererError {
    BackendUnavailable(String),
    RenderFailed(String),
    UnsupportedPrimitive {
        backend: String,
        primitive: PrimitiveKind,
        reason: String,
    },
    Resource(RendererResourceError),
}

impl RendererError {
    pub fn backend_unavailable(message: impl Into<String>) -> Self {
        Self::BackendUnavailable(message.into())
    }

    pub fn render_failed(message: impl Into<String>) -> Self {
        Self::RenderFailed(message.into())
    }

    pub fn unsupported_primitive(
        backend: impl Into<String>,
        primitive: PrimitiveKind,
        reason: impl Into<String>,
    ) -> Self {
        Self::UnsupportedPrimitive {
            backend: backend.into(),
            primitive,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable(message) => {
                write!(f, "renderer backend unavailable: {}", message)
            }
            Self::RenderFailed(message) => write!(f, "renderer failed: {}", message),
            Self::UnsupportedPrimitive {
                backend,
                primitive,
                reason,
            } => write!(
                f,
                "{backend} renderer does not support {} primitive: {reason}",
                primitive.name()
            ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererPrimitiveSupport {
    backend: &'static str,
    path_reason: Option<&'static str>,
}

impl RendererPrimitiveSupport {
    pub const fn recording() -> Self {
        Self {
            backend: "recording",
            path_reason: None,
        }
    }

    pub const fn metal() -> Self {
        Self {
            backend: "metal",
            path_reason: None,
        }
    }

    pub fn backend(self) -> &'static str {
        self.backend
    }

    pub fn supports(self, primitive: PrimitiveKind) -> bool {
        self.unsupported_reason(primitive).is_none()
    }

    pub fn unsupported_reason(self, primitive: PrimitiveKind) -> Option<&'static str> {
        match primitive {
            PrimitiveKind::Path => self.path_reason,
            _ => None,
        }
    }

    pub fn unsupported_primitives(self) -> Vec<RendererUnsupportedPrimitive> {
        PrimitiveKind::ALL
            .iter()
            .filter_map(|kind| {
                self.unsupported_reason(*kind)
                    .map(|reason| RendererUnsupportedPrimitive::new(self.backend, *kind, reason))
            })
            .collect()
    }

    pub fn validate_scene(self, scene: &Scene) -> Result<(), RendererError> {
        for primitive in scene.primitives() {
            let kind = primitive.kind();
            if let Some(reason) = self.unsupported_reason(kind) {
                return Err(RendererError::unsupported_primitive(
                    self.backend,
                    kind,
                    reason,
                ));
            }
        }
        Ok(())
    }
}

/// Captured frame from `RecordingRenderer`.
#[derive(Debug, Clone)]
pub struct RecordedScene {
    pub viewport_size: Size,
    pub primitives: Vec<Primitive>,
    pub batch: RendererBatchDiagnostics,
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
        RendererPrimitiveSupport::recording().validate_scene(scene)?;

        if let Some(error) = self.next_error.take() {
            return Err(error);
        }

        self.frames.push(RecordedScene {
            viewport_size,
            primitives: scene.primitives().to_vec(),
            batch: RendererBatchDiagnostics::from_scene(scene),
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
    use crate::elements::text::TextAlign;
    use crate::renderer::PathVertex;
    use crate::{ImageFit, ImageSource};

    fn sample_quad() -> Primitive {
        Primitive::Quad {
            bounds: Bounds::from_xywh(0.0, 0.0, 10.0, 10.0),
            background: Rgba::new(1.0, 0.0, 0.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        }
    }

    fn sample_path() -> Primitive {
        Primitive::Path {
            vertices: vec![
                PathVertex::new(0.0, 0.0),
                PathVertex::new(10.0, 0.0),
                PathVertex::new(10.0, 10.0),
            ],
            color: Rgba::new(0.1, 0.2, 0.3, 1.0),
            stroke_width: Some(1.0),
        }
    }

    fn sample_all_primitives() -> Vec<Primitive> {
        vec![
            sample_quad(),
            Primitive::Shadow {
                bounds: Bounds::from_xywh(1.0, 2.0, 10.0, 10.0),
                corner_radii: Corners::ZERO,
                blur_radius: 3.0,
                color: Rgba::new(0.0, 0.0, 0.0, 0.25),
            },
            Primitive::LinearGradient {
                bounds: Bounds::from_xywh(2.0, 3.0, 10.0, 10.0),
                start: Rgba::new(1.0, 0.0, 0.0, 1.0),
                end: Rgba::new(0.0, 0.0, 1.0, 1.0),
                angle: 45.0,
                border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            },
            Primitive::RadialGradient {
                bounds: Bounds::from_xywh(3.0, 4.0, 10.0, 10.0),
                inner: Rgba::new(1.0, 1.0, 1.0, 1.0),
                outer: Rgba::new(0.0, 0.0, 0.0, 1.0),
                border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            },
            Primitive::Text {
                bounds: Bounds::from_xywh(4.0, 5.0, 80.0, 20.0),
                content: String::from("renderer"),
                color: Rgba::new(0.0, 0.0, 0.0, 1.0),
                font_size: 14.0,
                font_weight: 400,
                font_family: None,
                line_height: 1.2,
                align: TextAlign::Left,
            },
            Primitive::Image {
                bounds: Bounds::from_xywh(5.0, 6.0, 10.0, 10.0),
                source: ImageSource::Data {
                    data: vec![255, 0, 0, 255],
                    width: 1,
                    height: 1,
                },
                fit: ImageFit::Fill,
                corner_radii: Corners::ZERO,
                opacity: 1.0,
            },
            sample_path(),
            Primitive::PushClip {
                bounds: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
                corner_radii: Corners::ZERO,
            },
            Primitive::PopClip,
        ]
    }

    #[test]
    fn primitive_kinds_cover_renderer_contract() {
        let kinds: Vec<PrimitiveKind> = sample_all_primitives()
            .iter()
            .map(Primitive::kind)
            .collect();

        assert_eq!(kinds, PrimitiveKind::ALL.to_vec());
        assert_eq!(PrimitiveKind::Path.name(), "path");
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
        assert_eq!(renderer.frames()[0].batch.draw_count, 1);
    }

    #[test]
    fn recording_renderer_records_every_primitive_variant() {
        let primitives = sample_all_primitives();
        let mut scene = Scene::new();
        for primitive in primitives.clone() {
            scene.insert(primitive);
        }
        let mut renderer = RecordingRenderer::new();

        let result = renderer.render(&scene, &(), Size::new(120.0, 80.0));

        assert_eq!(result, Ok(()));
        let frame = &renderer.frames()[0];
        assert_eq!(frame.primitives.len(), primitives.len());
        assert_eq!(
            frame
                .primitives
                .iter()
                .map(Primitive::kind)
                .collect::<Vec<_>>(),
            primitives.iter().map(Primitive::kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn renderer_support_declares_supported_primitives() {
        let recording = RendererPrimitiveSupport::recording();
        let metal = RendererPrimitiveSupport::metal();

        for primitive in sample_all_primitives() {
            assert!(recording.supports(primitive.kind()));
        }
        assert!(metal.supports(PrimitiveKind::Quad));
        assert!(metal.supports(PrimitiveKind::Text));
        assert!(metal.supports(PrimitiveKind::Image));
        assert!(metal.supports(PrimitiveKind::Path));
        assert!(metal.supports(PrimitiveKind::PushClip));
        assert_eq!(metal.unsupported_reason(PrimitiveKind::Path), None);
        assert_eq!(metal.unsupported_primitives(), Vec::new());
    }

    #[test]
    fn metal_support_accepts_path_scene() {
        let mut scene = Scene::new();
        scene.insert(sample_path());

        let result = RendererPrimitiveSupport::metal().validate_scene(&scene);

        assert_eq!(result, Ok(()));
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
