//! Incremental AccessKit updates: after the first, whole tree, each update
//! names only the nodes that differ from what the platform holds. A parent
//! whose children changed differs, which is how AccessKit learns of added,
//! moved and removed nodes; removed nodes are simply left out.
//!
//! An adapter that starts a reader afresh needs the whole tree again, and it
//! asks on its own thread: the window learns of it from an event that may
//! arrive after the next update is made. [`WholeTree`] is set while the
//! adapter asks, so that update is whole whatever the event's timing.

use super::UserEvent;
use accesskit::{ActivationHandler, Node, NodeId, TreeUpdate};
use accesskit_winit::{Event, WindowEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

/// The nodes the platform holds, by id.
pub(super) type HeldNodes = HashMap<NodeId, Node>;

/// The incremental form of `full`, an update carrying the whole tree, for a
/// platform holding `held` (None when it holds nothing), and the nodes the
/// platform holds after either.
pub(super) fn changes(
    full: &TreeUpdate,
    held: Option<&HeldNodes>,
) -> (Option<TreeUpdate>, HeldNodes) {
    let now: HeldNodes = full.nodes.iter().cloned().collect();
    let Some(held) = held else {
        return (None, now);
    };
    let changed = full
        .nodes
        .iter()
        .filter(|(id, node)| held.get(id) != Some(node))
        .cloned()
        .collect();
    let update = TreeUpdate {
        nodes: changed,
        // The tree info (root, toolkit) never changes after the first update.
        tree: None,
        tree_id: full.tree_id,
        focus: full.focus,
    };
    (Some(update), now)
}

/// The update to give an adapter: the incremental one, unless there is none
/// or the adapter waits for a whole tree.
pub(super) fn pick(full: TreeUpdate, changes: Option<TreeUpdate>, whole: &WholeTree) -> TreeUpdate {
    match changes {
        Some(changes) if !whole.take() => changes,
        _ => full,
    }
}

/// Whether the adapter waits for a whole tree, set on its thread.
#[derive(Debug, Clone, Default)]
pub(super) struct WholeTree(Arc<AtomicBool>);

impl WholeTree {
    #[cfg(test)]
    pub(super) fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the next update must be whole, clearing it.
    pub(super) fn take(&self) -> bool {
        self.0.swap(false, Ordering::SeqCst)
    }
}

/// The adapter's activation handler: what accesskit_winit's proxy handler
/// does (no tree now; ask the window through the event loop), and marks the
/// next update whole. `accesskit_unix` calls it holding the adapter's lock,
/// before it waits for the tree, and takes the same lock to apply an update,
/// so no partial update can reach a waiting adapter.
pub(super) struct Activation {
    pub(super) window_id: WindowId,
    pub(super) proxy: EventLoopProxy<UserEvent>,
    pub(super) whole: WholeTree,
}

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.whole.0.store(true, Ordering::SeqCst);
        let event = Event {
            window_id: self.window_id,
            window_event: WindowEvent::InitialTreeRequested,
        };
        self.proxy.send_event(event.into()).ok();
        None
    }
}
