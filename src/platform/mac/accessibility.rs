use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityAnnouncement, AccessibilityAnnouncementKind,
    AccessibilityBridge, AccessibilityError, AccessibilityNode, AccessibilityRole,
    AccessibilityTree,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MacAccessibilitySnapshot {
    nodes: Vec<MacAccessibilityNodeSnapshot>,
}

impl MacAccessibilitySnapshot {
    pub fn nodes(&self) -> &[MacAccessibilityNodeSnapshot] {
        &self.nodes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacAccessibilityNodeSnapshot {
    id: ElementId,
    native_role: &'static str,
    label: Option<String>,
    value: Option<String>,
    enabled: bool,
    read_only: bool,
    invalid: bool,
    focused: bool,
    selected: Option<bool>,
    checked: Option<bool>,
    native_actions: Vec<&'static str>,
    children: Vec<MacAccessibilityNodeSnapshot>,
}

impl MacAccessibilityNodeSnapshot {
    pub fn id(&self) -> ElementId {
        self.id
    }

    pub fn native_role(&self) -> &'static str {
        self.native_role
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn invalid(&self) -> bool {
        self.invalid
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn selected(&self) -> Option<bool> {
        self.selected
    }

    pub fn checked(&self) -> Option<bool> {
        self.checked
    }

    pub fn native_actions(&self) -> &[&'static str] {
        &self.native_actions
    }

    pub fn children(&self) -> &[MacAccessibilityNodeSnapshot] {
        &self.children
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacAccessibilityAnnouncementSnapshot {
    node_id: ElementId,
    native_notification: &'static str,
    message: String,
}

impl MacAccessibilityAnnouncementSnapshot {
    pub fn node_id(&self) -> ElementId {
        self.node_id
    }

    pub fn native_notification(&self) -> &'static str {
        self.native_notification
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Default)]
pub struct MacAccessibilityBridge {
    native_attached: bool,
}

impl MacAccessibilityBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn native_attached(&self) -> bool {
        self.native_attached
    }

    pub fn snapshot_tree(&self, tree: &AccessibilityTree) -> MacAccessibilitySnapshot {
        MacAccessibilitySnapshot {
            nodes: tree.roots().iter().map(snapshot_node).collect(),
        }
    }

    pub fn snapshot_announcement(
        &self,
        announcement: &AccessibilityAnnouncement,
    ) -> MacAccessibilityAnnouncementSnapshot {
        MacAccessibilityAnnouncementSnapshot {
            node_id: announcement.node_id(),
            native_notification: native_notification_name(announcement.kind()),
            message: announcement.message().to_string(),
        }
    }

    fn missing_native_bridge() -> AccessibilityError {
        AccessibilityError::BridgeFailure {
            message: "macOS accessibility bridge is not attached to a native AppKit host"
                .to_string(),
        }
    }
}

fn snapshot_node(node: &AccessibilityNode) -> MacAccessibilityNodeSnapshot {
    MacAccessibilityNodeSnapshot {
        id: node.a11y_id(),
        native_role: native_role_name(node.a11y_role()),
        label: node.a11y_label().map(str::to_string),
        value: node.a11y_value().map(str::to_string),
        enabled: node.a11y_enabled(),
        read_only: node.a11y_read_only(),
        invalid: node.a11y_invalid(),
        focused: node.a11y_focused(),
        selected: node.a11y_selected(),
        checked: node.a11y_checked(),
        native_actions: node
            .a11y_actions()
            .iter()
            .copied()
            .map(native_action_name)
            .collect(),
        children: node.a11y_children().iter().map(snapshot_node).collect(),
    }
}

fn native_role_name(role: AccessibilityRole) -> &'static str {
    match role {
        AccessibilityRole::Button => "AXButton",
        AccessibilityRole::Checkbox => "AXCheckBox",
        AccessibilityRole::DataList => "AXList",
        AccessibilityRole::DataListItem => "AXRow",
        AccessibilityRole::DataTableCell => "AXCell",
        AccessibilityRole::DataTableRow => "AXRow",
        AccessibilityRole::DataTree => "AXOutline",
        AccessibilityRole::DataTreeItem => "AXRow",
        AccessibilityRole::Dialog => "AXWindow",
        AccessibilityRole::Menu => "AXMenu",
        AccessibilityRole::MenuItem => "AXMenuItem",
        AccessibilityRole::Popover => "AXGroup",
        AccessibilityRole::ProgressIndicator => "AXProgressIndicator",
        AccessibilityRole::SegmentedControl => "AXGroup",
        AccessibilityRole::SegmentedOption => "AXRadioButton",
        AccessibilityRole::Tab => "AXRadioButton",
        AccessibilityRole::TabList => "AXTabGroup",
        AccessibilityRole::TabPanel => "AXGroup",
        AccessibilityRole::Text => "AXStaticText",
        AccessibilityRole::TextInput => "AXTextField",
        AccessibilityRole::ScrollArea => "AXScrollArea",
        AccessibilityRole::Toolbar => "AXToolbar",
    }
}

fn native_action_name(action: AccessibilityAction) -> &'static str {
    match action {
        AccessibilityAction::Activate => "AXPress",
        AccessibilityAction::SetValue => "AXSetValue",
        AccessibilityAction::ScrollForward => "AXScrollDown",
        AccessibilityAction::ScrollBackward => "AXScrollUp",
    }
}

fn native_notification_name(kind: AccessibilityAnnouncementKind) -> &'static str {
    match kind {
        AccessibilityAnnouncementKind::FocusChanged => "AXFocusedUIElementChanged",
        AccessibilityAnnouncementKind::ActionFeedback => "AXAnnouncementRequested",
    }
}

impl AccessibilityBridge for MacAccessibilityBridge {
    fn publish_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        Err(Self::missing_native_bridge())
    }

    fn announce(
        &mut self,
        _announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        Err(Self::missing_native_bridge())
    }
}
