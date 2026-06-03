use rui::advanced_ui::{
    Button, Checkbox, DataList, DataTableRow, DataTree, DataTreeItem, Dialog, Menu, Popover,
    ProgressBar, Scrollable, SegmentedControl, Tab, TabPanel, Tabs, TextField, button, container,
    text,
};
use rui::core::ElementId;
use rui::core::accessibility::{
    AccessibilityAction, AccessibilityAnnouncementKind, AccessibilityBridge, AccessibilityContext,
    AccessibilityError, AccessibilityNode, AccessibilityRole, AccessibilityScrollPosition,
    AccessibilityTextRange, AccessibilityTree, UnsupportedAccessibilityBridge,
};
use rui::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use rui::core::geometry::{Bounds, Point, Size};
use rui::core::text_editing::TextInputEvent;
use rui::elements::Element;
use rui::elements::Input;
use rui::elements::element::{
    EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use rui::renderer::Scene;
use taffy::TaffyTree;

fn accessibility_result<T>(result: Result<T, AccessibilityError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("accessibility operation failed: {err}"),
    }
}

fn first_node(nodes: Vec<AccessibilityNode>) -> AccessibilityNode {
    match nodes.into_iter().next() {
        Some(node) => node,
        None => panic!("expected one accessibility node"),
    }
}

fn layout_and_paint_scrollable(scrollable: &mut Scrollable, size: Size) -> TaffyTree<ElementId> {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, size);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(size.width),
            height: taffy::prelude::AvailableSpace::Definite(size.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(
        &mut scene,
        Bounds::from_xywh(0.0, 0.0, size.width, size.height),
        &taffy,
    );
    scrollable.paint(&mut paint_cx);
    taffy
}

fn accessibility_pointer(kind: PointerEventKind) -> PointerEvent {
    PointerEvent {
        kind,
        position: Point::new(4.0, 4.0),
        button: Some(MouseButton::Left),
    }
}

#[test]
fn accessibility_button_exposes_role_label_enabled_and_focus() {
    let id = ElementId::new();
    let button = Button::new("Save")
        .id(id)
        .disabled(true)
        .read_only(true)
        .invalid(true);
    let cx = AccessibilityContext::new(Some(id));

    let node = first_node(accessibility_result(button.accessibility_nodes(&cx)));
    assert_eq!(node.a11y_id(), id);
    assert_eq!(node.a11y_role(), AccessibilityRole::Button);
    assert_eq!(node.a11y_label(), Some("Save"));
    assert!(!node.a11y_enabled());
    assert!(node.a11y_read_only());
    assert!(node.a11y_invalid());
    assert!(node.a11y_focused());
    assert!(node.a11y_actions().is_empty());
}

#[test]
fn accessibility_checkbox_exposes_checked_value_and_action_feedback() {
    let id = ElementId::new();
    let mut checkbox = Checkbox::new("Sync").id(id);
    let node = first_node(accessibility_result(
        checkbox.accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(node.a11y_role(), AccessibilityRole::Checkbox);
    assert_eq!(node.a11y_value(), Some("unchecked"));
    assert_eq!(node.a11y_checked(), Some(false));
    assert_eq!(node.a11y_actions(), [AccessibilityAction::Activate]);

    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
        &taffy,
        &mut focused,
    );
    assert!(checkbox.handle_pointer_event(
        &mut event_cx,
        &accessibility_pointer(PointerEventKind::Down)
    ));
    assert!(
        checkbox.handle_pointer_event(&mut event_cx, &accessibility_pointer(PointerEventKind::Up))
    );

    let announcements = event_cx.take_accessibility_announcements();
    assert_eq!(announcements.len(), 1);
    assert_eq!(announcements[0].node_id(), id);
    assert_eq!(
        announcements[0].kind(),
        AccessibilityAnnouncementKind::ActionFeedback
    );
    assert_eq!(announcements[0].message(), "Sync checked");
}

#[test]
fn accessibility_segmented_control_exposes_selected_option_tree() {
    let id = ElementId::new();
    let control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "grid")
        .id(id)
        .accessibility_label("View mode");
    let tree = AccessibilityTree::new(accessibility_result(
        control.accessibility_nodes(&AccessibilityContext::new(Some(id))),
    ));

    let node = match tree.find(id) {
        Some(node) => node,
        None => panic!("segmented control should be in accessibility tree"),
    };
    assert_eq!(node.a11y_role(), AccessibilityRole::SegmentedControl);
    assert_eq!(node.a11y_label(), Some("View mode"));
    assert_eq!(node.a11y_value(), Some("grid"));
    assert!(node.a11y_focused());
    assert_eq!(node.a11y_children().len(), 2);
    assert_eq!(node.a11y_children()[0].a11y_selected(), Some(false));
    assert_eq!(node.a11y_children()[1].a11y_selected(), Some(true));
    assert_eq!(
        node.a11y_children()[0].a11y_actions(),
        [AccessibilityAction::Activate]
    );
    assert_eq!(
        node.a11y_children()[1].a11y_actions(),
        [AccessibilityAction::Activate]
    );
}

