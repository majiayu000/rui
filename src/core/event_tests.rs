use super::*;

// ==================== Page Up Key Detection Tests ====================

#[test]
fn test_page_up_key_detection() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true for PageUp key");
    assert!(!key.page_down, "page_down should be false");
    assert!(!key.up_arrow, "up_arrow should be false");
    assert!(!key.down_arrow, "down_arrow should be false");
}

#[test]
fn test_page_up_with_ctrl() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(!key.shift, "shift should be false");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_up_with_shift() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::shift());
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.shift, "shift should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_up_with_alt() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::alt());
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.alt, "alt should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.shift, "shift should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_up_with_meta() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::meta());
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.meta, "meta should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.shift, "shift should be false");
    assert!(!key.alt, "alt should be false");
}

#[test]
fn test_page_up_with_ctrl_shift() {
    let modifiers = Modifiers {
        shift: true,
        ctrl: true,
        alt: false,
        meta: false,
    };
    let event = KeyEvent::new(KeyCode::PageUp, modifiers);
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(key.shift, "shift should be true");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_up_with_all_modifiers() {
    let modifiers = Modifiers {
        shift: true,
        ctrl: true,
        alt: true,
        meta: true,
    };
    let event = KeyEvent::new(KeyCode::PageUp, modifiers);
    let key = Key::from_key_event(&event);

    assert!(key.page_up, "page_up should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(key.shift, "shift should be true");
    assert!(key.alt, "alt should be true");
    assert!(key.meta, "meta should be true");
}

// ==================== Page Down Key Detection Tests ====================

#[test]
fn test_page_down_key_detection() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true for PageDown key");
    assert!(!key.page_up, "page_up should be false");
    assert!(!key.up_arrow, "up_arrow should be false");
    assert!(!key.down_arrow, "down_arrow should be false");
}

#[test]
fn test_page_down_with_ctrl() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::ctrl());
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(!key.shift, "shift should be false");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_down_with_shift() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.shift, "shift should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_down_with_alt() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::alt());
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.alt, "alt should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.shift, "shift should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_down_with_meta() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::meta());
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.meta, "meta should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.shift, "shift should be false");
    assert!(!key.alt, "alt should be false");
}

#[test]
fn test_page_down_with_ctrl_shift() {
    let modifiers = Modifiers {
        shift: true,
        ctrl: true,
        alt: false,
        meta: false,
    };
    let event = KeyEvent::new(KeyCode::PageDown, modifiers);
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(key.shift, "shift should be true");
    assert!(!key.alt, "alt should be false");
    assert!(!key.meta, "meta should be false");
}

#[test]
fn test_page_down_with_alt_meta() {
    let modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: true,
        meta: true,
    };
    let event = KeyEvent::new(KeyCode::PageDown, modifiers);
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(!key.ctrl, "ctrl should be false");
    assert!(!key.shift, "shift should be false");
    assert!(key.alt, "alt should be true");
    assert!(key.meta, "meta should be true");
}

#[test]
fn test_page_down_with_all_modifiers() {
    let modifiers = Modifiers {
        shift: true,
        ctrl: true,
        alt: true,
        meta: true,
    };
    let event = KeyEvent::new(KeyCode::PageDown, modifiers);
    let key = Key::from_key_event(&event);

    assert!(key.page_down, "page_down should be true");
    assert!(key.ctrl, "ctrl should be true");
    assert!(key.shift, "shift should be true");
    assert!(key.alt, "alt should be true");
    assert!(key.meta, "meta should be true");
}

// ==================== Navigation Helper Method Tests ====================

#[test]
fn test_page_up_is_navigation() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(key.is_navigation(), "PageUp should be a navigation key");
}

#[test]
fn test_page_down_is_navigation() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(key.is_navigation(), "PageDown should be a navigation key");
}

#[test]
fn test_page_up_has_modifier_false() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(
        !key.has_modifier(),
        "has_modifier should be false without modifiers"
    );
}

#[test]
fn test_page_up_has_modifier_true() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());
    let key = Key::from_key_event(&event);

    assert!(key.has_modifier(), "has_modifier should be true with ctrl");
}

#[test]
fn test_page_down_has_modifier_false() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::none());
    let key = Key::from_key_event(&event);

    assert!(
        !key.has_modifier(),
        "has_modifier should be false without modifiers"
    );
}

