//! Encoding a planned frame into render passes.

use super::*;

impl MetalRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_frame(
        &mut self,
        frame: &mut FrameData,
        texture: &TextureRef,
        drawable_size: Size,
        viewport_size: Size,
        present_drawable: Option<&MetalDrawableRef>,
        wait_until_completed: bool,
        retained: Option<&Retained<'_>>,
    ) -> Result<(), RendererError> {
        // Glyphs placed while building are copied into their pages before any
        // draw samples them.

        let command_queue = self.command_queue.to_owned();
        let command_buffer = command_queue.new_command_buffer();
        let epoch = self.text_cache.epoch();
        let (cleared, uploads) = (
            self.text_cache.take_cleared_pages(),
            self.text_cache.take_uploads(),
        );
        let staging = self.glyph_pages.sync(
            &self.device,
            epoch,
            self.text_cache.atlas(),
            cleared,
            uploads,
            command_buffer,
            &mut self.instances,
        );
        let (cleared, uploads) = (
            self.text_cache.take_cleared_color_pages(),
            self.text_cache.take_color_uploads(),
        );
        let color_staging = self.color_pages.sync(
            &self.device,
            epoch,
            self.text_cache.color_atlas(),
            cleared,
            uploads,
            command_buffer,
            &mut self.instances,
        );

        let render_pass_desc = RenderPassDescriptor::new();
        let color_attachment = render_pass_desc.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(texture));
        color_attachment.set_store_action(MTLStoreAction::Store);
        // The clear color is linear; this is 0.1 in sRGB, as before.
        let clear = srgb_to_linear(0.1);
        // A retained frame keeps what the target shows outside the damage.
        let damage = frame.damage;
        if damage.is_some() {
            color_attachment.set_load_action(MTLLoadAction::Load);
        } else {
            color_attachment.set_load_action(MTLLoadAction::Clear);
            color_attachment.set_clear_color(MTLClearColor::new(clear, clear, clear, 1.0));
        }

        let encoder = command_buffer.new_render_command_encoder(render_pass_desc);
        // Clip space maps onto the drawn extent, which a kept texture may exceed.
        encoder.set_viewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: f64::from(drawable_size.width),
            height: f64::from(drawable_size.height),
            znear: 0.0,
            zfar: 1.0,
        });

        let uniforms = Uniforms {
            viewport_size: [viewport_size.width, viewport_size.height],
            drawable_size: [drawable_size.width, drawable_size.height],
        };
        let full = frame.scissors[0];
        encoder.set_scissor_rect(full);
        self.frame_stats.redrawn_pixels = full.width * full.height;
        if damage.is_some() && full.width > 0 && full.height > 0 {
            // The damage starts as a cleared target would.
            encoder.set_render_pipeline_state(&self.retained_pipelines.clear);
            let color = [clear as f32, clear as f32, clear as f32, 1.0f32];
            encoder.set_fragment_bytes(
                0,
                std::mem::size_of_val(&color) as u64,
                color.as_ptr() as *const _,
            );
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
            self.count_draw(1);
        }
        let mut scissor = 0;
        // Sprite references live at buffer 0, which image and path draws rebind.
        let mut sprites_bound = false;

        // All instance data of the frame goes in one buffer from the pool.
        let mut layout = BufferLayout::default();
        let refs_at = layout.reserve(&frame.plan.refs);
        let sprite_data = [
            (2, layout.reserve(&frame.quads)),
            (3, layout.reserve(&frame.shadows)),
            (4, layout.reserve(&frame.glyphs)),
            (5, layout.reserve(&frame.clip_edges)),
        ];
        let paths_at = layout.reserve(&frame.path_tiles);
        let edges_at = layout.reserve(&frame.path_edges);
        let instances = self.instances.acquire(&self.device, layout.bytes());
        write_slice(&instances, refs_at, &frame.plan.refs);
        write_slice(&instances, sprite_data[0].1, &frame.quads);
        write_slice(&instances, sprite_data[1].1, &frame.shadows);
        write_slice(&instances, sprite_data[2].1, &frame.glyphs);
        write_slice(&instances, sprite_data[3].1, &frame.clip_edges);
        write_slice(&instances, paths_at, &frame.path_tiles);
        write_slice(&instances, edges_at, &frame.path_edges);
        // Fragments read quads, shadows and clips by the index their vertices pass.
        for (index, offset) in sprite_data {
            encoder.set_vertex_buffer(index, Some(&instances), offset as u64);
            encoder.set_fragment_buffer(index, Some(&instances), offset as u64);
        }
        encoder.set_vertex_buffer(6, Some(&instances), paths_at as u64);
        encoder.set_fragment_buffer(0, Some(&instances), edges_at as u64);
        encoder.set_fragment_buffer(1, Some(&instances), paths_at as u64);

        for &draw in &frame.plan.draws {
            match draw {
                Draw::Sprites {
                    pages,
                    start,
                    count,
                } => {
                    if !sprites_bound {
                        encoder.set_render_pipeline_state(&self.sprite_pipeline);
                        encoder.set_vertex_buffer(0, Some(&instances), refs_at as u64);
                        bind_vertex_value(encoder, 1, &uniforms);
                        encoder.set_fragment_sampler_state(0, Some(&self.sampler));
                        sprites_bound = true;
                    }
                    if scissor != 0 {
                        encoder.set_scissor_rect(full);
                        scissor = 0;
                    }
                    let page_texture = match pages.coverage {
                        Some(page) => self.glyph_pages.page(page as usize).ok_or_else(|| {
                            RendererResourceError::missing(RendererResourceKind::Glyph, page)
                        })?,
                        None => &self.empty_page,
                    };
                    let color_texture = match pages.color {
                        Some(page) => self.color_pages.page(page as usize).ok_or_else(|| {
                            RendererResourceError::missing(RendererResourceKind::Glyph, page)
                        })?,
                        None => &self.empty_color_page,
                    };
                    encoder.set_fragment_texture(0, Some(page_texture));
                    encoder.set_fragment_texture(1, Some(color_texture));
                    encoder.draw_primitives_instanced_base_instance(
                        MTLPrimitiveType::Triangle,
                        0,
                        6,
                        u64::from(count),
                        u64::from(start),
                    );
                    self.count_draw(count as usize);
                }
                Draw::Image(index) => {
                    let image = &frame.images[index as usize];
                    if scissor != image.clip {
                        encoder.set_scissor_rect(frame.scissors[image.clip as usize]);
                        scissor = image.clip;
                    }
                    encoder.set_render_pipeline_state(&self.image_pipeline);
                    bind_vertex_value(encoder, 0, &image.instance);
                    bind_vertex_value(encoder, 1, &uniforms);
                    encoder.set_fragment_texture(0, Some(&image.texture));
                    encoder.set_fragment_sampler_state(0, Some(&self.sampler));
                    encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, 1);
                    self.count_draw(1);
                    sprites_bound = false;
                }
            }
        }

        encoder.end_encoding();
        if let Some(retained) = retained {
            // A render pass, not a blit: see `retained`.
            let copy_pass = RenderPassDescriptor::new();
            let attachment = copy_pass.color_attachments().object_at(0).ok_or_else(|| {
                RendererError::render_failed("render pass has no color attachment")
            })?;
            attachment.set_texture(Some(retained.copy_to));
            attachment.set_load_action(MTLLoadAction::DontCare);
            attachment.set_store_action(MTLStoreAction::Store);
            let copy = command_buffer.new_render_command_encoder(copy_pass);
            // Only the frame's extent: `copy_to` may be a larger kept texture.
            copy.set_viewport(MTLViewport {
                originX: 0.0,
                originY: 0.0,
                width: f64::from(drawable_size.width),
                height: f64::from(drawable_size.height),
                znear: 0.0,
                zfar: 1.0,
            });
            // The whole frame, not clip 0, which a retained frame cuts to its damage.
            copy.set_scissor_rect(MTLScissorRect {
                x: 0,
                y: 0,
                width: drawable_size.width as u64,
                height: drawable_size.height as u64,
            });
            copy.set_render_pipeline_state(&self.retained_pipelines.copy);
            copy.set_fragment_texture(0, Some(texture));
            copy.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
            copy.end_encoding();
        }
        command_buffer.commit();
        // A layer that presents with its transaction needs the drawable presented
        // after its commands are scheduled; any other layer accepts the same.
        if let Some(drawable) = present_drawable {
            let waited_at = self.profiling.then(std::time::Instant::now);
            command_buffer.wait_until_scheduled();
            self.scheduled_wait = waited_at.map(|waited_at| waited_at.elapsed());
            drawable.present();
        }
        self.instances.retire(command_buffer, instances);
        for staging in [staging, color_staging].into_iter().flatten() {
            self.instances.retire(command_buffer, staging);
        }
        self.frames_in_flight
            .push((self.text_cache.frame(), command_buffer.to_owned()));
        if wait_until_completed {
            command_buffer.wait_until_completed();
            if command_buffer.status() == MTLCommandBufferStatus::Error {
                return Err(RendererError::render_failed(
                    "metal command buffer failed during offscreen capture",
                ));
            }
            self.gpu_time = Some(gpu_time(command_buffer));
        }
        Ok(())
    }

    pub(super) fn count_draw(&mut self, instances: usize) {
        self.frame_stats.draw_calls += 1;
        self.frame_stats.instances += instances as u32;
    }
}
