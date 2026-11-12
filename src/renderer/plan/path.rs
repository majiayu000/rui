//! Paths drawn with analytic coverage, without an intermediate texture.
//!
//! A path's edges are kept in device pixels. A fill covers what its closed
//! polygon winds around (nonzero rule); a stroke covers each segment's
//! rectangle, `stroke_width` wide with butt ends, and a miter join at each
//! vertex (see [`super::stroke`]), all wound the same way, so a stroke is a
//! polygon too and joints are not covered twice. Coverage is the
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

use super::stroke::stroke_edges;
use crate::core::color::Rgba;
use crate::renderer::RendererError;
use crate::renderer::primitives::PathVertex;
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
pub(super) type HashKeyed<V> = HashMap<u64, V, BuildHasherDefault<KeyHasher>>;

/// Passes a `u64` key through as its own hash.
#[derive(Default)]
pub(super) struct KeyHasher(u64);

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
pub(crate) struct GpuPathTile {
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
    /// Replaces this geometry with `vertices` moved by `offset`, at
    /// `scale_factor` device pixels per logical pixel. Fails for paths that
    /// cannot cover anything.
    pub(super) fn set(
        &mut self,
        vertices: &[PathVertex],
        offset: [f32; 2],
        stroke_width: Option<f32>,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        self.edges.clear();
        let device = |vertex: &PathVertex| {
            [
                (vertex.position[0] + offset[0]) * scale_factor,
                (vertex.position[1] + offset[1]) * scale_factor,
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
                stroke_edges(vertices.iter().map(device), half_width, &mut self.edges);
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
            // An edge crossing no column enters at i64::MIN, which no
            // column takes.
            |edge| edge.enter.saturating_sub(first),
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

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