#[test]
fn test_page_down_has_modifier_true() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());
    let key = Key::from_key_event(&event);

    assert!(key.has_modifier(), "has_modifier should be true with shift");
}

// ==================== KeyCode Navigation Tests ====================

#[test]
fn test_keycode_page_up_is_navigation() {
    assert!(
        KeyCode::PageUp.is_navigation_key(),
        "PageUp should be a navigation key"
    );
}

#[test]
fn test_keycode_page_down_is_navigation() {
    assert!(
        KeyCode::PageDown.is_navigation_key(),
        "PageDown should be a navigation key"
    );
}

#[test]
fn test_keycode_page_up_not_arrow() {
    assert!(
        !KeyCode::PageUp.is_arrow_key(),
        "PageUp should not be an arrow key"
    );
}

#[test]
fn test_keycode_page_down_not_arrow() {
    assert!(
        !KeyCode::PageDown.is_arrow_key(),
        "PageDown should not be an arrow key"
    );
}

#[test]
fn test_keycode_page_up_not_modifier() {
    assert!(
        !KeyCode::PageUp.is_modifier_key(),
        "PageUp should not be a modifier key"
    );
}

#[test]
fn test_keycode_page_down_not_modifier() {
    assert!(
        !KeyCode::PageDown.is_modifier_key(),
        "PageDown should not be a modifier key"
    );
}

#[test]
fn test_keycode_page_up_not_function() {
    assert!(
        !KeyCode::PageUp.is_function_key(),
        "PageUp should not be a function key"
    );
}

#[test]
fn test_keycode_page_down_not_function() {
    assert!(
        !KeyCode::PageDown.is_function_key(),
        "PageDown should not be a function key"
    );
}

// ==================== Key Construction Tests ====================

#[test]
fn test_key_from_key_code_page_up() {
    let key = Key::from_key_code(KeyCode::PageUp);

    assert!(key.page_up, "page_up should be true");
    assert!(!key.page_down, "page_down should be false");
    assert!(!key.has_modifier(), "should have no modifiers");
}

#[test]
fn test_key_from_key_code_page_down() {
    let key = Key::from_key_code(KeyCode::PageDown);

    assert!(key.page_down, "page_down should be true");
    assert!(!key.page_up, "page_up should be false");
    assert!(!key.has_modifier(), "should have no modifiers");
}

#[test]
fn test_key_from_key_code_with_modifiers_page_up() {
    let key = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());

    assert!(key.page_up, "page_up should be true");
    assert!(key.ctrl, "ctrl should be true");
}

#[test]
fn test_key_from_key_code_with_modifiers_page_down() {
    let key = Key::from_key_code_with_modifiers(KeyCode::PageDown, Modifiers::shift());

    assert!(key.page_down, "page_down should be true");
    assert!(key.shift, "shift should be true");
}

#[test]
fn test_key_event_to_key_page_up() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::alt());
    let key = event.to_key();

    assert!(key.page_up, "page_up should be true");
    assert!(key.alt, "alt should be true");
}

#[test]
fn test_key_event_to_key_page_down() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::meta());
    let key = event.to_key();

    assert!(key.page_down, "page_down should be true");
    assert!(key.meta, "meta should be true");
}

// ==================== Key Default and New Tests ====================

#[test]
fn test_key_default_page_up_false() {
    let key = Key::default();
    assert!(!key.page_up, "page_up should be false by default");
}

#[test]
fn test_key_default_page_down_false() {
    let key = Key::default();
    assert!(!key.page_down, "page_down should be false by default");
}

#[test]
fn test_key_new_page_up_false() {
    let key = Key::new();
    assert!(!key.page_up, "page_up should be false for new Key");
}

#[test]
fn test_key_new_page_down_false() {
    let key = Key::new();
    assert!(!key.page_down, "page_down should be false for new Key");
}

// ==================== Equality Tests ====================

#[test]
fn test_key_equality_page_up() {
    let key1 = Key::from_key_code(KeyCode::PageUp);
    let key2 = Key::from_key_code(KeyCode::PageUp);

    assert_eq!(key1, key2, "Two PageUp keys should be equal");
}

#[test]
fn test_key_equality_page_down() {
    let key1 = Key::from_key_code(KeyCode::PageDown);
    let key2 = Key::from_key_code(KeyCode::PageDown);

    assert_eq!(key1, key2, "Two PageDown keys should be equal");
}

