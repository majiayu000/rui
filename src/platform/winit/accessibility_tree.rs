//! The presenter's accessibility tree as AccessKit nodes, and AccessKit
//! action requests as the requests every runner dispatches.

use super::accessibility_announce::ANNOUNCERS;
use super::accessibility_text::{offset, text_runs};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityNode, AccessibilityRole, AccessibilityTree,
};
use crate::core::text_editing::TextInputGeometry;
use crate::platform::accessibility_request::AccessibilityRequest;
use accesskit::{
    Action, ActionData, ActionRequest, Affine, Invalid, Live, Node, NodeId, Rect, Role, Toggled,
    TreeId, TreeInfo, TreeUpdate,
};
use std::collections::{HashMap, HashSet};

/// The window itself, the parent of the presenter's root nodes. Element ids
/// map to node ids one to one, and no element is given this one.
pub(super) const WINDOW_NODE: NodeId = NodeId(u64::MAX);

/// The layouts of the text inputs that have one, which place their
/// characters.
pub(super) type TextGeometry = HashMap<ElementId, TextInputGeometry>;

/// A full AccessKit tree for one window: a window node titled `title` over
/// `tree`'s roots. Element bounds are logical, so the window node scales them
/// by `scale_factor` to the physical pixels AccessKit expects. Focus is the
/// focused element, or the window when no element has focus.
pub(super) fn tree_update(
    tree: &AccessibilityTree,
    text: &TextGeometry,
    title: &str,
    scale_factor: f64,
) -> TreeUpdate {
    let mut builder = Builder {
        nodes: Vec::new(),
        seen: HashSet::from([WINDOW_NODE, ANNOUNCERS[0], ANNOUNCERS[1]]),
        focus: None,
        text,
    };
    let mut window = Node::new(Role::Window);
    window.set_label(title);
    window.set_transform(Affine::scale(scale_factor));
    window.set_children(
        tree.roots()
            .iter()
            .filter_map(|root| builder.push(root))
            .collect::<Vec<_>>(),
    );
    let Builder {
        mut nodes, focus, ..
    } = builder;
    nodes.push((WINDOW_NODE, window));
    let mut info = TreeInfo::new(WINDOW_NODE);
    info.toolkit_name = Some("rui".to_string());
    info.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_string());
    TreeUpdate {
        nodes,
        tree: Some(info),
        tree_id: TreeId::ROOT,
        focus: focus.unwrap_or(WINDOW_NODE),
    }
}

/// What a screen reader asked for, as the element it names and the request
/// the shared dispatch takes. A text selection names positions on the runs
/// of `published`, the tree the platform holds. None for the window node and
/// for actions rui does not offer.
pub(super) fn request_for_action(
    request: &ActionRequest,
    published: Option<&AccessibilityTree>,
) -> Option<(ElementId, AccessibilityRequest)> {
    if request.target_node == WINDOW_NODE || request.target_tree != TreeId::ROOT {
        return None;
    }
    let id = ElementId(request.target_node.0);
    let action = |action| AccessibilityRequest::Action {
        action,
        value: None,
    };
    let request = match request.action {
        Action::Click => action(AccessibilityAction::Activate),
        Action::Focus => AccessibilityRequest::Focus(true),
        Action::Blur => AccessibilityRequest::Focus(false),
        Action::Increment => action(AccessibilityAction::Increment),
        Action::Decrement => action(AccessibilityAction::Decrement),
        Action::ScrollDown | Action::ScrollRight => action(AccessibilityAction::ScrollForward),
        Action::ScrollUp | Action::ScrollLeft => action(AccessibilityAction::ScrollBackward),
        Action::SetValue => match &request.data {
            Some(ActionData::Value(value)) => AccessibilityRequest::Action {
                action: AccessibilityAction::SetValue,
                value: Some(value.to_string()),
            },
            Some(ActionData::NumericValue(value)) => AccessibilityRequest::Action {
                action: AccessibilityAction::SetValue,
                value: Some(value.to_string()),
            },
            _ => return None,
        },
        Action::ReplaceSelectedText => match &request.data {
            Some(ActionData::Value(text)) => {
                AccessibilityRequest::ReplaceSelectedText(text.to_string())
            }
            _ => return None,
        },
        Action::SetTextSelection => match &request.data {
            Some(ActionData::SetTextSelection(selection)) => {
                let node = published?.find(id)?;
                AccessibilityRequest::SetTextSelection {
                    anchor: offset(node, selection.anchor)?,
                    focus: offset(node, selection.focus)?,
                }
            }
            _ => return None,
        },
        _ => return None,
    };
    Some((id, request))
}

