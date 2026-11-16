use super::*;
use std::collections::HashSet;

// Test data structures for various test scenarios
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Counter {
    pub(super) value: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Player {
    pub(super) name: String,
    pub(super) health: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct EmptyStruct;

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
        let ids: Vec<_> = (0..10)
            .map(|i| store.insert(Counter { value: i }))
            .collect();

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
