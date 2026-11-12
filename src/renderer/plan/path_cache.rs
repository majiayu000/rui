//! Drawing paths, and paths kept by the identity of their vertices.
//!
//! A [`Primitive::Path`](crate::renderer::Primitive::Path) painted again with
//! the same `Arc` of vertices, offset, stroke width and scale has the same
//! device edges, so what planning worked out for it is kept: the edges, and the
//! atlas sprite or the tiles it is drawn with. A path is kept once it comes back
//! on a later frame; from then on a frame finds it by its key and only places
//! what was kept. Paths whose vertices are new each frame are drawn as before,
//! with the caches keyed by their edges (`PathSprites`, `PathTiles`).

use super::build::bounds_rect;
use super::frame::{ItemKind, Pages};
use super::path::{GpuPathTile, HashKeyed, PathGeometry, TileScratch};
use super::sprites::{SpriteCell, SpriteLookup};
use super::*;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Point};
use crate::renderer::Reach;
use crate::renderer::primitives::{PathVertex, path_reach};
use crate::renderer::text::AtlasSlot;
use std::sync::{Arc, Weak};

/// Kept paths unused for this many frames are dropped.
const UNUSED_FRAMES: u64 = 2;

/// What a kept path is found by: its vertex storage and how it is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PathKey {
    vertices: usize,
    offset: [u32; 2],
    stroke_width: Option<u32>,
    scale_factor: u32,
}

impl PathKey {
    fn new(
        vertices: &Arc<[PathVertex]>,
        offset: Point,
        stroke_width: Option<f32>,
        scale_factor: f32,
    ) -> Self {
        Self {
            vertices: Arc::as_ptr(vertices) as *const PathVertex as usize,
            offset: [offset.x.to_bits(), offset.y.to_bits()],
            stroke_width: stroke_width.map(f32::to_bits),
            scale_factor: scale_factor.to_bits(),
        }
    }

    fn hash(&self) -> u64 {
        let mut hash = (self.vertices as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for word in [
            u64::from(self.offset[0]) << 32 | u64::from(self.offset[1]),
            u64::from(self.stroke_width.unwrap_or(u32::MAX)) << 32 | u64::from(self.scale_factor),
        ] {
            hash = (hash ^ word)
                .wrapping_mul(0xff51_afd7_ed55_8ccd)
                .rotate_left(29);
        }
        hash
    }
}

/// Tiles of a kept path for one visible rectangle, edges listed from 0.
struct KeptTiles {
    visible: [f32; 4],
    tiles: Vec<GpuPathTile>,
    listed: Vec<[f32; 4]>,
    covered: Option<[f32; 4]>,
}

/// How a kept path is drawn from the atlas.
enum KeptSprite {
    /// Not placed yet: ask again next frame.
    Unknown,
    /// Too large for the atlas: always tiles.
    Never,
    /// In this slot of a page of this generation.
    Placed(AtlasSlot, u64, SpriteCell),
}

struct KeptPath {
    key: PathKey,
    /// Holds the storage the key points at, so no other path can take its address.
    _vertices: Arc<[PathVertex]>,
    geometry: PathGeometry,
    sprite: KeptSprite,
    tiles: Option<KeptTiles>,
    last_used: u64,
}

/// A path drawn once: its key, its storage, and the frame it was seen.
type SeenPath = (PathKey, Weak<[PathVertex]>, u64);

/// Paths kept by the identity of their vertices.
#[derive(Default)]
pub(super) struct PathCache {
    frame: u64,
    kept: HashKeyed<Vec<KeptPath>>,
    /// Paths drawn once, by key, with their storage, which keeps its address
    /// from being reused, and the frame they were seen.
    seen: HashKeyed<Vec<SeenPath>>,
    scratch: TileScratch,
}

impl PathCache {
    pub(super) fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.kept.retain(|_, paths| {
            paths.retain(|path| path.last_used + UNUSED_FRAMES >= frame);
            !paths.is_empty()
        });
        self.seen.retain(|_, paths| {
            paths.retain(|(_, _, last_seen)| *last_seen + UNUSED_FRAMES >= frame);
            !paths.is_empty()
        });
    }
}

