//! wgpu rendering backend (feature `wgpu`): Vulkan, DX12 and Metal.
//!
//! It draws the frames `renderer::plan` builds, as the Metal backend does, with
//! WGSL ports of the Metal shaders (ADR 0002). It renders into textures: an
//! offscreen capture, a [`RetainedTarget`], a caller's `Bgra8UnormSrgb`
//! surface texture, or a window through [`WindowSurface`].

mod atlas;
mod encode;
mod shaders;
mod surface;
mod target;
#[cfg(all(test, target_os = "macos", feature = "metal"))]
mod tests;

pub use surface::WindowSurface;
pub use target::RetainedTarget;

use crate::core::geometry::Size;
use crate::renderer::plan::{FrameData, FramePlanner, TextureUpload};
use crate::renderer::{
    Renderer, RendererDeviceDiagnostics, RendererDiagnostics, RendererError,
    RendererPrimitiveSupport, RendererResourceLimits, Scene,
};
use atlas::GlyphPages;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// Format of every render target. Blending happens in linear light and stores
/// sRGB-encoded bytes, as Metal's `BGRA8Unorm_sRGB` does.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// What the last rendered frame asked of the GPU.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WgpuFrameStats {
    /// Draw calls encoded.
    pub draw_calls: u32,
    /// Instances those draws drew.
    pub instances: u32,
    /// Device pixels drawn again: the whole target, or the damage of a
    /// retained frame.
    pub redrawn_pixels: u64,
}

/// An uploaded image, ready to bind.
#[derive(Clone)]
pub(crate) struct ImageTexture {
    bind_group: wgpu::BindGroup,
}

/// Uploads image textures for the planner.
struct Uploader<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    layout: &'a wgpu::BindGroupLayout,
    sampler: &'a wgpu::Sampler,
}

impl TextureUpload for Uploader<'_> {
    type Texture = ImageTexture;

    fn upload(&self, size: Size, pixels: &[u8]) -> ImageTexture {
        let width = size.width.max(1.0).round() as u32;
        let height = size.height.max(1.0).round() as u32;
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rui image"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            extent,
        );
        let view = texture.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rui image"),
            layout: self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(self.sampler),
                },
            ],
        });
        ImageTexture { bind_group }
    }
}

/// Bind group layouts the pipelines share with the bind groups of a frame.
struct Layouts {
    /// Sprite frame data: uniforms, sprite refs, instances, clips, paths, edges.
    sprites: wgpu::BindGroupLayout,
    /// A coverage page, a color page and the sampler.
    pages: wgpu::BindGroupLayout,
    /// Image frame data: uniforms and image instances.
    images: wgpu::BindGroupLayout,
    /// An image texture and the sampler.
    image: wgpu::BindGroupLayout,
    /// The retained texture a copy reads.
    copy: wgpu::BindGroupLayout,
    /// The clear color of a retained frame's damage.
    clear: wgpu::BindGroupLayout,
}

/// wgpu-based renderer.
pub struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    layouts: Layouts,
    sprite_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    copy_pipeline: wgpu::RenderPipeline,
    clear_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    /// Bound for sprite draws that sample no coverage or no color page.
    empty_page: wgpu::TextureView,
    empty_color_page: wgpu::TextureView,
    glyph_pages: GlyphPages,
    color_pages: GlyphPages,
    /// Page bind groups by (coverage page, color page), with the page
    /// generations they were made for.
    page_groups: HashMap<(Option<u32>, Option<u32>), wgpu::BindGroup>,
    page_generations: (u64, u64),
    /// Glyph, image and path caches, and the image textures uploaded for them.
    planner: FramePlanner<ImageTexture>,
    frame: FrameData<ImageTexture>,
    /// The frame's uniforms and instance data, written into `frame_buffer`.
    frame_bytes: Vec<u8>,
    frame_buffer: Option<wgpu::Buffer>,
    /// Text cache frame of the last frame submitted, and of the last one the
    /// GPU finished, which a queue callback records.
    submitted_frame: u64,
    completed_frame: Arc<AtomicU64>,
    frame_stats: WgpuFrameStats,
}

impl WgpuRenderer {
    pub fn new() -> Result<Self, RendererError> {
        Self::with_resource_limits(RendererResourceLimits::default())
    }

