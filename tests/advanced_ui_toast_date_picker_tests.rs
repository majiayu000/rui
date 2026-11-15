//! Toasts and the date picker driven through a mounted view: pointer and key
//! events in, view state and accessibility out.

use rui::advanced_ui::{self as ui, CalendarDate, Toast, Toaster, Weekday};
use rui::core::accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::Element;
use rui::testing::{HeadlessSession, mount_view};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const SAVE_ID: ElementId = ElementId(32_001);
const PICKER_ID: ElementId = ElementId(32_002);

fn key(key: KeyCode) -> KeyEvent {
    KeyEvent::new(key, Modifiers::none())
}

fn press<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &mut HeadlessSession<F, E>,
    code: KeyCode,
) {
    session.dispatch_key_event(&key(code));
    session.frame().expect("frame after key");
}

fn click<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &mut HeadlessSession<F, E>,
    x: f32,
    y: f32,
) {
    assert!(session.pointer_down(Point::new(x, y)), "press at {x},{y}");
    session.pointer_up(Point::new(x, y));
    session.frame().expect("frame after click");
}

fn tree<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
) -> AccessibilityTree {
    session.accessibility_tree().expect("accessibility tree")
}

fn find_label<'a>(nodes: &'a [AccessibilityNode], label: &str) -> Option<&'a AccessibilityNode> {
    nodes.iter().find_map(|node| {
        (node.a11y_label() == Some(label))
            .then_some(node)
            .or_else(|| find_label(node.a11y_children(), label))
    })
}

fn date(year: i32, month: u8, day: u8) -> CalendarDate {
    CalendarDate::new(year, month, day).expect("valid date")
}

struct ToastView {
    toaster: Toaster,
    toast: Rc<dyn Fn() -> Toast>,
    renders: Rc<Cell<u32>>,
}

impl View for ToastView {
    type Element = ui::Container;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        self.renders.set(self.renders.get() + 1);
        let (toaster, toast) = (self.toaster.clone(), Rc::clone(&self.toast));
        ui::container()
            .w_full()
            .h_full()
            .child(ui::button("Save").id(SAVE_ID).on_click(move || {
                toaster.show(toast());
            }))
            .child(self.toaster.render(cx))
    }
}

fn toast_session(
    toast: impl Fn() -> Toast + 'static,
) -> (
    HeadlessSession<impl FnMut(&mut AppContext) -> ui::Container, ui::Container>,
    Toaster,
    Rc<Cell<u32>>,
) {
    let toaster = Toaster::new();
    let renders = Rc::new(Cell::new(0));
    let view = ToastView {
        toaster: toaster.clone(),
        toast: Rc::new(toast),
        renders: Rc::clone(&renders),
    };
    (
        mount_view(Size::new(480.0, 320.0), view).expect("mount"),
        toaster,
        renders,
    )
}

#[test]
fn toast_shows_in_the_corner_as_a_status_with_its_buttons() {
    let undone = Rc::new(Cell::new(false));
    let undone_ref = Rc::clone(&undone);
    let (mut session, toaster, _) = toast_session(move || {
        let undone = Rc::clone(&undone_ref);
        Toast::success("Saved")
            .description("All changes stored")
            .action("Undo", move || undone.set(true))
    });

    click(&mut session, 10.0, 10.0);
    assert_eq!(toaster.toasts().len(), 1);
    let overlay = session.scene().overlays()[0];
    assert_eq!(overlay.bounds.max_x(), 480.0 - 16.0);
    assert!((overlay.bounds.max_y() - (320.0 - 16.0)).abs() < 1.0);
    assert!(!overlay.light_dismiss && !overlay.modal);

    let tree = tree(&session);
    let regions = tree.live_regions();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].a11y_role(), AccessibilityRole::Status);
    assert_eq!(regions[0].a11y_label(), Some("Saved"));
    assert_eq!(regions[0].a11y_value(), Some("All changes stored"));
    let undo = find_label(regions[0].a11y_children(), "Undo").expect("action button");
    assert_eq!(undo.a11y_role(), AccessibilityRole::Button);
    assert!(find_label(regions[0].a11y_children(), "Dismiss Saved").is_some());

    // The action runs its handler and closes the toast, from the keyboard.
    session.request_focus(Some(undo.a11y_id()));
    press(&mut session, KeyCode::Enter);
    assert!(undone.get());
    assert!(toaster.toasts().is_empty());
    assert!(tree_has_no_status(&session));
}

fn tree_has_no_status<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
) -> bool {
    tree(session).live_regions().is_empty()
}

