//! Small paths drawn again on a later frame, kept rasterized in the glyph atlas.

use super::path::{HashKeyed, PathGeometry};
use crate::renderer::text::AtlasSlot;

/// A hash of `edges`' exact coordinates, in two lanes, one point of each edge
/// each, so the multiplies of a long path overlap.
pub(super) fn hash_edges(edges: &[[f32; 4]]) -> u64 {
    let word = |x: f32, y: f32| (u64::from(x.to_bits()) << 32) | u64::from(y.to_bits());
    let (mut a, mut b) = (0xcbf2_9ce4_8422_2325u64, 0x9e37_79b9_7f4a_7c15u64);
    for &[ax, ay, bx, by] in edges {
        a = (a ^ word(ax, ay))
            .wrapping_mul(0x0000_0100_0000_01b3)
            .rotate_left(29);
        b = (b ^ word(bx, by))
            .wrapping_mul(0xff51_afd7_ed55_8ccd)
            .rotate_left(31);
    }
    (a ^ b.rotate_left(32)).wrapping_mul(0xc4ce_b9fe_1a85_ec53)
}

/// Largest path, in device pixels on either side, kept in the atlas.
const MAX_SPRITE_SIZE: f32 = 128.0;

/// Paths seen without a slot are forgotten after this many frames.
const UNPLACED_FRAMES: u64 = 2;

/// Where a path sits on the device pixel grid when drawn from the atlas: its
/// whole-pixel origin and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SpriteCell {
    pub origin: [f32; 2],
    pub width: u32,
    pub height: u32,
}

struct SpriteEntry {
    /// Edges relative to the cell origin.
    edges: Vec<[f32; 4]>,
    /// The slot and the generation of its page when the path was placed.
    slot: Option<(AtlasSlot, u64)>,
    last_seen: u64,
}

/// Small paths drawn again on a later frame, rasterized once into the glyph
/// atlas. A path's shape relative to its pixel cell is the key, so the same
/// icon at any whole-pixel position shares one entry. Paths seen only once, or
/// at a new fractional offset every frame, stay analytic.
#[derive(Default)]
pub(super) struct PathSprites {
    frame: u64,
    epoch: u64,
    entries: HashKeyed<Vec<SpriteEntry>>,
    /// Keys of shapes seen without an entry, and the frame they were last seen:
    /// a shape is kept only once it comes back.
    seen: HashKeyed<u64>,
    relative: Vec<[f32; 4]>,
}

/// What to draw a path with.
pub(super) enum SpriteLookup {
    /// Too large, or not seen on an earlier frame: draw tiles.
    Analytic,
    /// In the atlas already, placed in this generation of its page, under this
    /// key: if the page was emptied since, rasterize it and `place` it again.
    Placed(AtlasSlot, u64, SpriteCell, u64),
    /// Seen on an earlier frame: rasterize it and `place` the result under
    /// this key.
    Rasterize(SpriteCell, u64),
}

impl PathSprites {
    /// Starts a frame; slots from before an atlas reset at `epoch` are dropped.
    pub(super) fn begin_frame(&mut self, epoch: u64) {
        self.frame += 1;
        self.sync_epoch(epoch);
        let frame = self.frame;
        self.entries.retain(|_, entries| {
            entries
                .retain(|entry| entry.slot.is_some() || entry.last_seen + UNPLACED_FRAMES >= frame);
            !entries.is_empty()
        });
        self.seen
            .retain(|_, last_seen| *last_seen + UNPLACED_FRAMES >= frame);
    }

    /// Forgets every slot when the atlas they were placed in was emptied.
    pub(super) fn sync_epoch(&mut self, epoch: u64) {
        if epoch != self.epoch {
            self.entries.clear();
            self.seen.clear();
            self.epoch = epoch;
        }
    }

    pub(super) fn lookup(&mut self, geometry: &PathGeometry) -> SpriteLookup {
        let [x0, y0, x1, y1] = geometry.bounds;
        let origin = [x0.floor(), y0.floor()];
        let (width, height) = (x1.ceil() - origin[0], y1.ceil() - origin[1]);
        if width > MAX_SPRITE_SIZE || height > MAX_SPRITE_SIZE {
            return SpriteLookup::Analytic;
        }
        let cell = SpriteCell {
            origin,
            width: width as u32,
            height: height as u32,
        };
        self.relative.clear();
        self.relative
            .extend(geometry.edges.iter().map(|&[ax, ay, bx, by]| {
                [
                    ax - origin[0],
                    ay - origin[1],
                    bx - origin[0],
                    by - origin[1],
                ]
            }));
        let hash = hash_edges(&self.relative);
        let frame = self.frame;
        let relative = &self.relative;
        if let Some(entry) = self
            .entries
            .get_mut(&hash)
            .and_then(|entries| entries.iter_mut().find(|entry| entry.edges == *relative))
        {
            entry.last_seen = frame;
            return match entry.slot {
                Some((slot, generation)) => SpriteLookup::Placed(slot, generation, cell, hash),
                None => SpriteLookup::Rasterize(cell, hash),
            };
        }
        match self.seen.insert(hash, frame) {
            // Back from an earlier frame: keep it.
            Some(last_seen) if last_seen < frame => {
                self.entries.entry(hash).or_default().push(SpriteEntry {
                    edges: relative.clone(),
                    slot: None,
                    last_seen: frame,
                });
                SpriteLookup::Rasterize(cell, hash)
            }
            _ => SpriteLookup::Analytic,
        }
    }

    /// How many paths have a slot.
    #[cfg(test)]
    pub(crate) fn placed(&self) -> usize {
        self.entries
            .values()
            .flatten()
            .filter(|entry| entry.slot.is_some())
            .count()
    }

    /// Records the slot of the path `lookup` last asked to rasterize as `key`.
    pub(super) fn place(&mut self, key: u64, slot: AtlasSlot, generation: u64) {
        let relative = &self.relative;
        if let Some(entry) = self
            .entries
            .get_mut(&key)
            .and_then(|entries| entries.iter_mut().find(|entry| entry.edges == *relative))
        {
            entry.slot = Some((slot, generation));
        }
    }
}
