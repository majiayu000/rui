//! GPUI's headless renderer wrapped for the comparison: it counts draw calls and
//! instances per scene and tags each frame for the GPU sample. `run.sh` copies it next
//! to the fixtures, which declare `mod counting_renderer;`.

use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{DevicePixels, PlatformAtlas, PlatformHeadlessRenderer, PrimitiveBatch, Scene};

/// Batches and instances of each scene rendered since the last [`take_draws`].
pub(crate) static SCENES: std::sync::Mutex<Vec<(u64, u64)>> = std::sync::Mutex::new(Vec::new());
/// Scenes with at least one glyph since the last check in [`counted`].
pub(crate) static FRAMES_WITH_GLYPHS: AtomicUsize = AtomicUsize::new(0);

/// Frames RUI's comparison bench discards before it samples, and frames it samples:
/// GPU time is taken from the same frames of every GPUI window.
pub(crate) const GPU_WARMUP: usize = 10;
pub(crate) const GPU_SAMPLES: usize = 60;
/// Windows made so far; each gets its own renderer.
static WINDOWS: AtomicUsize = AtomicUsize::new(0);

/// GPUI's headless Metal renderer of one window, counting what each scene asks of the
/// GPU and tagging each frame with the window and its place in it, for
/// [`gpu_time::steady_samples`]. Its Metal renderer issues one instanced draw per
/// `Scene::batches()` entry; paths and surfaces would add more, but no fixture draws
/// them.
pub(crate) struct CountingRenderer {
    inner: Box<dyn PlatformHeadlessRenderer>,
    window: usize,
    frames: usize,
}

impl CountingRenderer {
    pub(crate) fn new(inner: Box<dyn PlatformHeadlessRenderer>) -> Self {
        Self {
            inner,
            window: WINDOWS.fetch_add(1, Ordering::Relaxed),
            frames: 0,
        }
    }

    fn next_frame(&mut self) {
        crate::gpu_time::set_frame(crate::gpu_time::tag(self.window, self.frames));
        self.frames += 1;
    }

    fn count(scene: &Scene) {
        let (mut batches, mut instances) = (0, 0);
        for batch in scene.batches() {
            batches += 1;
            instances += match batch {
                PrimitiveBatch::Shadows(range)
                | PrimitiveBatch::Quads(range)
                | PrimitiveBatch::Paths(range)
                | PrimitiveBatch::Underlines(range)
                | PrimitiveBatch::Surfaces(range)
                | PrimitiveBatch::MonochromeSprites { range, .. }
                | PrimitiveBatch::SubpixelSprites { range, .. }
                | PrimitiveBatch::PolychromeSprites { range, .. } => range.len() as u64,
            };
        }
        SCENES.lock().unwrap().push((batches, instances));
        if !scene.monochrome_sprites.is_empty() || !scene.subpixel_sprites.is_empty() {
            FRAMES_WITH_GLYPHS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl PlatformHeadlessRenderer for CountingRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: gpui::Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        Self::count(scene);
        self.next_frame();
        self.inner.render_scene_to_image(scene, size)
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        size: gpui::Size<DevicePixels>,
    ) -> anyhow::Result<()> {
        Self::count(scene);
        self.next_frame();
        self.inner.render_scene(scene, size)
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn PlatformAtlas> {
        self.inner.sprite_atlas()
    }
}

/// Median draw calls and instances per scene since the last call. The median skips
/// the empty first scene of each new window.
pub(crate) fn take_draws() -> (u64, u64) {
    let mut scenes = std::mem::take(&mut *SCENES.lock().unwrap());
    if scenes.is_empty() {
        return (0, 0);
    }
    let middle = scenes.len() / 2;
    let batches = *scenes.select_nth_unstable_by_key(middle, |scene| scene.0).1;
    let instances = *scenes.select_nth_unstable_by_key(middle, |scene| scene.1).1;
    (batches.0, instances.1)
}
