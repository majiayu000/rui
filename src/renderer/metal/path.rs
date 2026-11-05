//! Paths drawn with analytic coverage, without an intermediate texture.
//!
//! A path's edges are kept in device pixels. A fill covers what its closed
//! polygon winds around (nonzero rule); a stroke covers each segment's
//! rectangle, `stroke_width` wide with butt ends, all wound the same way, so a
//! stroke is a polygon too and joints are not covered twice. Coverage is the
//! exact share of the pixel square inside: each edge adds the signed area of
//! the square to its left, which sums to the integral of the winding number
//! over the square, and that is clamped to one, as font rasterizers do.
//!
//! On the GPU a path is drawn as tiles of 16 by 16 device pixels. A tile lists
//! only the edges that cross it; the edges wholly to its right add the same to
//! every pixel of a row, so the tile carries their sum per row, its backdrop.
//! A pixel then tests a handful of edges however large or long the path is, and
//! runs of tiles inside the path with no edge become one instance. [`coverage`]
//! is the same function on the CPU; it rasterizes paths into the atlas and is
//! what the GPU output is checked against.

use crate::core::color::Rgba;
use crate::renderer::RendererError;
use crate::renderer::primitives::PathVertex;
use crate::renderer::text::AtlasSlot;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Tile side in device pixels; also the rows of a backdrop.
const TILE: usize = 16;

/// Largest path, in device pixels on either side and in edges, drawn as one
/// instance rather than tiles.
const SMALL_PATH: f32 = 64.0;
const SMALL_PATH_EDGES: usize = 32;

/// Maps keyed by a hash already computed.
type HashKeyed<V> = HashMap<u64, V, BuildHasherDefault<KeyHasher>>;

/// Passes a `u64` key through as its own hash.
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 << 8) | u64::from(byte);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

/// How far past an edge a pixel square can reach, in device pixels.
const MARGIN: f32 = 1.0;

/// A tile of a path, or a run of tiles, as the sprite shader reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct GpuPathTile {
    /// x, y, width, height in logical pixels.
    pub bounds: [f32; 4],
    pub color: [f32; 4],
    /// First edge and edge count in the frame's edge buffer.
    pub edges: [u32; 2],
    /// Device y of the tile's first row.
    pub top: f32,
    pub _padding: f32,
    /// Per row, the winding area the edges right of the tile add to each pixel.
    pub backdrop: [f32; TILE],
}

/// A path in device pixels: polygon edges, and the rectangle `[x0, y0, x1, y1]`
/// coverage can reach.
#[derive(Debug, Default)]
pub(super) struct PathGeometry {
    /// `[x0, y0, x1, y1]` per edge.
    pub edges: Vec<[f32; 4]>,
    pub bounds: [f32; 4],
}