#[test]
fn accessibility_tabs_expose_tab_list_tabs_and_selected_panel() {
    let tabs_id = ElementId::new();
    let disabled_tab = Tab::new("audit", "Audit").disabled(true);
    let tabs = Tabs::new(
        [
            Tab::new("overview", "Overview"),
            Tab::new("logs", "Logs"),
            disabled_tab,
        ],
        "logs",
    )
    .id(tabs_id)
    .accessibility_label("Project sections")
    .panel(TabPanel::new("overview", text("Summary")))
    .panel(TabPanel::new("logs", text("Build logs")))
    .panel(TabPanel::new("audit", text("Audit log")));
    let tree = AccessibilityTree::new(accessibility_result(
        tabs.accessibility_nodes(&AccessibilityContext::default()),
    ));

    let tab_list = tree
        .roots()
        .iter()
        .find(|node| node.a11y_role() == AccessibilityRole::TabList)
        .expect("tab list should be an accessibility root");
    assert_eq!(tab_list.a11y_label(), Some("Project sections"));
    assert_eq!(tab_list.a11y_value(), Some("logs"));
    assert_eq!(tab_list.a11y_children().len(), 3);
    assert_eq!(
        tab_list.a11y_children()[0].a11y_role(),
        AccessibilityRole::Tab
    );
    assert_eq!(tab_list.a11y_children()[1].a11y_selected(), Some(true));
    assert_eq!(tab_list.a11y_children()[2].a11y_enabled(), false);
    assert!(tab_list.a11y_children()[2].a11y_actions().is_empty());

    let panel = tree
        .roots()
        .iter()
        .find(|node| node.a11y_role() == AccessibilityRole::TabPanel)
        .expect("selected tab panel should be an accessibility root");
    assert_eq!(panel.a11y_label(), Some("Logs"));
    assert_eq!(panel.a11y_value(), Some("logs"));
    assert_eq!(panel.a11y_children().len(), 1);
    assert_eq!(panel.a11y_children()[0].a11y_label(), Some("Build logs"));
}

