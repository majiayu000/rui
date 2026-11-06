use rui::core::color::{Color, Rgba};
use rui::core::drag::{ExternalPaths, FileDragEvent};
use rui::core::{AppContext, Bounds, Point, Size, View, ViewContext};
use rui::elements::{Div, Element, div};
use rui::renderer::Primitive;
use rui::testing::{HeadlessSession, mount, mount_view};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
struct Card(&'static str);

#[derive(Default)]
struct Log {
    dropped: RefCell<Vec<String>>,
    over: RefCell<Vec<bool>>,
    clicks: RefCell<u32>,
    files: RefCell<Vec<Vec<PathBuf>>>,
}

const SOURCE: Point = Point { x: 10.0, y: 10.0 };
const TARGET: Point = Point { x: 10.0, y: 90.0 };
const OUTSIDE: Point = Point { x: 10.0, y: 160.0 };
const PREVIEW: Color = Color::RED;

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(200.0, 200.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless drag session should mount: {err}"),
    }
}

/// A source card at the top and a target below it, both 200 x 60.
fn board(log: &Rc<Log>, target: impl Fn(Div, Rc<Log>) -> Div) -> Div {
    let clicks = Rc::clone(log);
    div()
        .flex_col()
        .child(
            div()
                .w(200.0)
                .h(60.0)
                .on_click(move || *clicks.clicks.borrow_mut() += 1)
                .on_drag(|| Card("card-1"), |_| div().w(40.0).h(20.0).bg(PREVIEW)),
        )
        .child(div().h(20.0))
        .child(target(div().w(200.0).h(60.0), Rc::clone(log)))
}

fn card_target(target: Div, log: Rc<Log>) -> Div {
    let over = Rc::clone(&log);
    target
        .on_drop::<Card>(move |card| log.dropped.borrow_mut().push(card.0.to_owned()))
        .on_drag_over::<Card>(move |is_over| over.over.borrow_mut().push(is_over))
}

fn preview_bounds<F, E>(session: &HeadlessSession<F, E>) -> Option<Bounds>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let red: Rgba = PREVIEW.to_rgba();
    session
        .primitives()
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Quad {
                bounds, background, ..
            } if *background == red => Some(*bounds),
            _ => None,
        })
}

#[test]
fn dragging_a_value_onto_a_target_drops_it_with_hover_state_and_a_preview() {
    let log = Rc::new(Log::default());
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| board(&tree_log, card_target));

    assert!(session.pointer_down(SOURCE));
    assert!(!session.is_dragging(), "a press alone is not a drag");
    session.pointer_move(Point::new(12.0, 30.0));
    assert!(session.is_dragging());
    if let Err(err) = session.frame() {
        panic!("frame during the drag failed: {err}");
    }
    // The preview keeps the grab point under the pointer: pressed 10 x 10 into
    // the source, so its corner is 10 x 10 above and left of the pointer.
    assert_eq!(
        preview_bounds(&session),
        Some(Bounds::from_xywh(2.0, 20.0, 40.0, 20.0))
    );

    session.pointer_move(TARGET);
    assert_eq!(log.over.borrow().as_slice(), [true]);
    session.pointer_up(TARGET);

    assert_eq!(log.dropped.borrow().as_slice(), ["card-1".to_owned()]);
    assert_eq!(log.over.borrow().as_slice(), [true, false]);
    assert_eq!(*log.clicks.borrow(), 0, "a drag does not click its source");
    assert!(!session.is_dragging());
    if let Err(err) = session.frame() {
        panic!("frame after the drop failed: {err}");
    }
    assert_eq!(
        preview_bounds(&session),
        None,
        "the preview goes with the drag"
    );
}

#[test]
fn a_press_and_release_without_moving_clicks_instead_of_dragging() {
    let log = Rc::new(Log::default());
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| board(&tree_log, card_target));

    session.pointer_down(SOURCE);
    session.pointer_move(Point::new(11.0, 11.0));
    session.pointer_up(Point::new(11.0, 11.0));

    assert_eq!(*log.clicks.borrow(), 1);
    assert!(log.dropped.borrow().is_empty());
    assert!(log.over.borrow().is_empty());
}