    /// Opens the default adapter of the platform's native backends.
    pub fn with_resource_limits(limits: RendererResourceLimits) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = request_adapter(&instance, None)?;
        Self::on_adapter(&adapter, limits)
    }

    /// A renderer on a device of `adapter`.
    fn on_adapter(
        adapter: &wgpu::Adapter,
        limits: RendererResourceLimits,
    ) -> Result<Self, RendererError> {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rui"),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|err| RendererError::backend_unavailable(format!("no wgpu device: {err}")))?;
        Self::from_device(device, queue, adapter.get_info(), limits)
    }

    /// A renderer drawing with `device`, which must allow
    /// [`wgpu::Limits::default`].
    pub fn from_device(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: wgpu::AdapterInfo,
        limits: RendererResourceLimits,
    ) -> Result<Self, RendererError> {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layouts = Layouts::new(&device);
        let module = |label: &str, source: &[&str]| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.concat().into()),
            })
        };
        let sprite = module(
            "rui sprite shader",
            &[shaders::COLOR_SHADER, shaders::SPRITE_SHADER],
        );
        let image = module(
            "rui image shader",
            &[shaders::COLOR_SHADER, shaders::IMAGE_SHADER],
        );
        let retained = module("rui retained shader", &[shaders::RETAINED_SHADER]);
        let blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        });
        let sprite_pipeline = pipeline(
            &device,
            &sprite,
            ("sprite_vertex", "sprite_fragment"),
            &[&layouts.sprites, &layouts.pages],
            blend,
        );
        let image_pipeline = pipeline(
            &device,
            &image,
            ("image_vertex", "image_fragment"),
            &[&layouts.images, &layouts.image],
            blend,
        );
        let copy_pipeline = pipeline(
            &device,
            &retained,
            ("fullscreen_vertex", "copy_fragment"),
            &[&layouts.copy],
            None,
        );
        let clear_pipeline = pipeline(
            &device,
            &retained,
            ("fullscreen_vertex", "clear_fragment"),
            &[&layouts.clear],
            None,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rui sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let empty = |format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("rui empty page"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let empty_page = empty(wgpu::TextureFormat::R8Unorm);
        let empty_color_page = empty(wgpu::TextureFormat::Rgba8UnormSrgb);
        if let Some(err) = pollster::block_on(scope.pop()) {
            return Err(RendererError::render_failed(format!(
                "failed to create the wgpu pipelines: {err}"
            )));
        }

        Ok(Self {
            device,
            queue,
            adapter_info,
            layouts,
            sprite_pipeline,
            image_pipeline,
            copy_pipeline,
            clear_pipeline,
            sampler,
            empty_page,
            empty_color_page,
            glyph_pages: GlyphPages::coverage(),
            color_pages: GlyphPages::color(),
            page_groups: HashMap::new(),
            page_generations: (0, 0),
            planner: FramePlanner::new(&limits),
            frame: FrameData::default(),
            frame_bytes: Vec::new(),
            frame_buffer: None,
            submitted_frame: 0,
            completed_frame: Arc::new(AtomicU64::new(0)),
            frame_stats: WgpuFrameStats::default(),
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn diagnostics(&self) -> RendererDiagnostics {
        <Self as Renderer>::diagnostics(self)
    }

    /// Draw calls and instances of the last frame rendered.
    pub fn last_frame_stats(&self) -> WgpuFrameStats {
        self.frame_stats
    }

    /// Render a scene into `target`, a `Bgra8UnormSrgb` texture such as a
    /// surface texture, `viewport_size` logical pixels across its extent.
    pub fn render(
        &mut self,
        scene: &Scene,
        target: &wgpu::Texture,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        check_format(target)?;
        let view = target.create_view(&Default::default());
        let extent = (u64::from(target.width()), u64::from(target.height()));
        self.render_to_view(scene, &view, extent, viewport_size, None)
    }

    /// Plans `scene` and draws it into the top left `extent` device pixels
    /// of `view`, through a kept texture when `retained` is given.
    fn render_to_view(
        &mut self,
        scene: &Scene,
        view: &wgpu::TextureView,
        extent: (u64, u64),
        viewport_size: Size,
        retained: Option<encode::Retained<'_>>,
    ) -> Result<(), RendererError> {
        RendererPrimitiveSupport::wgpu().validate_scene(scene)?;
        // Callbacks of finished submissions run here.
        let _ = self.device.poll(wgpu::PollType::Poll);
        let completed = self
            .completed_frame
            .load(std::sync::atomic::Ordering::Acquire);
        // Submissions finish in order, so every frame after `completed` up to
        // the last submitted is in flight.
        let oldest_in_flight = (completed < self.submitted_frame).then_some(completed + 1);
        self.planner.begin_frame(oldest_in_flight)?;
        self.frame_stats = WgpuFrameStats::default();

        let mut frame = std::mem::take(&mut self.frame);
        let uploader = Uploader {
            device: &self.device,
            queue: &self.queue,
            layout: &self.layouts.image,
            sampler: &self.sampler,
        };
        let damage = retained.as_ref().and_then(|retained| retained.damage);
        let result = self
            .planner
            .build(&mut frame, scene, extent, viewport_size, damage, &uploader)
            .and_then(|()| {
                self.encode_frame(&frame, view, extent, viewport_size, retained.as_ref())
            });
        self.frame = frame;
        result
    }
}

impl Renderer for WgpuRenderer {
    type Target = wgpu::Texture;

    fn render(
        &mut self,
        scene: &Scene,
        target: &Self::Target,
        viewport_size: Size,
    ) -> Result<(), RendererError> {
        WgpuRenderer::render(self, scene, target, viewport_size)
    }

    fn diagnostics(&self) -> RendererDiagnostics {
        RendererDiagnostics::new(
            RendererDeviceDiagnostics {
                backend: format!("wgpu-{:?}", self.adapter_info.backend).to_lowercase(),
                device_name: self.adapter_info.name.clone(),
                is_headless: true,
                unified_memory: None,
                recommended_max_working_set_size: None,
            },
            vec![
                self.planner.textures.stats(),
                self.planner.image_cache.stats(),
                self.planner.text_cache.resource_stats(),
            ],
        )
        .with_unsupported_primitives(RendererPrimitiveSupport::wgpu().unsupported_primitives())
    }
}

/// The default adapter of the instance's backends, able to present to
/// `surface` when one is given.
fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, RendererError> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        ..Default::default()
    }))
    .map_err(|err| RendererError::backend_unavailable(format!("no wgpu adapter: {err}")))
}

