//! Moving keyboard focus with Tab and Shift-Tab when no element takes them.

use super::{ActionId, SYNC_ACCESSIBILITY_FOCUS_ACTION};
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityNode, AccessibilityRole};
use crate::elements::Element;
use crate::elements::element::EventContext;

/// Focus the next (or previous) enabled control after the focused one, in
/// tree order, wrapping around. Returns whether focus moved. Inside an open
/// modal overlay, focus stays among the modal's controls.
pub(crate) fn move_focus<E: Element>(root: &mut E, cx: &mut EventContext, forward: bool) -> bool {
    let Some(order) = focus_order(root, cx.focused_id(), cx.modal_scope()) else {
        return false;
    };
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

/// The controls Tab moves between, in tree order: those inside `scope` when
/// it is in the tree, else all of them. `None` when the tree cannot say.
pub(crate) fn focus_order<E: Element>(
    root: &E,
    focused: Option<ElementId>,
    scope: Option<ElementId>,
) -> Option<Vec<ElementId>> {
    let nodes = match root.accessibility_nodes(&AccessibilityContext::new(focused)) {
        Ok(nodes) => nodes,
        Err(err) => {
            log::error!("cannot move focus: the tree's accessibility nodes failed: {err}");
            return None;
        }
    };
    let mut order = Vec::new();
    match scope.and_then(|scope| find_node(&nodes, scope)) {
        Some(scope) => collect_focusable(scope.a11y_children(), &mut order),
        None => collect_focusable(&nodes, &mut order),
    }
    Some(order)
}

/// Whether `id` is `scope` or inside it.
pub(crate) fn scope_contains<E: Element>(root: &E, scope: ElementId, id: ElementId) -> bool {
    let Ok(nodes) = root.accessibility_nodes(&AccessibilityContext::new(None)) else {
        return false;
    };
    find_node(&nodes, scope)
        .is_some_and(|scope| find_node(std::slice::from_ref(scope), id).is_some())
}

fn find_node(nodes: &[AccessibilityNode], id: ElementId) -> Option<&AccessibilityNode> {
    nodes.iter().find_map(|node| {
        if node.a11y_id() == id {
            Some(node)
        } else {
            find_node(node.a11y_children(), id)
        }
    })
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
