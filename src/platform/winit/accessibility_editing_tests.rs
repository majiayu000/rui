//! Text editing and range values from a screen reader in the winit runner:
//! selecting text, replacing the selection, setting a number, and where each
//! character of a field is. Each request goes the way an AccessKit platform
//! adapter sends it, built from the tree read back.

use super::accessibility::{WindowAccessibility, text_geometry};
use crate::advanced_ui::Slider;
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityTextRange;
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::elements::Element;
use crate::elements::{Div, div, input, text_area};
use crate::platform::accessibility_request::dispatch_accessibility_request;
use accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId, TreeUpdate};
use accesskit_winit::WindowEvent as AccessKitEvent;
use std::cell::Cell;
use std::rc::Rc;

const FIELD: ElementId = ElementId(92001);
const READ_ONLY: ElementId = ElementId(92002);
const NOTES: ElementId = ElementId(92003);
const VOLUME: ElementId = ElementId(92004);
const SIZE: Size = Size::new(600.0, 400.0);

fn editing_window(volume: &Rc<Cell<f32>>) -> Presenter<Div> {
    let volume = Rc::clone(volume);
    let root = div()
        .w(600.0)
        .h(400.0)
        .flex_col()
        .child(
            input()
                .id(FIELD)
                .accessibility_label("Filter")
                .value("notes"),
        )
        .child(
            input()
                .id(READ_ONLY)
                .accessibility_label("Path")
                .value("/tmp")
                .read_only(true),
        )
        .child(
            text_area()
                .id(NOTES)
                .accessibility_label("Notes")
                .value("ab\ncd"),
        )
        .child(
            Slider::new("Volume", 40.0)
                .id(VOLUME)
                .step(5.0)
                .on_change(move |value| volume.set(value)),
        );
    let mut presenter = Presenter::with_root(SIZE, root);
    frame(&mut presenter);
    presenter
}

fn frame(presenter: &mut Presenter<Div>) {
    presenter
        .layout(SIZE)
        .expect("editing window should lay out");
    presenter.paint();
}

struct Reader {
    accessibility: WindowAccessibility,
}

impl Reader {
    fn new() -> Self {
        let mut accessibility = WindowAccessibility::new(None);
        assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
        Self { accessibility }
    }

    /// Publish the presenter's tree, with its text layouts, and read back
    /// what the platform holds after it.
    fn read(&mut self, presenter: &Presenter<Div>) -> accesskit_consumer::Tree {
        let tree = presenter
            .accessibility_tree()
            .expect("accessibility tree should build");
        let text = text_geometry(presenter, &tree);
        let update: TreeUpdate = self
            .accessibility
            .next_update(tree, text, "Editing", 1.0)
            .expect("the tree changed");
        accesskit_consumer::Tree::new(update, true)
    }

    /// Send `request` as the platform would and dispatch it. Returns whether
    /// it was handled.
    fn perform(&mut self, presenter: &mut Presenter<Div>, request: ActionRequest) -> bool {
        if !self
            .accessibility
            .handle_event(AccessKitEvent::ActionRequested(request))
        {
            return false;
        }
        let requests = self.accessibility.take_requests();
        assert_eq!(requests.len(), 1);
        let (id, request) = &requests[0];
        let (handled, _) = dispatch_accessibility_request(
            presenter,
            *id,
            request,
            self.accessibility.published_tree(),
            "winit",
        );
        frame(presenter);
        handled
    }
}

fn node(tree: &accesskit_consumer::Tree, id: ElementId) -> accesskit_consumer::NodeRef<'_> {
    fn find(
        node: accesskit_consumer::NodeRef<'_>,
        id: NodeId,
    ) -> Option<accesskit_consumer::NodeRef<'_>> {
        if node.locate().0 == id {
            return Some(node);
        }
        node.children().find_map(|child| find(child, id))
    }
    find(tree.state().root(), NodeId(id.0)).unwrap_or_else(|| panic!("node {id:?} is in the tree"))
}

fn request(target: ElementId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: NodeId(target.0),
        data,
    }
}

/// The SetTextSelection AccessKit sends for characters `start..end` of the
/// field, as AT-SPI's `set_selection` and UI Automation's `Select` build it.
fn select(
    tree: &accesskit_consumer::Tree,
    id: ElementId,
    start: usize,
    end: usize,
) -> ActionRequest {
    let field = node(tree, id);
    let mut range = field
        .text_position_from_global_usv_index(start)
        .expect("start is in the text")
        .to_degenerate_range();
    range.set_end(
        field
            .text_position_from_global_usv_index(end)
            .expect("end is in the text"),
    );
    request(
        id,
        Action::SetTextSelection,
        Some(ActionData::SetTextSelection(range.to_text_selection())),
    )
}

fn selection(presenter: &Presenter<Div>, id: ElementId) -> Option<AccessibilityTextRange> {
    presenter
        .accessibility_tree()
        .expect("accessibility tree should rebuild")
        .find(id)
        .and_then(|node| node.a11y_text_selection())
}

fn value(presenter: &Presenter<Div>, id: ElementId) -> String {
    presenter
        .accessibility_tree()
        .expect("accessibility tree should rebuild")
        .find(id)
        .and_then(|node| node.a11y_value().map(str::to_string))
        .unwrap_or_default()
}

