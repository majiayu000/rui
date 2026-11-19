//! The fixtures of `pointer_frame`: trees a pointer moves over, presses and
//! drags in, the events of one round, and what those events must hit, so a
//! broken fixture fails instead of measuring nothing.

use rui::advanced_ui::{Button, popover};
use rui::core::event::{Cursor, Modifiers, MouseButton, ScrollEvent};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::elements::text::TextAlign;
use rui::elements::{Canvas, Div, canvas, div, scroll_view, text};
use rui::renderer::text::BaseDirection;
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Point, SharedStr, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

pub const VIEWPORT: Size = Size::new(1440.0, 900.0);
/// The size `buttons.resize_move` alternates with [`VIEWPORT`].
const RESIZED: Size = Size::new(1280.0, 800.0);
/// Height of a medium [`Button`].
const BUTTON_HEIGHT: f32 = 36.0;
/// Height of a row of `perf_gate`'s long list.
const LIST_ROW_HEIGHT: f32 = 28.0;
/// The 200x60 character grid of `examples/canvas_grid.rs`, in 7x15 cells so
/// it fits the window.
const GRID: (usize, usize) = (200, 60);
const CELL: (f32, f32) = (7.0, 15.0);
/// The canvas `canvas.capture_drag` presses in, above the rows.
const PAD: (f32, f32) = (400.0, 200.0);
/// The row the pointer rests on.
const ROW: usize = 10;
const CANVAS: ElementId = ElementId(10);
const OVERLAY_BUTTON: ElementId = ElementId(11);

fn row_id(index: usize) -> ElementId {
    ElementId(100_000 + index as u64)
}

/// The middle of row `index` of buttons that start at `top`, left of every label.
fn on_button(index: usize, top: f32) -> Point {
    Point::new(20.0, top + (index as f32 + 0.5) * BUTTON_HEIGHT)
}

/// What a fixture's tree reports to the benchmark.
#[derive(Default)]
pub struct Probe {
    /// Whether the first two rows are swapped (`buttons.swap_click`).
    pub swapped: Cell<bool>,
    /// Buttons clicked, in order.
    pub clicked: RefCell<Vec<ElementId>>,
    /// Calls of a canvas's paint closure.
    pub canvas_calls: Cell<usize>,
    /// What a canvas's pointer handler received, positions relative to it.
    pub canvas_events: RefCell<Vec<(PointerEventKind, Point)>>,
}

/// What to change before a round's events.
pub enum Setup {
    None,
    /// Swap the first two rows, which rebuilds the tree.
    Swap,
    /// Lay the window out at a new size, which rebuilds the tree as a resize does.
    Resize(Size),
}

/// What a round left, read after its last frame.
pub struct Seen<'a> {
    pub probe: &'a Probe,
    pub scene: &'a Scene,
    pub cursor: Cursor,
    pub root_bounds: Bounds,
}

#[derive(Clone, Copy)]
pub enum Fixture {
    /// (a) A move that stays on a row of 2000 buttons.
    StillMove,
    /// (b) The first two rows swap places, then a press and release on the first.
    SwapClick,
    /// (b) The window resizes before each move.
    ResizeMove,
    /// (c) The buttons in a scroll view scrolled to the middle.
    ScrolledMove,
    /// (d) An open popover over the buttons, the pointer on a button in it.
    OverlayMove,
    /// (e) A press on a canvas above the buttons, two moves over the buttons, the release.
    CaptureDrag,
    /// (f) A 200x60 character grid in one canvas with a pointer handler.
    Grid { cached: bool },
    /// (g) `perf_gate`'s long list, whose rows have no handlers.
    List,
}

pub const ALL: [Fixture; 9] = [
    Fixture::StillMove,
    Fixture::SwapClick,
    Fixture::ResizeMove,
    Fixture::ScrolledMove,
    Fixture::OverlayMove,
    Fixture::CaptureDrag,
    Fixture::Grid { cached: false },
    Fixture::Grid { cached: true },
    Fixture::List,
];

