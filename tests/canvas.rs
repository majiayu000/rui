use rui::core::color::Color;
use rui::core::{Bounds, ElementId, Point, Size};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::elements::{Canvas, Div, Painter, canvas, div};
use rui::renderer::Primitive;
use rui::testing::mount;
use std::cell::RefCell;
use std::rc::Rc;

const PADDING: f32 = 20.0;

fn canvas_id() -> ElementId {
    ElementId::from(7_000)
}

/// A 100x50 canvas inset by `PADDING` in a 400x300 window.
fn framed(canvas: Canvas) -> Div {
    div()
        .w(400.0)
        .h(300.0)
        .p(PADDING)
        .child(canvas.w(100.0).h(50.0))
}

fn quads(primitives: &[Primitive]) -> Vec<Bounds> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .collect()
}

#[test]
fn paints_cells_at_offsets_from_the_canvas_origin() {
    let painted = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&painted);
    let session = mount(Size::new(400.0, 300.0), move |_| {
        let seen = Rc::clone(&seen);
        framed(canvas(move |bounds, painter| {
            seen.replace(Some(bounds));
            for column in 0..3 {
                let cell = Bounds::from_xywh(
                    bounds.x() + column as f32 * 10.0,
                    bounds.y() + 5.0,
                    10.0,
                    10.0,
                );
                painter.fill(cell, Color::WHITE);
            }
        }))
    })
    .expect("mount");

    let bounds = painted.borrow().expect("canvas painted");
    assert_eq!(bounds, Bounds::from_xywh(PADDING, PADDING, 100.0, 50.0));
    assert_eq!(
        quads(session.primitives()),
        vec![
            Bounds::from_xywh(20.0, 25.0, 10.0, 10.0),
            Bounds::from_xywh(30.0, 25.0, 10.0, 10.0),
            Bounds::from_xywh(40.0, 25.0, 10.0, 10.0),
        ]
    );
}

#[test]
fn clips_to_its_bounds_and_keeps_clips_balanced() {
    let session = mount(Size::new(400.0, 300.0), |_| {
        framed(canvas(|bounds, painter| {
            let half = Bounds::from_xywh(bounds.x(), bounds.y(), 50.0, 50.0);
            painter.clip(half, |painter| {
                painter.clip(half, |painter| painter.fill(half, Color::BLACK));
            });
        }))
    })
    .expect("mount");

    let clips: Vec<_> = session
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::PushClip { bounds, .. } => Some(Some(*bounds)),
            Primitive::PopClip => Some(None),
            _ => None,
        })
        .collect();
    let canvas_bounds = Bounds::from_xywh(PADDING, PADDING, 100.0, 50.0);
    let half = Bounds::from_xywh(PADDING, PADDING, 50.0, 50.0);
    assert_eq!(
        clips,
        vec![
            Some(canvas_bounds),
            Some(half),
            Some(half),
            None,
            None,
            None
        ]
    );
    assert_eq!(session.scene().current_clip(), None);
}

#[test]
#[should_panic(expected = "Painter::clip")]
fn rejects_raw_clip_primitives() {
    let _ = mount(Size::new(400.0, 300.0), |_| {
        framed(canvas(|bounds, painter: &mut Painter| {
            painter.paint(Primitive::PushClip {
                bounds,
                corner_radii: Default::default(),
            });
        }))
    });
}

#[test]
fn pointer_events_reach_the_handler_in_local_coordinates() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    let mut session = mount(Size::new(400.0, 300.0), move |_| {
        let seen = Rc::clone(&seen);
        framed(
            canvas(|_, _| {})
                .id(canvas_id())
                .on_pointer(move |event: &PointerEvent| {
                    seen.borrow_mut().push((event.kind, event.position))
                }),
        )
    })
    .expect("mount");
    assert_eq!(
        session.scene().hit_test(Point::new(30.0, 30.0)),
        Some(canvas_id())
    );

    assert!(session.pointer_down(Point::new(25.0, 35.0)));
    session.pointer_move(Point::new(300.0, 200.0));
    assert!(session.pointer_up(Point::new(300.0, 200.0)));
    // A press outside never reaches the handler.
    assert!(!session.pointer_down(Point::new(300.0, 200.0)));

    assert_eq!(
        *events.borrow(),
        vec![
            (PointerEventKind::Down, Point::new(5.0, 15.0)),
            (PointerEventKind::Move, Point::new(280.0, 180.0)),
            (PointerEventKind::Up, Point::new(280.0, 180.0)),
        ]
    );
}

#[test]
fn a_canvas_without_a_handler_does_not_take_hits() {
    let session = mount(Size::new(400.0, 300.0), |_| {
        framed(canvas(|_, _| {}).id(canvas_id()))
    })
    .expect("mount");
    assert_eq!(session.scene().hit_test(Point::new(30.0, 30.0)), None);
}
