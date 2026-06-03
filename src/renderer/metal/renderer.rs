//! Metal renderer implementation

use super::capture::{create_capture_texture, read_texture_rgba, texture_extent};
use super::image_primitive::{GpuImage, calculate_fit_bounds};
use super::path::build_path_mesh;
use crate::core::geometry::{Bounds, Size};
use crate::renderer::primitives::{GpuQuad, GpuShadow, Primitive};
use crate::renderer::text::{TextRasterCache, TextRequest};
use crate::renderer::{
    Renderer, RendererDeviceDiagnostics, RendererDiagnostics, RendererError, RendererImageCache,
    RendererPrimitiveSupport, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceLimits, Scene,
};
use crate::{ImageFit, ImageSource};
use metal::*;
use std::collections::HashMap;
use std::mem;

/// Uniform data passed to shaders
#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    viewport_size: [f32; 2],
}

/// Metal-based renderer
pub struct MetalRenderer {
    device: Device,
    command_queue: CommandQueue,
    quad_pipeline: RenderPipelineState,
    shadow_pipeline: RenderPipelineState,
    image_pipeline: RenderPipelineState,
    path_pipeline: RenderPipelineState,
    sampler: SamplerState,
    textures: HashMap<u32, Texture>,
    texture_resources: RendererResourceCache<u32>,
    image_cache: RendererImageCache,
    text_cache: TextRasterCache,
}

impl MetalRenderer {
    pub fn new() -> Result<Self, RendererError> {
        Self::with_resource_limits(RendererResourceLimits::default())
    }