#[test]
fn toast_close_button_dismisses_it() {
    let (mut session, toaster, _) = toast_session(|| Toast::error("Upload failed").persistent());
    click(&mut session, 10.0, 10.0);
    click(&mut session, 10.0, 10.0);
    assert_eq!(toaster.toasts().len(), 2);
    let tree = tree(&session);
    let close = find_label(tree.roots(), "Dismiss Upload failed").expect("close button");
    session.request_focus(Some(close.a11y_id()));
    press(&mut session, KeyCode::Space);
    assert_eq!(toaster.toasts().len(), 1);
    // Showing past the visible limit closes the oldest.
    for _ in 0..4 {
        click(&mut session, 10.0, 10.0);
    }
    assert_eq!(toaster.toasts().len(), 3);
}

#[test]
fn toast_closes_after_its_duration_but_not_while_hovered() {
    let (mut session, toaster, _) =
        toast_session(|| Toast::info("Synced").duration(Duration::from_millis(60)));
    click(&mut session, 10.0, 10.0);
    let card = session.scene().overlays()[0].bounds;

    session.pointer_move(card.center());
    session.frame().expect("frame after hover");
    assert!(toaster.is_paused());
    std::thread::sleep(Duration::from_millis(120));
    // The wake armed before the hover may still come; it closes nothing and
    // arms no other while paused.
    session.timed_frame().expect("timed frame while hovered");
    assert_eq!(toaster.toasts().len(), 1, "hovering pauses the timer");
    assert!(session.next_frame_deadline().is_none());

    session.pointer_move(Point::new(10.0, 300.0));
    session.frame().expect("frame after leaving");
    assert!(!toaster.is_paused());
    assert!(
        session.next_frame_deadline().is_some(),
        "leaving re-arms the wake"
    );
    std::thread::sleep(Duration::from_millis(120));
    assert!(session.timed_frame().expect("timed frame").is_some());
    assert!(toaster.toasts().is_empty());
    assert!(tree_has_no_status(&session));
}

#[test]
fn toast_countdown_wakes_once_at_its_deadline() {
    let duration = Duration::from_millis(80);
    let (mut session, toaster, renders) =
        toast_session(move || Toast::info("Synced").duration(duration));
    let shown_at = Instant::now();
    click(&mut session, 10.0, 10.0);
    let rendered = renders.get();
    let deadline = session.next_frame_deadline().expect("a wake at the expiry");
    assert!(deadline > shown_at && deadline <= Instant::now() + duration);

    // Nothing draws between showing and the deadline.
    assert!(
        session
            .animation_frame()
            .expect("animation frame")
            .is_none()
    );
    assert!(session.timed_frame().expect("early").is_none());
    assert_eq!(renders.get(), rendered);
    assert_eq!(toaster.toasts().len(), 1);

    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    assert!(session.timed_frame().expect("timed frame").is_some());
    assert_eq!(renders.get(), rendered + 1, "one render, at the deadline");
    assert!(toaster.toasts().is_empty(), "closed on time");
    assert!(session.next_frame_deadline().is_none());
    assert!(session.timed_frame().expect("idle").is_none());
    assert!(session.animation_frame().expect("idle").is_none());
}

struct DateView {
    value: Option<CalendarDate>,
    changes: Rc<Cell<u32>>,
}

impl View for DateView {
    type Element = ui::DatePicker;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let changes = Rc::clone(&self.changes);
        ui::date_picker("Due date", self.value)
            .id(PICKER_ID)
            .today(date(2026, 11, 15))
            .min(date(2026, 11, 3))
            .max(date(2026, 12, 20))
            .disabled_dates(|day| day.weekday() == Weekday::Saturday)
            .on_change(cx.listener_with(move |this, value, _| {
                this.value = Some(value);
                changes.set(changes.get() + 1);
            }))
    }
}

fn date_session(
    value: Option<CalendarDate>,
) -> (
    HeadlessSession<impl FnMut(&mut AppContext) -> ui::DatePicker, ui::DatePicker>,
    Rc<Cell<u32>>,
) {
    let changes = Rc::new(Cell::new(0));
    let view = DateView {
        value,
        changes: Rc::clone(&changes),
    };
    (
        mount_view(Size::new(400.0, 400.0), view).expect("mount"),
        changes,
    )
}

fn picker<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
) -> AccessibilityNode {
    tree(session).find(PICKER_ID).expect("picker node").clone()
}

fn grid_value<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
) -> Option<String> {
    let picker = picker(session);
    let grid = picker.a11y_children().first()?;
    assert_eq!(grid.a11y_role(), AccessibilityRole::Grid);
    grid.a11y_value().map(str::to_string)
}

