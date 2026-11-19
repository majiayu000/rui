//! GPUI's headless renderer wrapped for the comparison: it counts draw calls and
//! instances per scene, and can redraw scenes during the dedicated GPU sample.
//! `run.sh` copies it next to the fixtures.

use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{DevicePixels, PlatformAtlas, PlatformHeadlessRenderer, PrimitiveBatch, Scene};

/// Batches and instances of each scene rendered since the last [`take_draws`].
pub(crate) static SCENES: std::sync::Mutex<Vec<(u64, u64)>> = std::sync::Mutex::new(Vec::new());
/// Scenes with at least one glyph since the last check in [`counted`].
pub(crate) static FRAMES_WITH_GLYPHS: AtomicUsize = AtomicUsize::new(0);

/// GPUI's headless Metal renderer of one window, counting what each scene asks of the
/// GPU. Its Metal renderer issues one instanced draw per
/// `Scene::batches()` entry; paths and surfaces would add more, but no fixture draws
/// them.
///
/// With `COMPARE_GPU_REDRAWS=N`, each scene is drawn N more times right after it, as
/// RUI's bench does. They run only in the GPU sample.
pub(crate) struct CountingRenderer {
    inner: Box<dyn PlatformHeadlessRenderer>,
    redraws: usize,
    workbench_case: Option<&'static str>,
    first_terminal: Option<u64>,
}

impl CountingRenderer {
    pub(crate) fn wrap(
        inner: Box<dyn PlatformHeadlessRenderer>,
        case: &'static str,
    ) -> Box<dyn PlatformHeadlessRenderer> {
        Box::new(Self {
            inner,
            redraws: crate::gpu_time::redraws(),
            workbench_case: case.starts_with("workbench_").then_some(case),
            first_terminal: None,
        })
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
        self.inner.render_scene_to_image(scene, size)
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        size: gpui::Size<DevicePixels>,
    ) -> anyhow::Result<()> {
        if let (Some(case), Some(frame)) = (self.workbench_case, crate::gpu_time::sample_frame()) {
            let last = crate::gpu_time::WARMUP_FRAMES + crate::gpu_time::sample_count() - 1;
            if frame == 0 {
                self.first_terminal = Some(crate::workbench::inspect_scene(case, scene, size));
            } else if frame == last {
                let initial = self
                    .first_terminal
                    .expect("initial terminal scene was submitted");
                let terminal = crate::workbench::inspect_scene(case, scene, size);
                assert_ne!(initial, terminal, "terminal glyphs advance");
            }
        }
        Self::count(scene);
        self.inner.render_scene(scene, size)?;
        if let Some(frame) = crate::gpu_time::sample_frame().filter(|_| self.redraws > 0) {
            crate::gpu_time::set_frame(crate::gpu_time::redraw_tag(frame));
            for _ in 0..self.redraws {
                self.inner.render_scene(scene, size)?;
            }
            crate::gpu_time::set_frame(frame);
        }
        Ok(())
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