    pub fn with_resource_limits(limits: RendererResourceLimits) -> Result<Self, RendererError> {
        let device = Device::system_default()
            .ok_or_else(|| RendererError::backend_unavailable("Metal device is unavailable"))?;
        let command_queue = device.new_command_queue();

        // Compile shaders
        let library = device
            .new_library_with_source(super::shaders::QUAD_SHADER, &CompileOptions::new())
            .expect("Failed to compile quad shader");

        let shadow_library = device
            .new_library_with_source(super::shaders::SHADOW_SHADER, &CompileOptions::new())
            .expect("Failed to compile shadow shader");

        let image_library = device
            .new_library_with_source(super::shaders::IMAGE_SHADER, &CompileOptions::new())
            .expect("Failed to compile image shader");

        let path_library = device
            .new_library_with_source(super::shaders::PATH_SHADER, &CompileOptions::new())
            .map_err(|err| {
                RendererError::render_failed(format!("failed to compile path shader: {err}"))
            })?;

        // Create quad pipeline
        let quad_vertex = library.get_function("quad_vertex", None).unwrap();
        let quad_fragment = library.get_function("quad_fragment", None).unwrap();

        let quad_pipeline_desc = RenderPipelineDescriptor::new();
        quad_pipeline_desc.set_vertex_function(Some(&quad_vertex));
        quad_pipeline_desc.set_fragment_function(Some(&quad_fragment));
        quad_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        // Enable blending for transparency
        let color_attachment = quad_pipeline_desc.color_attachments().object_at(0).unwrap();
        color_attachment.set_blending_enabled(true);
        color_attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
        color_attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
        color_attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let quad_pipeline = device
            .new_render_pipeline_state(&quad_pipeline_desc)
            .expect("Failed to create quad pipeline");

        // Create shadow pipeline
        let shadow_vertex = shadow_library.get_function("shadow_vertex", None).unwrap();
        let shadow_fragment = shadow_library
            .get_function("shadow_fragment", None)
            .unwrap();

        let shadow_pipeline_desc = RenderPipelineDescriptor::new();
        shadow_pipeline_desc.set_vertex_function(Some(&shadow_vertex));
        shadow_pipeline_desc.set_fragment_function(Some(&shadow_fragment));
        shadow_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let shadow_color = shadow_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap();
        shadow_color.set_blending_enabled(true);
        shadow_color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
        shadow_color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        shadow_color.set_source_alpha_blend_factor(MTLBlendFactor::One);
        shadow_color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let shadow_pipeline = device
            .new_render_pipeline_state(&shadow_pipeline_desc)
            .expect("Failed to create shadow pipeline");

        // Create image pipeline
        let image_vertex = image_library.get_function("image_vertex", None).unwrap();
        let image_fragment = image_library.get_function("image_fragment", None).unwrap();

        let image_pipeline_desc = RenderPipelineDescriptor::new();
        image_pipeline_desc.set_vertex_function(Some(&image_vertex));
        image_pipeline_desc.set_fragment_function(Some(&image_fragment));
        image_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let image_color = image_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap();
        image_color.set_blending_enabled(true);
        image_color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
        image_color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        image_color.set_source_alpha_blend_factor(MTLBlendFactor::One);
        image_color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let image_pipeline = device
            .new_render_pipeline_state(&image_pipeline_desc)
            .expect("Failed to create image pipeline");

        let path_vertex = path_library
            .get_function("path_vertex", None)
            .map_err(|err| {
                RendererError::render_failed(format!("missing path vertex shader: {err}"))
            })?;
        let path_fragment = path_library
            .get_function("path_fragment", None)
            .map_err(|err| {
                RendererError::render_failed(format!("missing path fragment shader: {err}"))
            })?;

        let path_pipeline_desc = RenderPipelineDescriptor::new();
        path_pipeline_desc.set_vertex_function(Some(&path_vertex));
        path_pipeline_desc.set_fragment_function(Some(&path_fragment));
        path_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let path_color = path_pipeline_desc.color_attachments().object_at(0).unwrap();
        path_color.set_blending_enabled(true);
        path_color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
        path_color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        path_color.set_source_alpha_blend_factor(MTLBlendFactor::One);
        path_color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let path_pipeline = device
            .new_render_pipeline_state(&path_pipeline_desc)
            .map_err(|err| {
                RendererError::render_failed(format!("failed to create path pipeline: {err}"))
            })?;

        // Sampler for images/text
        let sampler_desc = SamplerDescriptor::new();
        sampler_desc.set_min_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_mag_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_address_mode_s(MTLSamplerAddressMode::ClampToEdge);
        sampler_desc.set_address_mode_t(MTLSamplerAddressMode::ClampToEdge);
        let sampler = device.new_sampler(&sampler_desc);

        Ok(Self {
            device,
            command_queue,
            quad_pipeline,
            shadow_pipeline,
            image_pipeline,
            path_pipeline,
            sampler,
            textures: HashMap::new(),
            texture_resources: RendererResourceCache::new(
                RendererResourceKind::Texture,
                limits.texture_max_entries,
                limits.texture_max_bytes,
            ),
            image_cache: RendererImageCache::with_limits(
                limits.image_max_entries,
                limits.image_max_bytes,
            ),
            text_cache: TextRasterCache::with_limits(
                limits.glyph_max_entries,
                limits.glyph_max_bytes,
            ),
        })
    }

    /// Get the Metal device
    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn diagnostics(&self) -> RendererDiagnostics {
        <Self as Renderer>::diagnostics(self)
    }

    pub fn capture_frame_pixels(
        &mut self,
        scene: &Scene,
        viewport_size: Size,
    ) -> Result<Vec<u8>, RendererError> {
        let width = texture_extent(viewport_size.width, "width")?;
        let height = texture_extent(viewport_size.height, "height")?;
        let texture = create_capture_texture(&self.device, width, height);

        self.render_to_texture(scene, &texture, viewport_size, None, true)?;
        Ok(read_texture_rgba(&texture, width, height))
    }

    /// Render a scene to a drawable
    pub fn render(
        &mut self,
        scene: &Scene,
        drawable: &MetalDrawableRef,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        self.render_to_texture(
            scene,
            drawable.texture(),
            viewport_size,
            Some(drawable),
            false,
        )
    }

    fn render_to_texture(
        &mut self,
        scene: &Scene,
        texture: &TextureRef,
        viewport_size: Size,
        present_drawable: Option<&MetalDrawableRef>,
        wait_until_completed: bool,
    ) -> Result<(), RendererError> {
        RendererPrimitiveSupport::metal().validate_scene(scene)?;

        self.texture_resources.begin_frame();
        self.image_cache.begin_frame();
        self.text_cache.begin_frame();

        let command_queue = self.command_queue.to_owned();
        let command_buffer = command_queue.new_command_buffer();

        let render_pass_desc = RenderPassDescriptor::new();
        let color_attachment = render_pass_desc.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(texture));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        color_attachment.set_store_action(MTLStoreAction::Store);
        color_attachment.set_clear_color(MTLClearColor::new(0.1, 0.1, 0.1, 1.0));

