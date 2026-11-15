//! Tiles of analytic paths kept across frames by their exact edges, for paths
//! drawn again with new vertex storage that `path_cache` cannot find.

use super::path::{GpuPathTile, HashKeyed, PathGeometry, TileScratch};
use super::sprites::hash_edges;
use crate::core::color::Rgba;

/// Analytic paths unused for this many frames lose their kept tiles.
pub(super) const UNUSED_TILE_FRAMES: u64 = 2;

pub(super) struct TiledPath {
    /// The path's device edges and the visible rectangle it was tiled for.
    edges: Vec<[f32; 4]>,
    visible: [f32; 4],
    scale_factor: f32,
    tiles: Vec<GpuPathTile>,
    /// The edges the tiles list, from 0.
    listed: Vec<[f32; 4]>,
    covered: Option<[f32; 4]>,
    last_used: u64,
}

/// Tiles of analytic paths from earlier frames, so a path drawn again the same
/// way, as a still chart or a backdrop is, is copied rather than tiled again.
/// The key is the exact edges and visible rectangle; color is set on copy.
#[derive(Default)]
pub(super) struct PathTiles {
    frame: u64,
    pub(super) entries: HashKeyed<Vec<TiledPath>>,
    /// Keys of paths tiled without keeping, and the frame they were last seen:
    /// a path's tiles are kept only once it comes back.
    seen: HashKeyed<u64>,
    scratch: TileScratch,
}

impl PathTiles {
    pub(super) fn begin_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.entries.retain(|_, paths| {
            paths.retain(|path| path.last_used + UNUSED_TILE_FRAMES >= frame);
            !paths.is_empty()
        });
        self.seen
            .retain(|_, last_seen| *last_seen + UNUSED_TILE_FRAMES >= frame);
    }

    /// `PathGeometry::push_tiles`, from the kept tiles when there are some.
    pub(super) fn push(
        &mut self,
        geometry: &PathGeometry,
        color: Rgba,
        scale_factor: f32,
        visible: [f32; 4],
        tiles: &mut Vec<GpuPathTile>,
        edges: &mut Vec<[f32; 4]>,
    ) -> Option<[f32; 4]> {
        let hash = hash_edges(&geometry.edges)
            ^ hash_edges(&[visible]).rotate_left(1)
            ^ u64::from(scale_factor.to_bits());
        let frame = self.frame;
        let kept = self.entries.get_mut(&hash).and_then(|paths| {
            paths.iter().position(|path| {
                path.visible == visible
                    && path.scale_factor == scale_factor
                    && path.edges == geometry.edges
            })
        });
        let returning = || matches!(self.seen.get(&hash), Some(&last) if last < frame);
        if kept.is_none() && !returning() {
            // Seen first this frame: tile it and keep nothing yet.
            self.seen.insert(hash, frame);
            return geometry.push_tiles(
                color,
                scale_factor,
                visible,
                &mut self.scratch,
                tiles,
                edges,
            );
        }
        let paths = self.entries.entry(hash).or_default();
        let path = match kept {
            Some(index) => &mut paths[index],
            None => {
                let (mut kept_tiles, mut listed) = (Vec::new(), Vec::new());
                let covered = geometry.push_tiles(
                    color,
                    scale_factor,
                    visible,
                    &mut self.scratch,
                    &mut kept_tiles,
                    &mut listed,
                );
                paths.push(TiledPath {
                    edges: geometry.edges.clone(),
                    visible,
                    scale_factor,
                    tiles: kept_tiles,
                    listed,
                    covered,
                    last_used: frame,
                });
                paths.last_mut().expect("just pushed")
            }
        };
        path.last_used = frame;
        let base = edges.len() as u32;
        edges.extend_from_slice(&path.listed);
        let color = color.to_array();
        tiles.extend(path.tiles.iter().map(|tile| GpuPathTile {
            color,
            edges: [tile.edges[0] + base, tile.edges[1]],
            ..*tile
        }));
        path.covered
    }
}
