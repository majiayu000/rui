//! The inspector: elements picked under the pointer, what the frame knew about
//! them, and pointer events kept from the app while it is on.

use rui::advanced_ui::button;
use rui::devtools::PaintState;
use rui::renderer::metal::{MetalRenderer, RetainedTarget};
use rui::renderer::{Primitive, RendererError, Scene};
use rui::testing::mount;
use rui::{AppContext, Color, Div, ElementId, Point, Size, div, text};
use std::cell::Cell;
use std::rc::Rc;

const VIEWPORT: Size = Size {
    width: 400.0,
    height: 200.0,
};
const ROW_HEIGHT: f32 = 30.0;
const SAVE: ElementId = ElementId(42);

/// Four rows, the one at `edited` relabeled, over a button counting clicks.
fn app(
    edited: Rc<Cell<Option<usize>>>,
    clicks: Rc<Cell<u32>>,
) -> impl FnMut(&mut AppContext) -> Div {
    move |_| {
        let clicks = clicks.clone();
        div()
            .w(VIEWPORT.width)
            .flex_col()
            .children((0..4).map(|row| {
                let label = if edited.get() == Some(row) {
                    String::from("edited")
                } else {
                    format!("row {row}")
                };
                div()
                    .w(VIEWPORT.width)
                    .h(ROW_HEIGHT)
                    .bg(Color::hex(0x334455))
                    .child(text(label))
            }))
            .child(
                button("Save")
                    .id(SAVE)
                    .on_click(move || clicks.set(clicks.get() + 1)),
            )
    }
}

fn row_center(row: usize) -> Point {
    Point::new(20.0, row as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0)
}

