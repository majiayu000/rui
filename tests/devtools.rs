//! The profiling overlay: drawn into the scene only while it is on, and
//! redrawn through the damage without leaving stale pixels.

use rui::renderer::metal::{MetalRenderer, RetainedTarget};
use rui::renderer::{Primitive, Reach, RendererError, Scene};
use rui::testing::mount;
use rui::{AppContext, Bounds, Color, Div, Size, div, text};

const VIEWPORT: Size = Size {
    width: 640.0,
    height: 400.0,
};

fn app(_: &mut AppContext) -> Div {
    div().size(VIEWPORT).child(
        div()
            .size(VIEWPORT)
            .bg(Color::rgb(0.9, 0.9, 0.92))
            .child(text("profiled")),
    )
}

fn overlay_lines(scene: &Scene) -> Vec<&str> {
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } if content.starts_with("rui profiler") => {
                Some(content.as_str())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_overlay_is_drawn_only_while_the_profiler_is_on() {
    let mut session = mount(VIEWPORT, app).expect("mount");
    let without = session.primitives().len();
    assert!(overlay_lines(session.scene()).is_empty());
    assert!(session.profiler().is_none());

    session.app_context_mut().toggle_profiler();
    session.frame().expect("first profiled frame");
    assert_eq!(overlay_lines(session.scene()), ["rui profiler: measuring…"]);
    session.frame().expect("second profiled frame");
    assert_eq!(overlay_lines(session.scene()), ["rui profiler  frame 0"]);
    let profiler = session.profiler().expect("profiler while on");
    let last = profiler.last_frame().expect("a measured frame");
    assert_eq!(last.index, 1);
    assert!(last.interval_ns.is_some());
    // The app's primitives, without the overlay drawn over them.
    assert_eq!(last.primitives, without);
    // Headless has no backend, and this test binary no counting allocator.
    assert_eq!(last.render, None);
    assert_eq!(last.allocations, None);

    session.app_context_mut().toggle_profiler();
    session.frame().expect("frame after turning off");
    assert!(overlay_lines(session.scene()).is_empty());
    assert_eq!(session.primitives().len(), without);
    assert!(session.profiler().is_none());
}

#[test]
fn the_overlay_is_not_counted_as_damage_but_its_old_place_is_once_off() {
    let mut session = mount(VIEWPORT, app).expect("mount");
    let window = Bounds::from_xywh(0.0, 0.0, VIEWPORT.width, VIEWPORT.height);
    session.frame().expect("frame");
    assert_eq!(session.scene().damage(window), Reach::Nothing);
    session.app_context_mut().toggle_profiler();
    for _ in 0..3 {
        session.frame().expect("profiled frame");
    }
    // Nothing in the app changed, so all it damaged is nothing.
    let last = session.profiler().and_then(|p| p.last_frame()).copied();
    assert_eq!(last.map(|frame| frame.damage_fraction), Some(0.0));
    // The renderer still redraws the new overlay: a band at the top, as wide
    // as the window since text may reach past its bounds.
    let panel = match session.scene().damage(window) {
        Reach::Within(bounds) => bounds,
        other => panic!("the overlay should be bounded damage, got {other:?}"),
    };
    assert!(panel.height() < VIEWPORT.height / 2.0, "{panel:?}");

    session.app_context_mut().toggle_profiler();
    session.frame().expect("frame after turning off");
    match session.scene().damage(window) {
        Reach::Within(bounds) => assert_eq!(bounds, panel, "the old overlay is painted over"),
        other => panic!("expected the old overlay as damage, got {other:?}"),
    }
}

#[test]
fn retained_frames_with_the_overlay_match_full_redraws() {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let mut target = RetainedTarget::new();
    let mut session = mount(VIEWPORT, app).expect("mount");
    // Off, on for frames whose text differs, then off again.
    for (step, toggle) in [false, true, false, false, true, false]
        .into_iter()
        .enumerate()
    {
        if toggle {
            session.app_context_mut().toggle_profiler();
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
