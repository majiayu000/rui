//! Selecting and copying static text in a selection scope.

use rui::AppContext;
use rui::advanced_ui::Theme;
use rui::core::color::Rgba;
use rui::core::event::{Cursor, KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::{Point, Size};
use rui::elements::element::{Element, PointerEvent, PointerEventKind};
use rui::elements::{SelectionScope, div, selection_scope, text};
use rui::renderer::Primitive;
use rui::testing::{HeadlessSession, mount};

fn text_bounds(primitives: &[Primitive], needle: &str) -> Bounds {
    primitives
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Text {
                content, bounds, ..
            } if content.contains(needle) => Some(*bounds),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{needle} is drawn"))
}

fn selection_quads(primitives: &[Primitive]) -> Vec<Bounds> {
    let color = Theme::default().state.selection.to_rgba();
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad {
                bounds, background, ..
            } if same(*background, color) => Some(*bounds),
            _ => None,
        })
        .collect()
}

fn same(a: Rgba, b: Rgba) -> bool {
    a.to_array() == b.to_array()
}

/// Near the left edge of the first glyph of `needle`, mid-line.
fn left_of<F, E>(session: &HeadlessSession<F, E>, needle: &str) -> Point
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let bounds = text_bounds(session.primitives(), needle);
    Point::new(bounds.x() + 0.5, bounds.y() + bounds.height() * 0.5)
}

fn drag<F, E>(session: &mut HeadlessSession<F, E>, from: Point, to: Point)
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    session.pointer_down(from);
    session.pointer_move(Point::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5));
    session.pointer_move(to);
    session.pointer_up(to);
}

fn click<F, E>(session: &mut HeadlessSession<F, E>, at: Point, times: usize)
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    for _ in 0..times {
        session.pointer_down(at);
        session.pointer_up(at);
    }
}

fn copy<F, E>(session: &mut HeadlessSession<F, E>) -> Option<String>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    session
        .app_context()
        .write_to_clipboard(rui::core::text_editing::ClipboardItem::new_string(""))
        .expect("clear clipboard");
    session.dispatch_key_event(&KeyEvent::new(KeyCode::C, Modifiers::meta()));
    session
        .app_context()
        .read_from_clipboard()
        .expect("clipboard")
        .map(|item| item.text().to_string())
        .filter(|text| !text.is_empty())
}

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog and keeps running far away";

#[test]
fn a_drag_selects_across_wrapped_lines_and_copies_it() {
    let mut session = mount(Size::new(400.0, 300.0), |_| {
        div()
            .w(160.0)
            .p(10.0)
            .child(selection_scope(text(PARAGRAPH).wrap()))
    })
    .expect("mount");
    assert!(selection_quads(session.primitives()).is_empty());
    let from = left_of(&session, "The");
    // The start of the line "lazy" is drawn on.
    let to = left_of(&session, "lazy");
    assert!(to.y > from.y, "the paragraph wraps");
    let line = session
        .primitives()
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Text { content, .. } if content.contains("lazy") => {
                Some(content.to_string())
            }
            _ => None,
        })
        .expect("the line is drawn");
    let before_line = &PARAGRAPH[..PARAGRAPH.find(&line).expect("a line of the paragraph")];
    drag(&mut session, from, to);
    assert_eq!(copy(&mut session).as_deref(), Some(before_line));
    assert_eq!(selection_quads(session.primitives()).len(), 1);

    // Dragging back above the start selects backwards from the press.
    drag(&mut session, to, Point::new(from.x, from.y - 40.0));
    assert_eq!(copy(&mut session).as_deref(), Some(before_line));

    // Past the end of the last line selects to the end.
    drag(&mut session, from, Point::new(390.0, 290.0));
    assert_eq!(copy(&mut session).as_deref(), Some(PARAGRAPH));
    let lines = selection_quads(session.primitives());
    assert!(lines.len() >= 3, "one highlight per line: {lines:?}");
    assert!(lines.windows(2).all(|pair| pair[1].y() > pair[0].y()));
}

