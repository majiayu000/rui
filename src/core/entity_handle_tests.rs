use super::tests::{Counter, EmptyStruct, Player};
use super::*;
use std::collections::HashSet;

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
        let cloned = entity;

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
        let _cloned = entity;

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

        let ids: Vec<_> = (0..10)
            .map(|i| store.insert(Counter { value: i }))
            .collect();

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
        assert_eq!(
            store.get::<LargeStruct>(id).map(|l| l.data.len()),
            Some(1024)
        );
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
