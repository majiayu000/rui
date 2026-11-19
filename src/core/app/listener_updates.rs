//! Running child views' queued listener updates before a frame works out what
//! to render, so an entity one updates reaches every view that reads it in
//! that frame (#341).

use super::AppContext;
use crate::core::entity::EntityId;
use std::collections::HashSet;

impl AppContext {
    /// Apply the queued listener updates of the child views this window has
    /// pending, and of any view those updates notify in turn. What they change
    /// joins the window's pending notifications; the views render later.
    pub(crate) fn apply_child_view_updates(&mut self) {
        let mut applied = HashSet::<EntityId>::new();
        loop {
            let next: Vec<EntityId> = self
                .frame
                .pending_updates
                .iter()
                .copied()
                .filter(|id| self.child_views.contains_key(id) && !applied.contains(id))
                .collect();
            if next.is_empty() {
                return;
            }
            for id in next {
                applied.insert(id);
                // Taken out while it runs, as `render_child_view` does.
                if let Some(mut view) = self.child_views.remove(&id) {
                    view.apply_updates(self);
                    self.child_views.insert(id, view);
                }
            }
        }
    }
}
