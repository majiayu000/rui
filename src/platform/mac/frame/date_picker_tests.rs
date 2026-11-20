use super::dispatch_accessibility_action;
use super::tests::accessibility_request;
use crate::advanced_ui::{CalendarDate, DatePicker};
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityAction, AccessibilityAnnouncementKind};
use crate::core::action::{ActionId, StandardAction};
use crate::core::app::AppContext;
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Size};
use crate::core::presenter::Presenter;
use crate::elements::element::Element;
use crate::platform::frame_driver::dispatch_key;
use crate::platform::mac::accessibility::{MacAccessibilityActionRequest, MacAccessibilityRequest};
use std::cell::Cell;
use std::rc::Rc;

const SIZE: Size = Size::new(400.0, 400.0);

/// A picker with its calendar open on November 2026, with the date its
/// `on_change` last reported.
struct Opened {
    presenter: Presenter<DatePicker>,
    field: ElementId,
    /// The cell of the 18th.
    cell: ElementId,
    picked: Rc<Cell<Option<CalendarDate>>>,
}

fn open_picker() -> Opened {
    let field = ElementId::new();
    let picked = Rc::new(Cell::new(None));
    let sink = Rc::clone(&picked);
    let picker = DatePicker::new("Due date", None)
        .id(field)
        .today(CalendarDate::new(2026, 11, 15).expect("a date"))
        .on_change(move |date| sink.set(Some(date)));
    let mut presenter = Presenter::with_root(SIZE, picker);
    presenter.set_accessibility_announcements_enabled(true);
    assert!(
        dispatch_accessibility_action(
            &mut presenter,
            &accessibility_request(field, AccessibilityAction::Activate, None),
            None,
        )
        .0,
        "activating the field opens the calendar"
    );
    Opened {
        cell: cell_of(&presenter, field, "Wednesday, November 18, 2026"),
        presenter,
        field,
        picked,
    }
}

/// The cell of the day the open calendar reads out as `label`.
fn cell_of(presenter: &Presenter<DatePicker>, field: ElementId, label: &str) -> ElementId {
    let tree = presenter
        .accessibility_tree()
        .expect("date picker accessibility tree should build");
    let grid = &tree.find(field).expect("picker node").a11y_children()[0];
    let mut cells = grid.a11y_children().iter();
    let cell = cells
        .find(|cell| cell.a11y_label() == Some(label))
        .unwrap_or_else(|| panic!("the calendar should show {label}"));
    assert!(cell.a11y_actions().contains(&AccessibilityAction::Activate));
    cell.a11y_id()
}

/// Assistive technology puts focus on `cell`, as VoiceOver does while
/// keyboard focus follows its cursor.
fn focus(presenter: &mut Presenter<DatePicker>, cell: ElementId) {
    let focus = MacAccessibilityActionRequest {
        id: cell,
        request: MacAccessibilityRequest::Focus(true),
        bounds: Bounds::default(),
    };
    assert!(dispatch_accessibility_action(presenter, &focus, None).0);
    presenter.layout(SIZE).expect("layout");
    assert_eq!(presenter.focused_element(), Some(cell));
}

/// Press `key` on the keyboard and lay the next frame out. Gives what the
/// key had read out.
fn press(presenter: &mut Presenter<DatePicker>, key: KeyCode) -> Option<String> {
    presenter.take_accessibility_announcements();
    let event = KeyEvent::new(key, Modifiers::none());
    let (handled, _) = dispatch_key(presenter, &mut AppContext::new(), true, &event);
    assert!(handled, "{key:?} is a key of the open calendar");
    presenter.layout(SIZE).expect("layout");
    let said = presenter.take_accessibility_announcements();
    let mut feedback = said
        .iter()
        .filter(|said| said.kind() == AccessibilityAnnouncementKind::ActionFeedback);
    feedback.next_back().map(|said| said.message().to_string())
}

