//! Rendering without a window: pixel captures and offscreen frames.

use super::*;

impl MetalRenderer {
    pub fn capture_frame_pixels(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
    ) -> Result<Vec<u8>, RendererError> {
        self.capture_frame_pixels_at_scale(scene, viewport_size, 1.0)
    }

    /// Render and read back a frame at `scale_factor` device pixels per logical
    /// pixel. The pixels are `viewport_size * scale_factor` RGBA rows.
    pub fn capture_frame_pixels_at_scale(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
        scale_factor: f32,
    ) -> Result<Vec<u8>, RendererError> {
        let width = texture_extent(viewport_size.width * scale_factor, "width")?;
        let height = texture_extent(viewport_size.height * scale_factor, "height")?;
        let texture = create_capture_texture(&self.device, width, height);

        self.render_to_texture(
            scene,
            &texture,
            (width, height),
            viewport_size,
            None,
            true,
            None,
        )?;
        Ok(read_texture_rgba(&texture, width, height))
    }

    /// [`Self::capture_frame_pixels_at_scale`] through `target`, as a window
    /// presents: only the scene's damage is drawn again when `target` shows the
    /// previous frame.
    pub fn capture_retained_pixels_at_scale(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        viewport_size: Size,
        scale_factor: f32,
    ) -> Result<Vec<u8>, RendererError> {
        let width = texture_extent(viewport_size.width * scale_factor, "width")?;
        let height = texture_extent(viewport_size.height * scale_factor, "height")?;
        let capture = create_capture_texture(&self.device, width, height);
        self.render_retained_to(
            scene,
            target,
            &capture,
            (width, height),
            viewport_size,
            None,
            true,
        )?;
        Ok(read_texture_rgba(&capture, width, height))
    }

    /// Render a scene into a kept offscreen texture at `scale_factor` and commit
    /// without waiting, as a window's present does. For benchmarks.
    pub fn render_offscreen(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        let width = texture_extent(viewport_size.width * scale_factor, "width")?;
        let height = texture_extent(viewport_size.height * scale_factor, "height")?;
        let texture = self.offscreen_texture(width, height);
        // A window has a few drawables and waits for one when the GPU is behind;
        // offscreen frames wait the same way, rather than piling up in flight.
        // Every frame listed was committed, so each wait ends.
        while self.frames_in_flight.len() >= MAX_OFFSCREEN_FRAMES_IN_FLIGHT {
            let (_, oldest) = self.frames_in_flight.remove(0);
            oldest.wait_until_completed();
        }
        self.render_to_texture(
            scene,
            &texture,
            (width, height),
            viewport_size,
            None,
            false,
            None,
        )
    }

    /// [`Self::render_offscreen`], then wait for the GPU and return the command
    /// buffer's GPU time (`GPUEndTime - GPUStartTime`). For benchmarks.
    pub fn render_offscreen_gpu_time(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
        scale_factor: f32,
    ) -> Result<std::time::Duration, RendererError> {
        let width = texture_extent(viewport_size.width * scale_factor, "width")?;
        let height = texture_extent(viewport_size.height * scale_factor, "height")?;
        let texture = self.offscreen_texture(width, height);
        self.render_to_texture(
            scene,
            &texture,
            (width, height),
            viewport_size,
            None,
            true,
            None,
        )?;
        self.gpu_time
            .ok_or_else(|| RendererError::render_failed("the frame reported no GPU time"))
    }

    /// [`Self::render_offscreen_gpu_time`] through `target`, into the same kept
    /// offscreen texture, as a window presents. For benchmarks.
    pub fn render_retained_offscreen_gpu_time(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        viewport_size: Size,
        scale_factor: f32,
    ) -> Result<std::time::Duration, RendererError> {
        let width = texture_extent(viewport_size.width * scale_factor, "width")?;
        let height = texture_extent(viewport_size.height * scale_factor, "height")?;
        let texture = self.offscreen_texture(width, height);
        self.render_retained_to(
            scene,
            target,
            &texture,
            (width, height),
            viewport_size,
            None,
            true,
        )?;
        self.gpu_time
            .ok_or_else(|| RendererError::render_failed("the frame reported no GPU time"))
    }
}
