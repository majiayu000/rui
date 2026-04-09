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
        self.entities
            .get(id)
            .and_then(|entity| std::cell::Ref::filter_map(entity.borrow(), |e| e.downcast_ref::<T>()).ok())
    }

    /// Get a mutable reference to an entity
    pub fn get_mut<T: 'static>(&self, id: EntityId) -> Option<std::cell::RefMut<'_, T>> {
        self.entities.get(id).and_then(|entity| {
            std::cell::RefMut::filter_map(entity.borrow_mut(), |e| e.downcast_mut::<T>()).ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Test data structures for various test scenarios
    #[derive(Debug, Clone, PartialEq)]
    struct Counter {
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Player {
        name: String,
        health: u32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct EmptyStruct;

    // ============================================================
    // EntityStore Tests
    // ============================================================

    mod entity_store_creation {
        use super::*;

        #[test]
        fn new_creates_empty_store() {
            let store = EntityStore::new();
            // A new store should be empty - verified by inserting and checking
            let id = store.entities.keys().next();
            assert!(id.is_none());
        }

        #[test]
        fn default_creates_empty_store() {
            let store = EntityStore::default();
            let id = store.entities.keys().next();
            assert!(id.is_none());
        }
    }

    mod entity_store_insert {
        use super::*;

        struct InsertTestCase {
            name: &'static str,
            value: i32,
        }

        #[test]
        fn insert_returns_valid_id() {
            let test_cases = vec![
                InsertTestCase {
                    name: "zero value",
                    value: 0,
                },
                InsertTestCase {
                    name: "positive value",
                    value: 42,
                },
                InsertTestCase {
                    name: "negative value",
                    value: -100,
                },
                InsertTestCase {
                    name: "max value",
                    value: i32::MAX,
                },
                InsertTestCase {
                    name: "min value",
                    value: i32::MIN,
                },
            ];

            for tc in test_cases {
                let mut store = EntityStore::new();
                let id = store.insert(Counter { value: tc.value });
                assert!(
                    store.contains(id),
                    "Failed for case: {} - entity should exist after insert",
                    tc.name
                );
            }
        }

        #[test]
        fn insert_multiple_entities_returns_unique_ids() {
            let mut store = EntityStore::new();
            let mut ids = Vec::new();

            for i in 0..100 {
                let id = store.insert(Counter { value: i });
                ids.push(id);
            }

            // All IDs should be unique
            let unique_ids: HashSet<_> = ids.iter().collect();
            assert_eq!(
                ids.len(),
                unique_ids.len(),
                "All inserted entities should have unique IDs"
            );
        }

        #[test]
        fn insert_different_types() {
            let mut store = EntityStore::new();

            let counter_id = store.insert(Counter { value: 10 });
            let player_id = store.insert(Player {
                name: "Alice".to_string(),
                health: 100,
            });
            let empty_id = store.insert(EmptyStruct);

            assert!(store.contains(counter_id));
            assert!(store.contains(player_id));
            assert!(store.contains(empty_id));
            assert_ne!(counter_id, player_id);
            assert_ne!(player_id, empty_id);
        }

        #[test]
        fn insert_string_type() {
            let mut store = EntityStore::new();
            let id = store.insert("hello world".to_string());
            assert!(store.contains(id));

            let retrieved = store.get::<String>(id);
            assert!(retrieved.is_some());
            assert_eq!(*retrieved.unwrap(), "hello world");
        }

        #[test]
        fn insert_vec_type() {
            let mut store = EntityStore::new();
            let id = store.insert(vec![1, 2, 3, 4, 5]);
            assert!(store.contains(id));

            let retrieved = store.get::<Vec<i32>>(id);
            assert!(retrieved.is_some());
            assert_eq!(*retrieved.unwrap(), vec![1, 2, 3, 4, 5]);
        }
    }

    mod entity_store_get {
        use super::*;

        #[test]
        fn get_existing_entity_returns_some() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let result = store.get::<Counter>(id);
            assert!(result.is_some());
            assert_eq!(result.unwrap().value, 42);
        }

        #[test]
        fn get_with_wrong_type_returns_none() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            // Try to get as wrong type
            let result = store.get::<Player>(id);
            assert!(result.is_none());
        }

        #[test]
        fn get_nonexistent_id_returns_none() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            store.remove(id);

            let result = store.get::<Counter>(id);
            assert!(result.is_none());
        }

        #[test]
        fn get_preserves_data_integrity() {
            let mut store = EntityStore::new();
            let player = Player {
                name: "Bob".to_string(),
                health: 75,
            };
            let id = store.insert(player.clone());

            let retrieved = store.get::<Player>(id).unwrap();
            assert_eq!(retrieved.name, "Bob");
            assert_eq!(retrieved.health, 75);
        }

        #[test]
        fn get_multiple_times_returns_same_value() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 100 });

            for _ in 0..10 {
                let result = store.get::<Counter>(id);
                assert!(result.is_some());
                assert_eq!(result.unwrap().value, 100);
            }
        }
    }

    mod entity_store_get_mut {
        use super::*;

        #[test]
        fn get_mut_existing_entity_returns_some() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let result = store.get_mut::<Counter>(id);
            assert!(result.is_some());
            assert_eq!(result.unwrap().value, 42);
        }

        #[test]
        fn get_mut_allows_modification() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 0 });

            // Modify the entity
            {
                let mut counter = store.get_mut::<Counter>(id).unwrap();
                counter.value = 999;
            }

            // Verify modification persisted
            let counter = store.get::<Counter>(id).unwrap();
            assert_eq!(counter.value, 999);
        }

        #[test]
        fn get_mut_with_wrong_type_returns_none() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let result = store.get_mut::<Player>(id);
            assert!(result.is_none());
        }

        #[test]
        fn get_mut_nonexistent_id_returns_none() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            store.remove(id);

            let result = store.get_mut::<Counter>(id);
            assert!(result.is_none());
        }

        #[test]
        fn get_mut_multiple_modifications() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 0 });

            for i in 1..=10 {
                {
                    let mut counter = store.get_mut::<Counter>(id).unwrap();
                    counter.value = i;
                }
                let counter = store.get::<Counter>(id).unwrap();
                assert_eq!(counter.value, i);
            }
        }

        #[test]
        fn get_mut_complex_type_modification() {
            let mut store = EntityStore::new();
            let id = store.insert(Player {
                name: "Initial".to_string(),
                health: 100,
            });

            {
                let mut player = store.get_mut::<Player>(id).unwrap();
                player.name = "Updated".to_string();
                player.health = 50;
            }

            let player = store.get::<Player>(id).unwrap();
            assert_eq!(player.name, "Updated");
            assert_eq!(player.health, 50);
        }
    }

    mod entity_store_remove {
        use super::*;

        #[test]
        fn remove_existing_entity_returns_true() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let result = store.remove(id);
            assert!(result);
        }

        #[test]
        fn remove_nonexistent_entity_returns_false() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            store.remove(id);

            // Try to remove again
            let result = store.remove(id);
            assert!(!result);
        }

        #[test]
        fn remove_makes_entity_inaccessible() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            store.remove(id);

            assert!(!store.contains(id));
            assert!(store.get::<Counter>(id).is_none());
            assert!(store.get_mut::<Counter>(id).is_none());
        }

        #[test]
        fn remove_does_not_affect_other_entities() {
            let mut store = EntityStore::new();
            let id1 = store.insert(Counter { value: 1 });
            let id2 = store.insert(Counter { value: 2 });
            let id3 = store.insert(Counter { value: 3 });

            store.remove(id2);

            assert!(store.contains(id1));
            assert!(!store.contains(id2));
            assert!(store.contains(id3));
            assert_eq!(store.get::<Counter>(id1).unwrap().value, 1);
            assert_eq!(store.get::<Counter>(id3).unwrap().value, 3);
        }

        #[test]
        fn remove_multiple_entities() {
            let mut store = EntityStore::new();
            let ids: Vec<_> = (0..10).map(|i| store.insert(Counter { value: i })).collect();

            // Remove all even-indexed entities
            for (i, &id) in ids.iter().enumerate() {
                if i % 2 == 0 {
                    assert!(store.remove(id));
                }
            }

            // Verify state
            for (i, &id) in ids.iter().enumerate() {
                if i % 2 == 0 {
                    assert!(!store.contains(id));
                } else {
                    assert!(store.contains(id));
                }
            }
        }
    }

    mod entity_store_contains {
        use super::*;

        #[test]
        fn contains_existing_entity_returns_true() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            assert!(store.contains(id));
        }

        #[test]
        fn contains_removed_entity_returns_false() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            store.remove(id);

            assert!(!store.contains(id));
        }

        #[test]
        fn contains_is_type_agnostic() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            // contains doesn't check type, just if id exists
            assert!(store.contains(id));
        }
    }

    // ============================================================
    // Entity<T> Tests
    // ============================================================

    mod entity_creation {
        use super::*;

        #[test]
        fn new_creates_entity_with_given_id() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity: Entity<Counter> = Entity::new(id);
            assert_eq!(entity.id(), id);
        }

        #[test]
        fn id_method_returns_correct_id() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity: Entity<Counter> = Entity::new(id);
            assert_eq!(entity.id(), id);
            assert_eq!(entity.id, id);
        }
    }

    mod entity_clone_copy {
        use super::*;

        #[test]
        fn clone_creates_equal_entity() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity: Entity<Counter> = Entity::new(id);
            let cloned = entity.clone();

            assert_eq!(entity, cloned);
            assert_eq!(entity.id(), cloned.id());
        }

        #[test]
        fn copy_creates_equal_entity() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity: Entity<Counter> = Entity::new(id);
            let copied = entity; // Copy happens here

            assert_eq!(entity.id(), copied.id());
        }

        #[test]
        fn clone_and_original_are_independent() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity: Entity<Counter> = Entity::new(id);
            let _cloned = entity.clone();

            // Both still work independently
            assert_eq!(entity.id(), id);
        }
    }

    mod entity_equality {
        use super::*;

        #[test]
        fn same_id_entities_are_equal() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity1: Entity<Counter> = Entity::new(id);
            let entity2: Entity<Counter> = Entity::new(id);

            assert_eq!(entity1, entity2);
        }

        #[test]
        fn different_id_entities_are_not_equal() {
            let mut store = EntityStore::new();
            let id1 = store.insert(Counter { value: 1 });
            let id2 = store.insert(Counter { value: 2 });

            let entity1: Entity<Counter> = Entity::new(id1);
            let entity2: Entity<Counter> = Entity::new(id2);

            assert_ne!(entity1, entity2);
        }

        #[test]
        fn equality_is_reflexive() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            let entity: Entity<Counter> = Entity::new(id);

            assert_eq!(entity, entity);
        }

        #[test]
        fn equality_is_symmetric() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity1: Entity<Counter> = Entity::new(id);
            let entity2: Entity<Counter> = Entity::new(id);

            assert_eq!(entity1, entity2);
            assert_eq!(entity2, entity1);
        }

        #[test]
        fn equality_is_transitive() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity1: Entity<Counter> = Entity::new(id);
            let entity2: Entity<Counter> = Entity::new(id);
            let entity3: Entity<Counter> = Entity::new(id);

            assert_eq!(entity1, entity2);
            assert_eq!(entity2, entity3);
            assert_eq!(entity1, entity3);
        }
    }

    mod entity_hash {
        use super::*;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn calculate_hash<T: Hash>(t: &T) -> u64 {
            let mut s = DefaultHasher::new();
            t.hash(&mut s);
            s.finish()
        }

        #[test]
        fn same_id_entities_have_same_hash() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let entity1: Entity<Counter> = Entity::new(id);
            let entity2: Entity<Counter> = Entity::new(id);

            assert_eq!(calculate_hash(&entity1), calculate_hash(&entity2));
        }

        #[test]
        fn different_id_entities_likely_different_hash() {
            let mut store = EntityStore::new();
            let id1 = store.insert(Counter { value: 1 });
            let id2 = store.insert(Counter { value: 2 });

            let entity1: Entity<Counter> = Entity::new(id1);
            let entity2: Entity<Counter> = Entity::new(id2);

            // While hash collisions are possible, they should be rare
            assert_ne!(calculate_hash(&entity1), calculate_hash(&entity2));
        }

        #[test]
        fn entity_can_be_used_in_hashset() {
            let mut store = EntityStore::new();
            let mut set: HashSet<Entity<Counter>> = HashSet::new();

            let ids: Vec<_> = (0..10).map(|i| store.insert(Counter { value: i })).collect();

            for &id in &ids {
                let entity = Entity::new(id);
                set.insert(entity);
            }

            assert_eq!(set.len(), 10);

            // Verify each entity is in the set
            for &id in &ids {
                let entity: Entity<Counter> = Entity::new(id);
                assert!(set.contains(&entity));
            }
        }

        #[test]
        fn duplicate_entities_in_hashset() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            let mut set: HashSet<Entity<Counter>> = HashSet::new();

            // Insert same entity multiple times
            for _ in 0..5 {
                let entity = Entity::new(id);
                set.insert(entity);
            }

            // Should only have one entry
            assert_eq!(set.len(), 1);
        }
    }

    mod entity_debug {
        use super::*;

        #[test]
        fn debug_output_is_non_empty() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });
            let entity: Entity<Counter> = Entity::new(id);

            let debug_output = format!("{:?}", entity);
            assert!(!debug_output.is_empty());
            assert!(debug_output.contains("Entity"));
        }
    }

    // ============================================================
    // Integration Tests
    // ============================================================

    mod integration {
        use super::*;

        #[test]
        fn full_lifecycle_single_entity() {
            let mut store = EntityStore::new();

            // Create
            let id = store.insert(Counter { value: 0 });
            let entity: Entity<Counter> = Entity::new(id);

            // Read
            assert!(store.contains(entity.id()));
            assert_eq!(store.get::<Counter>(entity.id()).unwrap().value, 0);

            // Update
            {
                let mut counter = store.get_mut::<Counter>(entity.id()).unwrap();
                counter.value = 100;
            }
            assert_eq!(store.get::<Counter>(entity.id()).unwrap().value, 100);

            // Delete
            assert!(store.remove(entity.id()));
            assert!(!store.contains(entity.id()));
            assert!(store.get::<Counter>(entity.id()).is_none());
        }

        #[test]
        fn multiple_entity_types_in_same_store() {
            let mut store = EntityStore::new();

            let counter_id = store.insert(Counter { value: 42 });
            let player_id = store.insert(Player {
                name: "Test".to_string(),
                health: 100,
            });
            let string_id = store.insert("Hello".to_string());

            // All should exist
            assert!(store.contains(counter_id));
            assert!(store.contains(player_id));
            assert!(store.contains(string_id));

            // Each should return correct type
            assert!(store.get::<Counter>(counter_id).is_some());
            assert!(store.get::<Player>(player_id).is_some());
            assert!(store.get::<String>(string_id).is_some());

            // Wrong types should return None
            assert!(store.get::<Player>(counter_id).is_none());
            assert!(store.get::<String>(player_id).is_none());
            assert!(store.get::<Counter>(string_id).is_none());
        }

        #[test]
        fn stress_test_many_entities() {
            let mut store = EntityStore::new();
            let count = 1000;

            // Insert many entities
            let ids: Vec<_> = (0..count)
                .map(|i| store.insert(Counter { value: i }))
                .collect();

            // Verify all exist
            for &id in &ids {
                assert!(store.contains(id));
            }

            // Remove half
            for (i, &id) in ids.iter().enumerate() {
                if i % 2 == 0 {
                    store.remove(id);
                }
            }

            // Verify correct removal
            for (i, &id) in ids.iter().enumerate() {
                if i % 2 == 0 {
                    assert!(!store.contains(id));
                } else {
                    assert!(store.contains(id));
                    assert_eq!(store.get::<Counter>(id).unwrap().value, i as i32);
                }
            }
        }

        #[test]
        fn entity_handles_with_store() {
            let mut store = EntityStore::new();

            let id = store.insert(Counter { value: 10 });
            let entity: Entity<Counter> = Entity::new(id);

            // Use entity handle to access store
            let value = store.get::<Counter>(entity.id()).unwrap().value;
            assert_eq!(value, 10);

            // Modify through entity handle
            {
                let mut counter = store.get_mut::<Counter>(entity.id()).unwrap();
                counter.value += 5;
            }

            let value = store.get::<Counter>(entity.id()).unwrap().value;
            assert_eq!(value, 15);
        }
    }

    // ============================================================
    // Edge Cases and Boundary Tests
    // ============================================================

    mod edge_cases {
        use super::*;

        #[test]
        fn insert_and_remove_repeatedly() {
            let mut store = EntityStore::new();

            for _ in 0..100 {
                let id = store.insert(Counter { value: 42 });
                assert!(store.contains(id));
                store.remove(id);
                assert!(!store.contains(id));
            }
        }

        #[test]
        fn zero_sized_type() {
            let mut store = EntityStore::new();
            let id = store.insert(EmptyStruct);

            assert!(store.contains(id));
            assert!(store.get::<EmptyStruct>(id).is_some());
        }

        #[test]
        fn large_struct() {
            #[derive(Debug)]
            struct LargeStruct {
                data: [u8; 1024],
            }

            let mut store = EntityStore::new();
            let large = LargeStruct { data: [0u8; 1024] };
            let id = store.insert(large);

            assert!(store.contains(id));
            assert!(store.get::<LargeStruct>(id).is_some());
        }

        #[test]
        fn nested_types() {
            #[derive(Debug, PartialEq)]
            struct Outer {
                inner: Vec<Counter>,
            }

            let mut store = EntityStore::new();
            let outer = Outer {
                inner: vec![
                    Counter { value: 1 },
                    Counter { value: 2 },
                    Counter { value: 3 },
                ],
            };
            let id = store.insert(outer);

            let retrieved = store.get::<Outer>(id).unwrap();
            assert_eq!(retrieved.inner.len(), 3);
            assert_eq!(retrieved.inner[0].value, 1);
        }

        #[test]
        fn option_type() {
            let mut store = EntityStore::new();

            let some_id = store.insert(Some(Counter { value: 42 }));
            let none_id = store.insert(None::<Counter>);

            let some_val = store.get::<Option<Counter>>(some_id).unwrap();
            assert!(some_val.is_some());
            assert_eq!(some_val.as_ref().unwrap().value, 42);

            let none_val = store.get::<Option<Counter>>(none_id).unwrap();
            assert!(none_val.is_none());
        }

        #[test]
        fn entity_handles_for_different_types_with_same_id_structure() {
            let mut store = EntityStore::new();
            let id = store.insert(Counter { value: 42 });

            // Create entity handles with different type parameters
            let counter_entity: Entity<Counter> = Entity::new(id);
            let player_entity: Entity<Player> = Entity::new(id);

            // Both have same id
            assert_eq!(counter_entity.id(), player_entity.id());

            // But store access respects types
            assert!(store.get::<Counter>(counter_entity.id()).is_some());
            assert!(store.get::<Player>(player_entity.id()).is_none());
        }
    }
}
