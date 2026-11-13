//! Moving keyboard focus with Tab and Shift-Tab when no element takes them.

use super::{ActionId, SYNC_ACCESSIBILITY_FOCUS_ACTION};
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityNode, AccessibilityRole};
use crate::elements::Element;
use crate::elements::element::EventContext;

/// Focus the next (or previous) enabled control after the focused one, in
/// tree order, wrapping around. Returns whether focus moved.
pub(crate) fn move_focus<E: Element>(root: &mut E, cx: &mut EventContext, forward: bool) -> bool {
    let nodes = match root.accessibility_nodes(&AccessibilityContext::new(cx.focused_id())) {
        Ok(nodes) => nodes,
        Err(err) => {
            log::error!("cannot move focus: the tree's accessibility nodes failed: {err}");
            return false;
        }
    };
    let mut order = Vec::new();
    collect_focusable(&nodes, &mut order);
    let Some(last) = order.len().checked_sub(1) else {
        return false;
    };
    let current = cx
        .focused_id()
        .and_then(|focused| order.iter().position(|id| *id == focused));
    let next = match (current, forward) {
        (Some(index), true) if index < last => index + 1,
        (Some(_), true) | (None, true) => 0,
        (Some(index), false) if index > 0 => index - 1,
        (Some(_), false) | (None, false) => last,
    };
    if cx.focused_id() == Some(order[next]) {
        return false;
    }
    // Blur first, so the old element's on_blur runs before the new one's on_focus.
    cx.clear_focus();
    sync_focus(root, cx);
    cx.request_focus(Some(order[next]));
    sync_focus(root, cx);
    cx.request_redraw();
    true
}

/// Let elements whose focus changed react, running their focus handlers.
pub(crate) fn sync_focus<E: Element>(root: &mut E, cx: &mut EventContext) {
    root.dispatch_action(cx, &ActionId::custom(SYNC_ACCESSIBILITY_FOCUS_ACTION));
}

/// Controls that take keyboard focus as a whole. Their parts, such as a tab
/// list's tabs, are reached with the arrow keys once the control has focus.
fn takes_focus(role: AccessibilityRole) -> bool {
    matches!(
        role,
        AccessibilityRole::Button
            | AccessibilityRole::Checkbox
            | AccessibilityRole::TextInput
            | AccessibilityRole::SegmentedControl
            | AccessibilityRole::TabList
            | AccessibilityRole::Menu
            | AccessibilityRole::ComboBox
            | AccessibilityRole::RadioGroup
            | AccessibilityRole::Slider
            | AccessibilityRole::Splitter
            | AccessibilityRole::Switch
    )
}

fn collect_focusable(nodes: &[AccessibilityNode], order: &mut Vec<ElementId>) {
    for node in nodes {
        if takes_focus(node.a11y_role()) {
            if node.a11y_enabled() {
                order.push(node.a11y_id());
            }
            continue;
        }
        collect_focusable(node.a11y_children(), order);
    }
}
