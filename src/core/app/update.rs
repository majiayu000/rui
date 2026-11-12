//! Changing entities so the views that read them render again.

use super::AppContext;
use crate::core::entity::Entity;

impl AppContext {
    /// Change `entity`, then notify it, so every view that read it renders
    /// again. `None` once the entity was released. See [`Entity::update`].
    pub fn update_entity<T: 'static, R>(
        &mut self,
        entity: Entity<T>,
        update: impl FnOnce(&mut T, &mut AppContext) -> R,
    ) -> Option<R> {
        // Borrowed apart from the store, so `update` can reach other entities.
        let cell = self.entities.cell(entity.id())?;
        let result = {
            let mut value = cell.borrow_mut();
            update(value.downcast_mut::<T>()?, self)
        };
        self.notify(entity.id());
        Some(result)
    }
}

impl AsMut<AppContext> for AppContext {
    fn as_mut(&mut self) -> &mut AppContext {
        self
    }
}

impl<T: 'static> Entity<T> {
    /// Change the entity, then notify it, so every view that read it renders
    /// again. `None` once it was released.
    ///
    /// ```ignore
    /// self.saved.update(cx, |saved, _| *saved = value);
    /// ```
    pub fn update<R>(
        self,
        cx: &mut impl AsMut<AppContext>,
        update: impl FnOnce(&mut T, &mut AppContext) -> R,
    ) -> Option<R> {
        cx.as_mut().update_entity(self, update)
    }
}
