//! Metal renderer implementation

use super::batch::{BufferLayout, InstancePool, bind_vertex_value, write_slice};
use super::capture::{create_capture_texture, read_texture_rgba, texture_extent};
use super::frame::{
    Draw, FramePlan, ItemKind, KIND_COLOR_GLYPH, KIND_GLYPH, KIND_PATH, KIND_PATH_SPRITE,
    KIND_QUAD, KIND_SHADOW, Pages,
};
use super::glyph::{GlyphPages, GpuGlyph};
use super::image_primitive::GpuImage;
use super::order::Rect;
use super::path::{GpuPathTile, PathGeometry, PathSprites, PathTiles, SpriteLookup};
use super::retained::{RetainedPipelines, RetainedTarget, damage_scissor, intersect_scissors};
use crate::core::geometry::{Bounds, Size};
use crate::elements::fit_bounds;
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

/// Offscreen frames the GPU may be behind by, as a window's drawables allow.
const MAX_OFFSCREEN_FRAMES_IN_FLIGHT: usize = 3;

/// Steps, in pixels, the kept offscreen texture grows by.
const OFFSCREEN_GROWTH: u64 = 256;

/// Pixel format of every render target: drawables, captures and offscreen
/// textures. Blending happens in linear light and stores sRGB-encoded bytes.
pub(crate) const TARGET_PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm_sRGB;

/// Uniform data passed to shaders
#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    viewport_size: [f32; 2],
}

/// What the last rendered frame asked of the GPU.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetalFrameStats {
    /// Draw calls encoded.
    pub draw_calls: u32,
    /// Instances those draws drew.
    pub instances: u32,
    /// Device pixels drawn again: the whole target, or the damage of a
    /// retained frame.
    pub redrawn_pixels: u64,
}

/// A frame drawn into a [`RetainedTarget`] and copied to `copy_to`.
struct Retained<'a> {
    /// What to draw again, in logical pixels; `None` draws everything.
    damage: Option<Bounds>,
    copy_to: &'a TextureRef,
}

struct ImageDraw {
    texture: Texture,
    instance: GpuImage,
    clip: u32,
}

/// A frame's instance data and planned draws, kept to reuse their storage.
#[derive(Default)]
struct FrameData {
    plan: FramePlan,
    quads: Vec<GpuQuad>,
    shadows: Vec<GpuShadow>,
    glyphs: Vec<GpuGlyph>,
    images: Vec<ImageDraw>,
    path_tiles: Vec<GpuPathTile>,
    /// `[x0, y0, x1, y1]` in device pixels, the edges each path tile reads.
    path_edges: Vec<[f32; 4]>,
    /// The path being built, kept to reuse its storage.
    path: PathGeometry,
    /// Clip rectangles in device pixels, as scissor rectangles and as the
    /// `x0, y0, x1, y1` the sprite shader tests fragments against.
    scissors: Vec<MTLScissorRect>,
    clip_edges: Vec<[f32; 4]>,
    /// The device pixels a retained frame draws again; every clip is cut to it.
    damage: Option<MTLScissorRect>,
}

impl FrameData {
    fn clear(&mut self) {
        self.plan.clear();
        self.quads.clear();
        self.shadows.clear();
        self.glyphs.clear();
        self.images.clear();
        self.path_tiles.clear();
        self.path_edges.clear();
        self.scissors.clear();
        self.clip_edges.clear();
    }

    /// Adds a clip rectangle and returns its index.
    fn push_clip(&mut self, scissor: MTLScissorRect) -> u32 {
        let scissor = match self.damage {
            Some(damage) => intersect_scissors(scissor, damage),
            None => scissor,
        };
        self.scissors.push(scissor);
        self.clip_edges.push([
            scissor.x as f32,
            scissor.y as f32,
            (scissor.x + scissor.width) as f32,
            (scissor.y + scissor.height) as f32,
        ]);
        (self.scissors.len() - 1) as u32
    }