fn node_id(node: &AccessibilityNode) -> NodeId {
    NodeId(node.a11y_id().0)
}

/// The nodes of one update, each id once.
struct Builder<'a> {
    nodes: Vec<(NodeId, Node)>,
    seen: HashSet<NodeId>,
    /// The first focused node.
    focus: Option<NodeId>,
    text: &'a TextGeometry,
}

impl Builder<'_> {
    /// Add `node` and its subtree, returning its id. A node whose id is
    /// already in the update is left out with its subtree, as AccessKit
    /// rejects an update that names a node twice.
    fn push(&mut self, node: &AccessibilityNode) -> Option<NodeId> {
        let id = node_id(node);
        if !self.seen.insert(id) {
            log::warn!(
                "left element {:?} out of the winit accessibility tree: its id is used twice",
                node.a11y_id()
            );
            return None;
        }
        if node.a11y_focused() && self.focus.is_none() {
            self.focus = Some(id);
        }
        let mut out = accesskit_node(node);
        let mut children = Vec::new();
        if node.a11y_role() == AccessibilityRole::TextInput {
            let (runs, selection) = text_runs(node, self.text.get(&node.a11y_id()));
            if let Some(selection) = selection {
                out.set_text_selection(selection);
            }
            for (run, run_node) in runs {
                if self.seen.insert(run) {
                    children.push(run);
                    self.nodes.push((run, run_node));
                }
            }
        }
        children.extend(
            node.a11y_children()
                .iter()
                .filter_map(|child| self.push(child)),
        );
        out.set_children(children);
        self.nodes.push((id, out));
        Some(id)
    }
}

