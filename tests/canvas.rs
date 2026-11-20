use rui::core::color::{Color, Rgba};
use rui::core::event::MouseButton;
use rui::core::geometry::Edges;
use rui::core::{Bounds, ElementId, Point, Presenter, Size};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::elements::{Canvas, Div, Painter, canvas, div};
use rui::renderer::{Picture, Primitive};
use rui::testing::mount;
use std::cell::{Cell, RefCell};
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

/// The quads `primitives` draw, those of a picture moved by its offset.
fn drawn_quads(primitives: &[Primitive]) -> Vec<Bounds> {
    let mut drawn = Vec::new();
    for primitive in primitives {
        match primitive {
            Primitive::Quad { bounds, .. } => drawn.push(*bounds),
            Primitive::Picture { picture, offset } => {
                drawn.extend(drawn_quads(picture.primitives()).into_iter().map(|quad| {
                    Bounds::from_xywh(
                        quad.x() + offset.x,
                        quad.y() + offset.y,
                        quad.width(),
                        quad.height(),
                    )
                }));
            }
            _ => {}
        }
    }
    drawn
}

#[test]
fn a_cached_canvas_draws_the_pictures_its_closure_paints() {
    // A marker recorded once and stamped along a chart.
    let marker = Picture::new(vec![Primitive::Quad {
        bounds: Bounds::from_xywh(1.0, 2.0, 4.0, 4.0),
        background: Color::WHITE.to_rgba(),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Default::default(),
    }]);
    let stamped = |key: Option<u32>| {
        let marker = marker.clone();
        let session = mount(Size::new(400.0, 300.0), move |_| {
            let marker = marker.clone();
            let chart = canvas(move |bounds, painter| {
                painter.fill(bounds, Color::BLACK);
                for column in 0..2 {
                    painter.paint(Primitive::Picture {
                        picture: marker.clone(),
                        offset: Point::new(bounds.x() + column as f32 * 10.0, bounds.y()),
                    });
                }
            });
            framed(match key {
                Some(key) => chart.cache_key(key),
                None => chart,
            })
        })
        .expect("mount");
        drawn_quads(session.primitives())
    };

    let drawn = vec![
        Bounds::from_xywh(PADDING, PADDING, 100.0, 50.0),
        Bounds::from_xywh(21.0, 22.0, 4.0, 4.0),
        Bounds::from_xywh(31.0, 22.0, 4.0, 4.0),
    ];
    assert_eq!(stamped(None), drawn);
    // Recorded into the picture of the canvas, which holds no other picture.
    assert_eq!(stamped(Some(1)), drawn);
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

#[test]
fn hit_refresh_registers_the_canvas_without_running_its_painter() {
    let size = Size::new(400.0, 300.0);
    let paint_calls = Rc::new(Cell::new(0));
    let pressed = Rc::new(Cell::new(false));
    let calls = Rc::clone(&paint_calls);
    let color_state = Rc::clone(&pressed);
    let canvas = canvas(move |bounds, painter| {
        calls.set(calls.get() + 1);
        let color = if color_state.get() {
            Color::WHITE
        } else {
            Color::BLACK
        };
        painter.fill(bounds, color);
    })
    .id(canvas_id())
    .on_pointer(move |event| pressed.set(event.kind == PointerEventKind::Down));
    let mut presenter = Presenter::with_root(size, framed(canvas));
    presenter.layout(size).expect("initial layout");
    presenter.paint();
    let rendered_frame = presenter.scene().frame();
    let before = paint_calls.get();

    presenter.layout(size).expect("layout before input");
    let dispatch = presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Down,
        position: Point::new(30.0, 30.0),
        button: Some(MouseButton::Left),
    });
    assert!(dispatch.stopped, "the hit-only pass still finds the canvas");
    assert_eq!(paint_calls.get(), before, "input must not run the painter");
    assert_eq!(presenter.scene().frame(), rendered_frame);

    presenter.paint();
    assert!(
        paint_calls.get() > before,
        "real paint still runs the painter"
    );
    assert_eq!(presenter.scene().previous_frame(), rendered_frame);
    assert!(
        presenter
            .scene()
            .primitives()
            .iter()
            .any(|primitive| matches!(
                primitive,
                Primitive::Quad { background, .. } if *background == Color::WHITE.to_rgba()
            )),
        "the real frame includes the pointer handler's new paint"
    );
}