#[test]
fn date_picker_keyboard_moves_days_weeks_months_and_picks() {
    let (mut session, changes) = date_session(None);
    session.request_focus(Some(PICKER_ID));
    session.frame().expect("frame after focus");
    assert_eq!(picker(&session).a11y_value(), Some("Select date"));

    press(&mut session, KeyCode::Enter);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Sunday, November 15, 2026")
    );
    press(&mut session, KeyCode::ArrowRight);
    press(&mut session, KeyCode::ArrowDown);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Monday, November 23, 2026")
    );
    press(&mut session, KeyCode::PageDown);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Sunday, December 20, 2026"),
        "kept at the max"
    );
    press(&mut session, KeyCode::PageUp);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Friday, November 20, 2026")
    );
    press(&mut session, KeyCode::End);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Saturday, November 21, 2026")
    );

    // Saturdays are disabled: Enter does not pick one.
    press(&mut session, KeyCode::Enter);
    assert_eq!(changes.get(), 0);
    assert!(grid_value(&session).is_some(), "still open");
    press(&mut session, KeyCode::Home);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Sunday, November 15, 2026")
    );
    press(&mut session, KeyCode::ArrowLeft);
    press(&mut session, KeyCode::ArrowUp);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Saturday, November 7, 2026")
    );
    press(&mut session, KeyCode::ArrowUp);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Tuesday, November 3, 2026"),
        "kept at the min"
    );

    press(&mut session, KeyCode::Enter);
    assert_eq!(changes.get(), 1);
    let picked = picker(&session);
    assert!(picked.a11y_children().is_empty(), "picking closes it");
    assert_eq!(picked.a11y_value(), Some("Tuesday, November 3, 2026"));
    assert_eq!(session.focused_element(), Some(PICKER_ID));
}

#[test]
fn date_picker_escape_closes_without_changing() {
    let (mut session, changes) = date_session(Some(date(2026, 12, 1)));
    session.request_focus(Some(PICKER_ID));
    press(&mut session, KeyCode::ArrowDown);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Tuesday, December 1, 2026"),
        "opens on the value"
    );
    press(&mut session, KeyCode::ArrowRight);
    press(&mut session, KeyCode::Escape);
    assert!(picker(&session).a11y_children().is_empty());
    assert_eq!(
        picker(&session).a11y_value(),
        Some("Tuesday, December 1, 2026")
    );
    assert_eq!(changes.get(), 0);
}

#[test]
fn date_picker_grid_exposes_cells_selection_and_disabled_days() {
    let (mut session, _) = date_session(Some(date(2026, 11, 18)));
    session.request_focus(Some(PICKER_ID));
    press(&mut session, KeyCode::Space);
    let picker = picker(&session);
    assert_eq!(picker.a11y_role(), AccessibilityRole::ComboBox);
    let grid = &picker.a11y_children()[0];
    assert_eq!(grid.a11y_label(), Some("November 2026"));
    let cells = grid.a11y_children();
    assert_eq!(cells.len(), 42);
    assert!(
        cells
            .iter()
            .all(|cell| cell.a11y_role() == AccessibilityRole::GridCell)
    );
    // November 2026 starts on a Sunday, so the first cell is the 1st.
    assert_eq!(cells[0].a11y_label(), Some("Sunday, November 1, 2026"));
    assert!(!cells[0].a11y_enabled(), "before the min");
    assert!(!cells[6].a11y_enabled(), "a Saturday");
    assert!(cells[17].a11y_enabled());
    assert_eq!(cells[17].a11y_selected(), Some(true));
    assert_eq!(cells[16].a11y_selected(), Some(false));
}

#[test]
fn date_picker_pointer_pages_months_and_picks_a_day() {
    let (mut session, changes) = date_session(Some(date(2026, 11, 18)));
    click(&mut session, 10.0, 18.0);
    assert_eq!(
        grid_value(&session).as_deref(),
        Some("Wednesday, November 18, 2026")
    );

    // The calendar opens 4pt below the 36pt field: 8pt padding, 28pt cells,
    // the month header, the weekday row, then six weeks.
    click(&mut session, 190.0, 62.0);
    assert_eq!(
        picker(&session).a11y_children()[0].a11y_label(),
        Some("December 2026")
    );
    // December 2026 starts on a Tuesday: the third cell of the second week
    // is the 8th.
    click(
        &mut session,
        8.0 + 28.0 * 2.0 + 14.0,
        56.0 + 28.0 * 3.0 + 14.0,
    );
    assert_eq!(changes.get(), 1);
    assert_eq!(
        picker(&session).a11y_value(),
        Some("Tuesday, December 8, 2026")
    );
    assert!(picker(&session).a11y_children().is_empty());

    // A press outside closes it.
    click(&mut session, 10.0, 18.0);
    assert!(!picker(&session).a11y_children().is_empty());
    session.pointer_down(Point::new(380.0, 380.0));
    session.frame().expect("frame after press outside");
    assert!(picker(&session).a11y_children().is_empty());
}
