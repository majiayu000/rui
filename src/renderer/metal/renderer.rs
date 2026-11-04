//! Metal renderer implementation

use super::batch::{bind_vertex_slice, bind_vertex_value};
use super::capture::{create_capture_texture, read_texture_rgba, texture_extent};
use super::glyph::{GlyphPages, GpuGlyph};
use super::image_primitive::{GpuImage, calculate_fit_bounds};
use super::path::build_path_mesh;
use crate::core::geometry::{Bounds, Size};
use crate::renderer::primitives::{GpuQuad, GpuShadow, Primitive};
use crate::renderer::text::{SUBPIXEL_VARIANTS, TextGlyphCache, TextRequest};
use crate::renderer::{
    Renderer, RendererDeviceDiagnostics, RendererDiagnostics, RendererError, RendererImageCache,
    RendererPrimitiveSupport, RendererResourceCache, RendererResourceError, RendererResourceKind,
    RendererResourceLimits, Scene,
};
use crate::{ImageFit, ImageSource};
use metal::*;
use std::collections::HashMap;

/// Pixel format of every render target: drawables, captures and offscreen
/// textures. Blending happens in linear light and stores sRGB-encoded bytes.
pub(crate) const TARGET_PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm_sRGB;

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
    glyph_pipeline: RenderPipelineState,
    sampler: SamplerState,
    textures: HashMap<u32, Texture>,
    texture_resources: RendererResourceCache<u32>,
    image_cache: RendererImageCache,
    text_cache: TextGlyphCache,
    glyph_pages: GlyphPages,
    offscreen_target: Option<Texture>,
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
            .new_library_with_source(
                &with_color(super::shaders::QUAD_SHADER),
                &CompileOptions::new(),
            )
            .expect("Failed to compile quad shader");

        let shadow_library = device
            .new_library_with_source(
                &with_color(super::shaders::SHADOW_SHADER),
                &CompileOptions::new(),
            )
            .expect("Failed to compile shadow shader");

        let image_library = device
            .new_library_with_source(
                &with_color(super::shaders::IMAGE_SHADER),
                &CompileOptions::new(),
            )
            .expect("Failed to compile image shader");

        let path_library = device
            .new_library_with_source(
                &with_color(super::shaders::PATH_SHADER),
                &CompileOptions::new(),
            )
            .map_err(|err| {
                RendererError::render_failed(format!("failed to compile path shader: {err}"))
            })?;

        let glyph_library = device
            .new_library_with_source(
                &with_color(super::shaders::GLYPH_SHADER),
                &CompileOptions::new(),
            )
            .map_err(|err| {
                RendererError::render_failed(format!("failed to compile glyph shader: {err}"))
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
            .set_pixel_format(TARGET_PIXEL_FORMAT);

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
            .set_pixel_format(TARGET_PIXEL_FORMAT);

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
            .set_pixel_format(TARGET_PIXEL_FORMAT);

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
            .set_pixel_format(TARGET_PIXEL_FORMAT);

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

        let glyph_vertex = glyph_library
            .get_function("glyph_vertex", None)
            .map_err(|err| {
                RendererError::render_failed(format!("missing glyph vertex shader: {err}"))
            })?;
        let glyph_fragment = glyph_library
            .get_function("glyph_fragment", None)
            .map_err(|err| {
                RendererError::render_failed(format!("missing glyph fragment shader: {err}"))
            })?;

        let glyph_pipeline_desc = RenderPipelineDescriptor::new();
        glyph_pipeline_desc.set_vertex_function(Some(&glyph_vertex));
        glyph_pipeline_desc.set_fragment_function(Some(&glyph_fragment));
        let glyph_color = glyph_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap();
        glyph_color.set_pixel_format(TARGET_PIXEL_FORMAT);
        glyph_color.set_blending_enabled(true);
        glyph_color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
        glyph_color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        glyph_color.set_source_alpha_blend_factor(MTLBlendFactor::One);
        glyph_color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let glyph_pipeline = device
            .new_render_pipeline_state(&glyph_pipeline_desc)
            .map_err(|err| {
                RendererError::render_failed(format!("failed to create glyph pipeline: {err}"))
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
            glyph_pipeline,
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
            offscreen_target: None,
            text_cache: TextGlyphCache::with_limits(
                limits.glyph_max_entries,
                limits.glyph_max_bytes,
            ),
            glyph_pages: GlyphPages::default(),
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

        self.render_to_texture(scene, &texture, viewport_size, None, true)?;
        Ok(read_texture_rgba(&texture, width, height))
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
        let texture = match &self.offscreen_target {
            Some(texture) if texture.width() == width && texture.height() == height => {
                texture.clone()
            }
            _ => {
                let texture = create_capture_texture(&self.device, width, height);
                self.offscreen_target = Some(texture.clone());
                texture
            }
        };
        self.render_to_texture(scene, &texture, viewport_size, None, false)
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
        // The clear color is linear; this is 0.1 in sRGB, as before.
        let clear = srgb_to_linear(0.1);
        color_attachment.set_clear_color(MTLClearColor::new(clear, clear, clear, 1.0));

        let encoder = command_buffer.new_render_command_encoder(render_pass_desc);

        let uniforms = Uniforms {
            viewport_size: [viewport_size.width, viewport_size.height],
        };

        let scale_factor = texture.width() as f32 / viewport_size.width.max(1.0);
        let drawable_size = Size::new(texture.width() as f32, texture.height() as f32);

        let mut clip_stack: Vec<Bounds> = Vec::new();
        self.set_scissor_rect(encoder, None, scale_factor, drawable_size);

        // Adjacent quads share one instanced draw, and so do adjacent glyphs on one
        // atlas page. Anything else in the scene ends the run first, so paint order
        // and clipping stay as the scene has them.
        let mut pending_quads: Vec<GpuQuad> = Vec::new();
        let mut pending_glyphs: Vec<GpuGlyph> = Vec::new();
        let mut glyph_page = 0;

        let viewport = Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height);

        for primitive in scene.primitives() {
            // Nothing a primitive outside the current clip draws can be seen. Skipping
            // it also lets quads on either side of it share one draw.
            let visible = clip_stack.last().copied().unwrap_or(viewport);
            if !primitive.may_touch(&visible) {
                continue;
            }
            if !matches!(
                primitive,
                Primitive::Quad { .. }
                    | Primitive::LinearGradient { .. }
                    | Primitive::RadialGradient { .. }
            ) {
                self.flush_quads(encoder, &mut pending_quads, &uniforms);
            }
            if !matches!(primitive, Primitive::Text { .. }) {
                self.flush_glyphs(encoder, &mut pending_glyphs, glyph_page, &uniforms)?;
            }
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
                    pending_quads.push(instance);
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
                    pending_quads.push(instance);
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
                    pending_quads.push(instance);
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
                        scale_factor,
                        &mut pending_glyphs,
                        &mut glyph_page,
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

        self.flush_quads(encoder, &mut pending_quads, &uniforms);
        self.flush_glyphs(encoder, &mut pending_glyphs, glyph_page, &uniforms)?;
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

    fn flush_quads(
        &self,
        encoder: &RenderCommandEncoderRef,
        quads: &mut Vec<GpuQuad>,
        uniforms: &Uniforms,
    ) {
        if quads.is_empty() {
            return;
        }
        encoder.set_render_pipeline_state(&self.quad_pipeline);
        bind_vertex_slice(&self.device, encoder, 0, quads);
        bind_vertex_value(encoder, 1, uniforms);
        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, quads.len() as u64);
        quads.clear();
    }

    fn flush_glyphs(
        &mut self,
        encoder: &RenderCommandEncoderRef,
        glyphs: &mut Vec<GpuGlyph>,
        page: usize,
        uniforms: &Uniforms,
    ) -> Result<(), RendererError> {
        if glyphs.is_empty() {
            return Ok(());
        }
        self.glyph_pages.sync(&self.device, &mut self.text_cache);
        let texture = self.glyph_pages.page(page).ok_or_else(|| {
            RendererResourceError::missing(RendererResourceKind::Glyph, page as u32)
        })?;
        encoder.set_render_pipeline_state(&self.glyph_pipeline);
        bind_vertex_slice(&self.device, encoder, 0, glyphs);
        bind_vertex_value(encoder, 1, uniforms);
        encoder.set_fragment_texture(0, Some(texture));
        encoder.set_fragment_sampler_state(0, Some(&self.sampler));
        encoder.draw_primitives_instanced(MTLPrimitiveType::Triangle, 0, 6, glyphs.len() as u64);
        glyphs.clear();
        Ok(())
    }

    fn draw_shadow(
        &self,
        encoder: &RenderCommandEncoderRef,
        shadow: &GpuShadow,
        uniforms: &Uniforms,
    ) {
        encoder.set_render_pipeline_state(&self.shadow_pipeline);

        bind_vertex_value(encoder, 0, shadow);
        bind_vertex_value(encoder, 1, uniforms);

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

        bind_vertex_value(encoder, 0, instance);
        bind_vertex_value(encoder, 1, uniforms);
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

        bind_vertex_slice(&self.device, encoder, 0, vertices);
        bind_vertex_value(encoder, 1, uniforms);
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
        scale_factor: f32,
        glyphs: &mut Vec<GpuGlyph>,
        glyph_page: &mut usize,
        uniforms: &Uniforms,
    ) -> Result<(), RendererError> {
        let text_error =
            |err| RendererError::render_failed(format!("text rendering failed: {:?}", err));
        let shaped = match self.text_cache.shape(TextRequest::new(
            content,
            font_size,
            font_weight,
            font_family,
            line_height,
        )) {
            Ok(Some(shaped)) => shaped,
            Ok(None) => return Ok(()),
            Err(err) => return Err(text_error(err)),
        };

        let ink = shaped.metrics.ink_bounds;
        let mut x = bounds.x();
        match align {
            crate::elements::text::TextAlign::Left => {}
            crate::elements::text::TextAlign::Center => {
                x += (bounds.width() - ink.width()) * 0.5;
            }
            crate::elements::text::TextAlign::Right => {
                x += bounds.width() - ink.width();
            }
        }
        // Center the primary font's ascent and descent rather than the ink, so
        // every line of a wrapped paragraph lands on the same baseline.
        let font_height = shaped.ascent - shaped.descent;
        let line_top = bounds.y() + (bounds.height() - font_height) * 0.5;

        // Glyph x positions are relative to the ink box; baselines to the line top.
        let origin_x = x - ink.x();
        let origin_y = line_top;
        let device_font_size = font_size * scale_factor;
        let variants = f32::from(SUBPIXEL_VARIANTS);
        let color = color.to_array();

        for glyph in &shaped.glyphs {
            let pen_x = ((origin_x + glyph.x) * scale_factor * variants).round() / variants;
            let pen_y = ((origin_y + glyph.baseline_y) * scale_factor).round();
            let whole_x = pen_x.floor();
            let subpixel = ((pen_x - whole_x) * variants).round() as u8;
            let Some(atlas_glyph) = self
                .text_cache
                .glyph(glyph, device_font_size, subpixel)
                .map_err(text_error)?
            else {
                continue;
            };

            let slot = atlas_glyph.slot;
            if slot.page != *glyph_page {
                self.flush_glyphs(encoder, glyphs, *glyph_page, uniforms)?;
                *glyph_page = slot.page;
            }
            let (page_width, page_height) = self
                .text_cache
                .atlas()
                .page_extent(slot.page)
                .ok_or_else(|| {
                    RendererResourceError::missing(RendererResourceKind::Glyph, slot.page as u32)
                })?;
            let rect = slot.rect;
            glyphs.push(GpuGlyph {
                bounds: [
                    (whole_x + atlas_glyph.left as f32) / scale_factor,
                    (pen_y + atlas_glyph.top as f32) / scale_factor,
                    rect.width as f32 / scale_factor,
                    rect.height as f32 / scale_factor,
                ],
                uv: [
                    rect.x as f32 / page_width as f32,
                    rect.y as f32 / page_height as f32,
                    rect.width as f32 / page_width as f32,
                    rect.height as f32 / page_height as f32,
                ],
                color,
            });
        }
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
            desc.set_pixel_format(MTLPixelFormat::RGBA8Unorm_sRGB);
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

/// Shader source with the shared color helpers in front.
fn with_color(source: &str) -> String {
    [super::shaders::COLOR_SHADER, source].concat()
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
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
