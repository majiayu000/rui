//! Render targets: pixel captures and the texture a window keeps its last
//! frame in.

use super::encode::Retained;
use super::{TARGET_FORMAT, WgpuRenderer, check_format};
use crate::core::geometry::Size;
use crate::renderer::plan::RetainedFrame;
use crate::renderer::{RendererError, Scene};

/// A texture a window keeps its last frame in, so the next frame redraws only
/// the damage (`plan::retained`). One per window: it holds that window's pixels.
#[derive(Default)]
pub struct RetainedTarget {
    view: Option<wgpu::TextureView>,
    shown: RetainedFrame,
}

impl RetainedTarget {
    pub fn new() -> Self {
        Self::default()
    }

    /// The texture to draw `width` by `height` device pixels into, and the
    /// part of it `scene` has to draw again: `None` for all of it.
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        scene: &Scene,
        (width, height): (u32, u32),
        viewport_size: Size,
    ) -> (wgpu::TextureView, Option<crate::core::geometry::Bounds>) {
        let kept = self
            .view
            .as_ref()
            .filter(|view| view.texture().width() == width && view.texture().height() == height)
            .cloned();
        let damage = self.shown.damage(kept.is_some(), scene, viewport_size);
        let view = kept.unwrap_or_else(|| {
            let view = target_texture(
                device,
                width,
                height,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
            .create_view(&Default::default());
            self.view = Some(view.clone());
            view
        });
        (view, damage)
    }
}

fn target_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("rui target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET_FORMAT,
        usage,
        view_formats: &[],
    })
}

fn texture_extent(value: f32, axis: &str) -> Result<u32, RendererError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(RendererError::render_failed(format!(
            "offscreen capture {axis} must be positive and finite"
        )));
    }
    Ok(value.round().max(1.0) as u32)
}

impl WgpuRenderer {
    /// Render a scene to `output` through `target`, drawing again only what
    /// changed since the frame the target shows.
    pub fn render_retained(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        output: &wgpu::Texture,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        check_format(output)?;
        let view = output.create_view(&Default::default());
        self.render_retained_to(
            scene,
            target,
            &view,
            (output.width(), output.height()),
            viewport_size,
        )
    }

    fn render_retained_to(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        output: &wgpu::TextureView,
        extent: (u32, u32),
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        let (kept, damage) = target.prepare(&self.device, scene, extent, viewport_size);
        self.render_to_view(
            scene,
            &kept,
            (u64::from(extent.0), u64::from(extent.1)),
            viewport_size,
            Some(Retained {
                damage,
                copy_to: output,
            }),
        )?;
        target.shown.drew(scene);
        Ok(())
    }

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
        let texture = self.capture_texture(width, height);
        let view = texture.create_view(&Default::default());
        self.render_to_view(
            scene,
            &view,
            (u64::from(width), u64::from(height)),
            viewport_size,
            None,
        )?;
        self.read_rgba(&texture)
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
        let texture = self.capture_texture(width, height);
        let view = texture.create_view(&Default::default());
        self.render_retained_to(scene, target, &view, (width, height), viewport_size)?;
        self.read_rgba(&texture)
    }

    fn capture_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        target_texture(
            &self.device,
            width,
            height,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        )
    }

    /// Waits for the GPU and reads `texture` back as RGBA rows.
    fn read_rgba(&self, texture: &wgpu::Texture) -> Result<Vec<u8>, RendererError> {
        let (width, height) = (texture.width(), texture.height());
        let row = width * 4;
        let padded_row = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rui readback"),
            size: u64::from(padded_row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rui readback"),
            });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|err| RendererError::render_failed(format!("wgpu readback failed: {err}")))?;
        receiver
            .recv()
            .map_err(|_| RendererError::render_failed("wgpu readback was dropped"))?
            .map_err(|err| RendererError::render_failed(format!("wgpu readback failed: {err}")))?;
        let mapped = buffer.get_mapped_range(..);
        let mut pixels = Vec::with_capacity((row * height) as usize);
        for padded in mapped.chunks_exact(padded_row as usize) {
            pixels.extend_from_slice(&padded[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Ok(pixels)
    }
}
