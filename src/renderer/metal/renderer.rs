//! Metal renderer implementation

use crate::core::geometry::{Bounds, Size};
use crate::{ImageFit, ImageSource};
use crate::renderer::primitives::{GpuQuad, GpuShadow, Primitive};
use crate::renderer::Scene;
use metal::*;
use rusttype::{point, Font, Scale};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;
use std::sync::Arc;

/// Uniform data passed to shaders
#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    viewport_size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GpuImage {
    bounds: [f32; 4],
    corner_radii: [f32; 4],
    color: [f32; 4],
    opacity: f32,
    _padding: [f32; 3],
}

impl GpuImage {
    fn new(bounds: Bounds, corner_radii: crate::core::style::Corners, color: [f32; 4], opacity: f32) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            color,
            opacity,
            _padding: [0.0; 3],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ImageKey {
    File(String),
    Data { hash: u64, width: u32, height: u32 },
}

struct ImageEntry {
    id: u32,
    size: Size,
    pixels: Vec<u8>,
}

struct ImageCache {
    next_id: u32,
    entries: HashMap<ImageKey, Arc<ImageEntry>>,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            next_id: 1,
            entries: HashMap::new(),
        }
    }

    fn resolve(&mut self, source: &ImageSource) -> Option<Arc<ImageEntry>> {
        let key = match source {
            ImageSource::File(path) => ImageKey::File(path.clone()),
            ImageSource::Data { data, width, height } => {
                if data.len() != (*width as usize) * (*height as usize) * 4 {
                    log::error!("ImageSource::Data length mismatch: expected {} bytes", (*width as usize) * (*height as usize) * 4);
                    return None;
                }
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                data.hash(&mut hasher);
                let hash = hasher.finish();
                ImageKey::Data { hash, width: *width, height: *height }
            }
            ImageSource::Url(url) => {
                log::error!("ImageSource::Url not supported yet: {}", url);
                return None;
            }
            ImageSource::Texture(_) => {
                return None;
            }
        };

        if let Some(entry) = self.entries.get(&key) {
            return Some(entry.clone());
        }

        let entry = match source {
            ImageSource::File(path) => {
                let image = match image::open(path) {
                    Ok(img) => img,
                    Err(err) => {
                        log::error!("Failed to load image {}: {}", path, err);
                        return None;
                    }
                };
                let rgba = image.to_rgba8();
                let (w, h) = rgba.dimensions();
                ImageEntry {
                    id: self.alloc_id(),
                    size: Size::new(w as f32, h as f32),
                    pixels: rgba.into_raw(),
                }
            }
            ImageSource::Data { data, width, height } => ImageEntry {
                id: self.alloc_id(),
                size: Size::new(*width as f32, *height as f32),
                pixels: data.clone(),
            },
            ImageSource::Url(url) => {
                log::error!("ImageSource::Url not supported yet: {}", url);
                return None;
            }
            ImageSource::Texture(_) => return None,
        };

        let entry = Arc::new(entry);
        self.entries.insert(key.clone(), entry.clone());
        Some(entry)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TextKey {
    content: String,
    size: u32,
    weight: u16,
    font_family: Option<String>,
}

struct TextEntry {
    id: u32,
    size: Size,
    pixels: Vec<u8>,
}

struct TextCache {
    font: Option<Font<'static>>,
    next_id: u32,
    entries: HashMap<TextKey, Arc<TextEntry>>,
}

impl TextCache {
    fn new() -> Self {
        Self {
            font: load_system_font(),
            next_id: 1,
            entries: HashMap::new(),
        }
    }

    fn resolve(
        &mut self,
        content: &str,
        font_size: f32,
        font_weight: u16,
        font_family: Option<&str>,
    ) -> Option<Arc<TextEntry>> {
        if content.is_empty() {
            return None;
        }

        let key = TextKey {
            content: content.to_string(),
            size: font_size.max(1.0).round() as u32,
            weight: font_weight,
            font_family: font_family.map(|s| s.to_string()),
        };

        if let Some(entry) = self.entries.get(&key) {
            return Some(entry.clone());
        }

        let font = match &self.font {
            Some(font) => font,
            None => {
                log::error!("No font available for text rendering");
                return None;
            }
        };

        let scale = Scale::uniform(font_size.max(1.0));
        let v_metrics = font.v_metrics(scale);
        let glyphs: Vec<_> = font
            .layout(content, scale, point(0.0, v_metrics.ascent))
            .collect();

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for glyph in &glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                min_x = min_x.min(bb.min.x);
                min_y = min_y.min(bb.min.y);
                max_x = max_x.max(bb.max.x);
                max_y = max_y.max(bb.max.y);
            }
        }

        if min_x >= max_x || min_y >= max_y {
            return None;
        }

        let width = (max_x - min_x) as u32;
        let height = (max_y - min_y) as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        for glyph in glyphs {
            if let Some(bb) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let px = x as i32 + bb.min.x - min_x;
                    let py = y as i32 + bb.min.y - min_y;
                    if px < 0 || py < 0 {
                        return;
                    }
                    let px = px as u32;
                    let py = py as u32;
                    if px >= width || py >= height {
                        return;
                    }
                    let idx = ((py * width + px) * 4) as usize;
                    let alpha = (v * 255.0) as u8;
                    pixels[idx] = 255;
                    pixels[idx + 1] = 255;
                    pixels[idx + 2] = 255;
                    pixels[idx + 3] = alpha;
                });
            }
        }

        let entry = TextEntry {
            id: self.alloc_id(),
            size: Size::new(width as f32, height as f32),
            pixels,
        };

        let entry = Arc::new(entry);
        self.entries.insert(key.clone(), entry.clone());
        Some(entry)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn load_system_font() -> Option<Font<'static>> {
    let candidates = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Monaco.ttf",
        "/System/Library/Fonts/Geneva.ttf",
    ];

    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(font) = Font::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }

    None
}