impl Fixture {
    pub fn name(self) -> &'static str {
        match self {
            Self::StillMove => "buttons.still_move",
            Self::SwapClick => "buttons.swap_click",
            Self::ResizeMove => "buttons.resize_move",
            Self::ScrolledMove => "buttons.scrolled_move",
            Self::OverlayMove => "buttons.overlay_move",
            Self::CaptureDrag => "canvas.capture_drag",
            Self::Grid { cached: false } => "canvas_grid.move",
            Self::Grid { cached: true } => "canvas_grid.cached_move",
            Self::List => "list.move",
        }
    }

    /// The tree, with `rows` rows where it has rows.
    pub fn build(self, probe: &Rc<Probe>, rows: usize) -> Div {
        match self {
            Self::StillMove | Self::ResizeMove => buttons(probe, rows),
            Self::SwapClick => {
                let order = if probe.swapped.get() { [1, 0] } else { [0, 1] };
                let swapped = order.into_iter().chain(2..rows);
                div()
                    .w_full()
                    .flex_col()
                    .children(swapped.map(|index| button(probe, index)))
            }
            Self::ScrolledMove => div().w_full().h_full().child(
                scroll_view()
                    .w(VIEWPORT.width)
                    .h(VIEWPORT.height)
                    .child(buttons(probe, rows).h(rows as f32 * BUTTON_HEIGHT)),
            ),
            Self::OverlayMove => {
                let content = div()
                    .flex_col()
                    .p(8.0)
                    .child(text("Rows below"))
                    .child(Button::new("Overlay action").id(OVERLAY_BUTTON));
                div()
                    .w_full()
                    .flex_col()
                    .child(popover("Details", Button::new("Open"), content).open(true))
                    .children((0..rows).map(|index| button(probe, index)))
            }
            Self::CaptureDrag => div()
                .w_full()
                .flex_col()
                .child(pad(probe))
                .children((0..rows).map(|index| button(probe, index))),
            Self::Grid { cached } => div().child(grid(probe, cached)),
            Self::List => list(rows),
        }
    }

    /// The scroll that puts the scroll view at its middle row, once mounted.
    pub fn scroll(self, rows: usize) -> Option<ScrollEvent> {
        matches!(self, Self::ScrolledMove).then(|| ScrollEvent {
            position: Point::new(100.0, 100.0),
            delta_x: 0.0,
            delta_y: (rows / 2) as f32 * BUTTON_HEIGHT,
            modifiers: Modifiers::none(),
        })
    }

    /// Where the pointer goes, found in the first frame's scene where the
    /// layout decides it.
    pub fn pointer(self, scene: &Scene) -> Point {
        match self {
            Self::OverlayMove => {
                let region = scene
                    .layers()
                    .iter()
                    .flat_map(|layer| layer.hit_regions())
                    .find(|region| region.id == OVERLAY_BUTTON)
                    .expect("the popover's button has a hit region");
                region.bounds.center()
            }
            Self::Grid { .. } => Point::new(700.0, 450.0),
            Self::List => Point::new(200.0, 10.0),
            // The first row, which the swap replaces.
            Self::SwapClick => on_button(0, 0.0),
            _ => on_button(ROW, 0.0),
        }
    }

    pub fn setup(self, round: usize) -> Setup {
        match self {
            Self::SwapClick => Setup::Swap,
            Self::ResizeMove if round.is_multiple_of(2) => Setup::Resize(RESIZED),
            Self::ResizeMove => Setup::Resize(VIEWPORT),
            _ => Setup::None,
        }
    }

    /// The events of `round`, each dispatched in a frame of its own.
    pub fn events(self, round: usize, pointer: Point) -> Vec<PointerEvent> {
        match self {
            Self::SwapClick => vec![
                press(PointerEventKind::Down, pointer),
                press(PointerEventKind::Up, pointer),
            ],
            Self::CaptureDrag => {
                let inside = Point::new(PAD.0 / 2.0, PAD.1 / 2.0);
                vec![
                    press(PointerEventKind::Down, inside),
                    moved(on_button(ROW, PAD.1)),
                    moved(on_button(ROW + 2, PAD.1)),
                    press(PointerEventKind::Up, on_button(ROW + 2, PAD.1)),
                ]
            }
            // The pointer of `perf_gate`'s `list.pointer_move`.
            Self::List => vec![moved(Point::new(
                pointer.x,
                pointer.y + round as f32 * 13.0,
            ))],
            _ => vec![moved(pointer)],
        }
    }

    /// Asserts what the round's events hit.
    pub fn check(self, round: usize, pointer: Point, rows: usize, seen: &Seen) {
        let name = self.name();
        let hit = |point: Point| seen.scene.hit_test(point);
        match self {
            Self::StillMove | Self::ResizeMove => {
                assert_eq!(
                    hit(pointer),
                    Some(row_id(ROW)),
                    "{name}: the row under the pointer"
                );
                assert_eq!(
                    seen.cursor,
                    Cursor::Pointer,
                    "{name}: the move reached that row"
                );
            }
            Self::SwapClick => {
                let first = if seen.probe.swapped.get() { 1 } else { 0 };
                assert_eq!(
                    seen.probe.clicked.take(),
                    [row_id(first)],
                    "{name}: the click hits the row now first"
                );
            }
            Self::ScrolledMove => {
                // The middle row is at the top, so the pointer is ROW rows below it.
                assert_eq!(
                    hit(pointer),
                    Some(row_id(rows / 2 + ROW)),
                    "{name}: the row scrolled under the pointer"
                );
                assert_eq!(
                    seen.cursor,
                    Cursor::Pointer,
                    "{name}: the move reached that row"
                );
            }
            Self::OverlayMove => {
                assert_eq!(
                    seen.scene.overlays().len(),
                    1,
                    "{name}: the popover is open"
                );
                assert_eq!(
                    hit(pointer),
                    Some(OVERLAY_BUTTON),
                    "{name}: the overlay is on top"
                );
                assert_eq!(
                    seen.cursor,
                    Cursor::Pointer,
                    "{name}: the move reached its button"
                );
            }
            Self::CaptureDrag => {
                let kinds: Vec<PointerEventKind> = seen
                    .probe
                    .canvas_events
                    .take()
                    .iter()
                    .map(|(kind, position)| {
                        if *kind != PointerEventKind::Down {
                            assert!(
                                position.y > PAD.1,
                                "{name}: {kind:?} at {position:?} is outside the canvas"
                            );
                        }
                        *kind
                    })
                    .collect();
                use PointerEventKind::{Down, Move, Up};
                assert_eq!(
                    kinds,
                    [Down, Move, Move, Up],
                    "{name}: the canvas keeps the pointer until the release"
                );
                assert_eq!(
                    hit(on_button(ROW + 2, PAD.1)),
                    Some(row_id(ROW + 2)),
                    "{name}: the release is over a row"
                );
                assert!(
                    seen.probe.clicked.borrow().is_empty(),
                    "{name}: the release over a row does not click it"
                );
            }
            Self::Grid { .. } => {
                assert_eq!(
                    hit(pointer),
                    Some(CANVAS),
                    "{name}: the grid under the pointer"
                );
                let events = seen.probe.canvas_events.take();
                assert_eq!(events.len(), 1, "{name}: the grid gets the move");
                assert_eq!(events[0].0, PointerEventKind::Move);
            }
            Self::List => {
                let moved_to = Point::new(pointer.x, pointer.y + round as f32 * 13.0);
                assert_eq!(
                    hit(moved_to),
                    None,
                    "{name}: the rows register no hit regions"
                );
                assert_eq!(seen.cursor, Cursor::Default);
                assert_eq!(
                    seen.root_bounds.height(),
                    rows as f32 * LIST_ROW_HEIGHT,
                    "{name}: every row is laid out"
                );
            }
        }
    }
}

