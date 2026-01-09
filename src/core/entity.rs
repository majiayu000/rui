//! Entity system for state management
//!
//! RUI uses an Entity-Component-System inspired architecture where all state
//! is owned by the framework and accessed via EntityIds.

use slotmap::{new_key_type, SlotMap};
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
            let borrowed = entity.borrow();
            if borrowed.downcast_ref::<T>().is_some() {
                Some(std::cell::Ref::map(borrowed, |e| {
                    e.downcast_ref::<T>().unwrap()
                }))
            } else {
                None
            }
        })
    }

    /// Get a mutable reference to an entity
    pub fn get_mut<T: 'static>(&self, id: EntityId) -> Option<std::cell::RefMut<'_, T>> {
        self.entities.get(id).and_then(|entity| {
            let borrowed = entity.borrow_mut();
            if (*borrowed).downcast_ref::<T>().is_some() {
                Some(std::cell::RefMut::map(borrowed, |e| {
                    e.downcast_mut::<T>().unwrap()
                }))
            } else {
                None
            }
        })
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
    _marker: std::marker::PhantomData<T>,
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