/// Metal-based renderer
pub struct MetalRenderer {
    device: Device,
    command_queue: CommandQueue,
    quad_pipeline: RenderPipelineState,
    shadow_pipeline: RenderPipelineState,
    image_pipeline: RenderPipelineState,
    sampler: SamplerState,
    textures: HashMap<u32, Texture>,
    image_cache: ImageCache,
    text_cache: TextCache,
}

impl MetalRenderer {
    pub fn new() -> Option<Self> {
        let device = Device::system_default()?;
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
        let shadow_fragment = shadow_library.get_function("shadow_fragment", None).unwrap();

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

        // Sampler for images/text
        let sampler_desc = SamplerDescriptor::new();
        sampler_desc.set_min_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_mag_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_address_mode_s(MTLSamplerAddressMode::ClampToEdge);
        sampler_desc.set_address_mode_t(MTLSamplerAddressMode::ClampToEdge);
        let sampler = device.new_sampler(&sampler_desc);

        Some(Self {
            device,
            command_queue,
            quad_pipeline,
            shadow_pipeline,
            image_pipeline,
            sampler,
            textures: HashMap::new(),
            image_cache: ImageCache::new(),
            text_cache: TextCache::new(),
        })
    }

    /// Get the Metal device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Render a scene to a drawable
    pub fn render(&mut self, scene: &Scene, drawable: &MetalDrawableRef, viewport_size: Size) {
        let command_queue = self.command_queue.to_owned();
        let command_buffer = command_queue.new_command_buffer();

        let render_pass_desc = RenderPassDescriptor::new();
        let color_attachment = render_pass_desc.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(drawable.texture()));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        color_attachment.set_store_action(MTLStoreAction::Store);
        color_attachment.set_clear_color(MTLClearColor::new(0.1, 0.1, 0.1, 1.0));

        let encoder = command_buffer.new_render_command_encoder(render_pass_desc);

        let uniforms = Uniforms {
            viewport_size: [viewport_size.width, viewport_size.height],
        };

