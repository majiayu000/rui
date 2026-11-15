//! Encoding a planned frame into one render pass, and the retained copy.

use super::ImageTexture;
use super::{WgpuFrameStats, WgpuRenderer};
use crate::core::geometry::{Bounds, Size};
use crate::renderer::plan::{Draw, FrameData, ScissorRect, clear_linear};
use crate::renderer::{RendererError, RendererResourceError, RendererResourceKind};
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::Ordering;

/// Offsets of the arrays within the frame buffer; the largest value wgpu
/// allows for `min_storage_buffer_offset_alignment`, so every device accepts it.
const OFFSET_ALIGNMENT: usize = 256;

/// Smallest frame buffer allocated, so small frames reuse one buffer.
const MIN_FRAME_BUFFER_BYTES: usize = 64 * 1024;

/// Uniform data passed to shaders
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    viewport_size: [f32; 2],
    drawable_size: [f32; 2],
}

/// A frame drawn into a kept texture and copied to `copy_to`.
pub(super) struct Retained<'a> {
    /// What to draw again, in logical pixels; `None` draws everything.
    pub damage: Option<Bounds>,
    pub copy_to: &'a wgpu::TextureView,
}

/// Appends `values` to `bytes` at the next aligned offset and returns the
/// binding: at least one element, since a binding may not be empty.
fn reserve<T: Pod>(bytes: &mut Vec<u8>, values: &[T]) -> (u64, u64) {
    let offset = bytes.len().next_multiple_of(OFFSET_ALIGNMENT);
    bytes.resize(offset, 0);
    if values.is_empty() {
        bytes.resize(offset + std::mem::size_of::<T>(), 0);
    } else {
        bytes.extend_from_slice(bytemuck::cast_slice(values));
    }
    (offset as u64, (bytes.len() - offset) as u64)
}

/// [`reserve`] for values in parts, laid out one after another.
fn reserve_parts<'a, T: Pod>(
    bytes: &mut Vec<u8>,
    parts: impl Iterator<Item = &'a [T]>,
) -> (u64, u64) {
    let offset = bytes.len().next_multiple_of(OFFSET_ALIGNMENT);
    bytes.resize(offset, 0);
    for part in parts {
        bytes.extend_from_slice(bytemuck::cast_slice(part));
    }
    if bytes.len() == offset {
        bytes.resize(offset + std::mem::size_of::<T>(), 0);
    }
    (offset as u64, (bytes.len() - offset) as u64)
}

fn binding(buffer: &wgpu::Buffer, (offset, size): (u64, u64)) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        buffer,
        offset,
        size: wgpu::BufferSize::new(size),
    })
}

fn scissor(pass: &mut wgpu::RenderPass<'_>, rect: ScissorRect) {
    pass.set_scissor_rect(
        rect.x as u32,
        rect.y as u32,
        rect.width as u32,
        rect.height as u32,
    );
}

