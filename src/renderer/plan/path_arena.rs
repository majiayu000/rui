//! Tiles of kept paths, kept in the frame's buffers from one frame to the next.
//!
//! A kept path drawn twice from the same tiles has them moved, with its edges,
//! to the start of the frame's `path_tiles` and `path_edges`, which
//! [`FrameData::clear`] leaves in place. Later frames draw it by index there,
//! copying nothing, as Vello appends a scene encoded once: only the sprite
//! references are written. A path drawn in another color, or into a picture
//! being planned, copies its tiles out as before. Tiles of paths no longer
//! kept stay until they outnumber the rest, and the arena is then packed.

use super::path::GpuPathTile;
use super::path_cache::{KeptTiles, PathCache, PathKey, StoredTiles};
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tells apart the arenas of different caches, so a cache never reads one it
/// did not fill.
static NEXT_ARENA: AtomicU64 = AtomicU64::new(1);

/// The kept prefix of a frame's tile and edge buffers.
#[derive(Clone, Copy, Default)]
pub(super) struct ArenaMark {
    pub id: u64,
    pub tiles: usize,
    pub edges: usize,
}

/// What a [`PathCache`] keeps in the arena.
#[derive(Default)]
pub(super) struct PathArena {
    id: u64,
    /// Tiles in the arena of paths that still draw from it, and of the others.
    live: usize,
    garbage: usize,
    /// Kept paths to move into the arena when the next frame starts.
    promote: Vec<(u64, PathKey)>,
}

impl PathArena {
    pub(super) fn promote(&mut self, hash: u64, key: PathKey) {
        self.promote.push((hash, key));
    }

    /// Notes that `tiles` are drawn from no longer.
    pub(super) fn forget(&mut self, tiles: Option<&KeptTiles>) {
        if let Some(KeptTiles {
            stored: StoredTiles::Arena { tile_count, .. },
            ..
        }) = tiles
        {
            self.live -= *tile_count as usize;
            self.garbage += *tile_count as usize;
        }
    }
}

impl PathCache {
    /// Fits the arena to `frame`, just cleared down to it: moves the paths
    /// waiting into it, and packs it when it is mostly garbage.
    pub(super) fn sync_arena<T>(&mut self, frame: &mut FrameData<T>) {
        if self.arena.id == 0 || frame.arena.id != self.arena.id {
            for tiles in self.kept.values_mut().flatten().map(|path| &mut path.tiles) {
                if matches!(
                    tiles,
                    Some(KeptTiles {
                        stored: StoredTiles::Arena { .. },
                        ..
                    })
                ) {
                    *tiles = None;
                }
            }
            frame.path_tiles.clear();
            frame.path_edges.clear();
            self.arena.id = NEXT_ARENA.fetch_add(1, Ordering::Relaxed);
            self.arena.live = 0;
            self.arena.garbage = 0;
        }
        if self.arena.garbage > self.arena.live {
            self.pack_arena(frame);
        }
        for (hash, key) in std::mem::take(&mut self.arena.promote) {
            let Some(path) = self
                .kept
                .get_mut(&hash)
                .and_then(|paths| paths.iter_mut().find(|path| path.key == key))
            else {
                continue;
            };
            let Some(kept) = &mut path.tiles else {
                continue;
            };
            let StoredTiles::Own { tiles, listed, .. } = &kept.stored else {
                continue;
            };
            let at = frame.path_tiles.len() as u32;
            let edges = frame.path_edges.len() as u32;
            frame.path_edges.extend_from_slice(listed);
            frame
                .path_tiles
                .extend(tiles.iter().map(|tile| GpuPathTile {
                    color: kept.color,
                    edges: [tile.edges[0] + edges, tile.edges[1]],
                    ..*tile
                }));
            self.arena.live += tiles.len();
            kept.stored = StoredTiles::Arena {
                tiles: at,
                tile_count: tiles.len() as u32,
                edges,
                edge_count: listed.len() as u32,
            };
        }
        frame.arena = ArenaMark {
            id: self.arena.id,
            tiles: frame.path_tiles.len(),
            edges: frame.path_edges.len(),
        };
    }

    /// Copies the arena's tiles still drawn from into a new one.
    fn pack_arena<T>(&mut self, frame: &mut FrameData<T>) {
        let (mut packed_tiles, mut packed_edges) = (Vec::new(), Vec::new());
        for kept in self
            .kept
            .values_mut()
            .flatten()
            .filter_map(|path| path.tiles.as_mut())
        {
            let StoredTiles::Arena {
                tiles,
                tile_count,
                edges,
                edge_count,
            } = &mut kept.stored
            else {
                continue;
            };
            let at = packed_tiles.len() as u32;
            let moved = packed_edges.len() as u32;
            packed_edges.extend_from_slice(
                &frame.path_edges[*edges as usize..(*edges + *edge_count) as usize],
            );
            packed_tiles.extend(
                frame.path_tiles[*tiles as usize..(*tiles + *tile_count) as usize]
                    .iter()
                    .map(|tile| GpuPathTile {
                        edges: [tile.edges[0] - *edges + moved, tile.edges[1]],
                        ..*tile
                    }),
            );
            *tiles = at;
            *edges = moved;
        }
        frame.path_tiles = packed_tiles;
        frame.path_edges = packed_edges;
        self.arena.garbage = 0;
    }
}