fn check_format(texture: &wgpu::Texture) -> Result<(), RendererError> {
    if texture.format() == TARGET_FORMAT {
        Ok(())
    } else {
        Err(RendererError::render_failed(format!(
            "wgpu render targets must be {TARGET_FORMAT:?}, not {:?}",
            texture.format()
        )))
    }
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    (vertex, fragment): (&str, &str),
    layouts: &[&wgpu::BindGroupLayout],
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layouts: Vec<_> = layouts.iter().map(|layout| Some(*layout)).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(vertex),
        bind_group_layouts: &layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vertex),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: TARGET_FORMAT,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Layouts {
    fn new(device: &wgpu::Device) -> Self {
        use wgpu::ShaderStages as Stages;
        let buffer = |binding, visibility, uniform: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: if uniform {
                    wgpu::BufferBindingType::Uniform
                } else {
                    wgpu::BufferBindingType::Storage { read_only: true }
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: Stages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: Stages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        Self {
            sprites: layout(
                "rui sprites",
                &[
                    buffer(0, Stages::VERTEX, true),
                    buffer(1, Stages::VERTEX, false),
                    buffer(2, Stages::VERTEX, false),
                    buffer(3, Stages::VERTEX, false),
                    buffer(4, Stages::VERTEX, false),
                    buffer(5, Stages::VERTEX, false),
                    buffer(6, Stages::VERTEX_FRAGMENT, false),
                    buffer(7, Stages::FRAGMENT, false),
                ],
            ),
            pages: layout("rui pages", &[texture(0), texture(1), sampler(2)]),
            images: layout(
                "rui images",
                &[
                    buffer(0, Stages::VERTEX, true),
                    buffer(1, Stages::VERTEX, false),
                ],
            ),
            image: layout("rui image", &[texture(0), sampler(1)]),
            copy: layout(
                "rui copy",
                &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: Stages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            ),
            clear: layout("rui clear", &[buffer(1, Stages::FRAGMENT, true)]),
        }
    }
}
