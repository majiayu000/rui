//! Releasing child views once no tree places them, or when asked to.

use super::AppContext;
use crate::core::entity::EntityId;
use crate::core::view::ViewPlacement;
use std::rc::{Rc, Weak};

impl AppContext {
    /// The placement of the view `id`, shared with every other element that
    /// shows it.
    pub(crate) fn place_view(&mut self, id: EntityId) -> Rc<ViewPlacement> {
        if let Some(placement) = self.placements.get(&id).and_then(Weak::upgrade) {
            return placement;
        }
        let placement = Rc::new(ViewPlacement::new(id, self.unplaced_views.clone()));
        self.placements.insert(id, Rc::downgrade(&placement));
        placement
    }

    /// Release the views that no tree places any more.
    pub(crate) fn release_unplaced_views(&mut self) {
        loop {
            let unplaced = std::mem::take(&mut *self.unplaced_views.borrow_mut());
            if unplaced.is_empty() {
                return;
            }
            for id in unplaced {
                if self
                    .placements
                    .get(&id)
                    .is_some_and(|placement| placement.strong_count() == 0)
                {
                    self.release_view(id);
                }
            }
        }
    }

    /// Drop the child view `id` with what its last render read and the
    /// notifications pending for it. Returns whether it was a child view.
    pub(crate) fn release_view(&mut self, id: EntityId) -> bool {
        let Some(view) = self.child_views.remove(&id) else {
            return false;
        };
        self.placements.remove(&id);
        self.observers.remove(&id);
        for read in self.view_reads.remove(&id).unwrap_or_default() {
            if let Some(views) = self.observers.get_mut(&read) {
                views.remove(&id);
                if views.is_empty() {
                    self.observers.remove(&read);
                }
            }
        }
        self.notified_views
            .borrow_mut()
            .retain(|notified| *notified != id);
        self.for_each_window_frame(|frame| {
            frame.pending_updates.remove(&id);
        });
        self.entities.remove(id);
        // Dropped last: the view's own fields may hold anything.
        drop(view);
        true
    }

    /// The number of views made with [`Self::new_view`] not yet released.
    pub fn child_view_count(&self) -> usize {
        self.child_views.len()
    }
}