/// The path primitive's fields.
pub(super) struct PathPrimitive<'a> {
    pub vertices: &'a Arc<[PathVertex]>,
    pub offset: Point,
    pub color: Rgba,
    pub stroke_width: Option<f32>,
}

impl<T: Clone> FramePlanner<T> {
    /// Draws `path` within `visible`, logical pixels, from what was kept for it
    /// if it was drawn before with the same vertices.
    // Out of line, so the loop over every primitive stays small.
    #[inline(never)]
    pub(super) fn build_path_primitive(
        &mut self,
        frame: &mut FrameData<T>,
        path: PathPrimitive<'_>,
        visible: Option<Bounds>,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        let key = PathKey::new(path.vertices, path.offset, path.stroke_width, scale_factor);
        let hash = key.hash();
        let cache = &mut self.path_cache;
        let found = cache
            .kept
            .get(&hash)
            .and_then(|paths| paths.iter().position(|kept| kept.key == key));
        let index = match found {
            Some(index) => index,
            None => {
                let reach = path_reach(path.vertices, path.offset, path.stroke_width);
                let seen = match (reach, visible) {
                    (Reach::Within(reach), Some(visible)) => reach.intersects(&visible),
                    (Reach::Nothing, _) | (_, None) => false,
                    (Reach::Unbounded, Some(_)) => true,
                };
                if !seen {
                    return Ok(());
                }
                let frame_number = cache.frame;
                let seen = cache.seen.entry(hash).or_default();
                let returning = seen.iter().position(|(seen_key, storage, _)| {
                    *seen_key == key
                        && storage
                            .upgrade()
                            .is_some_and(|storage| Arc::ptr_eq(&storage, path.vertices))
                });
                let returning = match returning {
                    Some(at) => {
                        let (_, _, last_seen) = seen.swap_remove(at);
                        last_seen < frame_number
                    }
                    None => false,
                };
                if !returning {
                    seen.push((key, Arc::downgrade(path.vertices), frame_number));
                    let mut geometry = std::mem::take(&mut frame.path);
                    let built = geometry
                        .set(
                            path.vertices,
                            [path.offset.x, path.offset.y],
                            path.stroke_width,
                            scale_factor,
                        )
                        .and_then(|()| {
                            self.build_path(frame, &geometry, path.color, clip, scale_factor)
                        });
                    frame.path = geometry;
                    return built;
                }
                let mut geometry = PathGeometry::default();
                geometry.set(
                    path.vertices,
                    [path.offset.x, path.offset.y],
                    path.stroke_width,
                    scale_factor,
                )?;
                let paths = cache.kept.entry(hash).or_default();
                paths.push(KeptPath {
                    key,
                    _vertices: Arc::clone(path.vertices),
                    geometry,
                    sprite: KeptSprite::Unknown,
                    tiles: None,
                    last_used: frame_number,
                });
                paths.len() - 1
            }
        };
        self.draw_kept(frame, hash, index, path.color, visible, clip, scale_factor)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_kept(
        &mut self,
        frame: &mut FrameData<T>,
        hash: u64,
        index: usize,
        color: Rgba,
        visible: Option<Bounds>,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        let cache = &mut self.path_cache;
        let kept = &mut cache.kept.get_mut(&hash).expect("kept path")[index];
        kept.last_used = cache.frame;
        let Some(visible) = visible else {
            return Ok(());
        };
        let [x0, y0, x1, y1] = kept.geometry.bounds.map(|value| value / scale_factor);
        if x1 < visible.x()
            || y1 < visible.y()
            || x0 > visible.x() + visible.width()
            || y0 > visible.y() + visible.height()
        {
            return Ok(());
        }
        if let KeptSprite::Placed(slot, generation, cell) = kept.sprite {
            if self.text_cache.use_sprite(slot, generation) {
                return self.push_path_sprite(frame, slot, cell, color, clip, scale_factor);
            }
            kept.sprite = KeptSprite::Unknown;
        }
        if matches!(kept.sprite, KeptSprite::Unknown) {
            if !self.path_sprites.fits(&kept.geometry) {
                kept.sprite = KeptSprite::Never;
            } else {
                let geometry = std::mem::take(&mut kept.geometry);
                let placed = self.place_path_sprite(&geometry);
                let kept = &mut self.path_cache.kept.get_mut(&hash).expect("kept path")[index];
                kept.geometry = geometry;
                if let Some((slot, generation, cell)) = placed? {
                    kept.sprite = KeptSprite::Placed(slot, generation, cell);
                    return self.push_path_sprite(frame, slot, cell, color, clip, scale_factor);
                }
            }
        }

        let cache = &mut self.path_cache;
        let kept = &mut cache.kept.get_mut(&hash).expect("kept path")[index];
        let visible_edges = frame.clip_edges[clip as usize];
        if kept
            .tiles
            .as_ref()
            .is_none_or(|tiles| tiles.visible != visible_edges)
        {
            let (mut tiles, mut listed) = (Vec::new(), Vec::new());
            let covered = kept.geometry.push_tiles(
                color,
                scale_factor,
                visible_edges,
                &mut cache.scratch,
                &mut tiles,
                &mut listed,
            );
            kept.tiles = Some(KeptTiles {
                visible: visible_edges,
                tiles,
                listed,
                covered,
            });
        }
        let tiles = kept.tiles.as_ref().expect("tiled above");
        let Some(rect) = tiles.covered else {
            return Ok(());
        };
        let start = frame.path_tiles.len() as u32;
        let base = frame.path_edges.len() as u32;
        frame.path_edges.extend_from_slice(&tiles.listed);
        let color = color.to_array();
        frame
            .path_tiles
            .extend(tiles.tiles.iter().map(|tile| GpuPathTile {
                color,
                edges: [tile.edges[0] + base, tile.edges[1]],
                ..*tile
            }));
        let kind = ItemKind::Sprites {
            kind: KIND_PATH,
            start,
            count: frame.path_tiles.len() as u32 - start,
            pages: Pages::default(),
            simple: false,
        };
        frame.push_item(kind, clip, rect, scale_factor);
        Ok(())
    }

    /// The atlas slot `path` is drawn from, placing it there if it was seen on
    /// an earlier frame; `None` to draw it as tiles.
    fn place_path_sprite(
        &mut self,
        path: &PathGeometry,
    ) -> Result<Option<(AtlasSlot, u64, SpriteCell)>, RendererError> {
        // A font loaded earlier in this frame may have emptied the atlas.
        self.path_sprites.sync_epoch(self.text_cache.epoch());
        let (cell, key) = match self.path_sprites.lookup(path) {
            SpriteLookup::Analytic => return Ok(None),
            SpriteLookup::Placed(slot, generation, cell, key) => {
                if self.text_cache.use_sprite(slot, generation) {
                    return Ok(Some((slot, generation, cell)));
                }
                // Its atlas page was emptied: place it again.
                (cell, key)
            }
            SpriteLookup::Rasterize(cell, key) => (cell, key),
        };
        let pixels = path.rasterize(cell.origin, cell.width, cell.height);
        match self
            .text_cache
            .place_sprite(cell.width, cell.height, pixels)
        {
            Ok((slot, generation)) => {
                self.path_sprites.place(key, slot, generation);
                Ok(Some((slot, generation, cell)))
            }
            // Without atlas room the path is still drawn, as tiles.
            Err(RendererResourceError::ResourcePressure { .. }) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn push_path_sprite(
        &mut self,
        frame: &mut FrameData<T>,
        slot: AtlasSlot,
        cell: SpriteCell,
        color: Rgba,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        let (page_width, page_height) =
            self.text_cache
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
            simple: false,
        };
        frame.glyphs.push(sprite);
        frame.push_item(kind, clip, bounds_rect(sprite.bounds), scale_factor);
        Ok(())
    }

    /// Draws `path` from the atlas when it is a small shape drawn before, and as
    /// analytic tiles otherwise.
    pub(super) fn build_path(
        &mut self,
        frame: &mut FrameData<T>,
        path: &PathGeometry,
        color: Rgba,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        if let Some((slot, _, cell)) = self.place_path_sprite(path)? {
            return self.push_path_sprite(frame, slot, cell, color, clip, scale_factor);
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
                simple: false,
            };
            frame.push_item(kind, clip, rect, scale_factor);
        }
        Ok(())
    }
}