fn panel_text(scene: &Scene) -> Vec<&str> {
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_inspector_reads_each_element_of_the_last_frame() {
    let edited = Rc::new(Cell::new(None));
    let mut session = mount(VIEWPORT, app(edited.clone(), Rc::default())).expect("mount");
    session.frame().expect("frame");
    assert!(session.inspector().is_none(), "off by default");

    session.app_context_mut().toggle_inspector();
    session.frame().expect("unchanged frame");
    let inspector = session.inspector().expect("inspector on");
    let elements = inspector.elements();
    // The root, four rows of a div and its text, and the button.
    assert_eq!(elements.len(), 10, "{elements:#?}");
    let root = &elements[0];
    assert_eq!(
        (root.type_name.as_str(), root.id, root.parent, root.depth),
        ("Div", ElementId::ROOT, None, 0)
    );
    assert!(root.style.contains("col"), "{}", root.style);

    let row = &elements[1];
    assert_eq!((row.type_name.as_str(), row.parent), ("Div", Some(0)));
    assert!(!row.explicit_id, "rows have derived ids");
    assert_eq!(row.bounds.size, Size::new(VIEWPORT.width, ROW_HEIGHT));
    assert!(row.style.contains("bg #334455ff"), "{}", row.style);
    // Nothing changed, so every row replayed and its text was kept with it.
    for row in 0..4 {
        assert_eq!(elements[1 + row * 2].paint, PaintState::Replayed);
        assert_eq!(elements[2 + row * 2].paint, PaintState::Kept);
        assert_eq!(elements[2 + row * 2].type_name, "Text");
    }

    let save = inspector.find(SAVE).expect("the button");
    assert!(save.explicit_id);
    assert_eq!(save.type_name, "Button");
    assert_eq!(save.accessibility.as_deref(), Some("Button \"Save\""));
    assert_eq!(
        inspector.element_at(Point::new(5.0, save.bounds.origin.y + 2.0)),
        Some(save),
        "the element on top at a point"
    );

    edited.set(Some(2));
    session.app_context_mut().request_rebuild();
    session.frame().expect("edited frame");
    let elements = session.inspector().expect("inspector on").elements();
    assert_eq!(
        elements[5].paint,
        PaintState::Repainted,
        "{:#?}",
        elements[5]
    );
    assert_eq!(elements[3].paint, PaintState::Replayed);

    session.app_context_mut().toggle_inspector();
    session.frame().expect("frame after turning off");
    assert!(session.inspector().is_none());
    assert!(
        !panel_text(session.scene())
            .iter()
            .any(|line| line.starts_with("rui inspector"))
    );
}

#[test]
fn the_pointer_picks_elements_instead_of_reaching_the_app() {
    let clicks = Rc::new(Cell::new(0));
    let mut session = mount(VIEWPORT, app(Rc::default(), clicks.clone())).expect("mount");
    session.frame().expect("frame");
    let on_save = Point::new(5.0, 4.0 * ROW_HEIGHT + 5.0);
    assert_eq!(session.scene().hit_test(on_save), Some(SAVE));
    session.pointer_down(on_save);
    session.pointer_up(on_save);
    assert_eq!(clicks.get(), 1, "the app has the pointer while it is off");

    session.app_context_mut().toggle_inspector();
    session.frame().expect("frame");
    session.pointer_move(row_center(1));
    session.frame().expect("frame");
    let inspector = session.inspector().expect("inspector on");
    let hovered = inspector.hovered().expect("hovered");
    // The text drawn over the row is on top of it.
    assert_eq!(hovered.type_name, "Text");
    assert_eq!(hovered.parent, Some(3));
    assert!(inspector.selected().is_none());
    assert!(
        panel_text(session.scene())
            .iter()
            .any(|line| line.starts_with("> ") && line.contains("Text")),
        "the tree marks the hovered element"
    );

    session.pointer_down(on_save);
    session.pointer_up(on_save);
    session.frame().expect("frame");
    assert_eq!(clicks.get(), 1, "the press pinned the button instead");
    let inspector = session.inspector().expect("inspector on");
    assert_eq!(inspector.selected().map(|element| element.id), Some(SAVE));
    let lines = panel_text(session.scene());
    assert!(
        lines.iter().any(|line| line.contains("(explicit)")),
        "{lines:#?}"
    );

    session.pointer_move(row_center(0));
    session.frame().expect("frame");
    let inspector = session.inspector().expect("inspector on");
    assert_eq!(
        inspector.selected().map(|element| element.id),
        Some(SAVE),
        "pinned while hovering elsewhere"
    );

    session.pointer_down(on_save);
    session.pointer_up(on_save);
    session.frame().expect("frame");
    assert!(
        session.inspector().expect("on").selected().is_none(),
        "unpinned"
    );

    session.app_context_mut().toggle_inspector();
    session.frame().expect("frame");
    session.pointer_down(on_save);
    session.pointer_up(on_save);
    assert_eq!(clicks.get(), 2, "the app has the pointer back");
}

#[test]
fn retained_frames_with_the_inspector_match_full_redraws() {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let mut target = RetainedTarget::new();
    let edited = Rc::new(Cell::new(None));
    let mut session = mount(VIEWPORT, app(edited.clone(), Rc::default())).expect("mount");
    session.app_context_mut().toggle_inspector();
    // The highlight moves between rows, the panel moves from the bottom to
    // the top, a row is edited under it, and the inspector goes away.
    let steps: [(Option<Point>, bool, bool); 6] = [
        (None, false, false),
        (Some(row_center(0)), false, false),
        (Some(row_center(3)), false, false),
        (Some(Point::new(5.0, 4.0 * ROW_HEIGHT + 5.0)), true, false),
        (Some(row_center(1)), false, false),
        (None, false, true),
    ];
    for (step, (hover, edit, toggle)) in steps.into_iter().enumerate() {
        if let Some(point) = hover {
            session.pointer_move(point);
        }
        if edit {
            edited.set(Some(1));
            session.app_context_mut().request_rebuild();
        }
        if toggle {
            session.app_context_mut().toggle_inspector();
        }
        session.frame().expect("frame");
        let scene = session.scene();
        if step == 3 {
            // The button is in the lower half, so the panel moved to the top.
            let panel = scene
                .primitives()
                .iter()
                .find_map(|primitive| match primitive {
                    Primitive::Text {
                        content, bounds, ..
                    } if content.starts_with("rui inspector") => Some(*bounds),
                    _ => None,
                });
            assert!(
                panel.is_some_and(|panel| panel.origin.y < 20.0),
                "{panel:?}"
            );
        }
        let retained = renderer
            .capture_retained_pixels_at_scale(scene, &mut target, VIEWPORT, 2.0)
            .expect("retained capture");
        let full = renderer
            .capture_frame_pixels_at_scale(scene, VIEWPORT, 2.0)
            .expect("full capture");
        assert!(
            retained == full,
            "step {step}: retained frame differs from a full redraw"
        );
    }
}