#[test]
fn releasing_away_from_targets_or_over_another_type_drops_nothing() {
    let log = Rc::new(Log::default());
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| {
        board(&tree_log, |target, log| {
            let over = Rc::clone(&log);
            target
                .on_drop::<u32>(move |value| log.dropped.borrow_mut().push(value.to_string()))
                .on_drag_over::<u32>(move |is_over| over.over.borrow_mut().push(is_over))
        })
    });

    session.pointer_down(SOURCE);
    session.pointer_move(TARGET);
    session.pointer_up(TARGET);
    assert!(log.dropped.borrow().is_empty(), "a Card is not a u32");
    assert!(log.over.borrow().is_empty());

    session.pointer_down(SOURCE);
    session.pointer_move(OUTSIDE);
    session.pointer_up(OUTSIDE);
    assert!(log.dropped.borrow().is_empty());
    assert!(!session.is_dragging());
}

fn file_target(target: Div, log: Rc<Log>) -> Div {
    let over = Rc::clone(&log);
    target
        .on_drop_files(move |paths| log.files.borrow_mut().push(paths.to_vec()))
        .on_drag_over::<ExternalPaths>(move |is_over| over.over.borrow_mut().push(is_over))
}

#[test]
fn files_dragged_in_from_another_app_drop_on_the_target_under_them() {
    let log = Rc::new(Log::default());
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| board(&tree_log, file_target));
    let paths = vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b.png")];

    session.dispatch_file_drag(&FileDragEvent::Entered {
        position: SOURCE,
        paths: paths.clone(),
    });
    assert!(session.is_dragging());
    assert!(log.over.borrow().is_empty());
    session.dispatch_file_drag(&FileDragEvent::Moved { position: TARGET });
    assert_eq!(log.over.borrow().as_slice(), [true]);
    session.dispatch_file_drag(&FileDragEvent::Dropped { position: TARGET });

    assert_eq!(log.files.borrow().as_slice(), [paths]);
    assert_eq!(log.over.borrow().as_slice(), [true, false]);
    assert_eq!(*log.clicks.borrow(), 0);
    assert!(!session.is_dragging());
}

#[test]
fn files_that_leave_the_window_clear_the_hover_state_without_dropping() {
    let log = Rc::new(Log::default());
    let tree_log = Rc::clone(&log);
    let mut session = mount_or_panic(move |_| board(&tree_log, file_target));

    session.dispatch_file_drag(&FileDragEvent::Entered {
        position: TARGET,
        paths: vec![PathBuf::from("/tmp/a.txt")],
    });
    assert_eq!(log.over.borrow().as_slice(), [true]);
    session.dispatch_file_drag(&FileDragEvent::Exited);

    assert_eq!(log.over.borrow().as_slice(), [true, false]);
    assert!(log.files.borrow().is_empty());
    assert!(!session.is_dragging());
}

/// A view whose drop target's highlight is view state, so hovering re-renders.
struct Board {
    over: bool,
    log: Rc<Log>,
}

impl View for Board {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        self.log.over.borrow_mut().push(self.over);
        let target = div()
            .w(200.0)
            .h(60.0)
            .bg(if self.over { Color::BLUE } else { Color::WHITE })
            .on_drag_over::<Card>(cx.listener_with(|this, over, _| this.over = over))
            .on_drop::<Card>({
                let drop = cx.listener_with(|this: &mut Board, card: &'static str, _| {
                    this.log.dropped.borrow_mut().push(card.to_owned())
                });
                move |card| drop(card.0)
            });
        div()
            .flex_col()
            .child(
                div()
                    .w(200.0)
                    .h(60.0)
                    .on_drag(|| Card("card-2"), |_| div().w(40.0).h(20.0).bg(PREVIEW)),
            )
            .child(div().h(20.0))
            .child(target)
    }
}

#[test]
fn a_drag_survives_the_rebuilds_its_hover_state_causes() {
    let log = Rc::new(Log::default());
    let view = Board {
        over: false,
        log: Rc::clone(&log),
    };
    let mut session = match mount_view(Size::new(200.0, 200.0), view) {
        Ok(session) => session,
        Err(err) => panic!("board view should mount: {err}"),
    };
    let frame = |session: &mut rui::testing::HeadlessViewSession<Board>| {
        if let Err(err) = session.frame() {
            panic!("board frame failed: {err}");
        }
    };

    session.pointer_down(SOURCE);
    session.pointer_move(Point::new(10.0, 40.0));
    session.pointer_move(TARGET);
    frame(&mut session);
    session.pointer_move(Point::new(20.0, 95.0));
    frame(&mut session);
    assert!(session.is_dragging(), "rebuilding the view keeps the drag");
    assert_eq!(log.over.borrow().last(), Some(&true));

    session.pointer_up(TARGET);
    frame(&mut session);
    assert_eq!(log.dropped.borrow().as_slice(), ["card-2".to_owned()]);
    assert_eq!(log.over.borrow().last(), Some(&false));
    assert!(!session.is_dragging());
}