#[test]
fn a_screen_reader_selects_text_and_reads_the_selection_back() {
    let mut presenter = editing_window(&Rc::new(Cell::new(0.0)));
    let mut reader = Reader::new();
    let tree = reader.read(&presenter);
    assert!(
        node(&tree, FIELD)
            .data()
            .supports_action(Action::SetTextSelection)
    );

    assert!(reader.perform(&mut presenter, select(&tree, FIELD, 1, 3)));
    assert_eq!(
        selection(&presenter, FIELD),
        Some(AccessibilityTextRange::new(1, 3))
    );
    assert_eq!(presenter.focused_element(), Some(FIELD));
    let tree = reader.read(&presenter);
    let selected = node(&tree, FIELD)
        .text_selection()
        .expect("the field has a selection")
        .text();
    assert_eq!(selected, "ot");

    // Across the line break of a text area.
    assert!(reader.perform(&mut presenter, select(&tree, NOTES, 1, 4)));
    assert_eq!(
        selection(&presenter, NOTES),
        Some(AccessibilityTextRange::new(1, 4))
    );
    let tree = reader.read(&presenter);
    assert_eq!(
        node(&tree, NOTES)
            .text_selection()
            .map(|range| range.text()),
        Some("b\nc".to_string())
    );

    // A read-only field can be selected in, for copying.
    assert!(reader.perform(&mut presenter, select(&tree, READ_ONLY, 0, 4)));
    assert_eq!(
        selection(&presenter, READ_ONLY),
        Some(AccessibilityTextRange::new(0, 4))
    );
}

#[test]
fn replace_selected_text_replaces_only_the_selection() {
    let mut presenter = editing_window(&Rc::new(Cell::new(0.0)));
    let mut reader = Reader::new();
    let tree = reader.read(&presenter);
    assert!(
        node(&tree, FIELD)
            .data()
            .supports_action(Action::ReplaceSelectedText)
    );
    assert!(
        !node(&tree, READ_ONLY)
            .data()
            .supports_action(Action::ReplaceSelectedText)
    );

    assert!(reader.perform(&mut presenter, select(&tree, FIELD, 1, 3)));
    let tree = reader.read(&presenter);
    let replace = request(
        FIELD,
        Action::ReplaceSelectedText,
        Some(ActionData::Value("XY".into())),
    );
    assert!(reader.perform(&mut presenter, replace));
    assert_eq!(value(&presenter, FIELD), "nXYes");

    assert!(reader.perform(&mut presenter, select(&tree, READ_ONLY, 0, 4)));
    let replace = request(
        READ_ONLY,
        Action::ReplaceSelectedText,
        Some(ActionData::Value("/etc".into())),
    );
    assert!(
        !reader.perform(&mut presenter, replace),
        "the field is read-only"
    );
    assert_eq!(value(&presenter, READ_ONLY), "/tmp");
}

#[test]
fn a_number_sets_a_slider() {
    let volume = Rc::new(Cell::new(0.0));
    let mut presenter = editing_window(&volume);
    let mut reader = Reader::new();
    let tree = reader.read(&presenter);
    let slider = node(&tree, VOLUME);
    assert!(slider.data().supports_action(Action::SetValue));
    assert_eq!(slider.numeric_value(), Some(40.0));

    let set = |value| {
        request(
            VOLUME,
            Action::SetValue,
            Some(ActionData::NumericValue(value)),
        )
    };
    assert!(reader.perform(&mut presenter, set(75.0)));
    assert_eq!(volume.get(), 75.0, "the slider's change handler ran");
    let tree = reader.read(&presenter);
    assert_eq!(node(&tree, VOLUME).numeric_value(), Some(75.0));

    assert!(!reader.perform(&mut presenter, set(f64::NAN)));
    assert_eq!(volume.get(), 75.0);
}

#[test]
fn the_focused_fields_characters_have_the_bounds_it_draws_them_at() {
    let mut presenter = editing_window(&Rc::new(Cell::new(0.0)));
    let mut reader = Reader::new();
    let tree = reader.read(&presenter);
    assert!(
        node(&tree, FIELD)
            .document_range()
            .bounding_boxes()
            .is_empty(),
        "an unfocused field has no layout to place characters with"
    );

    assert!(reader.perform(&mut presenter, request(FIELD, Action::Focus, None)));
    let geometry = presenter
        .root()
        .text_input_snapshot(FIELD)
        .and_then(|snapshot| snapshot.geometry().cloned())
        .expect("the focused field has a layout");
    let tree = reader.read(&presenter);
    let field = node(&tree, FIELD);
    let mut previous_right = f64::MIN;
    for index in 0.."notes".len() {
        let mut range = field
            .text_position_from_global_usv_index(index)
            .expect("in the text")
            .to_degenerate_range();
        range.set_end(
            field
                .text_position_from_global_usv_index(index + 1)
                .expect("in the text"),
        );
        let boxes = range.bounding_boxes();
        assert_eq!(boxes.len(), 1, "character {index} has one box");
        let (_, drawn) = geometry
            .first_bounds_for_range(crate::core::text_editing::TextRange::ordered(
                index,
                index + 1,
            ))
            .expect("valid range")
            .expect("the character is drawn");
        let close = |a: f64, b: f32| (a - f64::from(b)).abs() < 0.01;
        let rect = boxes[0];
        assert!(
            close(rect.x0, drawn.x()),
            "character {index} starts where it is drawn"
        );
        assert!(close(rect.x1, drawn.x() + drawn.width()));
        assert!(close(rect.y0, drawn.y()));
        assert!(close(rect.y1, drawn.y() + drawn.height()));
        assert!(
            rect.x0 >= previous_right - 0.01,
            "characters go left to right"
        );
        previous_right = rect.x1;
    }
}
