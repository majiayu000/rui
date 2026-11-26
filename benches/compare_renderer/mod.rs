//! The renderer the comparison cases draw with: Metal, or with the `wgpu` feature and
//! `RUI_COMPARE_BACKEND=wgpu`, RUI's wgpu backend (on Metal underneath).
use rui::Size;
use rui::renderer::metal::{MetalRenderer, PendingMetalRenderer};
#[cfg(feature = "wgpu")]
use rui::renderer::wgpu::WgpuRenderer;
use rui::renderer::{RendererError, RendererResourceLimits, Scene};
#[cfg(feature = "wgpu")]
use std::sync::Arc;
#[cfg(feature = "wgpu")]
use std::sync::atomic::{AtomicU64, Ordering};

/// Frames submitted and not yet finished before the next one waits, as
/// `MetalRenderer::render_offscreen` allows.
#[cfg(feature = "wgpu")]
const MAX_FRAMES_IN_FLIGHT: u64 = 3;

/// The renderer every case draws with, picked by `RUI_COMPARE_BACKEND`.
#[allow(clippy::large_enum_variant)] // one per process
pub enum BenchRenderer {
    Metal(MetalRenderer),
    #[cfg(feature = "wgpu")]
    Wgpu {
        renderer: WgpuRenderer,
        /// One offscreen texture per size drawn, so the resize cases, which
        /// alternate two widths, allocate none after warmup, as Metal does.
        textures: Vec<wgpu::Texture>,
        submitted: u64,
        completed: Arc<AtomicU64>,
    },
}

/// A renderer being created on another thread.
pub enum PendingBenchRenderer {
    Metal(PendingMetalRenderer),
    #[cfg(feature = "wgpu")]
    Wgpu(std::thread::JoinHandle<Result<WgpuRenderer, RendererError>>),
}

impl PendingBenchRenderer {
    pub fn wait(self) -> Result<BenchRenderer, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Metal(pending) => BenchRenderer::Metal(pending.wait()?),
            #[cfg(feature = "wgpu")]
            Self::Wgpu(handle) => BenchRenderer::wgpu(
                handle
                    .join()
                    .map_err(|_| "wgpu renderer thread panicked")??,
            ),
        })
    }
}

fn wgpu_requested() -> Result<bool, Box<dyn std::error::Error>> {
    match std::env::var("RUI_COMPARE_BACKEND").as_deref() {
        Err(_) | Ok("metal") => Ok(false),
        Ok("wgpu") if cfg!(feature = "wgpu") => Ok(true),
        Ok("wgpu") => Err("RUI_COMPARE_BACKEND=wgpu needs `--features wgpu`".into()),
        Ok(other) => Err(format!("RUI_COMPARE_BACKEND must be metal or wgpu, not {other}").into()),
    }
}

impl BenchRenderer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "wgpu")]
        if wgpu_requested()? {
            return Ok(Self::wgpu(WgpuRenderer::new()?));
        }
        wgpu_requested()?;
        Ok(Self::Metal(MetalRenderer::new()?))
    }

    #[cfg(feature = "wgpu")]
    fn wgpu(renderer: WgpuRenderer) -> Self {
        Self::Wgpu {
            renderer,
            textures: Vec::new(),
            submitted: 0,
            completed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn spawn() -> Result<PendingBenchRenderer, Box<dyn std::error::Error>> {
        let wgpu = wgpu_requested()?;
        #[cfg(feature = "wgpu")]
        if wgpu {
            return Ok(PendingBenchRenderer::Wgpu(std::thread::spawn(
                WgpuRenderer::new,
            )));
        }
        debug_assert!(!wgpu, "wgpu_requested refuses wgpu without the feature");
        Ok(PendingBenchRenderer::Metal(MetalRenderer::spawn(
            RendererResourceLimits::default(),
        )))
    }

    pub fn render_offscreen(
        &mut self,
        scene: &Scene,
        viewport: Size,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        match self {
            Self::Metal(renderer) => renderer.render_offscreen(scene, viewport, scale_factor),
            #[cfg(feature = "wgpu")]
            Self::Wgpu {
                renderer,
                textures,
                submitted,
                completed,
            } => {
                let (width, height) = (
                    (viewport.width * scale_factor).round() as u32,
                    (viewport.height * scale_factor).round() as u32,
                );
                while *submitted - completed.load(Ordering::Acquire) >= MAX_FRAMES_IN_FLIGHT {
                    renderer
                        .device()
                        .poll(wgpu::PollType::wait_indefinitely())
                        .map_err(|err| RendererError::render_failed(err.to_string()))?;
                }
                let kept = textures
                    .iter()
                    .find(|t| t.width() == width && t.height() == height)
                    .cloned();
                let target = match kept {
                    Some(target) => target,
                    None => {
                        let target = renderer.device().create_texture(&wgpu::TextureDescriptor {
                            label: Some("compare_frame target"),
                            size: wgpu::Extent3d {
                                width,
                                height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: rui::renderer::wgpu::TARGET_FORMAT,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        });
                        textures.push(target.clone());
                        target
                    }
                };
                renderer.render(scene, &target, viewport)?;
                let done = Arc::clone(completed);
                renderer.queue().on_submitted_work_done(move || {
                    done.fetch_add(1, Ordering::Release);
                });
                *submitted += 1;
                Ok(())
            }
        }
    }

    pub fn capture_frame_pixels_at_scale(
        &mut self,
        scene: &Scene,
        viewport: Size,
        scale_factor: f32,
    ) -> Result<Vec<u8>, RendererError> {
        match self {
            Self::Metal(renderer) => {
                renderer.capture_frame_pixels_at_scale(scene, viewport, scale_factor)
            }
            #[cfg(feature = "wgpu")]
            Self::Wgpu { renderer, .. } => {
                renderer.capture_frame_pixels_at_scale(scene, viewport, scale_factor)
            }
        }
    }

    /// Whether each frame commits exactly one command buffer, as Metal's does.
    pub fn one_buffer_per_frame(&self) -> bool {
        matches!(self, Self::Metal(_))
    }

    /// Draw calls and instances of the last frame.
    pub fn last_frame_stats(&self) -> (u32, u32) {
        match self {
            Self::Metal(renderer) => {
                let stats = renderer.last_frame_stats();
                (stats.draw_calls, stats.instances)
            }
            #[cfg(feature = "wgpu")]
            Self::Wgpu { renderer, .. } => {
                let stats = renderer.last_frame_stats();
                (stats.draw_calls, stats.instances)
            }
        }
    }
}
