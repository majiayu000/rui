//! Frame planning shared by every GPU backend (ADR 0002).
//!
//! A [`FramePlanner`] turns a [`Scene`] into a [`FrameData`]: instance data in
//! the layouts the shaders read, clip rectangles, and a plan of draws in an
//! order that paints what scene order paints. A backend then only uploads the
//! data and atlas pages, and encodes the planned draws.

mod build;
mod frame;
mod instances;
mod order;
mod path;
mod retained;
mod simple;

pub(crate) use frame::{
    Draw, KIND_COLOR_GLYPH, KIND_GLYPH, KIND_PATH, KIND_PATH_SPRITE, KIND_QUAD, KIND_SHADOW,
};
pub(crate) use instances::{GpuGlyph, GpuImage};
pub(crate) use path::GpuPathTile;
pub(crate) use retained::{RetainedFrame, ScissorRect};

use crate::core::geometry::Size;
use crate::renderer::primitives::{GpuQuad, GpuShadow};
use crate::renderer::text::TextGlyphCache;
use crate::renderer::{
    RendererError, RendererImageCache, RendererResourceCache, RendererResourceError,
    RendererResourceKind, RendererResourceLimits, RendererResourceStats,
};
use frame::FramePlan;
use path::{PathGeometry, PathSprites, PathTiles};
use std::collections::HashMap;