#[test]
fn test_key_inequality_page_up_vs_page_down() {
    let key1 = Key::from_key_code(KeyCode::PageUp);
    let key2 = Key::from_key_code(KeyCode::PageDown);

    assert_ne!(key1, key2, "PageUp and PageDown keys should not be equal");
}

#[test]
fn test_key_inequality_same_key_different_modifier() {
    let key1 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::none());
    let key2 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());

    assert_ne!(
        key1, key2,
        "Same key with different modifiers should not be equal"
    );
}

#[test]
fn test_keycode_equality_page_up() {
    assert_eq!(KeyCode::PageUp, KeyCode::PageUp);
}

#[test]
fn test_keycode_equality_page_down() {
    assert_eq!(KeyCode::PageDown, KeyCode::PageDown);
}

#[test]
fn test_keycode_inequality_page_up_vs_page_down() {
    assert_ne!(KeyCode::PageUp, KeyCode::PageDown);
}

// ==================== Clone and Copy Tests ====================

#[test]
fn test_key_clone_page_up() {
    let key1 = Key::from_key_code_with_modifiers(KeyCode::PageUp, Modifiers::ctrl());
    let key2 = key1;

    assert_eq!(key1, key2);
    assert!(key2.page_up);
    assert!(key2.ctrl);
}

#[test]
fn test_key_clone_page_down() {
    let key1 = Key::from_key_code_with_modifiers(KeyCode::PageDown, Modifiers::shift());
    let key2 = key1;

    assert_eq!(key1, key2);
    assert!(key2.page_down);
    assert!(key2.shift);
}

#[test]
fn test_key_copy_page_up() {
    let key1 = Key::from_key_code(KeyCode::PageUp);
    let key2 = key1; // Copy

    assert!(key1.page_up);
    assert!(key2.page_up);
}

#[test]
fn test_key_copy_page_down() {
    let key1 = Key::from_key_code(KeyCode::PageDown);
    let key2 = key1; // Copy

    assert!(key1.page_down);
    assert!(key2.page_down);
}

#[test]
fn test_keycode_clone_page_up() {
    let code1 = KeyCode::PageUp;
    let code2 = code1;

    assert_eq!(code1, code2);
}

#[test]
fn test_keycode_clone_page_down() {
    let code1 = KeyCode::PageDown;
    let code2 = code1;

    assert_eq!(code1, code2);
}

// ==================== Debug Trait Tests ====================

#[test]
fn test_key_debug_page_up() {
    let key = Key::from_key_code(KeyCode::PageUp);
    let debug_str = format!("{:?}", key);

    assert!(
        debug_str.contains("page_up: true"),
        "Debug should show page_up: true"
    );
    assert!(
        debug_str.contains("page_down: false"),
        "Debug should show page_down: false"
    );
}

#[test]
fn test_key_debug_page_down() {
    let key = Key::from_key_code(KeyCode::PageDown);
    let debug_str = format!("{:?}", key);

    assert!(
        debug_str.contains("page_down: true"),
        "Debug should show page_down: true"
    );
    assert!(
        debug_str.contains("page_up: false"),
        "Debug should show page_up: false"
    );
}

#[test]
fn test_keycode_debug_page_up() {
    let debug_str = format!("{:?}", KeyCode::PageUp);
    assert_eq!(debug_str, "PageUp");
}

#[test]
fn test_keycode_debug_page_down() {
    let debug_str = format!("{:?}", KeyCode::PageDown);
    assert_eq!(debug_str, "PageDown");
}

// ==================== Hash Trait Tests for KeyCode ====================

#[test]
fn test_keycode_hash_page_up() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(KeyCode::PageUp);

    assert!(set.contains(&KeyCode::PageUp));
    assert!(!set.contains(&KeyCode::PageDown));
}

#[test]
fn test_keycode_hash_page_down() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(KeyCode::PageDown);

    assert!(set.contains(&KeyCode::PageDown));
    assert!(!set.contains(&KeyCode::PageUp));
}

// ==================== Modifiers Tests ====================

#[test]
fn test_modifiers_none() {
    let m = Modifiers::none();
    assert!(!m.shift);
    assert!(!m.ctrl);
    assert!(!m.alt);
    assert!(!m.meta);
}

