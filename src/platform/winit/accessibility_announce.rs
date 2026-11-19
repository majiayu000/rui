//! Action feedback ("Save activated", "Notify checked") the way AccessKit can
//! say it: a polite live node under the window, which AT-SPI announces
//! (`object:announcement`) and UI Automation raises `LiveRegionChanged` for
//! when it is added. Each message is a new node, alternating between two ids,
//! so the same words twice are announced twice; the node stays until the
//! next message.

use super::accessibility_tree::WINDOW_NODE;
use accesskit::{Live, Node, NodeId, Role, TreeUpdate};

/// The two ids the announcement node alternates between. No element is
/// given these, as none is given the window's.
pub(super) const ANNOUNCERS: [NodeId; 2] = [NodeId(u64::MAX - 1), NodeId(u64::MAX - 2)];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Announcement {
    id: NodeId,
    message: String,
}

/// The messages said so far in one window.
#[derive(Debug, Default)]
pub(super) struct Announcer {
    current: Option<Announcement>,
    said: usize,
}

impl Announcer {
    /// Say `messages` together, replacing what was said before. Nothing to
    /// say leaves the last message.
    pub(super) fn say(&mut self, messages: impl IntoIterator<Item = String>) {
        let messages: Vec<_> = messages.into_iter().collect();
        if messages.is_empty() {
            return;
        }
        self.current = Some(Announcement {
            id: ANNOUNCERS[self.said % ANNOUNCERS.len()],
            message: messages.join(". "),
        });
        self.said += 1;
    }

    pub(super) fn current(&self) -> Option<&Announcement> {
        self.current.as_ref()
    }

    /// Forget what was said, for a reader that starts afresh.
    pub(super) fn clear(&mut self) {
        self.current = None;
    }
}

/// Add `announcement` to `update`, a whole tree, as the window's last child.
pub(super) fn add_announcement(update: &mut TreeUpdate, announcement: &Announcement) {
    let mut node = Node::new(Role::Label);
    // A label is named by its value on AT-SPI and UI Automation.
    node.set_value(announcement.message.as_str());
    node.set_live(Live::Polite);
    if let Some((_, window)) = update.nodes.iter_mut().find(|(id, _)| *id == WINDOW_NODE) {
        window.push_child(announcement.id);
    }
    update.nodes.push((announcement.id, node));
}
