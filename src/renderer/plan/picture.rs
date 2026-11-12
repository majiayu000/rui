//! Pictures planned once and copied into later frames.
//!
//! A [`Picture`] drawn again at the same offset and scale, under the same
//! clip, in a frame of the same size, plans to the same instances and items,
//! so what it planned is kept and copied: its instances in one block per
//! buffer, its clips, and its items with their indices moved to where the
//! copies land. Items that run on in one draw are merged first, so a grid of
//! quads is one item. A picture is kept once it comes back on a later frame,
//! and only while the atlas pages it samples still hold what it placed there;
//! one with images, color glyphs or drawn into a retained frame's damage is
//! planned every time.

use super::build::Walk;
use super::frame::{Item, ItemKind};
use super::path::{GpuPathTile, HashKeyed};
use super::*;
use crate::core::geometry::Point;
use crate::renderer::Picture;
use crate::renderer::picture::PictureData;
use crate::renderer::primitives::{GpuQuad, GpuShadow};
use crate::renderer::text::{AtlasRect, AtlasSlot};
use std::sync::{Arc, Weak};

/// Kept pictures unused for this many frames are dropped.
const UNUSED_FRAMES: u64 = 2;

/// What a planned picture depends on besides its primitives.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PictureKey {
    picture: usize,
    offset: [u32; 2],
    scale_factor: u32,
    /// The logical clip it is drawn in, and the frame's sizes.
    visible: [u32; 4],
    viewport: [u32; 2],
    drawable: [u32; 2],
}

impl PictureKey {
    fn new(picture: &Picture, offset: Point, walk: &Walk) -> Self {
        let visible = walk.clip_stack.last().copied().unwrap_or(walk.viewport);
        Self {
            picture: Arc::as_ptr(picture.shared()) as usize,
            offset: [offset.x.to_bits(), offset.y.to_bits()],
            scale_factor: walk.scale_factor.to_bits(),
            visible: [visible.x(), visible.y(), visible.width(), visible.height()]
                .map(f32::to_bits),
            viewport: [walk.viewport.width(), walk.viewport.height()].map(f32::to_bits),
            drawable: [walk.drawable_size.width, walk.drawable_size.height].map(f32::to_bits),
        }
    }

    fn hash(&self) -> u64 {
        let mut hash = (self.picture as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let words = [
            self.offset[0],
            self.offset[1],
            self.scale_factor,
            self.visible[0],
            self.visible[1],
            self.visible[2],
            self.visible[3],
            self.viewport[0] ^ self.drawable[1],
        ];
        for pair in words.chunks(2) {
            let word = u64::from(pair[0]) << 32 | u64::from(pair[1]);
            hash = (hash ^ word)
                .wrapping_mul(0xff51_afd7_ed55_8ccd)
                .rotate_left(29);
        }
        hash
    }
}

/// What a picture planned, with its clips numbered from 1 after the clip it
/// was drawn in, which is 0, and its instances from 0 in each buffer.
struct PlannedPicture {
    key: PictureKey,
    /// Holds the storage the key points at, so no other picture takes its address.
    _picture: Picture,
    quads: Arc<[GpuQuad]>,
    shadows: Vec<GpuShadow>,
    glyphs: Vec<GpuGlyph>,
    path_tiles: Vec<GpuPathTile>,
    path_edges: Vec<[f32; 4]>,
    scissors: Vec<ScissorRect>,
    clip_edges: Vec<[f32; 4]>,
    clip_bounds: Vec<Option<[f32; 4]>>,
    items: Vec<Item>,
    /// The clip in effect after the picture.
    exit_clip: u32,
    /// The atlas pages its glyphs sample, the generation each was in, and the
    /// atlas epoch.
    pages: Vec<(usize, u64)>,
    epoch: u64,
    last_used: u64,
}

/// Pictures planned on earlier frames.
#[derive(Default)]
pub(super) struct PictureCache {
    frame: u64,
    kept: HashKeyed<Vec<PlannedPicture>>,
    /// Pictures planned once, by key, with their storage, which keeps its
    /// address from being reused, and the frame they were seen.
    seen: HashKeyed<Vec<(PictureKey, Weak<PictureData>, u64)>>,
}

impl PictureCache {
    pub(super) fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.kept.retain(|_, kept| {
            kept.retain(|picture| picture.last_used + UNUSED_FRAMES >= frame);
            !kept.is_empty()
        });
        self.seen.retain(|_, seen| {
            seen.retain(|(_, _, last_seen)| *last_seen + UNUSED_FRAMES >= frame);
            !seen.is_empty()
        });
    }

