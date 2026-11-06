//! The damage and replay visualizer: replayed subtrees filled, repainted ones
//! outlined, the damage outlined, and nothing left behind on screen.

use rui::renderer::metal::{MetalRenderer, RetainedTarget};
use rui::renderer::{Primitive, RendererError, Scene};
use rui::testing::mount;
use rui::{AppContext, Bounds, Color, Div, Rgba, Size, div, text};
use std::cell::Cell;
use std::rc::Rc;

const VIEWPORT: Size = Size {
    width: 400.0,
    height: 300.0,
};
const ROW_HEIGHT: f32 = 30.0;

fn rows(edited: Rc<Cell<Option<usize>>>) -> impl FnMut(&mut AppContext) -> Div {
    move |_| {
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
                    .bg(Color::rgb(0.9, 0.9, 0.92))
                    .child(text(label))
            }))
    }
}

#[derive(Debug, PartialEq)]
enum Tint {
    Replayed(Bounds),
    Repainted(Bounds),
    Damage(Bounds),
}

/// The visualizer's quads, told apart by their colors.
fn tints(scene: &Scene) -> Vec<Tint> {
    let is = |color: Rgba, r: f32, g: f32| color.a > 0.0 && color.r == r && color.g == g;
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad {
                bounds,
                background,
                border_color,
                ..
            } => {
                if is(*background, 0.15, 0.85) {
                    Some(Tint::Replayed(*bounds))
                } else if is(*border_color, 1.0, 0.25) {
                    Some(Tint::Repainted(*bounds))
                } else if is(*border_color, 1.0, 0.1) {
                    Some(Tint::Damage(*bounds))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

fn row_bounds(row: usize) -> Bounds {
    Bounds::from_xywh(0.0, row as f32 * ROW_HEIGHT, VIEWPORT.width, ROW_HEIGHT)
}

#[test]
fn the_visualizer_tints_replayed_and_repainted_subtrees_and_the_damage() {
    let edited = Rc::new(Cell::new(None));
    let mut session = mount(VIEWPORT, rows(edited.clone())).expect("mount");
    session.frame().expect("frame");
    assert!(tints(session.scene()).is_empty(), "off draws nothing");

    session.app_context_mut().toggle_damage_visualizer();
    session.frame().expect("unchanged frame");
    // Nothing changed: every row replays and nothing is damage.
    assert_eq!(
        tints(session.scene()),
        (0..4)
            .map(|row| Tint::Replayed(row_bounds(row)))
            .collect::<Vec<_>>()
    );

    edited.set(Some(2));
    session.app_context_mut().request_rebuild();
    session.frame().expect("edited frame");
    let drawn = tints(session.scene());
    for row in [0, 1, 3] {
        assert!(
            drawn.contains(&Tint::Replayed(row_bounds(row))),
            "{drawn:?}"
        );
    }
    assert!(drawn.contains(&Tint::Repainted(row_bounds(2))), "{drawn:?}");
    let damage = drawn.iter().find_map(|tint| match tint {
        Tint::Damage(bounds) => Some(*bounds),
        _ => None,
    });
    match damage {
        Some(damage) => {
            // The edited row and what its text may reach, not the whole column.
            assert!(damage.intersects(&row_bounds(2)), "{damage:?}");
            assert!(damage.height() < 4.0 * ROW_HEIGHT, "{damage:?}");
        }
        None => panic!("an edit should outline its damage: {drawn:?}"),
    }

    session.app_context_mut().toggle_damage_visualizer();
    session.frame().expect("frame after turning off");
    assert!(tints(session.scene()).is_empty());
}

#[test]
fn retained_frames_with_the_visualizer_match_full_redraws() {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let mut target = RetainedTarget::new();
    let edited = Rc::new(Cell::new(None));
    let mut session = mount(VIEWPORT, rows(edited.clone())).expect("mount");
    // Edits move the damage from row to row, so each frame's tint leaves the
    // place of the one before; the profiler is on part of the time.
    let steps: [(Option<usize>, bool, bool); 7] = [
        (None, false, false),
        (None, true, false),
        (Some(0), false, false),
        (Some(3), false, true),
        (Some(1), false, false),
        (Some(1), true, false),
        (None, false, true),
    ];
    for (step, (edit, toggle_damage, toggle_profiler)) in steps.into_iter().enumerate() {
        if toggle_damage {
            session.app_context_mut().toggle_damage_visualizer();
        }
        if toggle_profiler {
            session.app_context_mut().toggle_profiler();
        }
        if edited.get() != edit {
            edited.set(edit);
            session.app_context_mut().request_rebuild();
        }
        session.frame().expect("frame");
        let scene = session.scene();
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