fn accesskit_node(node: &AccessibilityNode) -> Node {
    let role = node.a11y_role();
    let mut out = Node::new(accesskit_role(node));
    match role {
        // A label's text is its value in AccessKit.
        AccessibilityRole::Text => {
            if let Some(text) = node.a11y_value().or(node.a11y_label()) {
                out.set_value(text);
            }
        }
        // Their state is the toggle below, not the words "checked" or "on".
        AccessibilityRole::Checkbox | AccessibilityRole::Switch => {
            if let Some(label) = node.a11y_label() {
                out.set_label(label);
            }
        }
        _ => {
            if let Some(label) = node.a11y_label() {
                out.set_label(label);
            }
            if let Some(value) = node.a11y_value() {
                out.set_value(value);
            }
        }
    }
    if let Some(numeric) = node.a11y_numeric_value() {
        out.set_numeric_value(numeric.now());
        out.set_min_numeric_value(numeric.min());
        out.set_max_numeric_value(numeric.max());
    } else if role == AccessibilityRole::ProgressIndicator
        && let Some(percent) = node
            .a11y_value()
            .and_then(|value| value.strip_suffix('%'))
            .and_then(|value| value.trim().parse::<f64>().ok())
    {
        out.set_numeric_value(percent);
        out.set_min_numeric_value(0.0);
        out.set_max_numeric_value(100.0);
    }
    if let Some(checked) = node.a11y_checked() {
        out.set_toggled(Toggled::from(checked));
    }
    if let Some(selected) = node.a11y_selected() {
        if out.role() == Role::RadioButton {
            out.set_toggled(Toggled::from(selected));
        } else {
            out.set_selected(selected);
        }
    }
    if !node.a11y_enabled() {
        out.set_disabled();
    }
    if node.a11y_read_only() {
        out.set_read_only();
    }
    if node.a11y_invalid() {
        out.set_invalid(Invalid::True);
    }
    if role == AccessibilityRole::Status {
        out.set_live(Live::Polite);
    }
    if let Some(bounds) = node.a11y_bounds() {
        out.set_bounds(Rect::new(
            f64::from(bounds.x()),
            f64::from(bounds.y()),
            f64::from(bounds.x() + bounds.width()),
            f64::from(bounds.y() + bounds.height()),
        ));
    }
    if let Some(scroll) = node.a11y_scroll_position() {
        out.set_scroll_x(f64::from(scroll.offset_x()));
        out.set_scroll_x_min(0.0);
        out.set_scroll_x_max(f64::from(scroll.max_x()));
        out.set_scroll_y(f64::from(scroll.offset_y()));
        out.set_scroll_y_min(0.0);
        out.set_scroll_y_max(f64::from(scroll.max_y()));
    }
    for action in node.a11y_actions() {
        out.add_action(match action {
            AccessibilityAction::Activate => Action::Click,
            AccessibilityAction::SetValue => Action::SetValue,
            AccessibilityAction::Increment => Action::Increment,
            AccessibilityAction::Decrement => Action::Decrement,
            AccessibilityAction::ScrollForward => Action::ScrollDown,
            AccessibilityAction::ScrollBackward => Action::ScrollUp,
        });
    }
    // Read-only text fields still accept focus for selection and copying.
    if node.a11y_enabled()
        && (role == AccessibilityRole::TextInput || !node.a11y_actions().is_empty())
    {
        out.add_action(Action::Focus);
    }
    if role == AccessibilityRole::TextInput && node.a11y_enabled() {
        out.add_action(Action::SetTextSelection);
        if node.a11y_actions().contains(&AccessibilityAction::SetValue) {
            out.add_action(Action::ReplaceSelectedText);
        }
    }
    out
}

fn accesskit_role(node: &AccessibilityNode) -> Role {
    match node.a11y_role() {
        AccessibilityRole::Button => Role::Button,
        AccessibilityRole::Checkbox => Role::CheckBox,
        AccessibilityRole::ComboBox => Role::ComboBox,
        AccessibilityRole::DataList => Role::List,
        AccessibilityRole::DataListItem => Role::ListItem,
        AccessibilityRole::DataTableCell => Role::Cell,
        AccessibilityRole::DataTableRow => Role::Row,
        AccessibilityRole::DataTree => Role::Tree,
        AccessibilityRole::DataTreeItem => Role::TreeItem,
        AccessibilityRole::Dialog => Role::Dialog,
        AccessibilityRole::Grid => Role::Grid,
        AccessibilityRole::GridCell => Role::GridCell,
        AccessibilityRole::Menu => Role::Menu,
        AccessibilityRole::MenuItem => Role::MenuItem,
        AccessibilityRole::Popover => Role::Group,
        AccessibilityRole::ProgressIndicator => Role::ProgressIndicator,
        AccessibilityRole::RadioButton | AccessibilityRole::SegmentedOption => Role::RadioButton,
        AccessibilityRole::RadioGroup | AccessibilityRole::SegmentedControl => Role::RadioGroup,
        AccessibilityRole::Slider => Role::Slider,
        AccessibilityRole::Status => Role::Status,
        AccessibilityRole::Splitter => Role::Splitter,
        AccessibilityRole::Switch => Role::Switch,
        AccessibilityRole::Tab => Role::Tab,
        AccessibilityRole::TabList => Role::TabList,
        AccessibilityRole::TabPanel => Role::TabPanel,
        AccessibilityRole::Text => Role::Label,
        AccessibilityRole::TextInput if node.a11y_multiline() => Role::MultilineTextInput,
        AccessibilityRole::TextInput => Role::TextInput,
        AccessibilityRole::ScrollArea => Role::ScrollView,
        AccessibilityRole::Toolbar => Role::Toolbar,
    }
}
