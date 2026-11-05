//! Pointer moves, releases and scrolls pass by subtrees with no handlers in
//! them; everything with a handler still gets them, and presses still clear
//! focus as they did.

use rui::core::event::MouseButton;
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::{Color, Div, Point, Presenter, Size, div, input, text};
use std::cell::Cell;
use std::rc::Rc;

const VIEWPORT: Size = Size {
    width: 320.0,
    height: 2_000.0,
};

#[derive(Default)]
struct Seen {
    clicks: [Cell<u32>; 2],
    hovered: Cell<Option<bool>>,
}

/// 100 rows 20 px tall. Rows 10 and 50 take clicks, row 20 reports hover, row
/// 30 holds an input; the rest have no handlers.
fn rows(seen: &Rc<Seen>) -> Div {
    div().w(320.0).flex_col().children((0..100).map(|row| {
        let line = div()
            .h(20.0)
            .bg(Color::hex(0x334455))
            .child(text(format!("row {row}")));
        match row {
            10 | 50 => {
                let seen = Rc::clone(seen);
                line.on_click(move || {
                    let slot = &seen.clicks[usize::from(row == 50)];
                    slot.set(slot.get() + 1);
                })
            }
            20 => {
                let seen = Rc::clone(seen);
                line.on_hover(move |hovered| seen.hovered.set(Some(hovered)))
            }
            30 => div().h(20.0).child(input().w(200.0).h(20.0)),
            _ => line,
        }
    }))
}

fn presenter(seen: &Rc<Seen>) -> Presenter<Div> {
    let mut presenter = Presenter::with_root(VIEWPORT, rows(seen));
    // Two frames: the second keeps the layout records dispatch reads.
    for _ in 0..2 {
        if let Err(err) = presenter.layout(VIEWPORT) {
            panic!("layout failed: {err}");
        }
        presenter.paint();
    }
    presenter
}

fn send(presenter: &mut Presenter<Div>, kind: PointerEventKind, y: f32) {
    presenter.dispatch_pointer_event(&PointerEvent {
        kind,
        position: Point::new(40.0, y),
        button: (!matches!(kind, PointerEventKind::Move)).then_some(MouseButton::Left),
    });
}

fn click(presenter: &mut Presenter<Div>, y: f32) {
    send(presenter, PointerEventKind::Down, y);
    send(presenter, PointerEventKind::Up, y);
}

#[test]
fn rows_with_handlers_still_get_clicks_and_hover() {
    let seen = Rc::new(Seen::default());
    let mut presenter = presenter(&seen);

    click(&mut presenter, 10.0 * 20.0 + 5.0);
    click(&mut presenter, 11.0 * 20.0 + 5.0);
    click(&mut presenter, 50.0 * 20.0 + 5.0);
    click(&mut presenter, 50.0 * 20.0 + 5.0);
    assert_eq!(seen.clicks[0].get(), 1, "row 10 is clicked once");
    assert_eq!(seen.clicks[1].get(), 2, "row 50 is clicked twice");

    send(&mut presenter, PointerEventKind::Move, 20.0 * 20.0 + 5.0);
    assert_eq!(seen.hovered.get(), Some(true));
    send(&mut presenter, PointerEventKind::Move, 70.0 * 20.0 + 5.0);
    assert_eq!(seen.hovered.get(), Some(false));
}

#[test]
fn a_press_on_a_row_without_handlers_still_clears_focus() {
    let seen = Rc::new(Seen::default());
    let mut presenter = presenter(&seen);

    click(&mut presenter, 30.0 * 20.0 + 5.0);
    assert!(
        presenter.focused_element().is_some(),
        "clicking the input focuses it"
    );
    // Moves over rows without handlers change nothing.
    send(&mut presenter, PointerEventKind::Move, 60.0 * 20.0 + 5.0);
    assert!(presenter.focused_element().is_some());
    click(&mut presenter, 60.0 * 20.0 + 5.0);
    assert_eq!(
        presenter.focused_element(),
        None,
        "a press elsewhere clears focus"
    );
}
