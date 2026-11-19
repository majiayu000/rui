//! The winit runner's accessibility without a screen reader: the AccessKit
//! trees it publishes, read back the way a platform adapter reads them, and
//! the requests that come back.

use super::accessibility::WindowAccessibility;
use super::accessibility_tree::{WINDOW_NODE, request_for_action};
use crate::advanced_ui::{Button, Checkbox};
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityAction;
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::elements::{Div, div, input, text};
use crate::platform::accessibility_request::{
    AccessibilityRequest, dispatch_accessibility_request,
};
use accesskit::{
    Action, ActionData, ActionRequest, NodeId, Rect, Role, Toggled, TreeId, TreeUpdate,
};
use accesskit_winit::WindowEvent as AccessKitEvent;
use std::cell::Cell;
use std::rc::Rc;

const FILTER: ElementId = ElementId(91001);
const DISABLED: ElementId = ElementId(91003);
const SAVE: ElementId = ElementId(91004);
const NOTIFY: ElementId = ElementId(91010);

/// The controls of `examples/interaction_acceptance.rs`, plus a checkbox.
fn acceptance_window(saved: &Rc<Cell<u32>>) -> Presenter<Div> {
    let saved = Rc::clone(saved);
    let root = div()
        .w(600.0)
        .h(400.0)
        .flex_col()
        .child(text("RUI interaction acceptance"))
        .child(
            input()
                .id(FILTER)
                .accessibility_label("Filter")
                .value("notes"),
        )
        .child(Button::new("Disabled").id(DISABLED).disabled(true))
        .child(
            Button::new("Save")
                .id(SAVE)
                .on_click(move || saved.set(saved.get() + 1)),
        )
        .child(Checkbox::new("Notify").id(NOTIFY).checked(true));
    let mut presenter = Presenter::with_root(Size::new(600.0, 400.0), root);
    presenter
        .layout(Size::new(600.0, 400.0))
        .expect("acceptance window should lay out");
    presenter.paint();
    presenter
}

/// A window a screen reader asked to read.
fn reading() -> WindowAccessibility {
    let mut accessibility = WindowAccessibility::new(None);
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    accessibility
}

fn publish(
    accessibility: &mut WindowAccessibility,
    presenter: &Presenter<Div>,
    scale: f64,
) -> Option<TreeUpdate> {
    let tree = presenter
        .accessibility_tree()
        .expect("accessibility tree should build");
    accessibility.next_update(tree, "RUI Interaction Acceptance", scale)
}

/// The tree an AccessKit platform adapter holds after `update`. It panics on
/// an update that is not a whole, connected tree.
fn read_back(update: TreeUpdate) -> accesskit_consumer::Tree {
    accesskit_consumer::Tree::new(update, true)
}

fn find<'a>(
    node: accesskit_consumer::NodeRef<'a>,
    id: ElementId,
) -> Option<accesskit_consumer::NodeRef<'a>> {
    if node.locate().0 == NodeId(id.0) {
        return Some(node);
    }
    node.children().find_map(|child| find(child, id))
}

fn node(tree: &accesskit_consumer::Tree, id: ElementId) -> accesskit_consumer::NodeRef<'_> {
    find(tree.state().root(), id).unwrap_or_else(|| panic!("node {id:?} is in the tree"))
}

fn action(target: ElementId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: NodeId(target.0),
        data,
    }
}

