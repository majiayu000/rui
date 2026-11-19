//! Table columns fit the text drawn in them.

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::{div, header_row, row, table};
use rui::renderer::text::{TextMeasureCache, TextRequest};
use rui::renderer::{Primitive, Scene};
use rui::{ElementId, Size};
use taffy::prelude::TaffyTree;

/// Bold headers used to run past their column's border when column widths
/// were guessed from byte counts (#340).
#[test]
fn cell_text_stays_inside_its_column() {
    let mut root = div().child(
        table()
            .row(header_row().cells(["Name", "Amount", "Wednesday", "Owner", "Revenue"]))
            .row(row().cells(["Ann", "1,250", "Mountain", "Bob", "WWWW"])),
    );
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut measurer = TextMeasureCache::new();
    let bounds = FramePipeline::layout_root_with_text_measurer(
        &mut root,
        &mut taffy,
        &mut measurer,
        Size::new(1200.0, 400.0),
    )
    .expect("layout");
    let mut scene = Scene::new();
    FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds);

    let mut texts = 0;
    for primitive in scene.primitives() {
        let Primitive::Text {
            bounds,
            content,
            font_size,
            font_weight,
            line_height,
            ..
        } = primitive
        else {
            continue;
        };
        texts += 1;
        let ink = measurer
            .measure_single_line(TextRequest::new(
                content,
                *font_size,
                *font_weight,
                None,
                *line_height,
            ))
            .expect("measure")
            .ink_bounds
            .width();
        // The text's bounds are its cell minus padding; left-aligned ink
        // starts at their left edge.
        assert!(
            bounds.x() + ink <= bounds.x() + bounds.width() + 0.01,
            "{content:?} draws {ink} px of ink in {} px",
            bounds.width()
        );
    }
    assert_eq!(texts, 10);
}
