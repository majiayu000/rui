use rui::advanced_ui::{Button, Checkbox, Scrollable, SegmentedControl, text};
use rui::core::ElementId;
use rui::core::accessibility::{
    AccessibilityAnnouncementKind, AccessibilityBridge, AccessibilityContext, AccessibilityError,
    AccessibilityNode, AccessibilityRole, AccessibilityTree, UnsupportedAccessibilityBridge,
};
use rui::core::event::MouseButton;
use rui::core::geometry::{Bounds, Point};
use rui::elements::Element;
use rui::elements::element::{EventContext, PointerEvent, PointerEventKind};
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
    let button = Button::new("Save").id(id).disabled(true);
    let cx = AccessibilityContext::new(Some(id));

    let node = first_node(accessibility_result(button.accessibility_nodes(&cx)));
    assert_eq!(node.a11y_id(), id);
    assert_eq!(node.a11y_role(), AccessibilityRole::Button);
    assert_eq!(node.a11y_label(), Some("Save"));
    assert!(!node.a11y_enabled());
    assert!(node.a11y_focused());
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
