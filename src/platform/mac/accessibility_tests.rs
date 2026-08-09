use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Default)]
struct RecordingState {
    published_roots: usize,
    announcements: Vec<(ElementId, AccessibilityAnnouncementKind, String)>,
    action_requests: Vec<MacAccessibilityActionRequest>,
}

struct RecordingHost {
    state: Rc<RefCell<RecordingState>>,
}

impl NativeAccessibilityHost for RecordingHost {
    fn publish_tree(&mut self, tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        self.state.borrow_mut().published_roots = tree.roots().len();
        Ok(())
    }

    fn announce(
        &mut self,
        announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        self.state.borrow_mut().announcements.push((
            announcement.node_id(),
            announcement.kind(),
            announcement.message().to_string(),
        ));
        Ok(())
    }

    fn take_action_request(&mut self) -> Option<MacAccessibilityActionRequest> {
        let mut state = self.state.borrow_mut();
        if state.action_requests.is_empty() {
            None
        } else {
            Some(state.action_requests.remove(0))
        }
    }
}

fn attached_bridge() -> (MacAccessibilityBridge, Rc<RefCell<RecordingState>>) {
    let state = Rc::new(RefCell::new(RecordingState::default()));
    let bridge = MacAccessibilityBridge::with_host(RecordingHost {
        state: Rc::clone(&state),
    });
    (bridge, state)
}

#[test]
fn accessibility_bridge_unattached_operations_fail_explicitly() {
    let id = ElementId::new();
    let tree = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(id, AccessibilityRole::Button, "Save")
            .expect("button metadata should be valid"),
    ]);
    let announcement =
        AccessibilityAnnouncement::new(id, AccessibilityAnnouncementKind::FocusChanged, "Focused");
    let mut bridge = MacAccessibilityBridge::new();

    assert!(!bridge.native_attached());
    assert!(matches!(
        bridge.publish_tree(&tree),
        Err(AccessibilityError::BridgeFailure { .. })
    ));
    assert_eq!(bridge.snapshot_tree(&tree).nodes()[0].id(), id);
    assert!(matches!(
        bridge.announce(&announcement),
        Err(AccessibilityError::BridgeFailure { .. })
    ));
}

#[test]
fn accessibility_bridge_attached_host_receives_tree_focus_and_announcement() {
    let root_id = ElementId::new();
    let child_id = ElementId::new();
    let child = AccessibilityNode::label_required(child_id, AccessibilityRole::Text, "Status")
        .expect("text metadata should be valid")
        .with_value("Ready");
    let root = AccessibilityNode::label_required(root_id, AccessibilityRole::Checkbox, "Sync")
        .expect("checkbox metadata should be valid")
        .with_value("checked")
        .with_focused(true)
        .with_checked(true)
        .with_action(AccessibilityAction::Activate)
        .with_child(child);
    let tree = AccessibilityTree::new(vec![root]);
    let announcement = AccessibilityAnnouncement::new(
        root_id,
        AccessibilityAnnouncementKind::ActionFeedback,
        "Sync checked",
    );
    let (mut bridge, state) = attached_bridge();

    assert!(bridge.native_attached());
    bridge
        .publish_tree(&tree)
        .expect("attached publish should succeed");
    bridge
        .announce(&announcement)
        .expect("attached announcement should succeed");

    let snapshot = bridge.snapshot_tree(&tree);
    let node = snapshot
        .nodes()
        .first()
        .expect("snapshot should have a root");
    assert_eq!(node.id(), root_id);
    assert_eq!(node.native_role(), "AXCheckBox");
    assert_eq!(node.label(), Some("Sync"));
    assert!(node.focused());
    assert_eq!(node.native_actions(), ["AXPress"]);
    assert_eq!(node.children()[0].id(), child_id);

    let state = state.borrow();
    assert_eq!(state.published_roots, 1);
    assert_eq!(
        state.announcements,
        vec![(
            root_id,
            AccessibilityAnnouncementKind::ActionFeedback,
            "Sync checked".to_string(),
        )]
    );
}

#[test]
fn accessibility_bridge_fails_closed_for_required_metadata() {
    let (mut bridge, state) = attached_bridge();
    let missing_label = AccessibilityTree::new(vec![AccessibilityNode::new(
        ElementId::new(),
        AccessibilityRole::Button,
    )]);
    assert_eq!(
        bridge.publish_tree(&missing_label),
        Err(AccessibilityError::MissingLabel {
            role: AccessibilityRole::Button,
        })
    );

    let missing_value = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(ElementId::new(), AccessibilityRole::Checkbox, "Sync")
            .expect("checkbox label should be valid"),
    ]);
    assert_eq!(
        bridge.publish_tree(&missing_value),
        Err(AccessibilityError::MissingValue {
            role: AccessibilityRole::Checkbox,
        })
    );
    assert_eq!(state.borrow().published_roots, 0);
}

#[test]
fn accessibility_bridge_accepts_an_unselected_data_list_without_a_value() {
    let (mut bridge, state) = attached_bridge();
    let tree = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(
            ElementId::new(),
            AccessibilityRole::DataList,
            "Projects",
        )
        .expect("data list label should be valid"),
    ]);

    bridge
        .publish_tree(&tree)
        .expect("an unselected data list does not require a value");
    assert_eq!(state.borrow().published_roots, 1);
}

