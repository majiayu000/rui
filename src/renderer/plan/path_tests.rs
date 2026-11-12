//! Tests of `path`: coverage, tiles, and the kept tiles.

use super::super::path_tiles::{PathTiles, UNUSED_TILE_FRAMES};
use super::*;

fn geometry(points: &[(f32, f32)], stroke_width: Option<f32>) -> PathGeometry {
    let vertices: Vec<PathVertex> = points.iter().map(|&(x, y)| PathVertex::new(x, y)).collect();
    let mut geometry = PathGeometry::default();
    geometry
        .set(&vertices, [0.0; 2], stroke_width, 1.0)
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
    assert!(geometry.set(&point, [0.0; 2], None, 1.0).is_err());
    assert!(geometry.set(&point, [0.0; 2], Some(1.0), 1.0).is_err());
    let line = [PathVertex::new(0.0, 0.0), PathVertex::new(1.0, 1.0)];
    assert!(geometry.set(&line, [0.0; 2], Some(0.0), 1.0).is_err());
    let same = [PathVertex::new(1.0, 1.0), PathVertex::new(1.0, 1.0)];
    assert!(geometry.set(&same, [0.0; 2], Some(1.0), 1.0).is_err());
    let nan = [
        PathVertex::new(0.0, 0.0),
        PathVertex::new(f32::NAN, 1.0),
        PathVertex::new(1.0, 0.0),
    ];
    assert!(geometry.set(&nan, [0.0; 2], None, 1.0).is_err());
}