#[test]
fn test_modifiers_shift() {
    let m = Modifiers::shift();
    assert!(m.shift);
    assert!(!m.ctrl);
    assert!(!m.alt);
    assert!(!m.meta);
}

#[test]
fn test_modifiers_ctrl() {
    let m = Modifiers::ctrl();
    assert!(!m.shift);
    assert!(m.ctrl);
    assert!(!m.alt);
    assert!(!m.meta);
}

#[test]
fn test_modifiers_alt() {
    let m = Modifiers::alt();
    assert!(!m.shift);
    assert!(!m.ctrl);
    assert!(m.alt);
    assert!(!m.meta);
}

#[test]
fn test_modifiers_meta() {
    let m = Modifiers::meta();
    assert!(!m.shift);
    assert!(!m.ctrl);
    assert!(!m.alt);
    assert!(m.meta);
}

// ==================== Other Navigation Keys (for comparison) ====================

#[test]
fn test_arrow_keys_not_page_keys() {
    let up = Key::from_key_code(KeyCode::ArrowUp);
    let down = Key::from_key_code(KeyCode::ArrowDown);

    assert!(!up.page_up, "ArrowUp should not set page_up");
    assert!(!up.page_down, "ArrowUp should not set page_down");
    assert!(!down.page_up, "ArrowDown should not set page_up");
    assert!(!down.page_down, "ArrowDown should not set page_down");
}

#[test]
fn test_home_end_not_page_keys() {
    let home = Key::from_key_code(KeyCode::Home);
    let end = Key::from_key_code(KeyCode::End);

    assert!(!home.page_up, "Home should not set page_up");
    assert!(!home.page_down, "Home should not set page_down");
    assert!(!end.page_up, "End should not set page_up");
    assert!(!end.page_down, "End should not set page_down");
}

#[test]
fn test_page_keys_not_other_navigation() {
    let page_up = Key::from_key_code(KeyCode::PageUp);
    let page_down = Key::from_key_code(KeyCode::PageDown);

    assert!(!page_up.up_arrow, "PageUp should not set up_arrow");
    assert!(!page_up.down_arrow, "PageUp should not set down_arrow");
    assert!(!page_up.home, "PageUp should not set home");
    assert!(!page_up.end, "PageUp should not set end");

    assert!(!page_down.up_arrow, "PageDown should not set up_arrow");
    assert!(!page_down.down_arrow, "PageDown should not set down_arrow");
    assert!(!page_down.home, "PageDown should not set home");
    assert!(!page_down.end, "PageDown should not set end");
}

// ==================== Is Editing Tests ====================

#[test]
fn test_page_up_not_editing() {
    let key = Key::from_key_code(KeyCode::PageUp);
    assert!(!key.is_editing(), "PageUp should not be an editing key");
}

#[test]
fn test_page_down_not_editing() {
    let key = Key::from_key_code(KeyCode::PageDown);
    assert!(!key.is_editing(), "PageDown should not be an editing key");
}

// ==================== Is Function Key Tests ====================

#[test]
fn test_page_up_not_function_key() {
    let key = Key::from_key_code(KeyCode::PageUp);
    assert!(
        !key.is_function_key(),
        "PageUp should not be a function key"
    );
}

#[test]
fn test_page_down_not_function_key() {
    let key = Key::from_key_code(KeyCode::PageDown);
    assert!(
        !key.is_function_key(),
        "PageDown should not be a function key"
    );
}

// ==================== KeyEvent Tests ====================

#[test]
fn test_key_event_new_page_up() {
    let event = KeyEvent::new(KeyCode::PageUp, Modifiers::ctrl());

    assert_eq!(event.key, KeyCode::PageUp);
    assert!(event.modifiers.ctrl);
    assert!(!event.is_repeat);
    assert!(event.char.is_none());
}

#[test]
fn test_key_event_new_page_down() {
    let event = KeyEvent::new(KeyCode::PageDown, Modifiers::shift());

    assert_eq!(event.key, KeyCode::PageDown);
    assert!(event.modifiers.shift);
    assert!(!event.is_repeat);
    assert!(event.char.is_none());
}

#[test]
fn test_key_event_with_char() {
    let event = KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a');

    assert_eq!(event.key, KeyCode::A);
    assert_eq!(event.char, Some('a'));
}