        let encoder = command_buffer.new_render_command_encoder(render_pass_desc);

        let uniforms = Uniforms {
            viewport_size: [viewport_size.width, viewport_size.height],
        };

        let scale_factor = texture.width() as f32 / viewport_size.width.max(1.0);
        let drawable_size = Size::new(texture.width() as f32, texture.height() as f32);

        let mut clip_stack: Vec<Bounds> = Vec::new();
        self.set_scissor_rect(encoder, None, scale_factor, drawable_size);

        for primitive in scene.primitives() {
            match primitive {
                Primitive::Shadow {
                    bounds,
                    corner_radii,
                    blur_radius,
                    color,
                } => {
                    let instance =
                        GpuShadow::from_primitive(*bounds, *corner_radii, *blur_radius, *color);
                    self.draw_shadow(encoder, &instance, &uniforms);
                }
                Primitive::Quad {
                    bounds,
                    background,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let instance = GpuQuad::solid(
                        *bounds,
                        *background,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    self.draw_quad(encoder, &instance, &uniforms);
                }
                Primitive::LinearGradient {
                    bounds,
                    start,
                    end,
                    angle,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let instance = GpuQuad::linear_gradient(
                        *bounds,
                        *start,
                        *end,
                        angle.to_radians(),
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    self.draw_quad(encoder, &instance, &uniforms);
                }
                Primitive::RadialGradient {
                    bounds,
                    inner,
                    outer,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let instance = GpuQuad::radial_gradient(
                        *bounds,
                        *inner,
                        *outer,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    self.draw_quad(encoder, &instance, &uniforms);
                }
                Primitive::Image {
                    bounds,
                    source,
                    fit,
                    corner_radii,
                    opacity,
                } => {
                    self.draw_image_primitive(
                        encoder,
                        bounds,
                        source,
                        *fit,
                        *corner_radii,
                        *opacity,
                        &uniforms,
                        scale_factor,
                        drawable_size,
                        &mut clip_stack,
                    )?;
                }
                Primitive::Text {
                    bounds,
                    content,
                    color,
                    font_size,
                    font_weight,
                    font_family,
                    line_height,
                    align,
                } => {
                    self.draw_text_primitive(
                        encoder,
                        bounds,
                        content,
                        *color,
                        *font_size,
                        *font_weight,
                        font_family.as_deref(),
                        *line_height,
                        *align,
                        &uniforms,
                    )?;
                }
                Primitive::Path {
                    vertices,
                    color,
                    stroke_width,
                } => {
                    let mesh = build_path_mesh(vertices, *color, *stroke_width)?;
                    self.draw_path(encoder, &mesh.vertices, &uniforms);
                }
                Primitive::PushClip { bounds, .. } => {
                    let new_clip = if let Some(prev) = clip_stack.last() {
                        prev.intersection(bounds)
                            .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
                    } else {
                        *bounds
                    };
                    clip_stack.push(new_clip);
                    self.set_scissor_rect(
                        encoder,
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
                    );
                }
                Primitive::PopClip => {
                    clip_stack.pop();
                    self.set_scissor_rect(
                        encoder,
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
                    );
                }
            }
        }

        encoder.end_encoding();
        if let Some(drawable) = present_drawable {
            command_buffer.present_drawable(drawable);
        }
        command_buffer.commit();
        if wait_until_completed {
            command_buffer.wait_until_completed();
            if command_buffer.status() == MTLCommandBufferStatus::Error {
                return Err(RendererError::render_failed(
                    "metal command buffer failed during offscreen capture",
                ));
            }
        }
        Ok(())
    }

    fn draw_quad(&self, encoder: &RenderCommandEncoderRef, quad: &GpuQuad, uniforms: &Uniforms) {
        encoder.set_render_pipeline_state(&self.quad_pipeline);

        let instance_buffer = self.device.new_buffer_with_data(
            quad as *const _ as *const _,
            mem::size_of::<GpuQuad>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&instance_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);

        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, 1);
    }

    fn draw_shadow(
        &self,
        encoder: &RenderCommandEncoderRef,
        shadow: &GpuShadow,
        uniforms: &Uniforms,
    ) {
        encoder.set_render_pipeline_state(&self.shadow_pipeline);

        let instance_buffer = self.device.new_buffer_with_data(
            shadow as *const _ as *const _,
            mem::size_of::<GpuShadow>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&instance_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);

        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, 1);
    }

