use super::dispatch_accessibility_action;
use super::tests::accessibility_request;
use crate::advanced_ui::{CalendarDate, DatePicker};
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityAction;
use crate::core::action::{ActionId, StandardAction};
use crate::core::geometry::{Bounds, Size};
use crate::core::presenter::Presenter;
use crate::elements::element::Element;
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
    assert!(
        dispatch_accessibility_action(
            &mut presenter,
            &accessibility_request(field, AccessibilityAction::Activate, None),
            None,
        )
        .0,
        "activating the field opens the calendar"
    );
    let tree = presenter
        .accessibility_tree()
        .expect("date picker accessibility tree should build");
    let grid = &tree.find(field).expect("picker node").a11y_children()[0];
    // November 2026 starts on a Sunday, the first column.
    let cell = &grid.a11y_children()[17];
    assert_eq!(cell.a11y_label(), Some("Wednesday, November 18, 2026"));
    assert!(cell.a11y_actions().contains(&AccessibilityAction::Activate));
    Opened {
        cell: cell.a11y_id(),
        presenter,
        field,
        picked,
    }
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

    let focus = MacAccessibilityActionRequest {
        id: cell,
        request: MacAccessibilityRequest::Focus(true),
        bounds: Bounds::default(),
    };
    assert!(dispatch_accessibility_action(&mut presenter, &focus, None).0);
    presenter.layout(SIZE).expect("layout");
    assert!(presenter.root().is_open());
    assert_eq!(presenter.focused_element(), Some(cell));

    // Escape closes the calendar, whose days go away with it.
    let cancel = ActionId::Standard(StandardAction::Cancel);
    let (outcome, _) = presenter.with_event_context(|root, cx| root.dispatch_action(cx, &cancel));
    assert!(outcome.is_handled());
    assert!(!presenter.root().is_open());
    assert_eq!(presenter.focused_element(), Some(field), "on the field");
    assert_eq!(picked.get(), None);
}
