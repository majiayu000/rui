use rui::advanced_ui::Button;
use rui::core::event::{Modifiers, ScrollEvent};
use rui::core::{ElementId, Point, Size, View, ViewContext};
use rui::elements::{VirtualList, VirtualListHandle, virtual_list};
use rui::testing::mount_view;
use std::cell::Cell;
use std::rc::Rc;

const ROW: f32 = 20.0;

type Session = rui::testing::HeadlessViewSession<Rows>;
type Mounted = (Session, Rc<Cell<usize>>, Rc<Cell<Option<usize>>>);

fn row_id(index: usize) -> ElementId {
    ElementId::from(1_000_000 + index as u64)
}

struct Rows {
    count: usize,
    handle: VirtualListHandle,
    built: Rc<Cell<usize>>,
    clicked: Rc<Cell<Option<usize>>>,
}

impl View for Rows {
    type Element = VirtualList;

    fn render(&mut self, _cx: &mut ViewContext<Self>) -> Self::Element {
        let built = Rc::clone(&self.built);
        let clicked = Rc::clone(&self.clicked);
        virtual_list(&self.handle, self.count, ROW, move |index| {
            built.set(built.get() + 1);
            let clicked = Rc::clone(&clicked);
            Button::new(format!("row {index}"))
                .id(row_id(index))
                .on_click(move || clicked.set(Some(index)))
        })
        .w(200.0)
        .h(200.0)
    }
}

fn mount(count: usize, handle: &VirtualListHandle) -> Mounted {
    let built = Rc::new(Cell::new(0));
    let clicked = Rc::new(Cell::new(None));
    let view = Rows {
        count,
        handle: handle.clone(),
        built: Rc::clone(&built),
        clicked: Rc::clone(&clicked),
    };
    let session = mount_view(Size::new(400.0, 400.0), view).expect("mount view");
    (session, built, clicked)
}

fn scroll(session: &mut Session, delta_y: f32) -> bool {
    session.dispatch_scroll_event(&ScrollEvent {
        position: Point::new(50.0, 50.0),
        delta_x: 0.0,
        delta_y,
        modifiers: Modifiers::default(),
    })
}

fn has_row(session: &Session, index: usize) -> bool {
    session
        .accessibility_tree()
        .expect("accessibility tree")
        .find(row_id(index))
        .is_some()
}

#[test]
fn only_rows_in_view_are_built_however_long_the_list() {
    let handle = VirtualListHandle::new();
    let (mut session, built, _) = mount(100_000, &handle);
    built.set(0);
    session.frame().expect("frame");
    // 200 px of 20 px rows, plus overscan on each side.
    assert!(built.get() <= 14, "built {} rows", built.get());
    assert!(has_row(&session, 0));
    assert!(!has_row(&session, 50));
}

#[test]
fn scrolling_moves_the_built_rows_and_clicks_hit_the_row_under_the_pointer() {
    let handle = VirtualListHandle::new();
    let (mut session, _, clicked) = mount(1_000, &handle);
    assert!(scroll(&mut session, 1_000.0));
    session.frame().expect("frame after scroll");
    assert_eq!(handle.offset(), 1_000.0);
    assert!(has_row(&session, 55));
    assert!(!has_row(&session, 0));

    // y = 30 inside the list is offset 1030, row 51.
    assert!(session.pointer_down(Point::new(10.0, 30.0)));
    assert!(session.pointer_up(Point::new(10.0, 30.0)));
    assert_eq!(clicked.get(), Some(51));
}

#[test]
fn scroll_position_survives_rebuilds_and_is_clamped_when_data_shrinks() {
    let handle = VirtualListHandle::new();
    {
        let (mut session, _, _) = mount(1_000, &handle);
        assert!(scroll(&mut session, 5_000.0));
        session.frame().expect("frame after scroll");
    }
    assert_eq!(handle.offset(), 5_000.0);

    // A new tree for the same data keeps the position.
    let (session, _, _) = mount(1_000, &handle);
    assert_eq!(handle.offset(), 5_000.0);
    assert!(has_row(&session, 255));
    drop(session);

    // Fewer rows than the position needs: it clamps to the last full view.
    let (session, _, _) = mount(100, &handle);
    assert_eq!(handle.offset(), 100.0 * ROW - 200.0);
    assert!(has_row(&session, 99));
}

#[test]
fn scrolling_past_either_end_is_not_handled() {
    let handle = VirtualListHandle::new();
    let (mut session, _, _) = mount(5, &handle);
    assert!(
        !scroll(&mut session, 40.0),
        "content shorter than the list cannot scroll"
    );
    let (mut session, _, _) = mount(50, &handle);
    assert!(!scroll(&mut session, -10.0));
}

