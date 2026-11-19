//! Incremental AccessKit updates: after the first, whole tree, each update
//! names only the nodes that differ from what the platform holds. A parent
//! whose children changed differs, which is how AccessKit learns of added,
//! moved and removed nodes; removed nodes are simply left out.

use accesskit::{Node, NodeId, TreeUpdate};
use std::collections::HashMap;

/// The nodes the platform holds, by id.
pub(super) type HeldNodes = HashMap<NodeId, Node>;

/// `full`, an update carrying the whole tree, as the update that brings a
/// platform holding `held` to it, and the nodes the platform holds after it.
/// With nothing held the whole tree goes, with its tree info.
pub(super) fn changes(full: TreeUpdate, held: Option<&HeldNodes>) -> (TreeUpdate, HeldNodes) {
    let TreeUpdate {
        nodes,
        tree,
        tree_id,
        focus,
    } = full;
    let Some(held) = held else {
        let now = nodes.iter().cloned().collect();
        let update = TreeUpdate {
            nodes,
            tree,
            tree_id,
            focus,
        };
        return (update, now);
    };
    let changed = nodes
        .iter()
        .filter(|(id, node)| held.get(id) != Some(node))
        .cloned()
        .collect();
    let update = TreeUpdate {
        nodes: changed,
        // The tree info (root, toolkit) never changes after the first update.
        tree: None,
        tree_id,
        focus,
    };
    (update, nodes.into_iter().collect())
}