#[test]
fn double_click_selects_a_word_and_triple_click_a_paragraph() {
    let mut session = mount(Size::new(400.0, 200.0), |_| {
        div().w(400.0).child(selection_scope(
            text("alpha beta gamma\nsecond line").wrap(),
        ))
    })
    .expect("mount");
    // The middle of the first line is in "beta".
    let bounds = text_bounds(session.primitives(), "alpha");
    let in_beta = Point::new(bounds.x() + bounds.width() * 0.5, bounds.y() + 4.0);
    click(&mut session, in_beta, 2);
    assert_eq!(copy(&mut session).as_deref(), Some("beta"));
    // Presses elsewhere start counting again.
    click(&mut session, Point::new(in_beta.x, in_beta.y + 30.0), 1);
    click(&mut session, in_beta, 3);
    assert_eq!(copy(&mut session).as_deref(), Some("alpha beta gamma"));
}

#[test]
fn a_drag_crosses_texts_in_document_order_and_shift_click_extends_it() {
    let mut session = mount(Size::new(400.0, 300.0), |_| {
        selection_scope(
            div()
                .w(400.0)
                .flex_col()
                .gap(8.0)
                .child(text("First block"))
                .child(div().h(20.0).bg(rui::core::color::Color::WHITE))
                .child(text("Second block"))
                .child(text("Third block")),
        )
    })
    .expect("mount");
    let first = left_of(&session, "First");
    let second = text_bounds(session.primitives(), "Second");
    let in_second = Point::new(second.x() + second.width() * 0.6, second.y() + 4.0);
    drag(&mut session, first, in_second);
    let copied = copy(&mut session).expect("a selection");
    assert!(copied.starts_with("First block\nSec"), "{copied:?}");
    assert!(!copied.contains("Third"), "{copied:?}");

    // Shift-click past the last block extends from the same start.
    let third = text_bounds(session.primitives(), "Third");
    let after = Point::new(third.x() + third.width() + 20.0, third.y() + 4.0);
    session.dispatch_pointer_event_with(press(PointerEventKind::Down, after), Modifiers::shift());
    session.dispatch_pointer_event_with(press(PointerEventKind::Up, after), Modifiers::shift());
    assert_eq!(
        copy(&mut session).as_deref(),
        Some("First block\nSecond block\nThird block")
    );

    // A press outside the scope drops the selection.
    click(&mut session, Point::new(390.0, 290.0), 1);
    assert!(selection_quads(session.primitives()).is_empty());
}

fn press(kind: PointerEventKind, position: Point) -> PointerEvent {
    PointerEvent {
        kind,
        position,
        button: Some(rui::core::event::MouseButton::Left),
    }
}

#[test]
fn select_all_selects_the_focused_scope_only() {
    let mut session = mount(Size::new(400.0, 300.0), |_| {
        div()
            .w(400.0)
            .flex_col()
            .child(selection_scope(
                div().flex_col().child(text("One")).child(text("Two")),
            ))
            .child(selection_scope(text("Elsewhere")))
    })
    .expect("mount");
    // Nothing focused: select all selects nothing here.
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta()));
    assert_eq!(copy(&mut session), None);

    let two = left_of(&session, "Two");
    click(&mut session, two, 1);
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta()));
    assert_eq!(copy(&mut session).as_deref(), Some("One\nTwo"));
    assert_eq!(selection_quads(session.primitives()).len(), 2);
}