#[test]
fn native_accessibility_activate_on_a_calendar_day_picks_it() {
    let Opened {
        mut presenter,
        field,
        cell,
        picked,
    } = open_picker();

    assert!(
        dispatch_accessibility_action(
            &mut presenter,
            &accessibility_request(cell, AccessibilityAction::Activate, None),
            None,
        )
        .0,
        "the day advertises Activate, which should pick it"
    );
    let day = CalendarDate::new(2026, 11, 18);
    assert_eq!(picked.get(), day);
    assert_eq!(presenter.root().selected_date(), day);
    assert!(!presenter.root().is_open());
    assert_eq!(presenter.focused_element(), Some(field), "on the field");
}

#[test]
fn native_accessibility_focus_on_a_calendar_day_leaves_the_calendar_open() {
    let Opened {
        mut presenter,
        field,
        cell,
        picked,
    } = open_picker();

    focus(&mut presenter, cell);
    assert!(presenter.root().is_open());

    // Escape closes the calendar, whose days go away with it.
    let cancel = ActionId::Standard(StandardAction::Cancel);
    let (outcome, _) = presenter.with_event_context(|root, cx| root.dispatch_action(cx, &cancel));
    assert!(outcome.is_handled());
    assert!(!presenter.root().is_open());
    assert_eq!(presenter.focused_element(), Some(field), "on the field");
    assert_eq!(picked.get(), None);
}

#[test]
fn arrow_key_from_a_focused_calendar_day_moves_from_it_and_enter_picks_the_day_read_out() {
    let Opened {
        mut presenter,
        field,
        cell,
        picked,
    } = open_picker();
    focus(&mut presenter, cell);

    let read_out = press(&mut presenter, KeyCode::ArrowRight).expect("the move is read out");
    press(&mut presenter, KeyCode::Enter);
    let day = picked.get().expect("Enter picks a day");
    assert_eq!(day.long_label(), read_out, "the day read out is picked");
    assert_eq!(Some(day), CalendarDate::new(2026, 11, 19), "after the 18th");
    assert_eq!(presenter.focused_element(), Some(field), "on the field");
}

#[test]
fn page_key_from_a_focused_calendar_day_turns_the_month_and_enter_picks_the_day_read_out() {
    let Opened {
        mut presenter,
        field,
        cell,
        picked,
    } = open_picker();
    focus(&mut presenter, cell);

    let read_out = press(&mut presenter, KeyCode::PageDown).expect("the move is read out");
    press(&mut presenter, KeyCode::Enter);
    let day = picked.get().expect("Enter picks a day");
    assert_eq!(day.long_label(), read_out, "the day read out is picked");
    assert_eq!(Some(day), CalendarDate::new(2026, 12, 18), "a month on");
    assert_eq!(presenter.focused_element(), Some(field), "on the field");
}

#[test]
fn native_accessibility_activate_on_a_day_the_month_turned_away_from_picks_nothing() {
    let Opened {
        mut presenter,
        cell,
        picked,
        ..
    } = open_picker();
    let november = presenter.accessibility_tree().expect("tree");
    // December's calendar starts on the 29th of November.
    press(&mut presenter, KeyCode::PageDown);

    let press_18th = accessibility_request(cell, AccessibilityAction::Activate, None);
    assert!(
        !dispatch_accessibility_action(&mut presenter, &press_18th, None).0,
        "the calendar has no cell for the 18th of November any more"
    );
    assert_eq!(picked.get(), None);
    assert!(presenter.root().is_open());

    // The press may be checked against the tree published before the key.
    assert!(!dispatch_accessibility_action(&mut presenter, &press_18th, Some(&november)).0);
    assert_eq!(picked.get(), None);
}

#[test]
fn native_accessibility_activate_on_a_day_the_turned_month_still_shows_picks_it() {
    let Opened {
        mut presenter,
        field,
        picked,
        ..
    } = open_picker();
    // In the last row of November's calendar and the first of December's.
    let cell = cell_of(&presenter, field, "Friday, December 4, 2026");
    press(&mut presenter, KeyCode::PageDown);

    assert!(
        dispatch_accessibility_action(
            &mut presenter,
            &accessibility_request(cell, AccessibilityAction::Activate, None),
            None,
        )
        .0
    );
    assert_eq!(picked.get(), CalendarDate::new(2026, 12, 4));
}