        let scale_factor = drawable.texture().width() as f32 / viewport_size.width.max(1.0);
        let drawable_size = Size::new(
            drawable.texture().width() as f32,
            drawable.texture().height() as f32,
        );

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
                    let instance = GpuShadow::from_primitive(
                        *bounds,
                        *corner_radii,
                        *blur_radius,
                        *color,
                    );
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
                    );
                }
                Primitive::Text {
                    bounds,
                    content,
                    color,
                    font_size,
                    font_weight,
                    font_family,
                    align,
                    ..
                } => {
                    self.draw_text_primitive(
                        encoder,
                        bounds,
                        content,
                        *color,
                        *font_size,
                        *font_weight,
                        font_family.as_deref(),
                        *align,
                        &uniforms,
                    );
                }
                Primitive::Path { .. } => {
                    // TODO: path rendering
                }
                Primitive::PushClip { bounds, .. } => {
                    let new_clip = if let Some(prev) = clip_stack.last() {
                        prev.intersection(bounds).unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
                    } else {
                        *bounds
                    };
                    clip_stack.push(new_clip);
                    self.set_scissor_rect(encoder, clip_stack.last().copied(), scale_factor, drawable_size);
                }
                Primitive::PopClip => {
                    clip_stack.pop();
                    self.set_scissor_rect(encoder, clip_stack.last().copied(), scale_factor, drawable_size);
                }
            }
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
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

    fn draw_shadow(&self, encoder: &RenderCommandEncoderRef, shadow: &GpuShadow, uniforms: &Uniforms) {
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
    ) {
        match source {
            ImageSource::Texture(id) => {
                if let Some(texture) = self.textures.get(id) {
                    let instance = GpuImage::new(*bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
                    self.draw_image(encoder, texture, &instance, uniforms);
                }
                return;
            }
            ImageSource::Url(url) => {
                log::error!("ImageSource::Url not supported yet: {}", url);
                return;
            }
            _ => {}
        }

        let entry = match self.image_cache.resolve(source) {
            Some(entry) => entry,
            None => return,
        };

        let texture = self.ensure_texture(entry.id, entry.size, &entry.pixels);

        let dest_bounds = calculate_fit_bounds(*bounds, entry.size, fit);

        // Clip to container for cover/contain
        let previous_clip = clip_stack.last().copied();
        let container_clip = if let Some(prev) = previous_clip {
            prev.intersection(bounds).unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
        } else {
            *bounds
        };
        clip_stack.push(container_clip);
        self.set_scissor_rect(encoder, clip_stack.last().copied(), scale_factor, drawable_size);

        let instance = GpuImage::new(dest_bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
        self.draw_image(encoder, &texture, &instance, uniforms);

        clip_stack.pop();
        self.set_scissor_rect(encoder, previous_clip, scale_factor, drawable_size);
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
        align: crate::elements::text::TextAlign,
        uniforms: &Uniforms,
    ) {
        let entry = match self.text_cache.resolve(content, font_size, font_weight, font_family) {
            Some(entry) => entry,
            None => return,
        };

        let texture = self.ensure_texture(entry.id, entry.size, &entry.pixels);

        let mut x = bounds.x();
        let mut y = bounds.y();
        let text_width = entry.size.width;
        let text_height = entry.size.height;

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
        let instance = GpuImage::new(text_bounds, crate::core::style::Corners::ZERO, color_array, 1.0);
        self.draw_image(encoder, &texture, &instance, uniforms);
    }

    fn ensure_texture(&mut self, id: u32, size: Size, pixels: &[u8]) -> Texture {
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

            let bytes_per_row = (width * 4) as u64;
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
            .expect("Texture missing")
            .to_owned()
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
                let rect = MTLScissorRect { x: 0, y: 0, width: 0, height: 0 };
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

            let rect = MTLScissorRect { x, y, width, height };
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

fn calculate_fit_bounds(container: Bounds, image_size: Size, fit: ImageFit) -> Bounds {
    if image_size.width <= 0.0 || image_size.height <= 0.0 {
        return container;
    }
    match fit {
        ImageFit::Fill => container,
        ImageFit::Contain => {
            let scale_x = container.width() / image_size.width;
            let scale_y = container.height() / image_size.height;
            let scale = scale_x.min(scale_y);
            let width = image_size.width * scale;
            let height = image_size.height * scale;
            let x = container.x() + (container.width() - width) / 2.0;
            let y = container.y() + (container.height() - height) / 2.0;
            Bounds::from_xywh(x, y, width, height)
        }
        ImageFit::Cover => {
            let scale_x = container.width() / image_size.width;
            let scale_y = container.height() / image_size.height;
            let scale = scale_x.max(scale_y);
            let width = image_size.width * scale;
            let height = image_size.height * scale;
            let x = container.x() + (container.width() - width) / 2.0;
            let y = container.y() + (container.height() - height) / 2.0;
            Bounds::from_xywh(x, y, width, height)
        }
        ImageFit::None => {
            let x = container.x() + (container.width() - image_size.width) / 2.0;
            let y = container.y() + (container.height() - image_size.height) / 2.0;
            Bounds::from_xywh(x, y, image_size.width, image_size.height)
        }
        ImageFit::ScaleDown => {
            if image_size.width <= container.width() && image_size.height <= container.height() {
                let x = container.x() + (container.width() - image_size.width) / 2.0;
                let y = container.y() + (container.height() - image_size.height) / 2.0;
                Bounds::from_xywh(x, y, image_size.width, image_size.height)
            } else {
                let scale_x = container.width() / image_size.width;
                let scale_y = container.height() / image_size.height;
                let scale = scale_x.min(scale_y);
                let width = image_size.width * scale;
                let height = image_size.height * scale;
                let x = container.x() + (container.width() - width) / 2.0;
                let y = container.y() + (container.height() - height) / 2.0;
                Bounds::from_xywh(x, y, width, height)
            }
        }
    }
}
