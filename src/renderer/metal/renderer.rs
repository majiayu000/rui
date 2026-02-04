//! Metal renderer implementation

use crate::core::geometry::Size;
use crate::renderer::primitives::{GpuQuad, GpuShadow};
use crate::renderer::Scene;
use metal::*;
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
    text_pipeline: RenderPipelineState,
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

        let text_library = device
            .new_library_with_source(super::shaders::TEXT_SHADER, &CompileOptions::new())
            .expect("Failed to compile text shader");

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

        // Create text pipeline
        let text_vertex = text_library.get_function("text_vertex", None).unwrap();
        let text_fragment = text_library.get_function("text_fragment", None).unwrap();

        let text_pipeline_desc = RenderPipelineDescriptor::new();
        text_pipeline_desc.set_vertex_function(Some(&text_vertex));
        text_pipeline_desc.set_fragment_function(Some(&text_fragment));
        text_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        // Premultiplied alpha blending for text overlay
        let text_color = text_pipeline_desc
            .color_attachments()
            .object_at(0)
            .unwrap();
        text_color.set_blending_enabled(true);
        text_color.set_source_rgb_blend_factor(MTLBlendFactor::One);
        text_color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        text_color.set_source_alpha_blend_factor(MTLBlendFactor::One);
        text_color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);

        let text_pipeline = device
            .new_render_pipeline_state(&text_pipeline_desc)
            .expect("Failed to create text pipeline");

        Some(Self {
            device,
            command_queue,
            quad_pipeline,
            shadow_pipeline,
            text_pipeline,
        })
    }

    /// Get the Metal device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Render a scene to a drawable
    pub fn render(&self, scene: &Scene, drawable: &MetalDrawableRef, viewport_size: Size) {
        let command_buffer = self.command_queue.new_command_buffer();

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

        // Render shadows first (behind everything)
        if !scene.shadows().is_empty() {
            self.render_shadows(encoder, scene.shadows(), &uniforms);
        }

        // Render quads
        if !scene.quads().is_empty() {
            self.render_quads(encoder, scene.quads(), &uniforms);
        }

        // Render text overlay
        if !scene.text_items().is_empty() {
            self.render_text(encoder, scene, viewport_size);
        }

        encoder.end_encoding();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
    }

    fn render_quads(&self, encoder: &RenderCommandEncoderRef, quads: &[GpuQuad], uniforms: &Uniforms) {
        encoder.set_render_pipeline_state(&self.quad_pipeline);

        // Create instance buffer
        let instance_buffer = self.device.new_buffer_with_data(
            quads.as_ptr() as *const _,
            (quads.len() * mem::size_of::<GpuQuad>()) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        // Create uniform buffer
        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&instance_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);

        // Draw all quads (6 vertices per quad, instanced)
        encoder.draw_primitives_instanced(
            MTLPrimitiveType::Triangle,
            0,
            6,
            quads.len() as u64,
        );
    }

    fn render_shadows(
        &self,
        encoder: &RenderCommandEncoderRef,
        shadows: &[GpuShadow],
        uniforms: &Uniforms,
    ) {
        encoder.set_render_pipeline_state(&self.shadow_pipeline);

        let instance_buffer = self.device.new_buffer_with_data(
            shadows.as_ptr() as *const _,
            (shadows.len() * mem::size_of::<GpuShadow>()) as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        let uniform_buffer = self.device.new_buffer_with_data(
            uniforms as *const _ as *const _,
            mem::size_of::<Uniforms>() as u64,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );

        encoder.set_vertex_buffer(0, Some(&instance_buffer), 0);
        encoder.set_vertex_buffer(1, Some(&uniform_buffer), 0);

        encoder.draw_primitives_instanced(
            MTLPrimitiveType::Triangle,
            0,
            6,
            shadows.len() as u64,
        );
    }

    fn render_text(
        &self,
        encoder: &RenderCommandEncoderRef,
        scene: &Scene,
        viewport_size: Size,
    ) {
        let w = viewport_size.width as u32;
        let h = viewport_size.height as u32;
        if w == 0 || h == 0 {
            return;
        }

        // Rasterize all text items into a pixel buffer
        let pixels = super::text_renderer::rasterize_text_items(scene.text_items(), w, h);

        // Create Metal texture
        let tex_desc = TextureDescriptor::new();
        tex_desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        tex_desc.set_width(w as u64);
        tex_desc.set_height(h as u64);
        tex_desc.set_usage(MTLTextureUsage::ShaderRead);

        let texture = self.device.new_texture(&tex_desc);
        texture.replace_region(
            MTLRegion::new_2d(0, 0, w as u64, h as u64),
            0,
            pixels.as_ptr() as *const _,
            (w as u64) * 4,
        );

        // Render full-screen textured quad
        encoder.set_render_pipeline_state(&self.text_pipeline);
        encoder.set_fragment_texture(0, Some(&texture));

        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 6);
    }
}