#[test]
fn text_outside_a_scope_is_not_selectable_and_unselectable_text_is_skipped() {
    let mut session = mount(Size::new(400.0, 300.0), |_| {
        div()
            .w(400.0)
            .flex_col()
            .child(text("Plain text"))
            .child(selection_scope(
                div()
                    .flex_row()
                    .gap(4.0)
                    .child(text("*").unselectable())
                    .child(text("Item")),
            ))
    })
    .expect("mount");
    let plain = left_of(&session, "Plain");
    drag(&mut session, plain, Point::new(300.0, plain.y));
    assert_eq!(copy(&mut session), None);
    assert!(selection_quads(session.primitives()).is_empty());

    let star = left_of(&session, "*");
    drag(
        &mut session,
        Point::new(star.x - 0.4, star.y),
        Point::new(300.0, star.y),
    );
    assert_eq!(copy(&mut session).as_deref(), Some("Item"));
}

#[test]
fn text_selectable_on_its_own_shows_an_i_beam() {
    let mut session = mount(Size::new(400.0, 100.0), |_| {
        div().w(400.0).child(text("Copy me please").selectable())
    })
    .expect("mount");
    let start = left_of(&session, "Copy");
    session.pointer_move(start);
    assert_eq!(session.cursor(), Cursor::Text);
    click(&mut session, Point::new(start.x + 2.0, start.y), 2);
    assert_eq!(copy(&mut session).as_deref(), Some("Copy"));
}

#[test]
fn right_to_left_and_thai_text_select_whole_words() {
    let mut session = mount(Size::new(400.0, 200.0), |_| {
        selection_scope(
            div()
                .w(400.0)
                .flex_col()
                .child(text("مرحبا بالعالم"))
                .child(text("สวัสดีชาวโลก")),
        )
    })
    .expect("mount");
    let arabic = text_bounds(session.primitives(), "مرحبا");
    // Right to left: the first word is at the right end.
    click(
        &mut session,
        Point::new(
            arabic.x() + arabic.width() - 4.0,
            arabic.y() + arabic.height() * 0.5,
        ),
        2,
    );
    assert_eq!(copy(&mut session).as_deref(), Some("مرحبا"));

    // A drag over all of both lines copies both whole.
    let thai = text_bounds(session.primitives(), "สวัสดี");
    drag(
        &mut session,
        Point::new(arabic.x() + arabic.width() + 10.0, arabic.y() + 2.0),
        Point::new(
            thai.x() + thai.width() + 10.0,
            thai.y() + thai.height() * 0.5,
        ),
    );
    assert_eq!(
        copy(&mut session).as_deref(),
        Some("مرحبا بالعالم\nสวัสดีชาวโลก")
    );
}

#[test]
fn an_overlay_opened_in_a_scope_is_not_part_of_its_selection() {
    use rui::advanced_ui as ui;

    let mut session = mount(Size::new(400.0, 300.0), |_| {
        selection_scope(
            div()
                .w(400.0)
                .flex_col()
                .gap(8.0)
                .child(text("Before"))
                .child(
                    ui::dialog("Details", text("Floating"))
                        .modal(false)
                        .open(true),
                )
                .child(text("After")),
        )
    })
    .expect("mount");
    assert_eq!(session.scene().overlays().len(), 1, "the dialog is open");
    let before = left_of(&session, "Before");
    let after = text_bounds(session.primitives(), "After");
    drag(
        &mut session,
        before,
        Point::new(after.x() + after.width() + 20.0, after.y() + 4.0),
    );
    assert_eq!(copy(&mut session).as_deref(), Some("Before\nAfter"));
    session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta()));
    assert_eq!(copy(&mut session).as_deref(), Some("Before\nAfter"));
    let highlighted = selection_quads(session.primitives()).len();
    assert_eq!(highlighted, 2, "nothing in the overlay is highlighted");

    // A press in the overlay leaves the scope's selection alone.
    let overlay = *session
        .scene()
        .overlays()
        .first()
        .expect("the dialog stays open");
    let inside = Point::new(
        overlay.bounds.x() + overlay.bounds.width() * 0.5,
        overlay.bounds.y() + overlay.bounds.height() * 0.5,
    );
    click(&mut session, inside, 1);
    assert_eq!(selection_quads(session.primitives()).len(), highlighted);
}