#[test]
fn the_acceptance_controls_are_read_back_with_roles_names_values_and_state() {
    let saved = Rc::new(Cell::new(0));
    let presenter = acceptance_window(&saved);
    let mut accessibility = reading();
    let tree = read_back(publish(&mut accessibility, &presenter, 1.0).expect("a first update"));

    let window = tree.state().root();
    assert_eq!(window.locate().0, WINDOW_NODE);
    assert_eq!(window.role(), Role::Window);
    assert_eq!(
        window.label().as_deref(),
        Some("RUI Interaction Acceptance")
    );

    let filter = node(&tree, FILTER);
    assert_eq!(filter.role(), Role::TextInput);
    assert_eq!(filter.label().as_deref(), Some("Filter"));
    assert_eq!(filter.value().as_deref(), Some("notes"));
    assert!(filter.data().supports_action(Action::SetValue));
    assert!(filter.data().supports_action(Action::Focus));
    assert!(
        filter.supports_text_ranges(),
        "AT-SPI reads and edits the value through its text runs"
    );
    assert_eq!(filter.document_range().text(), "notes");

    let save = node(&tree, SAVE);
    assert_eq!(save.role(), Role::Button);
    assert_eq!(save.label().as_deref(), Some("Save"));
    assert!(save.data().supports_action(Action::Click));
    assert!(!save.is_disabled());

    let disabled = node(&tree, DISABLED);
    assert!(disabled.is_disabled());
    assert!(!disabled.data().supports_action(Action::Click));
    assert!(!disabled.data().supports_action(Action::Focus));

    let notify = node(&tree, NOTIFY);
    assert_eq!(notify.role(), Role::CheckBox);
    assert_eq!(notify.label().as_deref(), Some("Notify"));
    assert_eq!(notify.toggled(), Some(Toggled::True));

    let heading = window
        .children()
        .flat_map(|root| std::iter::once(root).chain(root.children()))
        .find(|node| node.role() == Role::Label)
        .expect("the heading text is a label");
    // AT-SPI and UI Automation name a label by its value.
    assert!(heading.label_comes_from_value());
    assert_eq!(
        heading.value().as_deref(),
        Some("RUI interaction acceptance")
    );

    assert_eq!(
        tree.state().focus().map(|node| node.locate().0),
        Some(WINDOW_NODE),
        "with no focused element the window has focus"
    );
}

#[test]
fn bounds_are_logical_and_the_window_scales_them_to_pixels() {
    let saved = Rc::new(Cell::new(0));
    let presenter = acceptance_window(&saved);
    let logical = presenter
        .accessibility_tree()
        .expect("accessibility tree should build")
        .find(SAVE)
        .and_then(|node| node.a11y_bounds())
        .expect("the painted button has bounds");
    let mut accessibility = reading();
    let tree = read_back(publish(&mut accessibility, &presenter, 2.0).expect("a first update"));

    let expected = Rect::new(
        f64::from(logical.x()) * 2.0,
        f64::from(logical.y()) * 2.0,
        f64::from(logical.x() + logical.width()) * 2.0,
        f64::from(logical.y() + logical.height()) * 2.0,
    );
    assert_eq!(node(&tree, SAVE).bounding_box(), Some(expected));
}

#[test]
fn a_tree_is_published_only_while_read_and_only_when_it_changed() {
    let saved = Rc::new(Cell::new(0));
    let mut presenter = acceptance_window(&saved);
    let mut accessibility = WindowAccessibility::new(None);
    assert!(
        publish(&mut accessibility, &presenter, 1.0).is_none(),
        "no tree is built before a screen reader asks"
    );
    assert!(!accessibility.needs_tree());

    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    assert!(accessibility.needs_tree());
    let first = publish(&mut accessibility, &presenter, 1.0).expect("the initial tree");
    assert!(first.tree.is_some(), "the first update names the root");
    assert!(publish(&mut accessibility, &presenter, 1.0).is_none());
    assert!(
        publish(&mut accessibility, &presenter, 2.0).is_some(),
        "a new scale moves every node"
    );

    presenter.set_focused_element(Some(FILTER));
    let focused = publish(&mut accessibility, &presenter, 2.0).expect("focus changed the tree");
    assert_eq!(focused.focus, NodeId(FILTER.0));
    let tree = read_back(focused);
    let filter = tree.state().focus().expect("an element has focus");
    assert_eq!(filter.locate().0, NodeId(FILTER.0));
    assert!(filter.is_focused());
    let caret = filter
        .text_selection_focus()
        .expect("the focused input has a caret");
    assert_eq!(
        caret.to_global_usv_index(),
        "notes".len(),
        "the caret is after the value"
    );

    assert!(!accessibility.handle_event(AccessKitEvent::AccessibilityDeactivated));
    assert!(publish(&mut accessibility, &presenter, 2.0).is_none());
    assert!(accessibility.published_tree().is_none());
}