impl WgpuRenderer {
    pub(super) fn encode_frame(
        &mut self,
        frame: &FrameData<ImageTexture>,
        view: &wgpu::TextureView,
        extent: (u64, u64),
        viewport_size: Size,
        retained: Option<&Retained<'_>>,
    ) -> Result<(), RendererError> {
        // Glyphs placed while building are written into their pages before
        // this frame's commands run.
        let epoch = self.planner.text_cache.epoch();
        let text = &mut self.planner.text_cache;
        let (cleared, uploads) = (text.take_cleared_pages(), text.take_uploads());
        self.glyph_pages.sync(
            &self.device,
            &self.queue,
            epoch,
            text.atlas(),
            cleared,
            uploads,
        );
        let (cleared, uploads) = (text.take_cleared_color_pages(), text.take_color_uploads());
        self.color_pages.sync(
            &self.device,
            &self.queue,
            epoch,
            text.color_atlas(),
            cleared,
            uploads,
        );
        let generations = (self.glyph_pages.generation(), self.color_pages.generation());
        if generations != self.page_generations {
            self.page_groups.clear();
            self.page_generations = generations;
        }

        let clear = clear_linear();
        let drawable_size = Size::new(extent.0 as f32, extent.1 as f32);
        let uniforms = Uniforms {
            viewport_size: [viewport_size.width, viewport_size.height],
            drawable_size: [drawable_size.width, drawable_size.height],
        };
        let clear_color = [clear as f32, clear as f32, clear as f32, 1.0f32];

        // All of the frame's data goes in one buffer.
        let mut bytes = std::mem::take(&mut self.frame_bytes);
        bytes.clear();
        let uniforms_at = reserve(&mut bytes, &[uniforms]);
        let clear_at = reserve(&mut bytes, &[clear_color]);
        let sprite_data = [
            reserve(&mut bytes, &frame.plan.refs),
            reserve_parts(&mut bytes, frame.quad_parts()),
            reserve(&mut bytes, &frame.shadows),
            reserve(&mut bytes, &frame.glyphs),
            reserve(&mut bytes, &frame.clip_edges),
            reserve(&mut bytes, &frame.path_tiles),
            reserve(&mut bytes, &frame.path_edges),
        ];
        let instances: Vec<_> = frame.images.iter().map(|image| image.instance).collect();
        let images_at = reserve(&mut bytes, &instances);
        let buffer = self.frame_buffer(bytes.len());
        self.queue.write_buffer(&buffer, 0, &bytes);
        self.frame_bytes = bytes;

        let sprites = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rui sprites"),
            layout: &self.layouts.sprites,
            entries: &std::iter::once(uniforms_at)
                .chain(sprite_data)
                .enumerate()
                .map(|(index, at)| wgpu::BindGroupEntry {
                    binding: index as u32,
                    resource: binding(&buffer, at),
                })
                .collect::<Vec<_>>(),
        });
        let images = (!frame.images.is_empty()).then(|| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rui images"),
                layout: &self.layouts.images,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: binding(&buffer, uniforms_at),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: binding(&buffer, images_at),
                    },
                ],
            })
        });
        for draw in &frame.plan.draws {
            if let Draw::Sprites { pages, .. } = *draw {
                self.page_group(pages.coverage, pages.color)?;
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rui") });
        let mut stats = WgpuFrameStats::default();
        {
            // A retained frame keeps what the target shows outside the damage.
            let load = if frame.damage.is_some() {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color {
                    r: clear,
                    g: clear,
                    b: clear,
                    a: 1.0,
                })
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rui frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            // Clip space maps onto the drawn extent, which the target may exceed.
            pass.set_viewport(
                0.0,
                0.0,
                drawable_size.width,
                drawable_size.height,
                0.0,
                1.0,
            );
            let full = frame.scissors[0];
            scissor(&mut pass, full);
            stats.redrawn_pixels = full.width * full.height;
            if frame.damage.is_some() && full.width > 0 && full.height > 0 {
                // The damage starts as a cleared target would.
                let clear_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("rui clear"),
                    layout: &self.layouts.clear,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 1,
                        resource: binding(&buffer, clear_at),
                    }],
                });
                pass.set_pipeline(&self.clear_pipeline);
                pass.set_bind_group(0, &clear_group, &[]);
                pass.draw(0..3, 0..1);
                count(&mut stats, 1);
            }
            let mut clip = 0;
            let mut sprites_bound = false;
            for &draw in &frame.plan.draws {
                match draw {
                    // One pipeline draws every sprite here, simple or not.
                    Draw::Sprites {
                        pages,
                        start,
                        count: instances,
                        ..
                    } => {
                        if !sprites_bound {
                            pass.set_pipeline(&self.sprite_pipeline);
                            pass.set_bind_group(0, &sprites, &[]);
                            sprites_bound = true;
                        }
                        if clip != 0 {
                            scissor(&mut pass, full);
                            clip = 0;
                        }
                        pass.set_bind_group(
                            1,
                            &self.page_groups[&(pages.coverage, pages.color)],
                            &[],
                        );
                        pass.draw(0..6, start..start + instances);
                        count(&mut stats, instances);
                    }
                    Draw::Image(index) => {
                        let image = &frame.images[index as usize];
                        if clip != image.clip {
                            scissor(&mut pass, frame.scissors[image.clip as usize]);
                            clip = image.clip;
                        }
                        pass.set_pipeline(&self.image_pipeline);
                        if let Some(images) = &images {
                            pass.set_bind_group(0, images, &[]);
                        }
                        pass.set_bind_group(1, &image.texture.bind_group, &[]);
                        pass.draw(0..6, index..index + 1);
                        count(&mut stats, 1);
                        sprites_bound = false;
                    }
                }
            }
        }
        if let Some(retained) = retained {
            let source = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rui copy"),
                layout: &self.layouts.copy,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                }],
            });
            let mut copy = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rui copy"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: retained.copy_to,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            // Only the frame's extent: `copy_to` may be larger.
            copy.set_viewport(
                0.0,
                0.0,
                drawable_size.width,
                drawable_size.height,
                0.0,
                1.0,
            );
            copy.set_scissor_rect(0, 0, extent.0 as u32, extent.1 as u32);
            copy.set_pipeline(&self.copy_pipeline);
            copy.set_bind_group(0, &source, &[]);
            copy.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        let submitted = self.planner.text_cache.frame();
        self.submitted_frame = submitted;
        let completed = self.completed_frame.clone();
        self.queue.on_submitted_work_done(move || {
            completed.fetch_max(submitted, Ordering::AcqRel);
        });
        self.frame_stats = stats;
        Ok(())
    }

    /// The bind group of one coverage page and one color page, made once per
    /// pair until a page is replaced.
    fn page_group(
        &mut self,
        coverage: Option<u32>,
        color: Option<u32>,
    ) -> Result<(), RendererError> {
        if self.page_groups.contains_key(&(coverage, color)) {
            return Ok(());
        }
        let coverage_view = page_view(&self.glyph_pages, coverage, &self.empty_page)?;
        let color_view = page_view(&self.color_pages, color, &self.empty_color_page)?;
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rui pages"),
            layout: &self.layouts.pages,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(coverage_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.page_groups.insert((coverage, color), group);
        Ok(())
    }

    /// The frame buffer, grown to hold at least `bytes`. Queue writes are
    /// ordered after earlier submissions, so one buffer serves every frame.
    fn frame_buffer(&mut self, bytes: usize) -> wgpu::Buffer {
        match &self.frame_buffer {
            Some(buffer) if buffer.size() as usize >= bytes => buffer.clone(),
            _ => {
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rui frame"),
                    size: bytes.max(MIN_FRAME_BUFFER_BYTES).next_power_of_two() as u64,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.frame_buffer = Some(buffer.clone());
                buffer
            }
        }
    }
}

/// Page `page` of `pages`, or `empty` without one.
fn page_view<'a>(
    pages: &'a super::GlyphPages,
    page: Option<u32>,
    empty: &'a wgpu::TextureView,
) -> Result<&'a wgpu::TextureView, RendererError> {
    match page {
        Some(page) => pages.page(page as usize).ok_or_else(|| {
            RendererResourceError::missing(RendererResourceKind::Glyph, page).into()
        }),
        None => Ok(empty),
    }
}

fn count(stats: &mut WgpuFrameStats, instances: u32) {
    stats.draw_calls += 1;
    stats.instances += instances;
}