impl PathGeometry {
    /// Replaces this geometry with `vertices` at `scale_factor` device pixels per
    /// logical pixel. Fails for paths that cannot cover anything.
    pub(super) fn set(
        &mut self,
        vertices: &[PathVertex],
        stroke_width: Option<f32>,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        self.edges.clear();
        let device = |vertex: &PathVertex| {
            [
                vertex.position[0] * scale_factor,
                vertex.position[1] * scale_factor,
            ]
        };
        match stroke_width {
            None => {
                if vertices.len() < 3 {
                    return Err(RendererError::render_failed(
                        "filled path primitive requires at least three vertices",
                    ));
                }
                for (index, vertex) in vertices.iter().enumerate() {
                    let [x0, y0] = device(vertex);
                    let [x1, y1] = device(&vertices[(index + 1) % vertices.len()]);
                    if x0 != x1 || y0 != y1 {
                        self.edges.push([x0, y0, x1, y1]);
                    }
                }
            }
            Some(width) => {
                if width <= 0.0 || !width.is_finite() {
                    return Err(RendererError::render_failed(
                        "stroked path primitive requires a positive finite stroke width",
                    ));
                }
                if vertices.len() < 2 {
                    return Err(RendererError::render_failed(
                        "stroked path primitive requires at least two vertices",
                    ));
                }
                let half_width = width * 0.5 * scale_factor;
                for pair in vertices.windows(2) {
                    let [x0, y0] = device(&pair[0]);
                    let [x1, y1] = device(&pair[1]);
                    let length = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
                    if length <= f32::EPSILON {
                        continue;
                    }
                    // The segment's rectangle, wound the same way for every
                    // direction: the normal turns with the segment.
                    let nx = -(y1 - y0) / length * half_width;
                    let ny = (x1 - x0) / length * half_width;
                    let corners = [
                        [x0 + nx, y0 + ny],
                        [x1 + nx, y1 + ny],
                        [x1 - nx, y1 - ny],
                        [x0 - nx, y0 - ny],
                    ];
                    for index in 0..4 {
                        let [ax, ay] = corners[index];
                        let [bx, by] = corners[(index + 1) % 4];
                        self.edges.push([ax, ay, bx, by]);
                    }
                }
                if self.edges.is_empty() {
                    return Err(RendererError::render_failed(
                        "stroked path primitive contains only degenerate segments",
                    ));
                }
            }
        }
        if self
            .edges
            .iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(RendererError::render_failed(
                "path primitive has a vertex that is not finite",
            ));
        }
        let margin = MARGIN;
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for &[x0, y0, x1, y1] in &self.edges {
            bounds = [
                bounds[0].min(x0.min(x1)),
                bounds[1].min(y0.min(y1)),
                bounds[2].max(x0.max(x1)),
                bounds[3].max(y0.max(y1)),
            ];
        }
        self.bounds = [
            bounds[0] - margin,
            bounds[1] - margin,
            bounds[2] + margin,
            bounds[3] + margin,
        ];
        Ok(())
    }

    /// Appends tiles covering this path within `visible`, device pixels, to
    /// `tiles`, and the edges each reads to `edges`. Returns the logical
    /// rectangle the tiles cover, or `None` when none is visible.
    pub(super) fn push_tiles(
        &self,
        color: Rgba,
        scale_factor: f32,
        visible: [f32; 4],
        scratch: &mut TileScratch,
        tiles: &mut Vec<GpuPathTile>,
        edges: &mut Vec<[f32; 4]>,
    ) -> Option<[f32; 4]> {
        let area = [
            self.bounds[0].max(visible[0]),
            self.bounds[1].max(visible[1]),
            self.bounds[2].min(visible[2]),
            self.bounds[3].min(visible[3]),
        ];
        if area[0] >= area[2] || area[1] >= area[3] {
            return None;
        }
        // A small path is one instance listing every edge: cheaper to build than
        // tiles, and its pixels test few edges anyway.
        if area[2] - area[0] <= SMALL_PATH
            && area[3] - area[1] <= SMALL_PATH
            && self.edges.len() <= SMALL_PATH_EDGES
        {
            let first = edges.len() as u32;
            edges.extend_from_slice(&self.edges);
            let logical = area.map(|value| value / scale_factor);
            tiles.push(GpuPathTile {
                bounds: [
                    logical[0],
                    logical[1],
                    logical[2] - logical[0],
                    logical[3] - logical[1],
                ],
                color: color.to_array(),
                edges: [first, self.edges.len() as u32],
                top: area[1],
                _padding: 0.0,
                backdrop: [0.0; TILE],
            });
            return Some(logical);
        }
        let band = Band {
            color: color.to_array(),
            scale_factor,
            area,
        };
        let first = tiles.len();
        let size = TILE as f32;
        // Edges by top, so each band takes up only the edges reaching it.
        scratch.by_top.clear();
        scratch
            .by_top
            .extend((0..self.edges.len() as u32).filter(|&index| {
                let [_, ay, _, by] = self.edges[index as usize];
                ay != by
            }));
        let path_edges = &self.edges;
        let top_of = |index: u32| {
            let [_, ay, _, by] = path_edges[index as usize];
            ay.min(by)
        };
        scratch
            .by_top
            .sort_unstable_by(|&a, &b| top_of(a).total_cmp(&top_of(b)));
        scratch.reaching.clear();
        let mut next = 0;
        let mut top = (area[1] / size).floor() * size;
        while top < area[3] {
            let bottom = top + size;
            while next < scratch.by_top.len() && top_of(scratch.by_top[next]) < bottom {
                scratch.reaching.push(scratch.by_top[next]);
                next += 1;
            }
            scratch.reaching.retain(|&index| {
                let [_, ay, _, by] = path_edges[index as usize];
                ay.max(by) > top
            });
            self.push_band(&band, top, scratch, tiles, edges);
            top = bottom;
        }
        tiles[first..]
            .iter()
            .map(|tile| {
                let [x, y, width, height] = tile.bounds;
                [x, y, x + width, y + height]
            })
            .reduce(|a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    }

    /// The tiles of the band of rows from device y `top`, right to left.
    fn push_band(
        &self,
        band: &Band,
        top: f32,
        scratch: &mut TileScratch,
        tiles: &mut Vec<GpuPathTile>,
        edges: &mut Vec<[f32; 4]>,
    ) {
        let size = TILE as f32;
        let bottom = top + size;
        let column = |x: f32| (x / size).floor() as i64;
        // Tiles are cut to the visible area; columns are whole tiles.
        let (first, last) = (column(band.area[0]), column(band.area[2] - 1e-3));
        let mut backdrop = Backdrop::default();

        // Each edge crossing the band's rows enters the sweep at the column of
        // its right end and is wholly right of every column left of its left
        // end. Edges wholly right of all columns start in the backdrop.
        scratch.band.clear();
        let mut leftmost = i64::MAX;
        for &index in &scratch.reaching {
            let edge = self.edges[index as usize];
            let [ax, ay, bx, by] = edge;
            let (a, b) = ((top - ay) / (by - ay), (bottom - ay) / (by - ay));
            let (t0, t1) = (a.min(b).max(0.0), a.max(b).min(1.0));
            let (xa, xb) = (ax + (bx - ax) * t0, ax + (bx - ax) * t1);
            // An edge adds to a pixel only if the pixel's left side is left of
            // it, so an edge on a column's left side crosses no pixel there.
            let left = column(xa.min(xb));
            let right = (xa.max(xb) / size).ceil() as i64 - 1;
            if right < first && left < first {
                continue;
            }
            if left > last {
                backdrop.add(edge, top);
                continue;
            }
            leftmost = leftmost.min(left);
            scratch.band.push(BandEdge {
                edge,
                // An edge crossing no column only leaves, into the backdrop.
                enter: if right >= left {
                    right.min(last)
                } else {
                    i64::MIN
                },
                leave: left,
            });
        }
        if scratch.band.is_empty() {
            return;
        }
        // Nothing left of every edge is inside.
        let first = first.max(leftmost);
        let columns = (last - first + 1) as usize;
        bucket(
            &scratch.band,
            columns,
            |edge| edge.enter - first,
            &mut scratch.enter,
        );
        bucket(
            &scratch.band,
            columns,
            |edge| edge.leave - first,
            &mut scratch.leave,
        );

        let tile = |x0: f32, x1: f32, edges: [u32; 2], backdrop: [f32; TILE]| {
            let (y0, y1) = (top.max(band.area[1]), bottom.min(band.area[3]));
            let scale = band.scale_factor;
            GpuPathTile {
                bounds: [x0 / scale, y0 / scale, (x1 - x0) / scale, (y1 - y0) / scale],
                color: band.color,
                edges,
                top,
                _padding: 0.0,
                backdrop,
            }
        };
        scratch.active.clear();
        // A run of tiles crossed by no edge, all with one backdrop: `x0..x1`.
        let mut solid: Option<(f32, f32, [f32; TILE])> = None;
        // Whether the column to the right was crossed by nothing.
        let mut uncrossed = false;
        for offset in (0..columns).rev() {
            let x = (first + offset as i64) as f32 * size;
            let (x0, x1) = (x.max(band.area[0]), (x + size).min(band.area[2]));
            let entering = scratch.enter.column(offset);
            if uncrossed && entering.is_empty() && !backdrop.stale {
                // Crossed by nothing, with the backdrop of the column to its
                // right: the same as that column.
                if let Some(run) = solid.as_mut() {
                    run.0 = x0;
                }
            } else {
                scratch.active.extend_from_slice(entering);
                uncrossed = scratch.active.is_empty();
                self.push_column(
                    [x0, x1],
                    &scratch.active,
                    &scratch.band,
                    backdrop.rows(),
                    &mut solid,
                    &tile,
                    tiles,
                    edges,
                );
            }
            // Edges starting in this column are wholly right of the next one.
            let leaving = scratch.leave.column(offset);
            if !leaving.is_empty() {
                for &index in leaving {
                    backdrop.add(scratch.band[index as usize].edge, top);
                }
                let band_edges = &scratch.band;
                scratch
                    .active
                    .retain(|&index| band_edges[index as usize].leave != first + offset as i64);
            }
        }
        if let Some((run_x0, run_x1, rows)) = solid {
            tiles.push(tile(run_x0, run_x1, [0, 0], rows));
        }
    }

    /// The tile of one column, crossed by the `active` band edges, or the
    /// extension of the open run of uncrossed tiles inside.
    #[allow(clippy::too_many_arguments)]
    fn push_column(
        &self,
        [x0, x1]: [f32; 2],
        active: &[u32],
        band: &[BandEdge],
        rows: [f32; TILE],
        solid: &mut Option<(f32, f32, [f32; TILE])>,
        tile: &impl Fn(f32, f32, [u32; 2], [f32; TILE]) -> GpuPathTile,
        tiles: &mut Vec<GpuPathTile>,
        edges: &mut Vec<[f32; 4]>,
    ) {
        if !active.is_empty() {
            if let Some((run_x0, run_x1, run_rows)) = solid.take() {
                tiles.push(tile(run_x0, run_x1, [0, 0], run_rows));
            }
            let listed = edges.len();
            edges.extend(active.iter().map(|&index| band[index as usize].edge));
            let crossing = (edges.len() - listed) as u32;
            tiles.push(tile(x0, x1, [listed as u32, crossing], rows));
        } else if let Some(run) = solid.as_mut().filter(|run| run.2 == rows) {
            run.0 = x0;
        } else {
            if let Some((run_x0, run_x1, run_rows)) = solid.take() {
                tiles.push(tile(run_x0, run_x1, [0, 0], run_rows));
            }
            // Inside throughout, or outside, which draws nothing.
            if rows.iter().any(|value| *value != 0.0) {
                *solid = Some((x0, x1, rows));
            }
        }
    }

    /// Coverage of the pixel grid starting at device pixel `origin`, `width` by
    /// `height`, one byte per pixel.
    pub(super) fn rasterize(&self, origin: [f32; 2], width: u32, height: u32) -> Vec<u8> {
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for row in 0..height {
            for column in 0..width {
                let point = [
                    origin[0] + column as f32 + 0.5,
                    origin[1] + row as f32 + 0.5,
                ];
                let value = coverage(&self.edges, point);
                pixels.push((value * 255.0).round() as u8);
            }
        }
        pixels
    }
}

