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