#[test]
fn screen_reader_actions_map_to_the_shared_requests() {
    let activate = AccessibilityRequest::Action {
        action: AccessibilityAction::Activate,
        value: None,
    };
    assert_eq!(
        request_for_action(&action(SAVE, Action::Click, None)),
        Some((SAVE, activate))
    );
    assert_eq!(
        request_for_action(&action(FILTER, Action::Focus, None)),
        Some((FILTER, AccessibilityRequest::Focus(true)))
    );
    assert_eq!(
        request_for_action(&action(FILTER, Action::Blur, None)),
        Some((FILTER, AccessibilityRequest::Focus(false)))
    );
    assert_eq!(
        request_for_action(&action(
            FILTER,
            Action::SetValue,
            Some(ActionData::Value("draft".into()))
        )),
        Some((
            FILTER,
            AccessibilityRequest::Action {
                action: AccessibilityAction::SetValue,
                value: Some("draft".to_string()),
            }
        ))
    );
    assert_eq!(
        request_for_action(&action(FILTER, Action::SetValue, None)),
        None,
        "a set value without text"
    );
    assert_eq!(
        request_for_action(&action(SAVE, Action::ShowContextMenu, None)),
        None
    );
    let mut window = action(SAVE, Action::Click, None);
    window.target_node = WINDOW_NODE;
    assert_eq!(request_for_action(&window), None);
}

#[test]
fn press_focus_and_set_value_reach_the_elements() {
    let saved = Rc::new(Cell::new(0));
    let mut presenter = acceptance_window(&saved);
    let mut accessibility = reading();
    publish(&mut accessibility, &presenter, 1.0).expect("the initial tree");

    let mut perform = |presenter: &mut Presenter<Div>, request: ActionRequest| {
        assert!(accessibility.handle_event(AccessKitEvent::ActionRequested(request)));
        let requests = accessibility.take_requests();
        assert_eq!(requests.len(), 1);
        let (id, request) = &requests[0];
        dispatch_accessibility_request(
            presenter,
            *id,
            request,
            accessibility.published_tree(),
            "winit",
        )
    };

    assert!(perform(&mut presenter, action(SAVE, Action::Click, None)).0);
    assert_eq!(saved.get(), 1, "press ran the button's click handler");
    assert_eq!(presenter.focused_element(), Some(SAVE));

    assert!(perform(&mut presenter, action(FILTER, Action::Focus, None)).0);
    assert_eq!(presenter.focused_element(), Some(FILTER));

    let set = action(
        FILTER,
        Action::SetValue,
        Some(ActionData::Value("drafts".into())),
    );
    assert!(perform(&mut presenter, set).0);
    let tree = presenter
        .accessibility_tree()
        .expect("accessibility tree should rebuild");
    assert_eq!(
        tree.find(FILTER).and_then(|node| node.a11y_value()),
        Some("drafts")
    );

    assert!(
        !perform(&mut presenter, action(DISABLED, Action::Click, None)).0,
        "the disabled button offers no press"
    );
    assert_eq!(saved.get(), 1);
}

#[test]
fn each_window_publishes_its_own_tree() {
    let saved = Rc::new(Cell::new(0));
    let first = acceptance_window(&saved);
    let second = Presenter::with_root(
        Size::new(200.0, 100.0),
        div().child(Button::new("Close").id(ElementId(7))),
    );
    let mut first_accessibility = reading();
    let mut second_accessibility = reading();

    let first_tree =
        read_back(publish(&mut first_accessibility, &first, 1.0).expect("first window"));
    let tree = second
        .accessibility_tree()
        .expect("second accessibility tree should build");
    let second_tree = read_back(
        second_accessibility
            .next_update(tree, "Second", 1.0)
            .expect("second window"),
    );

    assert!(find(first_tree.state().root(), SAVE).is_some());
    assert!(find(first_tree.state().root(), ElementId(7)).is_none());
    assert!(find(second_tree.state().root(), ElementId(7)).is_some());
    assert!(find(second_tree.state().root(), SAVE).is_none());
    assert_eq!(
        second_tree.state().root().label().as_deref(),
        Some("Second")
    );
}
