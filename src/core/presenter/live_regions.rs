//! Live regions: a status node (such as a toast) that appears in the tree is
//! announced once, the way a screen reader reads an ARIA live region.

use super::Presenter;
use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityAnnouncementKind, AccessibilityTree,
};

impl<E> Presenter<E> {
    /// Queue an announcement for each live region in `tree` that was not in
    /// the tree passed last time, and remember the ones in `tree`.
    pub fn queue_live_region_announcements(&mut self, tree: &AccessibilityTree) {
        let regions = tree.live_regions();
        if self.accessibility_announcements_enabled {
            for node in &regions {
                if self.live_regions.contains(&node.a11y_id()) {
                    continue;
                }
                let message = match (node.a11y_label(), node.a11y_value()) {
                    (Some(label), Some(value)) => format!("{label}. {value}"),
                    (Some(text), None) | (None, Some(text)) => text.to_string(),
                    (None, None) => continue,
                };
                self.pending_accessibility_announcements
                    .push(AccessibilityAnnouncement::new(
                        node.a11y_id(),
                        AccessibilityAnnouncementKind::ActionFeedback,
                        message,
                    ));
            }
        }
        self.live_regions = regions.iter().map(|node| node.a11y_id()).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ElementId;
    use crate::core::accessibility::{AccessibilityNode, AccessibilityRole};
    use crate::core::geometry::Size;

    fn status(id: ElementId, label: &str) -> AccessibilityNode {
        AccessibilityNode::new(id, AccessibilityRole::Status)
            .with_label(label)
            .with_value("details")
    }

    #[test]
    fn new_live_regions_are_announced_once() {
        let mut presenter = Presenter::with_root(Size::new(10.0, 10.0), ());
        presenter.set_accessibility_announcements_enabled(true);
        let (a, b) = (ElementId(41_001), ElementId(41_002));

        presenter
            .queue_live_region_announcements(&AccessibilityTree::new(vec![status(a, "Saved")]));
        let first = presenter.take_accessibility_announcements();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].message(), "Saved. details");
        assert_eq!(
            first[0].kind(),
            AccessibilityAnnouncementKind::ActionFeedback
        );

        let both = AccessibilityTree::new(vec![
            AccessibilityNode::new(ElementId(41_003), AccessibilityRole::Text)
                .with_label("page")
                .with_child(status(a, "Saved"))
                .with_child(status(b, "Failed")),
        ]);
        presenter.queue_live_region_announcements(&both);
        let second = presenter.take_accessibility_announcements();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].node_id(), b);
    }
}