    fn draw_image(
        &self,
        encoder: &RenderCommandEncoderRef,
        texture: &Texture,
        instance: &GpuImage,
        uniforms: &Uniforms,
    ) {
        encoder.set_render_pipeline_state(&self.image_pipeline);

        let instance_buffer = self.device.new_buffer_with_data(
            instance as *const _ as *const _,
            mem::size_of::<GpuImage>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&instance_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);
        encoder.set_fragment_texture(0, Some(texture));
        encoder.set_fragment_sampler_state(0, Some(&self.sampler));

        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, 1);
    }

    fn draw_path(
        &self,
        encoder: &RenderCommandEncoderRef,
        vertices: &[super::path::GpuPathVertex],
        uniforms: &Uniforms,
    ) {
        encoder.set_render_pipeline_state(&self.path_pipeline);

        let vertex_buffer = self.device.new_buffer_with_data(
            vertices.as_ptr() as *const _,
            std::mem::size_of_val(vertices) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&vertex_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
    }

    fn draw_image_primitive(
        &mut self,
        encoder: &RenderCommandEncoderRef,
        bounds: &Bounds,
        source: &ImageSource,
        fit: ImageFit,
        corner_radii: crate::core::style::Corners,
        opacity: f32,
        uniforms: &Uniforms,
        scale_factor: f32,
        drawable_size: Size,
        clip_stack: &mut Vec<Bounds>,
    ) -> Result<(), RendererError> {
        if let ImageSource::Texture(id) = source {
            let texture = self.textures.get(id).ok_or_else(|| {
                RendererResourceError::missing(RendererResourceKind::Texture, *id)
            })?;
            let instance = GpuImage::new(*bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
            self.draw_image(encoder, texture, &instance, uniforms);
            return Ok(());
        }

        let entry = self.image_cache.resolve(source)?;

        let texture = self.ensure_texture(entry.handle.id.as_u32(), entry.size, &entry.pixels)?;

        let dest_bounds = calculate_fit_bounds(*bounds, entry.size, fit);

        // Clip to container for cover/contain
        let previous_clip = clip_stack.last().copied();
        let container_clip = if let Some(prev) = previous_clip {
            prev.intersection(bounds)
                .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
        } else {
            *bounds
        };
        clip_stack.push(container_clip);
        self.set_scissor_rect(
            encoder,
            clip_stack.last().copied(),
            scale_factor,
            drawable_size,
        );

        let instance = GpuImage::new(dest_bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
        self.draw_image(encoder, &texture, &instance, uniforms);

        clip_stack.pop();
        self.set_scissor_rect(encoder, previous_clip, scale_factor, drawable_size);
        Ok(())
    }

    fn draw_text_primitive(
        &mut self,
        encoder: &RenderCommandEncoderRef,
        bounds: &Bounds,
        content: &str,
        color: crate::core::color::Rgba,
        font_size: f32,
        font_weight: u16,
        font_family: Option<&str>,
        line_height: f32,
        align: crate::elements::text::TextAlign,
        uniforms: &Uniforms,
    ) -> Result<(), RendererError> {
        let entry = match self.text_cache.resolve(TextRequest::new(
            content,
            font_size,
            font_weight,
            font_family,
            line_height,
        )) {
            Ok(Some(entry)) => entry,
            Ok(None) => return Ok(()),
            Err(err) => {
                return Err(RendererError::render_failed(format!(
                    "text rendering failed: {:?}",
                    err
                )));
            }
        };

        let texture = self.ensure_texture(
            entry.id,
            Size::new(
                entry.metrics.ink_bounds.width(),
                entry.metrics.ink_bounds.height(),
            ),
            &entry.pixels,
        )?;

        let mut x = bounds.x();
        let mut y = bounds.y();
        let text_width = entry.metrics.ink_bounds.width();
        let text_height = entry.metrics.ink_bounds.height();

        match align {
            crate::elements::text::TextAlign::Left => {}
            crate::elements::text::TextAlign::Center => {
                x += (bounds.width() - text_width) * 0.5;
            }
            crate::elements::text::TextAlign::Right => {
                x += bounds.width() - text_width;
            }
        }

        y += (bounds.height() - text_height) * 0.5;

        let text_bounds = Bounds::from_xywh(x, y, text_width, text_height);
        let color_array = color.to_array();
        let instance = GpuImage::new(
            text_bounds,
            crate::core::style::Corners::ZERO,
            color_array,
            1.0,
        );
        self.draw_image(encoder, &texture, &instance, uniforms);
        Ok(())
    }

    fn ensure_texture(
        &mut self,
        id: u32,
        size: Size,
        pixels: &[u8],
    ) -> Result<Texture, RendererError> {
        let allocation = self.texture_resources.resolve(id, pixels.len())?;
        for evicted_id in allocation.evicted {
            self.textures.remove(&evicted_id);
        }

        if !self.textures.contains_key(&id) {
            let width = size.width.max(1.0).round() as u64;
            let height = size.height.max(1.0).round() as u64;

            let desc = TextureDescriptor::new();
            desc.set_texture_type(MTLTextureType::D2);
            desc.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
            desc.set_width(width);
            desc.set_height(height);
            desc.set_usage(MTLTextureUsage::ShaderRead);

            let texture = self.device.new_texture(&desc);

            let bytes_per_row = width * 4;
            let region = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width,
                    height,
                    depth: 1,
                },
            };

            texture.replace_region(region, 0, pixels.as_ptr() as *const _, bytes_per_row);
            self.textures.insert(id, texture);
        }

        self.textures
            .get(&id)
            .cloned()
            .ok_or_else(|| RendererResourceError::missing(RendererResourceKind::Texture, id).into())
    }

    fn set_scissor_rect(
        &self,
        encoder: &RenderCommandEncoderRef,
        clip: Option<Bounds>,
        scale_factor: f32,
        drawable_size: Size,
    ) {
        if let Some(bounds) = clip {
            if bounds.is_empty() {
                let rect = MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                };
                encoder.set_scissor_rect(rect);
                return;
            }

            let x = (bounds.x() * scale_factor).round().max(0.0) as u64;
            let y = (bounds.y() * scale_factor).round().max(0.0) as u64;
            let mut width = (bounds.width() * scale_factor).round().max(0.0) as u64;
            let mut height = (bounds.height() * scale_factor).round().max(0.0) as u64;

            let max_w = drawable_size.width as u64;
            let max_h = drawable_size.height as u64;

            if x + width > max_w {
                width = max_w.saturating_sub(x);
            }
            if y + height > max_h {
                height = max_h.saturating_sub(y);
            }

            let rect = MTLScissorRect {
                x,
                y,
                width,
                height,
            };
            encoder.set_scissor_rect(rect);
        } else {
            let rect = MTLScissorRect {
                x: 0,
                y: 0,
                width: drawable_size.width as u64,
                height: drawable_size.height as u64,
            };
            encoder.set_scissor_rect(rect);
        }
    }
}

impl Renderer for MetalRenderer {
    type Target = MetalDrawableRef;

    fn render(
        &mut self,
        scene: &Scene,
        target: &Self::Target,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        MetalRenderer::render(self, scene, target, viewport_size)
    }

    fn diagnostics(&self) -> RendererDiagnostics {
        RendererDiagnostics::new(
            RendererDeviceDiagnostics {
                backend: String::from("metal"),
                device_name: self.device.name().to_string(),
                is_headless: self.device.is_headless(),
                unified_memory: Some(self.device.has_unified_memory()),
                recommended_max_working_set_size: Some(
                    self.device.recommended_max_working_set_size(),
                ),
            },
            vec![
                self.texture_resources.stats(),
                self.image_cache.stats(),
                self.text_cache.resource_stats(),
            ],
        )
        .with_unsupported_primitives(RendererPrimitiveSupport::metal().unsupported_primitives())
    }
}