fn moved(position: Point) -> PointerEvent {
    PointerEvent {
        kind: PointerEventKind::Move,
        position,
        button: None,
    }
}

fn press(kind: PointerEventKind, position: Point) -> PointerEvent {
    PointerEvent {
        kind,
        position,
        button: Some(MouseButton::Left),
    }
}

fn button(probe: &Rc<Probe>, index: usize) -> Button {
    let (probe, id) = (Rc::clone(probe), row_id(index));
    Button::new(format!("Row {index}"))
        .id(id)
        .on_click(move || probe.clicked.borrow_mut().push(id))
}

fn buttons(probe: &Rc<Probe>, rows: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..rows).map(|index| button(probe, index)))
}

/// A canvas that reports what it gets, above the rows of `canvas.capture_drag`.
fn pad(probe: &Rc<Probe>) -> Canvas {
    let (calls, events) = (Rc::clone(probe), Rc::clone(probe));
    canvas(move |bounds, painter| {
        calls.canvas_calls.set(calls.canvas_calls.get() + 1);
        painter.fill(bounds, Color::hex(0x1a202c));
    })
    .id(CANVAS)
    .w(PAD.0)
    .h(PAD.1)
    .on_pointer(move |event| {
        let mut events = events.canvas_events.borrow_mut();
        events.push((event.kind, event.position));
    })
}