/// Coverage of the pixel square centered on `point`, as the sprite shader
/// computes it.
pub(super) fn coverage(edges: &[[f32; 4]], point: [f32; 2]) -> f32 {
    signed_area(edges, point).abs().min(1.0)
}

/// The winding number of `edges` integrated over the pixel square centered on
/// `point`: what each edge adds for the part of the square to its left.
fn signed_area(edges: &[[f32; 4]], point: [f32; 2]) -> f32 {
    let (left, top, bottom) = (point[0] - 0.5, point[1] - 0.5, point[1] + 0.5);
    let mut area = 0.0f32;
    for &[ax, ay, bx, by] in edges {
        if ay == by {
            continue;
        }
        // Downward edges add, upward ones subtract.
        let (sign, x0, y0, x1, y1) = if ay < by {
            (1.0, ax, ay, bx, by)
        } else {
            (-1.0, bx, by, ax, ay)
        };
        let (low, high) = (y0.max(top), y1.min(bottom));
        if low >= high {
            continue;
        }
        let slope = (x1 - x0) / (y1 - y0);
        let u0 = x0 + (low - y0) * slope - left;
        let u1 = x0 + (high - y0) * slope - left;
        area += sign * (high - low) * mean_clamped(u0, u1);
    }
    area
}

/// The mean of `clamp(u, 0, 1)` as `u` runs linearly from `u0` to `u1`.
fn mean_clamped(u0: f32, u1: f32) -> f32 {
    // An antiderivative of clamp(u, 0, 1).
    let integral = |u: f32| {
        if u <= 0.0 {
            0.0
        } else if u <= 1.0 {
            u * u * 0.5
        } else {
            u - 0.5
        }
    };
    if (u1 - u0).abs() < 1e-4 {
        ((u0 + u1) * 0.5).clamp(0.0, 1.0)
    } else {
        (integral(u1) - integral(u0)) / (u1 - u0)
    }
}