    fn push_item(&mut self, kind: ItemKind, clip: u32, rect: Rect, scale_factor: f32) {
        if let Some(rect) = visible_rect(&self.clip_edges, rect, clip, scale_factor) {
            self.plan.push(kind, clip, Some(rect));
        }
    }

    fn push_quad(&mut self, quad: GpuQuad, clip: u32, scale_factor: f32) {
        let rect = bounds_rect(quad.bounds);
        let kind = ItemKind::Sprites {
            kind: KIND_QUAD,
            start: self.quads.len() as u32,
            count: 1,
            pages: Pages::default(),
        };
        self.quads.push(quad);
        self.push_item(kind, clip, rect, scale_factor);
    }
}

/// `rect` cut to clip `clip`, in logical pixels; `None` when nothing is left.
fn visible_rect(clip_edges: &[[f32; 4]], rect: Rect, clip: u32, scale_factor: f32) -> Option<Rect> {
    let edges = clip_edges[clip as usize];
    let visible = [
        rect[0].max(edges[0] / scale_factor),
        rect[1].max(edges[1] / scale_factor),
        rect[2].min(edges[2] / scale_factor),
        rect[3].min(edges[3] / scale_factor),
    ];
    (visible[0] < visible[2] && visible[1] < visible[3]).then_some(visible)
}

/// `[x, y, width, height]` as the rectangle it covers.
fn bounds_rect(bounds: [f32; 4]) -> Rect {
    let [x, y, width, height] = bounds;
    [
        x.min(x + width),
        y.min(y + height),
        x.max(x + width),
        y.max(y + height),
    ]
}

fn outset(rect: Rect, by: f32) -> Rect {
    [rect[0] - by, rect[1] - by, rect[2] + by, rect[3] + by]
}