    /// Whether `key` was planned on an earlier frame, noting it as seen now.
    fn returning(&mut self, key: PictureKey, hash: u64, picture: &Picture) -> bool {
        let frame = self.frame;
        let seen = self.seen.entry(hash).or_default();
        let found = seen.iter().position(|(seen_key, storage, _)| {
            *seen_key == key
                && storage
                    .upgrade()
                    .is_some_and(|storage| Arc::ptr_eq(&storage, picture.shared()))
        });
        match found {
            Some(at) => {
                let last_seen = std::mem::replace(&mut seen[at].2, frame);
                last_seen < frame
            }
            None => {
                seen.push((key, Arc::downgrade(picture.shared()), frame));
                false
            }
        }
    }
}

/// Where each of a frame's buffers ended before a picture was planned.
#[derive(Clone, Copy)]
struct Marks {
    quads: usize,
    shadows: usize,
    glyphs: usize,
    path_tiles: usize,
    path_edges: usize,
    clips: usize,
    items: usize,
    images: usize,
}

impl Marks {
    fn of<T>(frame: &FrameData<T>) -> Self {
        Self {
            quads: frame.quads.len(),
            shadows: frame.shadows.len(),
            glyphs: frame.glyphs.len(),
            path_tiles: frame.path_tiles.len(),
            path_edges: frame.path_edges.len(),
            clips: frame.scissors.len(),
            items: frame.plan.items().len(),
            images: frame.images.len(),
        }
    }

    /// Where items of `kind` begin in their buffer.
    fn start_of(&self, kind: u32) -> usize {
        match kind {
            KIND_QUAD => self.quads,
            KIND_SHADOW => self.shadows,
            KIND_PATH => self.path_tiles,
            _ => self.glyphs,
        }
    }
}

impl<T: Clone> FramePlanner<T> {
    /// Plans `picture` drawn at `offset`, or copies what it planned before.
    pub(super) fn build_picture(
        &mut self,
        frame: &mut FrameData<T>,
        picture: &Picture,
        offset: Point,
        walk: &mut Walk,
        upload: &impl TextureUpload<Texture = T>,
    ) -> Result<(), RendererError> {
        let keeps = frame.damage.is_none();
        let key = PictureKey::new(picture, offset, walk);
        let hash = key.hash();
        if keeps && self.copy_kept_picture(frame, key, hash, walk) {
            return Ok(());
        }
        let marks = Marks::of(frame);
        let entry_clip = walk.clip;
        let epoch = self.text_cache.epoch();
        for primitive in picture.primitives() {
            if offset == Point::ZERO {
                self.build_inner_primitive(frame, primitive, walk, upload)?;
            } else {
                let mut primitive = primitive.clone();
                primitive.translate(offset.x, offset.y);
                self.build_inner_primitive(frame, &primitive, walk, upload)?;
            }
        }
        if keeps
            && self.text_cache.epoch() == epoch
            && self.picture_cache.returning(key, hash, picture)
            && let Some(planned) = self.planned(frame, &marks, entry_clip, walk.clip, key, picture)
        {
            self.picture_cache
                .kept
                .entry(hash)
                .or_default()
                .push(planned);
        }
        Ok(())
    }

    /// Copies what the picture under `key` planned on an earlier frame into
    /// `frame`; false when it has nothing kept that is still good.
    fn copy_kept_picture(
        &mut self,
        frame: &mut FrameData<T>,
        key: PictureKey,
        hash: u64,
        walk: &mut Walk,
    ) -> bool {
        let FramePlanner {
            text_cache,
            picture_cache,
            ..
        } = self;
        let Some(kept) = picture_cache.kept.get_mut(&hash) else {
            return false;
        };
        let Some(at) = kept.iter().position(|planned| planned.key == key) else {
            return false;
        };
        let planned = &mut kept[at];
        let live = planned.epoch == text_cache.epoch()
            && planned.pages.iter().all(|&(page, generation)| {
                let slot = AtlasSlot {
                    page,
                    rect: AtlasRect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                };
                text_cache.use_sprite(slot, generation)
            });
        if !live {
            kept.swap_remove(at);
            return false;
        }
        planned.last_used = picture_cache.frame;
        let marks = Marks::of(frame);
        let kept_quads = (!planned.quads.is_empty()).then(|| frame.keep_quads(&planned.quads));
        frame.shadows.extend_from_slice(&planned.shadows);
        frame.glyphs.extend_from_slice(&planned.glyphs);
        frame.path_edges.extend_from_slice(&planned.path_edges);
        let edges = marks.path_edges as u32;
        frame
            .path_tiles
            .extend(planned.path_tiles.iter().map(|tile| GpuPathTile {
                edges: [tile.edges[0] + edges, tile.edges[1]],
                ..*tile
            }));
        frame.scissors.extend_from_slice(&planned.scissors);
        frame.clip_edges.extend_from_slice(&planned.clip_edges);
        frame.clip_bounds.extend_from_slice(&planned.clip_bounds);
        let entry_clip = walk.clip;
        let clip = |local: u32| {
            if local == 0 {
                entry_clip
            } else {
                marks.clips as u32 + local - 1
            }
        };
        for item in &planned.items {
            let ItemKind::Sprites {
                kind,
                start,
                count,
                pages,
                simple,
            } = item.kind
            else {
                unreachable!("pictures with images are not kept");
            };
            let start = match (kind, kept_quads) {
                (KIND_QUAD, Some(kept)) => kept + start,
                _ => start + marks.start_of(kind) as u32,
            };
            let kind = ItemKind::Sprites {
                kind,
                start,
                count,
                pages,
                simple,
            };
            frame.plan.push(kind, clip(item.clip), item.rect);
        }
        walk.clip = clip(planned.exit_clip);
        true
    }