/// The winding area per row that edges wholly right of a tile add, kept as
/// each row's share of partly covered rows plus the steps of whole ones, so
/// adding an edge is constant work and rows are summed only when read.
#[derive(Default)]
struct Backdrop {
    partial: [f32; TILE],
    steps: [f32; TILE + 1],
    rows: [f32; TILE],
    stale: bool,
}

impl Backdrop {
    /// Adds the edge's span of the rows from device y `top`.
    fn add(&mut self, edge: [f32; 4], top: f32) {
        let [_, ay, _, by] = edge;
        let (sign, low, high) = if ay < by {
            (1.0, ay, by)
        } else {
            (-1.0, by, ay)
        };
        let size = TILE as f32;
        let (low, high) = ((low - top).clamp(0.0, size), (high - top).clamp(0.0, size));
        if low >= high {
            return;
        }
        let (first, last) = (low.floor() as usize, high.floor() as usize);
        if first == last {
            self.partial[first] += sign * (high - low);
        } else {
            self.partial[first] += sign * (first as f32 + 1.0 - low);
            self.steps[first + 1] += sign;
            self.steps[last] -= sign;
            if last < TILE {
                self.partial[last] += sign * (high - last as f32);
            }
        }
        self.stale = true;
    }

    fn rows(&mut self) -> [f32; TILE] {
        if self.stale {
            let mut whole = 0.0;
            for row in 0..TILE {
                whole += self.steps[row];
                self.rows[row] = self.partial[row] + whole;
            }
            self.stale = false;
        }
        self.rows
    }
}

