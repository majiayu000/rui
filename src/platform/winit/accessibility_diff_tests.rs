//! Incremental updates are right: after each change, the tree a platform
//! adapter holds from applying only the changed nodes is the tree it would
//! hold from the whole update.

use super::accessibility::WindowAccessibility;
use super::accessibility_tree::{TextGeometry, WINDOW_NODE, tree_update};
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use accesskit::TreeUpdate;
use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler};
use accesskit_winit::WindowEvent as AccessKitEvent;

/// Applies updates and records nothing.
pub(super) struct Ignore;

impl TreeChangeHandler for Ignore {
    fn node_added(&mut self, _: &NodeRef) {}
    fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
    fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
    fn node_removed(&mut self, _: &NodeRef) {}
}

/// Apply `update` to `held`, the tree a platform adapter holds, creating it
/// from the first update.
pub(super) fn apply(held: &mut Option<Tree>, update: TreeUpdate) {
    match held {
        Some(tree) => tree.update_and_process_changes(update, &mut Ignore),
        None => *held = Some(Tree::new(update, true)),
    }
}

/// Every node reachable from the root, with its data and children in order.
pub(super) fn describe(tree: &Tree) -> Vec<String> {
    fn walk(node: NodeRef<'_>, out: &mut Vec<String>) {
        let children: Vec<_> = node.children().map(|child| child.locate().0).collect();
        out.push(format!(
            "{:?} {:?} {:?}",
            node.locate().0,
            node.data(),
            children
        ));
        for child in node.children() {
            walk(child, out);
        }
    }
    let mut out = vec![format!(
        "focus {:?}",
        tree.state().focus().map(|node| node.locate().0)
    )];
    walk(tree.state().root(), &mut out);
    out
}

fn button(id: u64, label: &str) -> AccessibilityNode {
    AccessibilityNode::new(ElementId(id), AccessibilityRole::Button).with_label(label)
}

fn group(id: u64, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode::new(ElementId(id), AccessibilityRole::Toolbar)
        .with_label("group")
        .with_children(children)
}

fn field(id: u64, value: &str) -> AccessibilityNode {
    AccessibilityNode::new(ElementId(id), AccessibilityRole::TextInput)
        .with_label("field")
        .with_value(value)
        .with_text_caret(value.len())
}

#[test]
fn incremental_updates_build_the_same_tree_as_whole_ones() {
    let start = AccessibilityTree::new(vec![
        group(1, vec![button(2, "a"), button(3, "b")]),
        field(4, "x"),
    ]);
    let steps = [
        start.clone(),
        // A label and a value change; the value grows a line (a new run).
        AccessibilityTree::new(vec![
            group(1, vec![button(2, "A"), button(3, "b")]),
            field(4, "x\ny"),
        ]),
        // b moves into a new group; the field goes; a button comes.
        AccessibilityTree::new(vec![
            group(1, vec![button(2, "A")]),
            group(5, vec![button(3, "b")]),
            button(6, "new"),
        ]),
        // Focus moves.
        AccessibilityTree::new(vec![
            group(1, vec![button(2, "A")]),
            group(5, vec![button(3, "b")]),
            button(6, "new").with_focused(true),
        ]),
        // Back to the start: removed ids come back.
        start,
        // Children reorder.
        AccessibilityTree::new(vec![
            field(4, "x"),
            group(1, vec![button(3, "b"), button(2, "a")]),
        ]),
    ];
    let mut accessibility = WindowAccessibility::new(None);
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    let mut held = None;
    for (index, (step, scale, title)) in steps
        .iter()
        .zip([1.0, 1.0, 2.0, 2.0, 2.0, 1.0])
        .zip(["One", "One", "One", "Two", "Two", "Two"])
        .map(|((step, scale), title)| (step, scale, title))
        .enumerate()
    {
        let update = accessibility
            .next_update(step.clone(), TextGeometry::new(), title, scale)
            .unwrap_or_else(|| panic!("step {index} changed the tree"));
        if index > 0 {
            assert!(update.tree.is_none(), "step {index} repeats no tree info");
        }
        let sent = update.nodes.len();
        apply(&mut held, update);
        let whole = Tree::new(tree_update(step, &TextGeometry::new(), title, scale), true);
        assert_eq!(
            describe(held.as_ref().expect("a tree is held")),
            describe(&whole),
            "step {index}: incremental (sent {sent} nodes) == whole"
        );
    }
}

#[test]
fn an_update_names_only_the_nodes_that_changed() {
    let mut accessibility = WindowAccessibility::new(None);
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    let tree = |label: &str| {
        AccessibilityTree::new(vec![group(
            1,
            (2..50)
                .map(|id| button(id, "same"))
                .chain([button(50, label)])
                .collect(),
        )])
    };
    let first = accessibility
        .next_update(tree("before"), TextGeometry::new(), "Window", 1.0)
        .expect("the whole tree");
    assert_eq!(first.nodes.len(), 51, "49 buttons, the group, the window");
    assert!(first.tree.is_some());

    let second = accessibility
        .next_update(tree("after"), TextGeometry::new(), "Window", 1.0)
        .expect("a label changed");
    let ids: Vec<_> = second.nodes.iter().map(|(id, _)| id.0).collect();
    assert_eq!(ids, [50], "only the relabelled button");

    let renamed = accessibility
        .next_update(tree("after"), TextGeometry::new(), "Renamed", 1.0)
        .expect("the title changed");
    let ids: Vec<_> = renamed.nodes.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [WINDOW_NODE], "only the window");

    // A reader that starts again gets the whole tree.
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    let again = accessibility
        .next_update(tree("after"), TextGeometry::new(), "Renamed", 1.0)
        .expect("the whole tree again");
    assert_eq!(again.nodes.len(), 51);
    assert!(again.tree.is_some());
}