/// The clear color, linear: 0.1 in sRGB.
pub(crate) fn clear_linear() -> f64 {
    let c: f64 = 0.1;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// What planning needs from a backend: a texture of an image's pixels.
pub(crate) trait TextureUpload {
    type Texture: Clone;

    /// A texture of `size` holding `pixels`, RGBA8 rows in sRGB.
    fn upload(&self, size: Size, pixels: &[u8]) -> Self::Texture;
}

/// Image textures a backend has uploaded, within the renderer's limits.
pub(crate) struct ImageTextures<T> {
    textures: HashMap<u32, T>,
    resources: RendererResourceCache<u32>,
}

impl<T: Clone> ImageTextures<T> {
    fn new(limits: &RendererResourceLimits) -> Self {
        Self {
            textures: HashMap::new(),
            resources: RendererResourceCache::new(
                RendererResourceKind::Texture,
                limits.texture_max_entries,
                limits.texture_max_bytes,
            ),
        }
    }

    fn get(&self, id: u32) -> Option<&T> {
        self.textures.get(&id)
    }

    /// The texture of image `id`, uploading `pixels` if it has none.
    fn ensure(
        &mut self,
        id: u32,
        size: Size,
        pixels: &[u8],
        upload: &impl TextureUpload<Texture = T>,
    ) -> Result<T, RendererError> {
        let allocation = self.resources.resolve(id, pixels.len())?;
        for evicted_id in allocation.evicted {
            self.textures.remove(&evicted_id);
        }
        Ok(self
            .textures
            .entry(id)
            .or_insert_with(|| upload.upload(size, pixels))
            .clone())
    }

    pub(crate) fn stats(&self) -> RendererResourceStats {
        self.resources.stats()
    }
}

pub(crate) struct ImageDraw<T> {
    pub texture: T,
    pub instance: GpuImage,
    pub clip: u32,
}

/// A frame's instance data and planned draws, kept to reuse their storage.
pub(crate) struct FrameData<T> {
    pub plan: FramePlan,
    pub quads: Vec<GpuQuad>,
    pub shadows: Vec<GpuShadow>,
    pub glyphs: Vec<GpuGlyph>,
    pub images: Vec<ImageDraw<T>>,
    pub path_tiles: Vec<GpuPathTile>,
    /// `[x0, y0, x1, y1]` in device pixels, the edges each path tile reads.
    pub path_edges: Vec<[f32; 4]>,
    /// The path being built, kept to reuse its storage.
    path: PathGeometry,
    /// Clip rectangles in device pixels, as scissor rectangles and as the
    /// `x0, y0, x1, y1` the sprite shader tests fragments against. Clip 0 is
    /// the whole frame, or the damage of a retained one.
    pub scissors: Vec<ScissorRect>,
    pub clip_edges: Vec<[f32; 4]>,
    /// Per clip, the flipped device pixel bounds a sprite lies within when none
    /// of its pixels needs the clip test: see `simple::clip_bounds`.
    pub(super) clip_bounds: Vec<Option<[f32; 4]>>,
    /// The device pixels a retained frame draws again; every clip is cut to it.
    pub damage: Option<ScissorRect>,
}

impl<T> Default for FrameData<T> {
    fn default() -> Self {
        Self {
            plan: FramePlan::default(),
            quads: Vec::new(),
            shadows: Vec::new(),
            glyphs: Vec::new(),
            images: Vec::new(),
            path_tiles: Vec::new(),
            path_edges: Vec::new(),
            path: PathGeometry::default(),
            scissors: Vec::new(),
            clip_edges: Vec::new(),
            clip_bounds: Vec::new(),
            damage: None,
        }
    }
}

/// The CPU side of a renderer: glyph, image and path caches, and the image
/// textures a backend uploaded for them.
pub(crate) struct FramePlanner<T> {
    pub text_cache: TextGlyphCache,
    pub image_cache: RendererImageCache,
    path_sprites: PathSprites,
    path_tiles: PathTiles,
    pub textures: ImageTextures<T>,
}

impl<T: Clone> FramePlanner<T> {
    pub(crate) fn new(limits: &RendererResourceLimits) -> Self {
        Self {
            text_cache: TextGlyphCache::with_limits(
                limits.glyph_max_entries,
                limits.glyph_max_bytes,
            ),
            image_cache: RendererImageCache::with_limits(
                limits.image_max_entries,
                limits.image_max_bytes,
            ),
            path_sprites: PathSprites::default(),
            path_tiles: PathTiles::default(),
            textures: ImageTextures::new(limits),
        }
    }

    /// Starts a frame. `oldest_in_flight` is the text cache frame of the oldest
    /// frame the GPU has not finished: atlas slots only finished frames drew
    /// from may be reused.
    pub(crate) fn begin_frame(
        &mut self,
        oldest_in_flight: Option<u64>,
    ) -> Result<(), RendererError> {
        self.textures.resources.begin_frame();
        self.image_cache.begin_frame();
        for handle in self.image_cache.release_dropped()? {
            let id = handle.id.as_u32();
            self.textures.textures.remove(&id);
            if self.textures.resources.contains(&id) {
                self.textures.resources.dispose(&id)?;
            }
        }
        self.text_cache.begin_frame();
        self.text_cache.set_completed_frame(
            oldest_in_flight.map_or(self.text_cache.frame() - 1, |frame| frame - 1),
        );
        self.path_sprites.begin_frame(self.text_cache.epoch());
        self.path_tiles.begin_frame();
        Ok(())
    }

    /// Plans `scene` into `frame` for a target `extent` device pixels across
    /// `viewport_size` logical ones. With `damage`, only what can touch it.
    pub(crate) fn build(
        &mut self,
        frame: &mut FrameData<T>,
        scene: &crate::renderer::Scene,
        extent: (u64, u64),
        viewport_size: Size,
        damage: Option<crate::core::geometry::Bounds>,
        upload: &impl TextureUpload<Texture = T>,
    ) -> Result<(), RendererError> {
        let scale_factor = extent.0 as f32 / viewport_size.width.max(1.0);
        let drawable_size = Size::new(extent.0 as f32, extent.1 as f32);
        let epoch = self.text_cache.epoch();
        let mut built = self.build_frame(
            frame,
            scene,
            viewport_size,
            scale_factor,
            drawable_size,
            damage,
            upload,
        );
        if built.is_ok() && self.text_cache.epoch() != epoch {
            // A font loaded during the frame emptied the glyph atlas, so glyphs
            // placed before then name slots that no longer hold them. The fonts
            // are loaded now; place everything again.
            let epoch = self.text_cache.epoch();
            built = self.build_frame(
                frame,
                scene,
                viewport_size,
                scale_factor,
                drawable_size,
                damage,
                upload,
            );
            if built.is_ok() && self.text_cache.epoch() != epoch {
                built = Err(RendererError::render_failed(
                    "the glyph atlas was emptied twice while building one frame",
                ));
            }
        }
        built
    }

    /// How many paths are drawn from the atlas.
    #[cfg(test)]
    pub(crate) fn placed_path_sprites(&self) -> usize {
        self.path_sprites.placed()
    }
}