/// The scrollbar's track and thumb, as painted: quads along the list's right
/// edge, the 200 px list's x 192 to 200.
fn scrollbar(session: &Session) -> Option<(rui::Bounds, rui::Bounds)> {
    let quads: Vec<rui::Bounds> = session
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            rui::renderer::Primitive::Quad { bounds, .. }
                if bounds.x() == 192.0 && bounds.width() == 8.0 =>
            {
                Some(*bounds)
            }
            _ => None,
        })
        .collect();
    match quads.as_slice() {
        [] => None,
        [track, thumb] => Some((*track, *thumb)),
        other => panic!("unexpected scrollbar quads: {other:?}"),
    }
}

fn press(session: &mut Session, position: Point) {
    assert!(session.pointer_down(position));
}

#[test]
fn the_scrollbar_shows_position_and_size_and_hides_when_rows_fit() {
    let handle = VirtualListHandle::new();
    let (mut session, _, _) = mount(40, &handle);
    // 200 px of 800: the thumb is a quarter of the track, at the top.
    let (track, thumb) = scrollbar(&session).expect("rows overflow");
    assert_eq!((track.y(), track.height()), (0.0, 200.0));
    assert_eq!((thumb.y(), thumb.height()), (0.0, 50.0));

    handle.scroll_to(300.0);
    session.frame().expect("frame");
    let (_, thumb) = scrollbar(&session).expect("rows overflow");
    // 300 of 600 scrolled: halfway along the 150 px the thumb can travel.
    assert_eq!(thumb.y(), 75.0);

    let (session, _, _) = mount(5, &VirtualListHandle::new());
    assert!(scrollbar(&session).is_none(), "five rows fit");
}

#[test]
fn a_long_list_keeps_a_thumb_that_can_be_grabbed() {
    let handle = VirtualListHandle::new();
    let (mut session, _, _) = mount(100_000, &handle);
    let (_, thumb) = scrollbar(&session).expect("rows overflow");
    assert_eq!(thumb.height(), 20.0, "a tenth of the track at least");

    // Dragging the thumb to the bottom reaches the last row.
    press(&mut session, Point::new(196.0, 5.0));
    session.pointer_move(Point::new(196.0, 400.0));
    session.pointer_up(Point::new(196.0, 400.0));
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 100_000.0 * ROW - 200.0);
    assert!(has_row(&session, 99_999));
}

#[test]
fn dragging_the_thumb_scrolls_and_clicks_no_row() {
    let handle = VirtualListHandle::new();
    let (mut session, _, clicked) = mount(40, &handle);
    // Take the 50 px thumb 10 px from its top and move it down 75 px.
    press(&mut session, Point::new(196.0, 10.0));
    assert!(handle.is_dragging());
    session.pointer_move(Point::new(196.0, 85.0));
    session.frame().expect("frame while dragging");
    assert_eq!(handle.offset(), 300.0);
    let (_, thumb) = scrollbar(&session).expect("rows overflow");
    assert_eq!(thumb.y(), 75.0, "the thumb follows the pointer");

    // The drag goes on outside the list, and ends at the release.
    session.pointer_move(Point::new(300.0, 500.0));
    assert_eq!(handle.offset(), 600.0);
    session.pointer_up(Point::new(300.0, 500.0));
    assert!(!handle.is_dragging());
    session.pointer_move(Point::new(196.0, 10.0));
    assert_eq!(handle.offset(), 600.0);
    assert_eq!(clicked.get(), None);
}

#[test]
fn a_press_on_the_track_moves_a_page_toward_it() {
    let handle = VirtualListHandle::new();
    let (mut session, _, clicked) = mount(40, &handle);
    press(&mut session, Point::new(196.0, 150.0));
    session.pointer_up(Point::new(196.0, 150.0));
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 200.0);
    assert!(has_row(&session, 15));

    // Above the thumb, now at 50 to 100: back a page.
    press(&mut session, Point::new(196.0, 20.0));
    session.pointer_up(Point::new(196.0, 20.0));
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 0.0);
    assert_eq!(clicked.get(), None, "the rows under the track see nothing");

    // Left of the track, the row gets the click.
    press(&mut session, Point::new(100.0, 30.0));
    session.pointer_up(Point::new(100.0, 30.0));
    assert_eq!(clicked.get(), Some(1));
}

#[test]
fn scroll_to_item_moves_as_little_as_it_needs() {
    let handle = VirtualListHandle::new();
    let (mut session, _, _) = mount(1_000, &handle);

    // Below the view: the row comes to its bottom.
    handle.scroll_to_item(500);
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 501.0 * ROW - 200.0);
    assert!(has_row(&session, 500));

    // Already in view: nothing moves.
    handle.scroll_to_item(495);
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 501.0 * ROW - 200.0);

    // Above the view: the row comes to its top.
    handle.scroll_to_item(10);
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 10.0 * ROW);

    // Past the end: the last row.
    handle.scroll_to_item(5_000);
    session.frame().expect("frame");
    assert_eq!(handle.offset(), 1_000.0 * ROW - 200.0);
    assert!(has_row(&session, 999));
}