fn union(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// A [`MetalRenderer`] being created on another thread; see
/// [`MetalRenderer::spawn`].
pub struct PendingMetalRenderer {
    creation: Result<std::thread::JoinHandle<Result<MetalRenderer, RendererError>>, RendererError>,
}

impl PendingMetalRenderer {
    /// Waits for the renderer, or for the error that stopped its creation.
    pub fn wait(self) -> Result<MetalRenderer, RendererError> {
        self.creation?.join().unwrap_or_else(|_| {
            Err(RendererError::render_failed(
                "creating the Metal renderer panicked",
            ))
        })
    }
}

/// Metal-based renderer
pub struct MetalRenderer {
    device: Device,
    command_queue: CommandQueue,
    sprite_pipeline: RenderPipelineState,
    image_pipeline: RenderPipelineState,
    retained_pipelines: RetainedPipelines,
    sampler: SamplerState,
    /// Bound for sprite draws that sample no atlas page.
    empty_page: Texture,
    /// Bound for sprite draws that sample no color atlas page.
    empty_color_page: Texture,
    textures: HashMap<u32, Texture>,
    texture_resources: RendererResourceCache<u32>,
    image_cache: RendererImageCache,
    text_cache: TextGlyphCache,
    glyph_pages: GlyphPages,
    color_pages: GlyphPages,
    path_sprites: PathSprites,
    path_tiles: PathTiles,
    offscreen_target: Option<Texture>,
    frame: FrameData,
    instances: InstancePool,
    /// Command buffers the GPU has not finished, with the frame each drew.
    frames_in_flight: Vec<(u64, CommandBuffer)>,
    frame_stats: MetalFrameStats,
    /// GPU time of the last frame that waited for completion.
    gpu_time: Option<std::time::Duration>,
}

/// A pipeline drawing into the target format with source-over blending.
fn blended_pipeline(
    device: &Device,
    source: &str,
    vertex: &str,
    fragment: &str,
) -> Result<RenderPipelineState, RendererError> {
    let library = device
        .new_library_with_source(&with_color(source), &CompileOptions::new())
        .map_err(|err| {
            RendererError::render_failed(format!("failed to compile {vertex} shader: {err}"))
        })?;
    let function = |name: &str| {
        library
            .get_function(name, None)
            .map_err(|err| RendererError::render_failed(format!("missing {name} shader: {err}")))
    };
    let desc = RenderPipelineDescriptor::new();
    let (vertex_function, fragment_function) = (function(vertex)?, function(fragment)?);
    desc.set_vertex_function(Some(&vertex_function));
    desc.set_fragment_function(Some(&fragment_function));
    let color = desc
        .color_attachments()
        .object_at(0)
        .ok_or_else(|| RendererError::render_failed("render pipeline has no color attachment"))?;
    color.set_pixel_format(TARGET_PIXEL_FORMAT);
    color.set_blending_enabled(true);
    color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    color.set_source_alpha_blend_factor(MTLBlendFactor::One);
    color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device.new_render_pipeline_state(&desc).map_err(|err| {
        RendererError::render_failed(format!("failed to create {vertex} pipeline: {err}"))
    })
}

impl MetalRenderer {
    pub fn new() -> Result<Self, RendererError> {
        Self::with_resource_limits(RendererResourceLimits::default())
    }

    /// Starts creating a renderer on another thread and returns at once, so the
    /// caller can load fonts and lay out its first frame meanwhile. Opening the
    /// Metal device and building pipelines is most of what a renderer costs to
    /// create.
    pub fn spawn(limits: RendererResourceLimits) -> PendingMetalRenderer {
        let creation = std::thread::Builder::new()
            .name(String::from("rui-renderer-init"))
            .spawn(move || Self::with_resource_limits(limits))
            .map_err(|err| {
                RendererError::render_failed(format!(
                    "failed to start the renderer creation thread: {err}"
                ))
            });
        PendingMetalRenderer { creation }
    }

    pub fn with_resource_limits(limits: RendererResourceLimits) -> Result<Self, RendererError> {
        let device = Device::system_default()
            .ok_or_else(|| RendererError::backend_unavailable("Metal device is unavailable"))?;
        let command_queue = device.new_command_queue();

        // The shaders compile at the same time: on a first launch, before the
        // system's shader cache holds them, each takes tens of milliseconds.
        let (sprite_pipeline, image_pipeline, retained_pipelines) = std::thread::scope(|scope| {
            let retained = scope.spawn(|| RetainedPipelines::new(&device));
            let sprite = scope.spawn(|| {
                blended_pipeline(
                    &device,
                    super::shaders::SPRITE_SHADER,
                    "sprite_vertex",
                    "sprite_fragment",
                )
            });
            let image = blended_pipeline(
                &device,
                super::shaders::IMAGE_SHADER,
                "image_vertex",
                "image_fragment",
            );
            let sprite = sprite.join().unwrap_or_else(|_| {
                Err(RendererError::render_failed(
                    "compiling the sprite shader panicked",
                ))
            });
            let retained = retained.join().unwrap_or_else(|_| {
                Err(RendererError::render_failed(
                    "compiling the copy shader panicked",
                ))
            });
            (sprite, image, retained)
        });
        let (sprite_pipeline, image_pipeline, retained_pipelines) =
            (sprite_pipeline?, image_pipeline?, retained_pipelines?);

        // Sampler for images/text
        let sampler_desc = SamplerDescriptor::new();
        sampler_desc.set_min_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_mag_filter(MTLSamplerMinMagFilter::Linear);
        sampler_desc.set_address_mode_s(MTLSamplerAddressMode::ClampToEdge);
        sampler_desc.set_address_mode_t(MTLSamplerAddressMode::ClampToEdge);
        let sampler = device.new_sampler(&sampler_desc);

        let page_desc = TextureDescriptor::new();
        page_desc.set_texture_type(MTLTextureType::D2);
        page_desc.set_pixel_format(MTLPixelFormat::R8Unorm);
        page_desc.set_width(1);
        page_desc.set_height(1);
        page_desc.set_usage(MTLTextureUsage::ShaderRead);
        let empty_page = device.new_texture(&page_desc);
        page_desc.set_pixel_format(MTLPixelFormat::RGBA8Unorm_sRGB);
        let empty_color_page = device.new_texture(&page_desc);

        Ok(Self {
            device,
            command_queue,
            sprite_pipeline,
            image_pipeline,
            retained_pipelines,
            sampler,
            empty_page,
            empty_color_page,
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
            glyph_pages: GlyphPages::coverage(),
            color_pages: GlyphPages::color(),
            path_sprites: PathSprites::default(),
            path_tiles: PathTiles::default(),
            frame: FrameData::default(),
            instances: InstancePool::default(),
            frames_in_flight: Vec::new(),
            frame_stats: MetalFrameStats::default(),
            gpu_time: None,
        })
    }

    /// Get the Metal device
    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn diagnostics(&self) -> RendererDiagnostics {
        <Self as Renderer>::diagnostics(self)
    }

    /// Draw calls and instances of the last frame rendered.
    pub fn last_frame_stats(&self) -> MetalFrameStats {
        self.frame_stats
    }

    /// GPU time of the last frame, when it waited for the GPU (captures).
    pub fn last_gpu_time(&self) -> Option<std::time::Duration> {
        self.gpu_time
    }

    /// How many paths are drawn from the atlas.
    #[cfg(test)]
    pub(super) fn placed_path_sprites(&self) -> usize {
        self.path_sprites.placed()
    }

    /// The kept offscreen texture.
    #[cfg(test)]
    pub(super) fn offscreen_target(&self) -> Option<Texture> {
        self.offscreen_target.clone()
    }

    /// Plan draws in scene order rather than overlap order.
    #[cfg(test)]
    pub(super) fn set_scene_order(&mut self, scene_order: bool) {
        self.frame.plan.scene_order = scene_order;
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

    /// Render a scene to a drawable through the window's `target`, drawing
    /// again only what changed since the frame the target shows.
    pub fn render_retained(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        drawable: &MetalDrawableRef,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        let output = drawable.texture();
        self.render_retained_to(
            scene,
            target,
            output,
            (output.width(), output.height()),
            viewport_size,
            Some(drawable),
            false,
        )
    }

    /// Draws through `target` and copies the frame to the top left `extent`
    /// device pixels of `output`; the target is that size.
    #[allow(clippy::too_many_arguments)]
    fn render_retained_to(
        &mut self,
        scene: &Scene,
        target: &mut RetainedTarget,
        output: &TextureRef,
        extent: (u64, u64),
        viewport_size: Size,
        present_drawable: Option<&MetalDrawableRef>,
        wait_until_completed: bool,
    ) -> Result<(), RendererError> {
        let (texture, damage) =
            target.prepare(&self.device, scene, extent.0, extent.1, viewport_size);
        self.render_to_texture(
            scene,
            &texture,
            extent,
            viewport_size,
            present_drawable,
            wait_until_completed,
            Some(Retained {
                damage,
                copy_to: output,
            }),
        )?;
        target.drew(scene);
        Ok(())
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

    /// The kept offscreen texture, at least `width` by `height`. It only grows,
    /// in steps of [`OFFSCREEN_GROWTH`] pixels, and frames draw into its top
    /// left, so resizing back and forth reuses one texture.
    fn offscreen_texture(&mut self, width: u64, height: u64) -> Texture {
        match &self.offscreen_target {
            Some(texture) if texture.width() >= width && texture.height() >= height => {
                texture.clone()
            }
            kept => {
                let (kept_width, kept_height) = kept
                    .as_ref()
                    .map_or((0, 0), |texture| (texture.width(), texture.height()));
                let texture = create_capture_texture(
                    &self.device,
                    width.max(kept_width).next_multiple_of(OFFSCREEN_GROWTH),
                    height.max(kept_height).next_multiple_of(OFFSCREEN_GROWTH),
                );
                self.offscreen_target = Some(texture.clone());
                texture
            }
        }
    }

    /// Render a scene to a drawable
    pub fn render(
        &mut self,
        scene: &Scene,
        drawable: &MetalDrawableRef,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        let texture = drawable.texture();
        self.render_to_texture(
            scene,
            texture,
            (texture.width(), texture.height()),
            viewport_size,
            Some(drawable),
            false,
            None,
        )
    }

    /// Draws `scene` into the top left `extent` device pixels of `texture`.
    #[allow(clippy::too_many_arguments)]
    fn render_to_texture(
        &mut self,
        scene: &Scene,
        texture: &TextureRef,
        extent: (u64, u64),
        viewport_size: Size,
        present_drawable: Option<&MetalDrawableRef>,
        wait_until_completed: bool,
        retained: Option<Retained<'_>>,
    ) -> Result<(), RendererError> {
        RendererPrimitiveSupport::metal().validate_scene(scene)?;

        self.texture_resources.begin_frame();
        self.image_cache.begin_frame();
        self.text_cache.begin_frame();
        // Atlas pages only finished frames drew from may be evicted.
        self.frames_in_flight.retain(|(_, command_buffer)| {
            !matches!(
                command_buffer.status(),
                MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
            )
        });
        let oldest_in_flight = self.frames_in_flight.iter().map(|(frame, _)| *frame).min();
        self.text_cache.set_completed_frame(
            oldest_in_flight.map_or(self.text_cache.frame() - 1, |frame| frame - 1),
        );
        self.path_sprites.begin_frame(self.text_cache.epoch());
        self.path_tiles.begin_frame();
        self.frame_stats = MetalFrameStats::default();
        self.gpu_time = None;

        let scale_factor = extent.0 as f32 / viewport_size.width.max(1.0);
        let drawable_size = Size::new(extent.0 as f32, extent.1 as f32);

        let mut frame = std::mem::take(&mut self.frame);
        let damage = retained.as_ref().and_then(|retained| retained.damage);
        let epoch = self.text_cache.epoch();
        let mut built = self.build_frame(
            &mut frame,
            scene,
            viewport_size,
            scale_factor,
            drawable_size,
            damage,
        );
        if built.is_ok() && self.text_cache.epoch() != epoch {
            // A font loaded during the frame emptied the glyph atlas, so glyphs
            // placed before then name slots that no longer hold them. The fonts
            // are loaded now; place everything again.
            let epoch = self.text_cache.epoch();
            built = self.build_frame(
                &mut frame,
                scene,
                viewport_size,
                scale_factor,
                drawable_size,
                damage,
            );
            if built.is_ok() && self.text_cache.epoch() != epoch {
                built = Err(RendererError::render_failed(
                    "the glyph atlas was emptied twice while building one frame",
                ));
            }
        }
        let result = built.and_then(|()| {
            self.encode_frame(
                &mut frame,
                texture,
                drawable_size,
                viewport_size,
                present_drawable,
                wait_until_completed,
                retained.as_ref(),
            )
        });
        self.frame = frame;
        result
    }

    /// Turns the scene into instance data and a plan of draws. With `damage`,
    /// only what can touch it, cut to it.
    fn build_frame(
        &mut self,
        frame: &mut FrameData,
        scene: &Scene,
        viewport_size: Size,
        scale_factor: f32,
        drawable_size: Size,
        damage: Option<Bounds>,
    ) -> Result<(), RendererError> {
        frame.clear();
        frame.damage = damage.map(|damage| damage_scissor(damage, scale_factor, drawable_size));
        // The damage as the device pixels it covers, which is what gets drawn.
        let redrawn = frame.damage.map(|scissor| {
            Bounds::from_xywh(
                scissor.x as f32 / scale_factor,
                scissor.y as f32 / scale_factor,
                scissor.width as f32 / scale_factor,
                scissor.height as f32 / scale_factor,
            )
        });
        let viewport = Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height);
        let mut clip_stack: Vec<Bounds> = Vec::new();
        let mut clip = frame.push_clip(scissor_rect(None, scale_factor, drawable_size));

        for primitive in scene.primitives() {
            // Nothing a primitive outside the current clip draws can be seen,
            // nor, in a retained frame, anything outside the damage.
            let visible = clip_stack.last().copied().unwrap_or(viewport);
            let visible = match redrawn {
                Some(redrawn) => visible.intersection(&redrawn),
                None => Some(visible),
            };
            let clips = matches!(primitive, Primitive::PushClip { .. } | Primitive::PopClip);
            if !clips && !visible.is_some_and(|visible| primitive.may_touch(&visible)) {
                continue;
            }
            match primitive {
                Primitive::Shadow {
                    bounds,
                    corner_radii,
                    blur_radius,
                    color,
                } => {
                    let shadow =
                        GpuShadow::from_primitive(*bounds, *corner_radii, *blur_radius, *color);
                    // As far as the shader's blur margin reaches, and half a
                    // pixel more for rounding.
                    let margin = 3.0 * (blur_radius.max(0.0) * 0.5) + 1.5;
                    let kind = ItemKind::Sprites {
                        kind: KIND_SHADOW,
                        start: frame.shadows.len() as u32,
                        count: 1,
                        pages: Pages::default(),
                    };
                    frame.shadows.push(shadow);
                    let rect = outset(bounds_rect(shadow.bounds), margin);
                    frame.push_item(kind, clip, rect, scale_factor);
                }
                Primitive::Quad {
                    bounds,
                    background,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let quad = GpuQuad::solid(
                        *bounds,
                        *background,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
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
                    let quad = GpuQuad::linear_gradient(
                        *bounds,
                        *start,
                        *end,
                        angle.to_radians(),
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
                }
                Primitive::RadialGradient {
                    bounds,
                    inner,
                    outer,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let quad = GpuQuad::radial_gradient(
                        *bounds,
                        *inner,
                        *outer,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
                }
                Primitive::Image {
                    bounds,
                    source,
                    fit,
                    corner_radii,
                    opacity,
                } => {
                    self.build_image(
                        frame,
                        bounds,
                        source,
                        *fit,
                        *corner_radii,
                        *opacity,
                        clip,
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
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
                    self.build_text(
                        frame,
                        bounds,
                        content,
                        *color,
                        *font_size,
                        *font_weight,
                        font_family.as_deref(),
                        *line_height,
                        *align,
                        clip,
                        scale_factor,
                    )?;
                }
                Primitive::Path {
                    vertices,
                    color,
                    stroke_width,
                } => {
                    let mut path = std::mem::take(&mut frame.path);
                    let built = path
                        .set(vertices, *stroke_width, scale_factor)
                        .and_then(|()| self.build_path(frame, &path, *color, clip, scale_factor));
                    frame.path = path;
                    built?;
                }
                Primitive::PushClip { bounds, .. } => {
                    let new_clip = if let Some(prev) = clip_stack.last() {
                        prev.intersection(bounds)
                            .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
                    } else {
                        *bounds
                    };
                    clip_stack.push(new_clip);
                    clip = frame.push_clip(scissor_rect(
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
                    ));
                }
                Primitive::PopClip => {
                    clip_stack.pop();
                    clip = frame.push_clip(scissor_rect(
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
                    ));
                }
            }
        }

        // Text runs are pushed without their extent, which only ordering needs.
        let FrameData {
            plan,
            glyphs,
            clip_edges,
            ..
        } = frame;
        plan.plan(
            viewport_size.width.max(1.0),
            viewport_size.height.max(1.0),
            |kind, clip| {
                let ItemKind::Sprites { start, count, .. } = kind else {
                    return None;
                };
                let rect = glyphs[start as usize..(start + count) as usize]
                    .iter()
                    .map(|glyph| bounds_rect(glyph.bounds))
                    .reduce(union)?;
                visible_rect(clip_edges, rect, clip, scale_factor)
            },
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_frame(
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
        for (index, offset) in sprite_data {
            encoder.set_vertex_buffer(index, Some(&instances), offset as u64);
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
        if let Some(drawable) = present_drawable {
            command_buffer.present_drawable(drawable);
        }
        command_buffer.commit();
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

    fn count_draw(&mut self, instances: usize) {
        self.frame_stats.draw_calls += 1;
        self.frame_stats.instances += instances as u32;
    }

    /// Draws `path` from the atlas when it is a small shape drawn before, and as
    /// analytic tiles otherwise.
    fn build_path(
        &mut self,
        frame: &mut FrameData,
        path: &PathGeometry,
        color: crate::core::color::Rgba,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        // A font loaded earlier in this frame may have emptied the atlas.
        self.path_sprites.sync_epoch(self.text_cache.epoch());
        let (cell, key) = match self.path_sprites.lookup(path) {
            SpriteLookup::Analytic => (None, 0),
            SpriteLookup::Placed(slot, generation, cell, key) => {
                if self.text_cache.use_sprite(slot, generation) {
                    (Some((Some(slot), cell)), key)
                } else {
                    // Its atlas page was emptied: place it again.
                    (Some((None, cell)), key)
                }
            }
            SpriteLookup::Rasterize(cell, key) => (Some((None, cell)), key),
        };
        let placed = match cell {
            None => None,
            Some((Some(slot), cell)) => Some((slot, cell)),
            Some((None, cell)) => {
                let pixels = path.rasterize(cell.origin, cell.width, cell.height);
                match self
                    .text_cache
                    .place_sprite(cell.width, cell.height, pixels)
                {
                    Ok((slot, generation)) => {
                        self.path_sprites.place(key, slot, generation);
                        Some((slot, cell))
                    }
                    // Without atlas room the path is still drawn, as tiles.
                    Err(RendererResourceError::ResourcePressure { .. }) => None,
                    Err(err) => return Err(err.into()),
                }
            }
        };

        if let Some((slot, cell)) = placed {
            let (page_width, page_height) = self
                .text_cache
                .atlas()
                .page_extent(slot.page)
                .ok_or_else(|| {
                    RendererResourceError::missing(RendererResourceKind::Glyph, slot.page as u32)
                })?;
            let rect = slot.rect;
            let sprite = GpuGlyph {
                bounds: [
                    cell.origin[0] / scale_factor,
                    cell.origin[1] / scale_factor,
                    cell.width as f32 / scale_factor,
                    cell.height as f32 / scale_factor,
                ],
                uv: [
                    rect.x as f32 / page_width as f32,
                    rect.y as f32 / page_height as f32,
                    rect.width as f32 / page_width as f32,
                    rect.height as f32 / page_height as f32,
                ],
                color: color.to_array(),
            };
            let kind = ItemKind::Sprites {
                kind: KIND_PATH_SPRITE,
                start: frame.glyphs.len() as u32,
                count: 1,
                pages: Pages::coverage(slot.page as u32),
            };
            frame.glyphs.push(sprite);
            frame.push_item(kind, clip, bounds_rect(sprite.bounds), scale_factor);
            return Ok(());
        }

        let start = frame.path_tiles.len() as u32;
        let visible = frame.clip_edges[clip as usize];
        if let Some(rect) = self.path_tiles.push(
            path,
            color,
            scale_factor,
            visible,
            &mut frame.path_tiles,
            &mut frame.path_edges,
        ) {
            let kind = ItemKind::Sprites {
                kind: KIND_PATH,
                start,
                count: frame.path_tiles.len() as u32 - start,
                pages: Pages::default(),
            };
            frame.push_item(kind, clip, rect, scale_factor);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_image(
        &mut self,
        frame: &mut FrameData,
        bounds: &Bounds,
        source: &ImageSource,
        fit: ImageFit,
        corner_radii: crate::core::style::Corners,
        opacity: f32,
        clip: u32,
        current_clip: Option<Bounds>,
        scale_factor: f32,
        drawable_size: Size,
    ) -> Result<(), RendererError> {
        if let ImageSource::Texture(id) = source {
            let texture = self
                .textures
                .get(id)
                .ok_or_else(|| RendererResourceError::missing(RendererResourceKind::Texture, *id))?
                .clone();
            let instance = GpuImage::new(*bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
            let rect = bounds_rect(instance.bounds);
            frame.images.push(ImageDraw {
                texture,
                instance,
                clip,
            });
            let kind = ItemKind::Image((frame.images.len() - 1) as u32);
            frame.push_item(kind, clip, rect, scale_factor);
            return Ok(());
        }

        let entry = self.image_cache.resolve(source)?;

        let texture = self.ensure_texture(entry.handle.id.as_u32(), entry.size, &entry.pixels)?;

        let dest_bounds = fit_bounds(*bounds, entry.size, fit);

        // Clip to container for cover/contain
        let container_clip = if let Some(prev) = current_clip {
            prev.intersection(bounds)
                .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
        } else {
            *bounds
        };
        let image_clip = frame.push_clip(scissor_rect(
            Some(container_clip),
            scale_factor,
            drawable_size,
        ));

        let instance = GpuImage::new(dest_bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
        let rect = bounds_rect(instance.bounds);
        frame.images.push(ImageDraw {
            texture,
            instance,
            clip: image_clip,
        });
        let kind = ItemKind::Image((frame.images.len() - 1) as u32);
        frame.push_item(kind, image_clip, rect, scale_factor);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_text(
        &mut self,
        frame: &mut FrameData,
        bounds: &Bounds,
        content: &str,
        color: crate::core::color::Rgba,
        font_size: f32,
        font_weight: u16,
        font_family: Option<&str>,
        line_height: f32,
        align: crate::elements::text::TextAlign,
        clip: u32,
        scale_factor: f32,
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

        // Glyphs of one kind on one atlas page form one item; a color glyph, or
        // a glyph on another page, starts the next, ordered after it where they
        // overlap.
        let mut run: Option<(bool, u32, u32)> = None;
        let flush = |frame: &mut FrameData, run: Option<(bool, u32, u32)>| {
            if let Some((color, page, start)) = run {
                let (kind, pages) = if color {
                    (KIND_COLOR_GLYPH, Pages::color(page))
                } else {
                    (KIND_GLYPH, Pages::coverage(page))
                };
                let kind = ItemKind::Sprites {
                    kind,
                    start,
                    count: frame.glyphs.len() as u32 - start,
                    pages,
                };
                frame.plan.push(kind, clip, None);
            }
        };

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
            let page = slot.page as u32;
            let atlas = if atlas_glyph.color {
                self.text_cache.color_atlas()
            } else {
                self.text_cache.atlas()
            };
            let (page_width, page_height) = atlas.page_extent(slot.page).ok_or_else(|| {
                RendererResourceError::missing(RendererResourceKind::Glyph, slot.page as u32)
            })?;
            let rect = slot.rect;
            let sprite = GpuGlyph {
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
            };
            let key = (atlas_glyph.color, page);
            if run.is_none_or(|(color, open, _)| (color, open) != key) {
                flush(frame, run);
                run = Some((key.0, key.1, frame.glyphs.len() as u32));
            }
            frame.glyphs.push(sprite);
        }
        flush(frame, run);
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
}

/// The device pixels `clip` keeps, or the whole drawable without one.
fn scissor_rect(clip: Option<Bounds>, scale_factor: f32, drawable_size: Size) -> MTLScissorRect {
    let Some(bounds) = clip else {
        return MTLScissorRect {
            x: 0,
            y: 0,
            width: drawable_size.width as u64,
            height: drawable_size.height as u64,
        };
    };
    if bounds.is_empty() {
        return MTLScissorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
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

    MTLScissorRect {
        x,
        y,
        width,
        height,
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

/// `GPUEndTime - GPUStartTime` of a completed command buffer; metal-rs does not
/// wrap these properties.
fn gpu_time(command_buffer: &CommandBufferRef) -> std::time::Duration {
    // SAFETY: a metal-rs `CommandBufferRef` is the `MTLCommandBuffer` object itself.
    let object = unsafe {
        &*(command_buffer as *const CommandBufferRef as *const objc2::runtime::AnyObject)
    };
    // SAFETY: both properties are `CFTimeInterval` (f64) getters on MTLCommandBuffer.
    let (start, end): (f64, f64) = unsafe {
        (
            objc2::msg_send![object, GPUStartTime],
            objc2::msg_send![object, GPUEndTime],
        )
    };
    std::time::Duration::from_secs_f64((end - start).max(0.0))
}
