//! Presenting frames to a window: a `wgpu::Surface` configured for
//! [`TARGET_FORMAT`] and drawn through a [`RetainedTarget`].

use super::target::RetainedTarget;
use super::{TARGET_FORMAT, WgpuRenderer, request_adapter};
use crate::core::geometry::Size;
use crate::renderer::{RendererError, RendererResourceLimits, Scene};

/// A window's swapchain.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl WgpuRenderer {
    /// A renderer on an adapter that can present to `surface`, and the surface
    /// configured for it at `width` by `height` device pixels.
    pub fn with_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        (width, height): (u32, u32),
        limits: RendererResourceLimits,
    ) -> Result<(Self, WindowSurface), RendererError> {
        let adapter = request_adapter(instance, Some(&surface))?;
        let renderer = Self::on_adapter(&adapter, limits)?;
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or_else(|| {
                RendererError::backend_unavailable("the wgpu adapter cannot present to the window")
            })?;
        let formats = surface.get_capabilities(&adapter).formats;
        // Pipelines draw TARGET_FORMAT; a surface without it still takes an
        // sRGB view of its linear twin.
        let linear = TARGET_FORMAT.remove_srgb_suffix();
        config.format = if formats.contains(&TARGET_FORMAT) {
            TARGET_FORMAT
        } else if formats.contains(&linear) {
            linear
        } else {
            return Err(RendererError::backend_unavailable(format!(
                "the window surface supports neither {TARGET_FORMAT:?} nor {linear:?}: {formats:?}"
            )));
        };
        config.view_formats = vec![TARGET_FORMAT];
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(renderer.device(), &config);
        Ok((renderer, WindowSurface { surface, config }))
    }
}

impl WindowSurface {
    /// Size of the swapchain in device pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Resize the swapchain to `width` by `height` device pixels.
    pub fn resize(&mut self, renderer: &WgpuRenderer, (width, height): (u32, u32)) {
        let size = (width.max(1), height.max(1));
        if size == self.size() {
            return;
        }
        (self.config.width, self.config.height) = size;
        self.surface.configure(renderer.device(), &self.config);
    }

    /// Draw `scene` through `target` and present it. Returns `false` when the
    /// surface gave no frame (occluded, timed out or reconfigured); the caller
    /// tries again on a later frame.
    pub fn present(
        &mut self,
        renderer: &mut WgpuRenderer,
        scene: &Scene,
        target: &mut RetainedTarget,
        viewport_size: Size,
    ) -> Result<bool, RendererError> {
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(renderer.device(), &self.config);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RendererError::render_failed(
                    "wgpu rejected the window surface texture",
                ));
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(TARGET_FORMAT),
            ..Default::default()
        });
        renderer.render_retained_to(scene, target, &view, self.size(), viewport_size)?;
        frame.present();
        if suboptimal {
            self.surface.configure(renderer.device(), &self.config);
        }
        Ok(true)
    }
}
