//! Metal renderer implementation

use super::batch::{BufferLayout, InstancePool, bind_vertex_value, write_slice};
use super::capture::{create_capture_texture, read_texture_rgba, texture_extent};
use super::glyph::GlyphPages;
use super::retained::{RetainedPipelines, RetainedTarget};
use crate::core::geometry::{Bounds, Size};
use crate::renderer::plan::{Draw, FrameData, FramePlanner, ScissorRect, clear_linear};
use crate::renderer::{
    Renderer, RendererDeviceDiagnostics, RendererDiagnostics, RendererError,
    RendererPrimitiveSupport, RendererResourceError, RendererResourceKind, RendererResourceLimits,
    Scene,
};
use metal::*;

mod encoding;
mod offscreen;
mod resources;

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
    drawable_size: [f32; 2],
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

impl From<ScissorRect> for MTLScissorRect {
    fn from(rect: ScissorRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
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
    /// Glyph, image and path caches, and the image textures uploaded for them.
    planner: FramePlanner<Texture>,
    glyph_pages: GlyphPages,
    color_pages: GlyphPages,
    offscreen_target: Option<Texture>,
    frame: FrameData<Texture>,
    instances: InstancePool,
    /// Command buffers the GPU has not finished, with the frame each drew.
    frames_in_flight: Vec<(u64, CommandBuffer)>,
    frame_stats: MetalFrameStats,
    /// GPU time of the last frame that waited for completion.
    gpu_time: Option<std::time::Duration>,
    /// Time what the profiler shows: GPU time of frames found finished, and
    /// the wait for a presented frame's commands to be scheduled.
    profiling: bool,
    completed_gpu_time: Option<std::time::Duration>,
    scheduled_wait: Option<std::time::Duration>,
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

/// The GPU to draw with. `MTLCreateSystemDefaultDevice` picks the GPU of the
/// main display, which took 35-60 ms in a fresh process, against 13-30 ms to
/// list the devices. With one GPU, as on Apple silicon, there is no choice to make.
fn default_device() -> Option<Device> {
    let mut devices = Device::all();
    if devices.len() == 1 {
        devices.pop()
    } else {
        Device::system_default()
    }
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
        let device = default_device()
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
            planner: FramePlanner::new(&limits),
            offscreen_target: None,
            glyph_pages: GlyphPages::coverage(),
            color_pages: GlyphPages::color(),
            frame: FrameData::default(),
            instances: InstancePool::default(),
            frames_in_flight: Vec::new(),
            frame_stats: MetalFrameStats::default(),
            gpu_time: None,
            profiling: false,
            completed_gpu_time: None,
            scheduled_wait: None,
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

    /// Whether to time frames for [`Self::completed_gpu_time`] and
    /// [`Self::scheduled_wait`]. Off by default.
    pub fn set_profiling(&mut self, on: bool) {
        self.profiling = on;
        if !on {
            self.completed_gpu_time = None;
            self.scheduled_wait = None;
        }
    }

    /// GPU time of the latest frame the GPU finished, while
    /// [`Self::set_profiling`] is on. Frames are found finished when a later
    /// one begins, so this is a frame or more behind.
    pub fn completed_gpu_time(&self) -> Option<std::time::Duration> {
        self.completed_gpu_time
    }

    /// How long the last presented frame waited for its commands to be
    /// scheduled before presenting, while [`Self::set_profiling`] is on.
    pub fn scheduled_wait(&self) -> Option<std::time::Duration> {
        self.scheduled_wait
    }

    /// How many paths are drawn from the atlas.
    #[cfg(test)]
    pub(super) fn placed_path_sprites(&self) -> usize {
        self.planner.placed_path_sprites()
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

        // Atlas pages only finished frames drew from may be evicted.
        let time_completed = self.profiling;
        let mut completed_gpu_time = None;
        self.frames_in_flight.retain(|(_, command_buffer)| {
            let status = command_buffer.status();
            if time_completed && status == MTLCommandBufferStatus::Completed {
                // Oldest first, so the newest finished frame is kept.
                completed_gpu_time = Some(gpu_time(command_buffer));
            }
            !matches!(
                status,
                MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
            )
        });
        if completed_gpu_time.is_some() {
            self.completed_gpu_time = completed_gpu_time;
        }
        let oldest_in_flight = self.frames_in_flight.iter().map(|(frame, _)| *frame).min();
        self.planner.begin_frame(oldest_in_flight)?;
        self.frame_stats = MetalFrameStats::default();
        self.gpu_time = None;

        let drawable_size = Size::new(extent.0 as f32, extent.1 as f32);

        let mut frame = std::mem::take(&mut self.frame);
        let damage = retained.as_ref().and_then(|retained| retained.damage);
        let built = self.planner.build(
            &mut frame,
            scene,
            extent,
            viewport_size,
            damage,
            &self.device,
        );
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
}

/// Shader source with the shared color helpers in front.
fn with_color(source: &str) -> String {
    [super::shaders::COLOR_SHADER, source].concat()
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
                self.planner.textures.stats(),
                self.planner.image_cache.stats(),
                self.planner.text_cache.resource_stats(),
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