#[test]
fn accessibility_text_and_scrollable_expose_semantic_tree() {
    let scroll_id = ElementId::new();
    let text_id = ElementId::new();
    let scrollable = Scrollable::new(text("Activity").id(text_id))
        .id(scroll_id)
        .accessibility_label("Activity feed");

    let tree = AccessibilityTree::new(accessibility_result(
        scrollable.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let scroll_node = match tree.find(scroll_id) {
        Some(node) => node,
        None => panic!("scroll area should be in accessibility tree"),
    };
    assert_eq!(scroll_node.a11y_role(), AccessibilityRole::ScrollArea);
    assert_eq!(scroll_node.a11y_label(), Some("Activity feed"));
    assert!(scroll_node.a11y_actions().is_empty());
    assert_eq!(scroll_node.a11y_children().len(), 1);
    assert_eq!(
        scroll_node.a11y_children()[0].a11y_role(),
        AccessibilityRole::Text
    );
    assert_eq!(
        scroll_node.a11y_children()[0].a11y_label(),
        Some("Activity")
    );
}

#[test]
fn accessibility_scrollable_tree_exposes_actual_scroll_position() {
    let scroll_id = ElementId::new();
    let mut scrollable = Scrollable::new(container().w(100.0).h(300.0))
        .id(scroll_id)
        .h(100.0)
        .accessibility_label("Activity feed");

    let taffy = layout_and_paint_scrollable(&mut scrollable, Size::new(100.0, 100.0));
    let tree = AccessibilityTree::new(accessibility_result(
        scrollable.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let scroll_node = match tree.find(scroll_id) {
        Some(node) => node,
        None => panic!("scroll area should be in accessibility tree"),
    };
    assert_eq!(
        scroll_node.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 0.0, 0.0, 200.0))
    );
    assert_eq!(
        scroll_node.a11y_actions(),
        [AccessibilityAction::ScrollForward]
    );

    {
        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            &taffy,
            &mut focused,
        );
        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(4.0, 4.0),
                delta_x: 0.0,
                delta_y: 24.0,
                modifiers: Modifiers::default(),
            },
        ));
    }

    let tree = AccessibilityTree::new(accessibility_result(
        scrollable.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let scroll_node = match tree.find(scroll_id) {
        Some(node) => node,
        None => panic!("scroll area should be in accessibility tree"),
    };
    assert_eq!(
        scroll_node.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
    );

    layout_and_paint_scrollable(&mut scrollable, Size::new(100.0, 100.0));
    let tree = AccessibilityTree::new(accessibility_result(
        scrollable.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let scroll_node = match tree.find(scroll_id) {
        Some(node) => node,
        None => panic!("scroll area should be in accessibility tree"),
    };
    assert_eq!(
        scroll_node.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
    );
}

#[test]
fn accessibility_input_exposes_text_editing_semantics() {
    let id = ElementId::new();
    let mut input = Input::new()
        .id(id)
        .accessibility_label("Search")
        .value("alpha");
    input
        .apply_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::shift()))
        .expect("shift-left should create a selection");

    let node = first_node(accessibility_result(
        input.accessibility_nodes(&AccessibilityContext::new(Some(id))),
    ));
    assert_eq!(node.a11y_role(), AccessibilityRole::TextInput);
    assert_eq!(node.a11y_label(), Some("Search"));
    assert_eq!(node.a11y_value(), Some("alpha"));
    assert_eq!(node.a11y_text_caret(), Some(4));
    assert_eq!(
        node.a11y_text_selection(),
        Some(AccessibilityTextRange::new(4, 5))
    );
    assert!(node.a11y_focused());
    assert_eq!(node.a11y_actions(), [AccessibilityAction::SetValue]);
}

#[test]
fn accessibility_input_exposes_composition_range() {
    let id = ElementId::new();
    let mut input = Input::new().id(id).placeholder("Message").value("Hi ");
    input
        .apply_text_input_event(TextInputEvent::BeginComposition("你".to_string()))
        .expect("composition should begin");

    let node = first_node(accessibility_result(
        input.accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(node.a11y_label(), Some("Message"));
    assert_eq!(node.a11y_value(), Some("Hi 你"));
    assert_eq!(
        node.a11y_text_composition(),
        Some(AccessibilityTextRange::new(3, 6))
    );
}

#[test]
fn accessibility_text_field_applies_advanced_contracts_to_input_semantics() {
    let id = ElementId::new();
    let mut field = TextField::new("Search")
        .id(id)
        .value("alpha")
        .read_only(true)
        .invalid(true);
    let ignored = field
        .apply_text_input_event(TextInputEvent::InsertText("x".to_string()))
        .expect("read-only text field should report an ignored edit");
    assert!(!ignored.changed);

    let node = first_node(accessibility_result(
        field.accessibility_nodes(&AccessibilityContext::new(Some(id))),
    ));
    assert_eq!(node.a11y_role(), AccessibilityRole::TextInput);
    assert_eq!(node.a11y_label(), Some("Search"));
    assert_eq!(node.a11y_value(), Some("alpha"));
    assert_eq!(node.a11y_text_caret(), Some(5));
    assert!(node.a11y_read_only());
    assert!(node.a11y_invalid());
    assert!(node.a11y_actions().is_empty());
}

#[test]
fn accessibility_progress_bar_exposes_value_and_validation_state() {
    let id = ElementId::new();
    let bar = ProgressBar::new(0.42)
        .id(id)
        .accessibility_label("Load progress")
        .invalid(true);

    let node = first_node(accessibility_result(
        bar.accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(node.a11y_role(), AccessibilityRole::ProgressIndicator);
    assert_eq!(node.a11y_label(), Some("Load progress"));
    assert_eq!(node.a11y_value(), Some("42%"));
    assert!(node.a11y_invalid());
}

#[test]
fn accessibility_menu_popover_and_dialog_expose_semantics() {
    let menu_id = ElementId::new();
    let menu = Menu::new("File", [("new", "New"), ("open", "Open")])
        .id(menu_id)
        .selected("open");
    let menu_node = first_node(accessibility_result(
        menu.accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(menu_node.a11y_role(), AccessibilityRole::Menu);
    assert_eq!(menu_node.a11y_label(), Some("File"));
    assert_eq!(menu_node.a11y_value(), Some("open"));
    assert_eq!(menu_node.a11y_children().len(), 2);
    assert_eq!(
        menu_node.a11y_children()[1].a11y_role(),
        AccessibilityRole::MenuItem
    );
    assert_eq!(menu_node.a11y_children()[1].a11y_selected(), Some(true));
    assert_eq!(
        menu_node.a11y_children()[1].a11y_actions(),
        [AccessibilityAction::Activate]
    );

    let popover_id = ElementId::new();
    let popover = Popover::new("Inspector", button("Open"), text("Details"))
        .id(popover_id)
        .open(true);
    let popover_node = first_node(accessibility_result(
        popover.accessibility_nodes(&AccessibilityContext::new(Some(popover_id))),
    ));
    assert_eq!(popover_node.a11y_role(), AccessibilityRole::Popover);
    assert_eq!(popover_node.a11y_label(), Some("Inspector"));
    assert!(popover_node.a11y_focused());
    assert_eq!(popover_node.a11y_children().len(), 2);

    let dialog_id = ElementId::new();
    let dialog = Dialog::new(
        "Confirm delete",
        container().w(120.0).h(80.0).child(text("Delete?")),
    )
    .id(dialog_id);
    let dialog_node = first_node(accessibility_result(
        dialog.accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(dialog_node.a11y_role(), AccessibilityRole::Dialog);
    assert_eq!(dialog_node.a11y_label(), Some("Confirm delete"));
    assert_eq!(dialog_node.a11y_children().len(), 1);
}

#[test]
fn accessibility_missing_required_labels_are_errors() {
    let button_panic = std::panic::catch_unwind(|| drop(Button::new(" ")));
    assert!(button_panic.is_err());

    let control = SegmentedControl::new([("list", "List")], "list");
    let error = match control.accessibility_nodes(&AccessibilityContext::default()) {
        Ok(_) => panic!("segmented control without accessibility label should fail"),
        Err(err) => err,
    };
    assert_eq!(
        error,
        AccessibilityError::MissingLabel {
            role: AccessibilityRole::SegmentedControl
        }
    );

    let tabs = Tabs::new([("overview", "Overview")], "overview");
    let error = match tabs.accessibility_nodes(&AccessibilityContext::default()) {
        Ok(_) => panic!("tabs without accessibility label should fail"),
        Err(err) => err,
    };
    assert_eq!(
        error,
        AccessibilityError::MissingLabel {
            role: AccessibilityRole::TabList
        }
    );

    let id = ElementId::new();
    let input = Input::new().id(id);
    let error = match input.accessibility_nodes(&AccessibilityContext::default()) {
        Ok(_) => panic!("input without accessibility label or placeholder should fail"),
        Err(err) => err,
    };
    assert_eq!(
        error,
        AccessibilityError::MissingLabel {
            role: AccessibilityRole::TextInput
        }
    );

    let node = first_node(accessibility_result(
        ProgressBar::new(0.5).accessibility_nodes(&AccessibilityContext::default()),
    ));
    assert_eq!(node.a11y_label(), Some("Progress"));
}

#[test]
fn accessibility_data_primitives_expose_semantic_trees() {
    let list_id = ElementId::new();
    let list = DataList::new([("open", "Open work"), ("done", "Done work")])
        .id(list_id)
        .accessibility_label("Work queue")
        .selected("done");
    let list_tree = AccessibilityTree::new(accessibility_result(
        list.accessibility_nodes(&AccessibilityContext::new(Some(list_id))),
    ));
    let list_node = match list_tree.find(list_id) {
        Some(node) => node,
        None => panic!("data list should be in accessibility tree"),
    };
    assert_eq!(list_node.a11y_role(), AccessibilityRole::DataList);
    assert_eq!(list_node.a11y_label(), Some("Work queue"));
    assert_eq!(list_node.a11y_value(), Some("done"));
    assert!(list_node.a11y_focused());
    assert_eq!(list_node.a11y_children().len(), 2);
    assert_eq!(
        list_node.a11y_children()[1].a11y_role(),
        AccessibilityRole::DataListItem
    );
    assert_eq!(list_node.a11y_children()[1].a11y_selected(), Some(true));

    let tree_id = ElementId::new();
    let child_id = ElementId::new();
    let tree = DataTree::new([DataTreeItem::new("src", "src")
        .child(DataTreeItem::new("advanced", "advanced_ui").id(child_id))])
    .id(tree_id)
    .accessibility_label("Project tree")
    .selected("advanced");
    let data_tree = AccessibilityTree::new(accessibility_result(
        tree.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let tree_node = match data_tree.find(tree_id) {
        Some(node) => node,
        None => panic!("data tree should be in accessibility tree"),
    };
    assert_eq!(tree_node.a11y_role(), AccessibilityRole::DataTree);
    assert_eq!(tree_node.a11y_children().len(), 1);
    let child = match data_tree.find(child_id) {
        Some(node) => node,
        None => panic!("data tree child should be in accessibility tree"),
    };
    assert_eq!(child.a11y_role(), AccessibilityRole::DataTreeItem);
    assert_eq!(child.a11y_selected(), Some(true));

    let row_id = ElementId::new();
    let row = DataTableRow::new(["Name", "Status"])
        .id(row_id)
        .accessibility_label("Build row")
        .selected(true);
    let row_tree = AccessibilityTree::new(accessibility_result(
        row.accessibility_nodes(&AccessibilityContext::default()),
    ));
    let row_node = match row_tree.find(row_id) {
        Some(node) => node,
        None => panic!("data table row should be in accessibility tree"),
    };
    assert_eq!(row_node.a11y_role(), AccessibilityRole::DataTableRow);
    assert_eq!(row_node.a11y_label(), Some("Build row"));
    assert_eq!(row_node.a11y_selected(), Some(true));
    assert_eq!(row_node.a11y_children().len(), 2);
    assert_eq!(
        row_node.a11y_children()[0].a11y_role(),
        AccessibilityRole::DataTableCell
    );
}

#[test]
fn accessibility_focus_announcements_are_testable() {
    let id = ElementId::new();
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 40.0, 20.0),
        &taffy,
        &mut focused,
    );

    event_cx.request_focus(Some(id));
    let announcements = event_cx.accessibility_announcements();
    assert_eq!(announcements.len(), 1);
    assert_eq!(announcements[0].node_id(), id);
    assert_eq!(
        announcements[0].kind(),
        AccessibilityAnnouncementKind::FocusChanged
    );
}

#[test]
fn accessibility_unsupported_bridge_returns_explicit_error() {
    let mut bridge = UnsupportedAccessibilityBridge::new("test accessibility bridge");
    let tree = AccessibilityTree::default();
    let error = match bridge.publish_tree(&tree) {
        Ok(_) => panic!("unsupported bridge should fail"),
        Err(err) => err,
    };
    assert_eq!(
        error,
        AccessibilityError::UnsupportedPlatformFeature {
            feature: "test accessibility bridge".to_string()
        }
    );
}

#[cfg(target_os = "macos")]
#[test]
fn accessibility_macos_bridge_reports_missing_native_host() {
    let mut bridge = rui::platform::mac::MacAccessibilityBridge::new();
    let tree = AccessibilityTree::default();
    let error = match bridge.publish_tree(&tree) {
        Ok(_) => panic!("macOS bridge without native host should fail"),
        Err(err) => err,
    };
    assert!(matches!(error, AccessibilityError::BridgeFailure { .. }));
}
