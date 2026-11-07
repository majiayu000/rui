//! Redrawing only what changed into a texture kept across frames.
//!
//! The frame is drawn into a [`RetainedTarget`], as `plan::retained` describes.
//! A render pass then copies the texture to the drawable; a blit encoder would
//! make the driver allocate far more memory (#175).

use super::renderer::TARGET_PIXEL_FORMAT;
use super::shader_cache::ShaderLibrary;
use crate::core::geometry::{Bounds, Size};
use crate::renderer::plan::RetainedFrame;
use crate::renderer::{RendererError, Scene};
use metal::*;

pub(super) const RETAINED_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct FullscreenOut {
    float4 position [[position]];
};

vertex FullscreenOut fullscreen_vertex(uint vertex_id [[vertex_id]]) {
    // One triangle over the whole target.
    float2 corner = float2((vertex_id << 1) & 2, vertex_id & 2);
    FullscreenOut out;
    out.position = float4(corner * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fragment float4 copy_fragment(
    FullscreenOut in [[stage_in]],
    texture2d<float> source [[texture(0)]]
) {
    return source.read(uint2(in.position.xy));
}

fragment float4 clear_fragment(constant float4& color [[buffer(0)]]) {
    return color;
}
"#;

/// The pipelines a retained frame needs on top of the drawing ones.
pub(super) struct RetainedPipelines {
    /// Copies a retained texture to a drawable, pixel for pixel.
    pub(super) copy: RenderPipelineState,
    /// Fills the damage with the clear color, as a cleared target starts.
    pub(super) clear: RenderPipelineState,
}

impl RetainedPipelines {
    /// The pipelines, and how their shader was loaded for the `RUI_PROFILE`
    /// line.
    pub(super) fn new(device: &Device) -> Result<(Self, String), RendererError> {
        let library = ShaderLibrary::load(
            device,
            "copy",
            RETAINED_SHADER,
            &["fullscreen_vertex", "copy_fragment", "clear_fragment"],
        )
        .map_err(|err| {
            RendererError::render_failed(format!("failed to compile the copy shader: {err}"))
        })?;
        let started = std::time::Instant::now();
        let pipeline = |fragment: &str| {
            let function = |name: &str| {
                library.function(name).map_err(|err| {
                    RendererError::render_failed(format!("missing {name} shader: {err}"))
                })
            };
            let desc = RenderPipelineDescriptor::new();
            let (vertex_function, fragment_function) =
                (function("fullscreen_vertex")?, function(fragment)?);
            desc.set_vertex_function(Some(&vertex_function));
            desc.set_fragment_function(Some(&fragment_function));
            let color = desc.color_attachments().object_at(0).ok_or_else(|| {
                RendererError::render_failed("render pipeline has no color attachment")
            })?;
            color.set_pixel_format(TARGET_PIXEL_FORMAT);
            color.set_blending_enabled(false);
            let state = library.pipeline(device, &desc).map_err(|err| {
                RendererError::render_failed(format!("failed to create {fragment} pipeline: {err}"))
            })?;
            Ok::<_, RendererError>((state, desc))
        };
        let (copy, copy_desc) = pipeline("copy_fragment")?;
        let (clear, clear_desc) = pipeline("clear_fragment")?;
        let pipelines = started.elapsed();
        library.store(device, &[&copy_desc, &clear_desc]);
        Ok((Self { copy, clear }, library.report(pipelines)))
    }
}

/// A texture a window keeps its last frame in, so the next frame redraws only
/// the damage. One per window: it holds that window's pixels.
#[derive(Default)]
pub struct RetainedTarget {
    texture: Option<Texture>,
    shown: RetainedFrame,
}

impl RetainedTarget {
    pub fn new() -> Self {
        Self::default()
    }

    /// The texture to draw `width` by `height` device pixels into, and the
    /// part of it `scene` has to draw again: `None` for all of it.
    pub(super) fn prepare(
        &mut self,
        device: &Device,
        scene: &Scene,
        width: u64,
        height: u64,
        viewport_size: Size,
    ) -> (Texture, Option<Bounds>) {
        let kept = match &self.texture {
            Some(texture) if texture.width() == width && texture.height() == height => {
                Some(texture.clone())
            }
            _ => None,
        };
        let damage = self.shown.damage(kept.is_some(), scene, viewport_size);
        let texture = kept.unwrap_or_else(|| {
            let desc = TextureDescriptor::new();
            desc.set_texture_type(MTLTextureType::D2);
            desc.set_pixel_format(TARGET_PIXEL_FORMAT);
            desc.set_width(width);
            desc.set_height(height);
            desc.set_storage_mode(MTLStorageMode::Private);
            desc.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
            let texture = device.new_texture(&desc);
            self.texture = Some(texture.clone());
            texture
        });
        (texture, damage)
    }

    /// Record that the texture now shows `scene`.
    pub(super) fn drew(&mut self, scene: &Scene) {
        self.shown.drew(scene);
    }
}
