#![cfg(all(target_os = "macos", feature = "metal"))]
//! Check the example's fixed cells with a headless 2× Metal readback.

#[path = "../examples/canvas_grid/fixed_cells.rs"]
mod fixed_cells;

use fixed_cells::{CELL_HEIGHT, FAMILY, FIXTURE_HEIGHT};
use rui::elements::canvas;
use rui::prelude::*;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::GlyphResolver;
use rui::renderer::{Primitive, RendererError, Scene};
use rui::testing::mount;

#[test]
fn graphemes_preserve_columns_and_box_cells_join_at_2x() {
    const SCALE: f32 = 2.0;
    let mut resolver = GlyphResolver::new(FAMILY, 400);
    let (cell_width, baseline) = fixed_cells::cell_metrics(&mut resolver);
    let sentinel = resolver.resolve('e').expect("the monospace font has ASCII");
    let viewport = Size::new((11.0 * cell_width).ceil(), FIXTURE_HEIGHT);
    let session = mount(viewport, move |_| {
        canvas(move |bounds, painter| {
            fixed_cells::paint(bounds, painter, cell_width, baseline, Some(sentinel));
        })
        .w(viewport.width)
        .h(viewport.height)
    })
    .expect("mount canvas");

    let sentinels: Vec<_> = session
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Glyph { glyph, origin, .. } => Some((glyph, origin)),
            _ => None,
        })
        .collect();
    assert_eq!(sentinels.len(), 3);
    for ((glyph, origin), column) in sentinels.into_iter().zip([2.0, 5.0, 9.0]) {
        assert_eq!(*glyph, sentinel);
        assert!((origin.x - column * cell_width).abs() < 0.001);
        assert!((origin.y - baseline).abs() < 0.001);
    }

    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(message)) => {
            eprintln!("2× Metal pixel capture skipped: {message}");
            return;
        }
        Err(err) => panic!("Metal should initialize: {err}"),
    };
    let mut scene = Scene::new();
    for primitive in session.primitives().iter().cloned() {
        scene.insert(primitive);
    }
    scene.finish();
    let pixels = renderer
        .capture_frame_pixels_at_scale(&scene, viewport, SCALE)
        .expect("capture the canvas");
    let width = (viewport.width * SCALE) as usize;
    assert_eq!(pixels.len(), width * (viewport.height * SCALE) as usize * 4);
    let rgb = |x: usize, y: usize| {
        let offset = (y * width + x) * 4;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    };
    let lit = |x: usize, y: usize| rgb(x, y).iter().any(|&value| value > 96);
    let column_start = |column: usize| (column as f32 * cell_width * SCALE - 0.5).ceil() as usize;
    let ink_rows = |column: usize, span: usize| {
        (0..(CELL_HEIGHT * SCALE) as usize)
            .filter(|&y| (column_start(column)..column_start(column + span)).any(|x| lit(x, y)))
            .collect::<Vec<_>>()
    };

    for (column, span) in [(0, 2), (4, 1), (7, 2)] {
        assert!(!ink_rows(column, span).is_empty(), "cluster at {column}");
    }
    assert!(
        (0..(CELL_HEIGHT * SCALE) as usize).any(|y| {
            (column_start(7)..column_start(9)).any(|x| {
                let rgb = rgb(x, y);
                rgb.iter().max().unwrap() - rgb.iter().min().unwrap() > 30
            })
        }),
        "the ZWJ emoji uses color glyphs"
    );
    for column in [2, 5, 9] {
        assert!(!ink_rows(column, 1).is_empty(), "sentinel at {column}");
    }
    for column in [3, 6, 10] {
        assert!(ink_rows(column, 1).is_empty(), "blank column {column}");
    }

    // Compare the accent row to its cached ASCII e at the next column.
    let accent = ink_rows(4, 1);
    let ascii = ink_rows(5, 1);
    assert!(accent.first() < ascii.first(), "acute accent above the e");
    assert!(accent[accent.len() - 1].abs_diff(ascii[ascii.len() - 1]) <= 1);

    for y in [1.5, 3.5] {
        let x0 = (0.5 * cell_width * SCALE).ceil() as usize;
        let x1 = (3.5 * cell_width * SCALE).floor() as usize;
        let y = (y * CELL_HEIGHT * SCALE) as usize;
        assert!((x0..x1).all(|x| lit(x, y)), "horizontal stroke has a gap");
    }
    let y0 = (1.5 * CELL_HEIGHT * SCALE) as usize;
    let y1 = (3.5 * CELL_HEIGHT * SCALE) as usize;
    for column in [0.5, 3.5] {
        let x = (column * cell_width * SCALE).floor() as usize;
        assert!((y0..y1).all(|y| lit(x, y)), "vertical stroke has a gap");
    }
    let block_y = (2.5 * CELL_HEIGHT * SCALE) as usize;
    let x0 = (cell_width * SCALE).ceil() as usize;
    let x1 = (3.0 * cell_width * SCALE).floor() as usize;
    assert!((x0..x1).all(|x| lit(x, block_y)), "block cells have a gap");
}