/// Selects the text of a scope, opens the popover beside it, and presses the
/// button in the scope; `place` puts the scope in the window.
fn press_a_control_beside_an_open_popover<E: Element>(place: fn(SelectionScope) -> E) {
    use rui::advanced_ui as ui;
    use std::cell::Cell;
    use std::rc::Rc;

    let (open, dismissed) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(0)));
    let (is_open, on_dismiss) = (Rc::clone(&open), Rc::clone(&dismissed));
    let mut session = mount(Size::new(400.0, 300.0), move |_| {
        let on_dismiss = Rc::clone(&on_dismiss);
        place(selection_scope(
            div()
                .flex_col()
                .p(8.0)
                .gap(60.0)
                .child(text("Selected words"))
                .child(
                    ui::popover("Details", ui::button("Anchor"), text("Floating"))
                        .open(is_open.get())
                        .on_dismiss(move || on_dismiss.set(on_dismiss.get() + 1)),
                )
                .child(ui::button("Done")),
        ))
    })
    .expect("mount");
    let words = text_bounds(session.primitives(), "Selected words");
    let from = left_of(&session, "Selected");
    drag(
        &mut session,
        from,
        Point::new(words.x() + words.width() + 10.0, from.y),
    );
    open.set(true);
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame that opens the popover");
    assert_eq!(session.scene().overlays().len(), 1, "the popover is open");
    assert_eq!(selection_quads(session.primitives()).len(), 1);

    let done = text_bounds(session.primitives(), "Done");
    click(
        &mut session,
        Point::new(done.x() + 2.0, done.y() + done.height() * 0.5),
        1,
    );
    session.frame().expect("frame after the press");
    assert_eq!(dismissed.get(), 1, "the press closed the popover");
    assert_eq!(
        selection_quads(session.primitives()).len(),
        1,
        "a press on a control in the scope leaves its selection"
    );
}

#[test]
fn a_press_that_closes_a_popover_in_a_scope_leaves_the_selection() {
    // The scope as the window's root, and inside a page.
    press_a_control_beside_an_open_popover(|scope| scope);
    press_a_control_beside_an_open_popover(|scope| div().child(scope));
}

#[test]
fn a_drag_held_while_the_text_it_started_in_goes_away_selects_nothing() {
    use std::cell::Cell;
    use std::rc::Rc;

    let shown = Rc::new(Cell::new(3));
    let count = Rc::clone(&shown);
    let mut session = mount(Size::new(400.0, 300.0), move |_| {
        let blocks = ["First block", "Second block", "Third block"];
        selection_scope(
            div()
                .w(400.0)
                .flex_col()
                .children(blocks.into_iter().take(count.get()).map(text)),
        )
    })
    .expect("mount");
    let third = text_bounds(session.primitives(), "Third");
    let from = left_of(&session, "Third");
    let to = Point::new(third.x() + third.width() * 0.5, from.y);
    session.pointer_down(from);
    session.pointer_move(to);
    session.frame().expect("frame with the selection");
    assert_eq!(selection_quads(session.primitives()).len(), 1);

    // The button is still down when the last two blocks are removed.
    shown.set(1);
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame without the blocks");
    assert!(selection_quads(session.primitives()).is_empty());
    session.pointer_move(Point::new(to.x + 10.0, to.y));
    session.pointer_up(Point::new(to.x + 10.0, to.y));
    session.frame().expect("frame after the release");
    assert_eq!(copy(&mut session), None);
    assert!(selection_quads(session.primitives()).is_empty());

    // The next drag selects from where it presses.
    let first = text_bounds(session.primitives(), "First");
    let from = left_of(&session, "First");
    drag(
        &mut session,
        from,
        Point::new(first.x() + first.width() + 10.0, from.y),
    );
    assert_eq!(copy(&mut session).as_deref(), Some("First block"));
}
