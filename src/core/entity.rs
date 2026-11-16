//! Entity system for state management
//!
//! RUI uses an Entity-Component-System inspired architecture where all state
//! is owned by the framework and accessed via EntityIds.

use slotmap::{SlotMap, new_key_type};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

new_key_type! {
    /// Unique identifier for entities in the application
    pub struct EntityId;
}

/// Type-erased entity storage
pub(crate) type AnyEntity = Rc<RefCell<dyn Any>>;

/// Entity storage - owns all application state
pub struct EntityStore {
    entities: SlotMap<EntityId, AnyEntity>,
}

impl EntityStore {
    pub fn new() -> Self {
        Self {
            entities: SlotMap::with_key(),
        }
    }

    /// Insert a new entity and return its ID
    pub fn insert<T: 'static>(&mut self, entity: T) -> EntityId {
        self.entities.insert(Rc::new(RefCell::new(entity)))
    }

    /// Get a reference to an entity
    pub fn get<T: 'static>(&self, id: EntityId) -> Option<std::cell::Ref<'_, T>> {
        self.entities.get(id).and_then(|entity| {
            std::cell::Ref::filter_map(entity.borrow(), |e| e.downcast_ref::<T>()).ok()
        })
    }

    /// Get a mutable reference to an entity
    pub fn get_mut<T: 'static>(&self, id: EntityId) -> Option<std::cell::RefMut<'_, T>> {
        self.entities.get(id).and_then(|entity| {
            std::cell::RefMut::filter_map(entity.borrow_mut(), |e| e.downcast_mut::<T>()).ok()
        })
    }

    /// The entity's shared cell, borrowed apart from the store.
    pub(crate) fn cell(&self, id: EntityId) -> Option<AnyEntity> {
        self.entities.get(id).cloned()
    }

    /// Remove an entity
    pub fn remove(&mut self, id: EntityId) -> bool {
        self.entities.remove(id).is_some()
    }

    /// Check if an entity exists
    pub fn contains(&self, id: EntityId) -> bool {
        self.entities.contains_key(id)
    }
}

impl Default for EntityStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to an entity with type information
#[derive(Debug)]
pub struct Entity<T> {
    pub(crate) id: EntityId,
    // A handle is only an id, so it can cross threads whatever `T` is.
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Entity<T> {
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }
}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Copy for Entity<T> {}

impl<T> PartialEq for Entity<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for Entity<T> {}

impl<T> std::hash::Hash for Entity<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[cfg(test)]
#[path = "entity_handle_tests.rs"]
mod handle_tests;
#[cfg(test)]
#[path = "entity_tests.rs"]
mod tests;