#[test]
fn accessibility_bridge_drains_native_action_requests_once() {
    let (mut bridge, state) = attached_bridge();
    let first = MacAccessibilityActionRequest {
        id: ElementId::new(),
        request: MacAccessibilityRequest::Action {
            action: AccessibilityAction::Activate,
            value: None,
        },
        bounds: Bounds::from_xywh(10.0, 20.0, 30.0, 40.0),
    };
    let second = MacAccessibilityActionRequest {
        id: ElementId::new(),
        request: MacAccessibilityRequest::Focus(true),
        bounds: Bounds::from_xywh(50.0, 60.0, 70.0, 80.0),
    };
    state
        .borrow_mut()
        .action_requests
        .extend([first.clone(), second.clone()]);

    assert_eq!(bridge.take_action_request(), Some(first));
    assert_eq!(bridge.take_action_request(), Some(second));
    assert_eq!(bridge.take_action_request(), None);
}

#[test]
fn accessibility_layout_notifications_only_cover_structure_and_geometry() {
    let id = ElementId::new();
    let previous = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(id, AccessibilityRole::Text, "Status")
            .expect("text metadata should be valid")
            .with_value("Ready")
            .with_bounds(Bounds::from_xywh(1.0, 2.0, 30.0, 40.0)),
    ]);
    let value_changed = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(id, AccessibilityRole::Text, "Status")
            .expect("text metadata should be valid")
            .with_value("Done")
            .with_bounds(Bounds::from_xywh(1.0, 2.0, 30.0, 40.0)),
    ]);
    let bounds_changed = AccessibilityTree::new(vec![
        AccessibilityNode::label_required(id, AccessibilityRole::Text, "Status")
            .expect("text metadata should be valid")
            .with_value("Done")
            .with_bounds(Bounds::from_xywh(2.0, 2.0, 30.0, 40.0)),
    ]);

    assert!(!accessibility_layout_changed(&previous, &value_changed));
    assert!(accessibility_layout_changed(
        &value_changed,
        &bounds_changed
    ));
}

#[test]
fn native_frame_is_relative_and_flips_the_rui_y_axis() {
    let frame = native_frame_in_parent(
        Bounds::from_xywh(10.0, 20.0, 30.0, 40.0),
        Bounds::from_xywh(0.0, 0.0, 200.0, 100.0),
    );

    assert_eq!(frame.origin.x, 10.0);
    assert_eq!(frame.origin.y, 40.0);
    assert_eq!(frame.size.width, 30.0);
    assert_eq!(frame.size.height, 40.0);
}

#[test]
fn native_text_ranges_convert_utf8_offsets_to_utf16_units() {
    let value = "Hi 你😀";
    assert_eq!(
        native_text_range(value, 3, 6).expect("Chinese character range should convert"),
        NSRange::new(3, 1)
    );
    assert_eq!(
        native_text_range(value, 6, 10).expect("emoji range should convert"),
        NSRange::new(4, 2)
    );
}

#[test]
fn accessibility_bridge_rejects_invalid_text_ranges() {
    let (mut bridge, state) = attached_bridge();
    let node =
        AccessibilityNode::label_required(ElementId::new(), AccessibilityRole::TextInput, "Name")
            .expect("text input label should be valid")
            .with_value("你")
            .with_text_selection(crate::core::accessibility::AccessibilityTextRange::new(
                1, 2,
            ));

    assert!(matches!(
        bridge.publish_tree(&AccessibilityTree::new(vec![node])),
        Err(AccessibilityError::BridgeFailure { .. })
    ));
    assert_eq!(state.borrow().published_roots, 0);
}

#[test]
fn native_selector_mapping_is_limited_to_declared_action_selectors() {
    assert_eq!(
        action_for_selector(sel!(accessibilityPerformPress)),
        Some(AccessibilityAction::Activate)
    );
    assert_eq!(
        action_for_selector(sel!(setAccessibilityValue:)),
        Some(AccessibilityAction::SetValue)
    );
    assert_eq!(
        action_for_selector(sel!(accessibilityPerformIncrement)),
        Some(AccessibilityAction::ScrollForward)
    );
    assert_eq!(
        action_for_selector(sel!(accessibilityPerformDecrement)),
        Some(AccessibilityAction::ScrollBackward)
    );
    assert_eq!(action_for_selector(sel!(accessibilityLabel)), None);
}

#[test]
fn dialogs_publish_as_group_containers_instead_of_nested_windows() {
    let (bridge, _) = attached_bridge();
    let node =
        AccessibilityNode::label_required(ElementId::new(), AccessibilityRole::Dialog, "Confirm")
            .expect("dialog label should be valid");
    let snapshot = bridge.snapshot_tree(&AccessibilityTree::new(vec![node]));

    assert_eq!(snapshot.nodes()[0].native_role(), "AXWindow");
}

#[test]
fn snapshot_action_names_remain_compatible_with_the_public_api() {
    let node = AccessibilityNode::label_required(
        ElementId::new(),
        AccessibilityRole::ScrollArea,
        "Results",
    )
    .expect("scroll area label should be valid")
    .with_actions([
        AccessibilityAction::ScrollForward,
        AccessibilityAction::ScrollBackward,
    ]);
    let snapshot = MacAccessibilityBridge::new().snapshot_tree(&AccessibilityTree::new(vec![node]));

    assert_eq!(
        snapshot.nodes()[0].native_actions(),
        ["AXScrollDown", "AXScrollUp"]
    );
}