/// The grid of `examples/canvas_grid.rs` as it was before it drew a glyph per
/// cell: a quad per cell and a text run per row, painted by one canvas. Kept so
/// pointer costs stay comparable across runs.
fn grid(probe: &Rc<Probe>, cached: bool) -> Canvas {
    let lines: Vec<SharedStr> = (0..GRID.1)
        .map(|row| {
            let line: String = (0..GRID.0)
                .map(|column| char::from(b'!' + ((row * 7 + column) % 94) as u8))
                .collect();
            SharedStr::new(Arc::from(line))
        })
        .collect();
    let (calls, events) = (Rc::clone(probe), Rc::clone(probe));
    let grid = canvas(move |bounds, painter| {
        calls.canvas_calls.set(calls.canvas_calls.get() + 1);
        for (row, line) in lines.iter().enumerate() {
            let y = bounds.y() + row as f32 * CELL.1;
            for column in 0..GRID.0 {
                let x = bounds.x() + column as f32 * CELL.0;
                let hue = column as f32 / GRID.0 as f32 * 360.0;
                let lightness = 0.15 + row as f32 / GRID.1 as f32 * 0.25;
                painter.fill(
                    Bounds::from_xywh(x, y, CELL.0, CELL.1),
                    Color::hsl(hue, 0.6, lightness),
                );
            }
            painter.paint(Primitive::Text {
                bounds: Bounds::from_xywh(bounds.x(), y, bounds.width(), CELL.1),
                content: line.clone(),
                color: Color::WHITE.to_rgba(),
                font_size: 12.0,
                font_weight: 400,
                font_family: Some("SF Mono".into()),
                line_height: 1.0,
                align: TextAlign::Left,
                direction: BaseDirection::Auto,
            });
        }
    })
    .id(CANVAS)
    .w(GRID.0 as f32 * CELL.0)
    .h(GRID.1 as f32 * CELL.1)
    .on_pointer(move |event| {
        let mut events = events.canvas_events.borrow_mut();
        events.push((event.kind, event.position));
    });
    if cached { grid.cache_key(0u8) } else { grid }
}

/// `perf_gate`'s long list: 28 px rows of a background and two texts, the
/// first row selected.
fn list(rows: usize) -> Div {
    div().w_full().flex_col().children((0..rows).map(|index| {
        let background = if index == 0 { 0x2d3748 } else { 0x1a202c };
        div()
            .w_full()
            .h(LIST_ROW_HEIGHT)
            .flex_row()
            .justify_between()
            .bg(Color::hex(background))
            .child(text(format!("task {index}: fix the failing check")))
            .child(text(format!("{} min", index % 60)))
    }))
}
