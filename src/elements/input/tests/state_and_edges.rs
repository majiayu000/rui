use super::support::*;

// ==================== Callback Multiple Invocations Tests ====================

#[test]
fn test_on_change_multiple_calls() {
    let call_count = Rc::new(RefCell::new(0));
    let count_clone = call_count.clone();

    let inp = Input::new().value("test").on_change(move |_| {
        *count_clone.borrow_mut() += 1;
    });

    if let Some(handler) = &inp.on_change {
        handler("a");
        handler("ab");
        handler("abc");
    }
    assert_eq!(*call_count.borrow(), 3);
}

#[test]
fn test_on_submit_multiple_calls() {
    let call_count = Rc::new(RefCell::new(0));
    let count_clone = call_count.clone();

    let inp = Input::new().value("test").on_submit(move |_| {
        *count_clone.borrow_mut() += 1;
    });

    if let Some(handler) = &inp.on_submit {
        handler("first");
        handler("second");
    }
    assert_eq!(*call_count.borrow(), 2);
}

// ==================== Input State Modification Tests ====================

#[test]
fn test_state_modification() {
    let mut inp = Input::new().value("test");

    inp.state.focused = true;
    assert!(inp.state.focused);

    inp.state.hovered = true;
    assert!(inp.state.hovered);

    inp.state.cursor_position = 2;
    assert_eq!(inp.state.cursor_position, 2);

    inp.state.selection_start = Some(0);
    inp.state.selection_end = Some(2);
    assert_eq!(inp.state.selection_start, Some(0));
    assert_eq!(inp.state.selection_end, Some(2));
}

// ==================== Width Tests ====================

#[test]
fn test_width_table() {
    struct TestCase {
        width: f32,
    }

    let test_cases = [
        TestCase { width: 100.0 },
        TestCase { width: 200.0 },
        TestCase { width: 300.0 },
        TestCase { width: 0.0 },
        TestCase { width: 500.5 },
    ];

    for tc in test_cases {
        let inp = Input::new().w(tc.width);
        assert_eq!(inp.width, Some(tc.width));
    }
}

// ==================== Rounded Corners Tests ====================

#[test]
fn test_rounded_table() {
    struct TestCase {
        radius: f32,
    }

    let test_cases = [
        TestCase { radius: 0.0 },
        TestCase { radius: 4.0 },
        TestCase { radius: 8.0 },
        TestCase { radius: 16.0 },
        TestCase { radius: 100.0 },
    ];

    for tc in test_cases {
        let inp = Input::new().rounded(tc.radius);
        assert_eq!(inp.style.border.radius, Corners::all(tc.radius));
    }
}

// ==================== Edge Cases Tests ====================

#[test]
fn test_empty_placeholder() {
    let inp = Input::new().placeholder("");
    assert_eq!(inp.placeholder, "");
}

#[test]
fn test_long_value() {
    let long_string = "a".repeat(10000);
    let inp = Input::new().value(long_string.clone());
    assert_eq!(inp.state.value, long_string);
    assert_eq!(inp.state.cursor_position, 10000);
}

#[test]
fn test_unicode_value() {
    let inp = Input::new().value("Unicode test string");
    assert_eq!(inp.state.value, "Unicode test string");
}

#[test]
fn test_special_characters() {
    let inp = Input::new().value("!@#$%^&*()_+-=[]{}|;':\",./<>?");
    assert_eq!(inp.state.value, "!@#$%^&*()_+-=[]{}|;':\",./<>?");
}

#[test]
fn unicode_insert_advances_cursor_to_utf8_boundary() {
    let mut inp = Input::new();
    inp.state.focused = true;
    let taffy = TaffyTree::new();
    let mut focused = None;
    let mut cx = focused_context(&taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &typed_char_event('é')));

    assert_eq!(inp.state.value, "é");
    assert_eq!(inp.state.cursor_position, "é".len());
    assert!(inp.state.value.is_char_boundary(inp.state.cursor_position));
}

#[test]
fn unicode_backspace_removes_previous_character_without_panic() {
    let mut inp = Input::new().value("é");
    inp.state.focused = true;
    let taffy = TaffyTree::new();
    let mut focused = None;
    let mut cx = focused_context(&taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::Backspace)));

    assert_eq!(inp.state.value, "");
    assert_eq!(inp.state.cursor_position, 0);
}

#[test]
fn unicode_arrow_navigation_moves_by_character_boundaries() {
    let mut inp = Input::new().value("éx");
    inp.state.focused = true;
    let taffy = TaffyTree::new();
    let mut focused = None;
    let mut cx = focused_context(&taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::ArrowLeft)));
    assert_eq!(inp.state.cursor_position, "é".len());

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::ArrowLeft)));
    assert_eq!(inp.state.cursor_position, 0);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::ArrowRight)));
    assert_eq!(inp.state.cursor_position, "é".len());
}

#[test]
fn unicode_delete_removes_next_character_without_panic() {
    let mut inp = Input::new().value("éx");
    inp.state.focused = true;
    inp.state.cursor_position = 0;
    let taffy = TaffyTree::new();
    let mut focused = None;
    let mut cx = focused_context(&taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::Delete)));

    assert_eq!(inp.state.value, "x");
    assert_eq!(inp.state.cursor_position, 0);
}

// ==================== Callback Not Panicking Tests ====================

#[test]
fn test_on_change_not_set_no_panic() {
    let inp = Input::new().value("test");
    // This should not panic
    if let Some(handler) = &inp.on_change {
        handler(&inp.state.value);
    }
}

#[test]
fn test_on_submit_not_set_no_panic() {
    let inp = Input::new().value("test");
    // This should not panic
    if let Some(handler) = &inp.on_submit {
        handler(&inp.state.value);
    }
}

#[test]
fn test_on_focus_not_set_no_panic() {
    let inp = Input::new();
    // This should not panic
    if let Some(handler) = &inp.on_focus {
        handler();
    }
}

#[test]
fn test_on_blur_not_set_no_panic() {
    let inp = Input::new();
    // This should not panic
    if let Some(handler) = &inp.on_blur {
        handler();
    }
}
