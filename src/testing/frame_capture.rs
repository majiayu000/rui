use crate::core::geometry::Size;
use crate::renderer::{RendererError, Scene};

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub viewport_size: Size,
    pub pixels: Vec<u8>,
}

pub trait FrameCaptureBackend {
    fn capture_frame(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
    ) -> Result<CapturedFrame, RendererError>;
}

#[derive(Debug, Default)]
pub struct MissingFrameCaptureBackend;

impl FrameCaptureBackend for MissingFrameCaptureBackend {
    fn capture_frame(
        &mut self,
        _scene: &Scene,
        _viewport_size: Size,
    ) -> Result<CapturedFrame, RendererError> {
        Err(RendererError::backend_unavailable(
            "frame capture requires a backend-specific offscreen target",
        ))
    }
}

pub fn capture_frame_with_backend(
    backend: &mut impl FrameCaptureBackend,
    scene: &Scene,
    viewport_size: Size,
) -> Result<CapturedFrame, RendererError> {
    backend.capture_frame(scene, viewport_size)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub struct MetalFrameCaptureBackend {
    renderer: crate::renderer::metal::MetalRenderer,
}

#[cfg(all(target_os = "macos", feature = "metal"))]
impl MetalFrameCaptureBackend {
    pub fn new() -> Result<Self, RendererError> {
        Ok(Self {
            renderer: crate::renderer::metal::MetalRenderer::new()?,
        })
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
impl FrameCaptureBackend for MetalFrameCaptureBackend {
    fn capture_frame(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
    ) -> Result<CapturedFrame, RendererError> {
        Ok(CapturedFrame {
            viewport_size,
            pixels: self.renderer.capture_frame_pixels(scene, viewport_size)?,
        })
    }
}