    /// What the primitives planned since `marks`, to keep; `None` when they
    /// cannot be copied into another frame.
    fn planned(
        &self,
        frame: &FrameData<T>,
        marks: &Marks,
        entry_clip: u32,
        exit_clip: u32,
        key: PictureKey,
        picture: &Picture,
    ) -> Option<PlannedPicture> {
        if frame.images.len() != marks.images {
            return None;
        }
        let local_clip = |clip: u32| {
            if clip == entry_clip {
                Some(0)
            } else {
                (clip as usize >= marks.clips).then(|| clip - marks.clips as u32 + 1)
            }
        };
        let mut items: Vec<Item> = Vec::new();
        let mut pages: Vec<(usize, u64)> = Vec::new();
        for item in &frame.plan.items()[marks.items..] {
            let ItemKind::Sprites {
                kind,
                start,
                count,
                pages: sampled,
                simple,
            } = item.kind
            else {
                return None;
            };
            if sampled.color.is_some() {
                return None;
            }
            if let Some(page) = sampled.coverage
                && !pages.iter().any(|&(kept, _)| kept == page as usize)
            {
                let generation = self.text_cache.atlas().page_generation(page as usize)?;
                pages.push((page as usize, generation));
            }
            let kind = ItemKind::Sprites {
                kind,
                start: start - marks.start_of(kind) as u32,
                count,
                pages: sampled,
                simple,
            };
            let item = Item {
                kind,
                clip: local_clip(item.clip)?,
                rect: item.rect,
            };
            match items.last_mut() {
                Some(last) if runs_on(last, &item) => merge(last, &item),
                _ => items.push(item),
            }
        }
        let edges = marks.path_edges as u32;
        Some(PlannedPicture {
            key,
            _picture: picture.clone(),
            quads: frame.quads[marks.quads..].into(),
            shadows: frame.shadows[marks.shadows..].to_vec(),
            glyphs: frame.glyphs[marks.glyphs..].to_vec(),
            path_tiles: frame.path_tiles[marks.path_tiles..]
                .iter()
                .map(|tile| GpuPathTile {
                    edges: [tile.edges[0] - edges, tile.edges[1]],
                    ..*tile
                })
                .collect(),
            path_edges: frame.path_edges[marks.path_edges..].to_vec(),
            scissors: frame.scissors[marks.clips..].to_vec(),
            clip_edges: frame.clip_edges[marks.clips..].to_vec(),
            clip_bounds: frame.clip_bounds[marks.clips..].to_vec(),
            items,
            exit_clip: local_clip(exit_clip)?,
            pages,
            epoch: self.text_cache.epoch(),
            last_used: self.picture_cache.frame,
        })
    }
}

/// Whether `next` continues `last`'s instances in the same draw: the same
/// kind, pages, pipeline and clip, from where `last` ends.
fn runs_on(last: &Item, next: &Item) -> bool {
    match (last.kind, next.kind) {
        (
            ItemKind::Sprites {
                kind,
                start,
                count,
                pages,
                simple,
            },
            ItemKind::Sprites {
                kind: next_kind,
                start: next_start,
                pages: next_pages,
                simple: next_simple,
                ..
            },
        ) => {
            kind == next_kind
                && start + count == next_start
                && pages == next_pages
                && simple == next_simple
                && last.clip == next.clip
                && last.rect.is_some() == next.rect.is_some()
        }
        _ => false,
    }
}

/// `last` taking in `next`'s instances; see [`runs_on`].
fn merge(last: &mut Item, next: &Item) {
    if let (
        ItemKind::Sprites { count, .. },
        ItemKind::Sprites {
            count: next_count, ..
        },
    ) = (&mut last.kind, next.kind)
    {
        *count += next_count;
    }
    last.rect = match (last.rect, next.rect) {
        (Some(a), Some(b)) => Some(super::build::union(a, b)),
        _ => None,
    };
}