/// What every tile of one path shares.
struct Band {
    color: [f32; 4],
    scale_factor: f32,
    /// The device rectangle tiles are cut to.
    area: [f32; 4],
}

/// Storage `push_tiles` reuses.
#[derive(Default)]
pub(super) struct TileScratch {
    by_top: Vec<u32>,
    /// The edges reaching the current band.
    reaching: Vec<u32>,
    band: Vec<BandEdge>,
    /// Band edges by the column they enter the sweep in, and leave it in.
    enter: Buckets,
    leave: Buckets,
    /// The band edges crossing the column being swept.
    active: Vec<u32>,
}

/// An edge crossing a band, with the tile columns it spans there.
struct BandEdge {
    edge: [f32; 4],
    enter: i64,
    leave: i64,
}

/// Indices grouped by column, by counting sort.
#[derive(Default)]
struct Buckets {
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl Buckets {
    fn column(&self, column: usize) -> &[u32] {
        &self.items[self.starts[column] as usize..self.starts[column + 1] as usize]
    }
}

/// Groups `edges` into `columns` buckets by `key`; keys outside are dropped.
fn bucket(
    edges: &[BandEdge],
    columns: usize,
    key: impl Fn(&BandEdge) -> i64,
    buckets: &mut Buckets,
) {
    buckets.starts.clear();
    buckets.starts.resize(columns + 2, 0);
    let slot = |edge: &BandEdge| usize::try_from(key(edge)).ok().filter(|&k| k < columns);
    for edge in edges {
        if let Some(k) = slot(edge) {
            buckets.starts[k + 2] += 1;
        }
    }
    for k in 2..buckets.starts.len() {
        buckets.starts[k] += buckets.starts[k - 1];
    }
    buckets.items.clear();
    buckets
        .items
        .resize(buckets.starts[columns + 1] as usize, 0);
    for (index, edge) in edges.iter().enumerate() {
        if let Some(k) = slot(edge) {
            let at = &mut buckets.starts[k + 1];
            buckets.items[*at as usize] = index as u32;
            *at += 1;
        }
    }
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
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for bits in self.relative.iter().flatten().map(|value| value.to_bits()) {
            hash = (hash ^ u64::from(bits)).wrapping_mul(0x0000_0100_0000_01b3);
        }
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
    pub(super) fn placed(&self) -> usize {
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

/// Analytic paths unused for this many frames lose their kept tiles.
const UNUSED_TILE_FRAMES: u64 = 2;

struct TiledPath {
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
    entries: HashKeyed<Vec<TiledPath>>,
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
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for bits in geometry
            .edges
            .iter()
            .flatten()
            .chain(&visible)
            .chain(std::iter::once(&scale_factor))
            .map(|value| value.to_bits())
        {
            hash = (hash ^ u64::from(bits)).wrapping_mul(0x0000_0100_0000_01b3);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(points: &[(f32, f32)], stroke_width: Option<f32>) -> PathGeometry {
        let vertices: Vec<PathVertex> =
            points.iter().map(|&(x, y)| PathVertex::new(x, y)).collect();
        let mut geometry = PathGeometry::default();
        geometry
            .set(&vertices, stroke_width, 1.0)
            .expect("valid path");
        geometry
    }

    #[test]
    fn fill_coverage_is_the_area_inside_and_follows_nonzero() {
        let square = geometry(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], None);
        let at = |x, y| coverage(&square.edges, [x, y]);
        assert_eq!(at(5.5, 5.5), 1.0);
        assert_eq!(at(20.5, 5.5), 0.0);
        assert!((at(10.0, 5.5) - 0.5).abs() < 1e-6, "half the square inside");
        assert!((at(10.25, 5.5) - 0.25).abs() < 1e-6);
        assert!((at(10.0, 10.0) - 0.25).abs() < 1e-6, "a corner");

        // A concave arrow: the notch is outside, which a triangle fan gets wrong.
        let arrow = geometry(&[(0.0, 0.0), (20.0, 10.0), (0.0, 20.0), (8.0, 10.0)], None);
        let at = |x, y| coverage(&arrow.edges, [x, y]);
        assert_eq!(at(2.5, 10.5), 0.0, "in the notch");
        assert_eq!(at(12.5, 10.5), 1.0, "in the body");

        // Wound twice (a square traced two times) still covers once, and the
        // other direction covers the same.
        let twice = geometry(
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
            ],
            None,
        );
        assert_eq!(coverage(&twice.edges, [5.5, 5.5]), 1.0);
        let reversed = geometry(&[(0.0, 10.0), (10.0, 10.0), (10.0, 0.0), (0.0, 0.0)], None);
        assert!((coverage(&reversed.edges, [10.25, 5.5]) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn stroke_covers_segments_once_with_butt_ends() {
        let corner = geometry(&[(0.0, 10.0), (20.0, 10.0), (20.0, 30.0)], Some(4.0));
        let at = |x, y| coverage(&corner.edges, [x, y]);
        assert_eq!(at(10.5, 10.0), 1.0);
        assert_eq!(at(10.5, 13.0), 0.0, "past half the width");
        assert!((at(10.5, 12.0) - 0.5).abs() < 1e-6);
        assert_eq!(at(20.5, 11.5), 1.0, "the joint, covered once");
        assert_eq!(at(-1.0, 10.0), 0.0, "past the butt end");
        // Either direction draws the same stroke.
        let back = geometry(&[(20.0, 30.0), (20.0, 10.0), (0.0, 10.0)], Some(4.0));
        for point in [[10.5, 12.0], [20.5, 11.5], [21.75, 20.5]] {
            assert!((coverage(&back.edges, point) - at(point[0], point[1])).abs() < 1e-5);
        }
    }

    fn tiled(geometry: &PathGeometry, visible: [f32; 4]) -> (Vec<GpuPathTile>, Vec<[f32; 4]>) {
        let (mut tiles, mut edges) = (Vec::new(), Vec::new());
        geometry.push_tiles(
            Rgba::WHITE,
            1.0,
            visible,
            &mut TileScratch::default(),
            &mut tiles,
            &mut edges,
        );
        (tiles, edges)
    }

    /// Coverage of a pixel center as the shader takes it from a tile.
    fn tile_coverage(tiles: &[GpuPathTile], edges: &[[f32; 4]], point: [f32; 2]) -> f32 {
        let Some(tile) = tiles.iter().find(|tile| {
            let [x, y, width, height] = tile.bounds;
            point[0] >= x && point[0] < x + width && point[1] >= y && point[1] < y + height
        }) else {
            return 0.0;
        };
        let row = ((point[1] - tile.top) as usize).min(TILE - 1);
        let [first, count] = tile.edges;
        let listed = &edges[first as usize..(first + count) as usize];
        let area = tile.backdrop[row] + signed_area(listed, point);
        area.abs().min(1.0)
    }

    #[test]
    fn tiles_cover_what_the_whole_path_covers() {
        let chart: Vec<(f32, f32)> = (0..60)
            .map(|i| (i as f32 * 4.0 + 0.3, 100.0 + (i as f32 * 0.7).sin() * 40.0))
            .collect();
        let star: Vec<(f32, f32)> = (0..10)
            .map(|point| {
                let angle = point as f32 * std::f32::consts::PI / 5.0;
                let radius = if point % 2 == 0 { 70.0 } else { 30.0 };
                (100.0 + angle.sin() * radius, 90.0 - angle.cos() * radius)
            })
            .collect();
        let comb: Vec<(f32, f32)> = (0..6)
            .flat_map(|tooth| {
                let x = 10.0 + tooth as f32 * 30.0;
                [
                    (x, 10.0),
                    (x + 15.0, 10.0),
                    (x + 15.0, 150.0),
                    (x + 30.0, 150.0),
                ]
            })
            .chain([(190.0, 170.0), (10.0, 170.0)])
            .collect();
        for (name, points, stroke) in [
            ("chart", chart, Some(2.0)),
            ("star", star, None),
            ("comb", comb, None),
        ] {
            let path = geometry(&points, stroke);
            let (tiles, edges) = tiled(&path, [0.0, 0.0, 1000.0, 1000.0]);
            for y in 0..200 {
                for x in 0..250 {
                    let point = [x as f32 + 0.5, y as f32 + 0.5];
                    let whole = coverage(&path.edges, point);
                    let tiled = tile_coverage(&tiles, &edges, point);
                    assert!(
                        (whole - tiled).abs() < 1e-3,
                        "{name} at {point:?}: {whole} vs {tiled}"
                    );
                }
            }
            // Each pixel tests far fewer edges than the path has.
            let most = tiles.iter().map(|tile| tile.edges[1]).max().unwrap_or(0);
            assert!(
                (most as usize) * 4 < path.edges.len(),
                "{name}: a tile lists {most} of {} edges",
                path.edges.len()
            );
        }
    }

    #[test]
    fn tiles_inside_merge_and_only_visible_ones_are_made() {
        let square = geometry(
            &[(0.0, 0.0), (160.0, 0.0), (160.0, 160.0), (0.0, 160.0)],
            None,
        );
        let (tiles, _) = tiled(&square, [0.0, 0.0, 1000.0, 1000.0]);
        // Both sides lie on tile edges, so no tile is crossed: per band of 16
        // rows, one run of inside tiles, the right side in their backdrop.
        assert_eq!(tiles.len(), 10, "{tiles:#?}");
        assert!(tiles.iter().all(|tile| tile.edges[1] == 0));
        let (visible, _) = tiled(&square, [0.0, 0.0, 1000.0, 40.0]);
        assert_eq!(visible.len(), 3);
        // Moved off the tile edges, each band has the two crossed tiles too.
        let moved = geometry(
            &[(4.0, 0.0), (164.0, 0.0), (164.0, 160.0), (4.0, 160.0)],
            None,
        );
        let (tiles, _) = tiled(&moved, [0.0, 0.0, 1000.0, 1000.0]);
        assert_eq!(tiles.len(), 10 * 3);
    }

    #[test]
    fn kept_tiles_match_new_ones_in_the_new_color() {
        let star: Vec<(f32, f32)> = (0..10)
            .map(|point| {
                let angle = point as f32 * std::f32::consts::PI / 5.0;
                let radius = if point % 2 == 0 { 70.0 } else { 30.0 };
                (100.0 + angle.sin() * radius, 90.0 - angle.cos() * radius)
            })
            .collect();
        let path = geometry(&star, None);
        let visible = [0.0, 0.0, 300.0, 300.0];
        let (fresh, fresh_edges) = tiled(&path, visible);
        let mut kept = PathTiles::default();
        let red = Rgba::new(1.0, 0.0, 0.0, 1.0);
        for frame in 0..3 {
            kept.begin_frame();
            // Something drawn first, so edge offsets must move.
            let (mut tiles, mut edges) = (Vec::new(), vec![[0.0; 4]; frame]);
            kept.push(&path, red, 1.0, visible, &mut tiles, &mut edges);
            assert_eq!(tiles.len(), fresh.len());
            assert_eq!(&edges[frame..], &fresh_edges[..]);
            for (tile, new) in tiles.iter().zip(&fresh) {
                assert_eq!(tile.color, red.to_array());
                assert_eq!(tile.edges, [new.edges[0] + frame as u32, new.edges[1]]);
                assert_eq!((tile.bounds, tile.backdrop), (new.bounds, new.backdrop));
            }
        }
        // Unused for long enough, it is dropped.
        for _ in 0..=UNUSED_TILE_FRAMES {
            kept.begin_frame();
        }
        assert!(kept.entries.is_empty());
    }

    #[test]
    fn invalid_paths_fail_explicitly() {
        let mut geometry = PathGeometry::default();
        let point = [PathVertex::new(0.0, 0.0)];
        assert!(geometry.set(&point, None, 1.0).is_err());
        assert!(geometry.set(&point, Some(1.0), 1.0).is_err());
        let line = [PathVertex::new(0.0, 0.0), PathVertex::new(1.0, 1.0)];
        assert!(geometry.set(&line, Some(0.0), 1.0).is_err());
        let same = [PathVertex::new(1.0, 1.0), PathVertex::new(1.0, 1.0)];
        assert!(geometry.set(&same, Some(1.0), 1.0).is_err());
        let nan = [
            PathVertex::new(0.0, 0.0),
            PathVertex::new(f32::NAN, 1.0),
            PathVertex::new(1.0, 0.0),
        ];
        assert!(geometry.set(&nan, None, 1.0).is_err());
    }
}
