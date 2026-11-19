//! A cell grid drawn glyph by glyph shapes nothing, edited or not; drawn as
//! one text per row, an edited cell reshapes its row.

use super::MetalRenderer;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Point, Size};
use crate::elements::text::TextAlign;
use crate::renderer::text::{BaseDirection, GlyphResolver};
use crate::renderer::{Primitive, RendererError, Scene};

const COLUMNS: usize = 200;
const ROWS: usize = 60;
const FONT_SIZE: f32 = 13.0;
const CELL_HEIGHT: f32 = 16.0;

fn character(row: usize, column: usize, edit: usize) -> char {
    let edited = usize::from(row == 7 && column == 42) * edit;
    char::from(b'!' + ((row * 7 + column + edited) % 94) as u8)
}

fn renderer() -> Option<MetalRenderer> {
    match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    }
}

#[test]
fn editing_one_cell_of_a_glyph_grid_shapes_nothing() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut resolver = GlyphResolver::new(Some("SF Mono"), 400);
    let cell = resolver.resolve('0').expect("digits").advance(FONT_SIZE);
    let viewport = Size::new(COLUMNS as f32 * cell, ROWS as f32 * CELL_HEIGHT);
    let mut grid = |edit| {
        let mut scene = Scene::new();
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let glyph = resolver
                    .resolve(character(row, column, edit))
                    .expect("ASCII");
                scene.insert(Primitive::Glyph {
                    glyph,
                    origin: Point::new(column as f32 * cell, row as f32 * CELL_HEIGHT + 12.0),
                    font_size: FONT_SIZE,
                    color: Rgba::new(1.0, 1.0, 1.0, 1.0),
                });
            }
        }
        scene.finish();
        scene
    };
    for edit in 0..4 {
        renderer
            .capture_frame_pixels_at_scale(&grid(edit), viewport, 2.0)
            .expect("capture");
    }
    assert_eq!(renderer.planner.text_cache.shape_misses(), 0);
}

#[test]
fn editing_one_cell_of_a_row_text_reshapes_that_row() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let viewport = Size::new(1600.0, ROWS as f32 * CELL_HEIGHT);
    let rows = |edit| {
        let mut scene = Scene::new();
        for row in 0..ROWS {
            let content: String = (0..COLUMNS)
                .map(|column| character(row, column, edit))
                .collect();
            scene.insert(Primitive::Text {
                bounds: Bounds::from_xywh(0.0, row as f32 * CELL_HEIGHT, 1600.0, CELL_HEIGHT),
                content: content.into(),
                color: Rgba::new(1.0, 1.0, 1.0, 1.0),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: Some("SF Mono".into()),
                line_height: 1.0,
                align: TextAlign::Left,
                direction: BaseDirection::Auto,
            });
        }
        scene.finish();
        scene
    };
    let mut misses = Vec::new();
    for edit in 0..3 {
        renderer
            .capture_frame_pixels_at_scale(&rows(edit), viewport, 2.0)
            .expect("capture");
        misses.push(renderer.planner.text_cache.shape_misses());
    }
    assert_eq!(misses, vec![ROWS, ROWS + 1, ROWS + 2]);
}
