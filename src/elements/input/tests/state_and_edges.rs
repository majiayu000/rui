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

#[test]
fn input_key_events_edit_grapheme_clusters_and_selection_ranges() {
    let mut inp = Input::new().value("a 🧑‍💻");
    inp.state.focused = true;
    let taffy = TaffyTree::new();
    let mut focused = None;
    let mut cx = focused_context(&taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::Backspace)));
    assert_eq!(inp.state.value, "a ");
    assert_eq!(inp.state.cursor_position, "a ".len());

    inp.state.value = "alpha".to_string();
    inp.state.cursor_position = "alpha".len();
    inp.state.selection_start = None;
    inp.state.selection_end = None;

    assert!(inp.handle_key_event(&mut cx, &shifted_key_event(KeyCode::ArrowLeft)));
    assert_eq!(inp.state.cursor_position, 4);
    assert_eq!(inp.state.selection_start, Some(4));
    assert_eq!(inp.state.selection_end, Some(5));

    assert!(inp.handle_key_event(&mut cx, &typed_char_event('Z')));
    assert_eq!(inp.state.value, "alphZ");
    assert_eq!(inp.state.cursor_position, 5);
    assert_eq!(inp.state.selection_start, None);
    assert_eq!(inp.state.selection_end, None);
}

#[test]
fn input_ime_composition_exposes_marked_text_and_replacement_range() {
    let mut inp = Input::new().value("hello");
    inp.state.selection_start = Some(0);
    inp.state.selection_end = Some(5);

    let outcome = match inp.apply_text_input_event(TextInputEvent::BeginComposition("你".into())) {
        Ok(outcome) => outcome,
        Err(err) => panic!("begin composition failed: {err}"),
    };
    assert!(outcome.changed);
    assert_eq!(inp.state.value, "你");
    assert_eq!(inp.state.marked_text.as_deref(), Some("你"));
    assert_eq!(inp.state.composition_range, Some(range(0, "你".len())));

    if let Err(err) = inp.apply_text_input_event(TextInputEvent::UpdateComposition("你好".into()))
    {
        panic!("update composition failed: {err}");
    }
    assert_eq!(inp.state.value, "你好");
    assert_eq!(inp.state.marked_text.as_deref(), Some("你好"));
    assert_eq!(inp.state.composition_range, Some(range(0, "你好".len())));

    if let Err(err) = inp.apply_text_input_event(TextInputEvent::CommitComposition("您好".into()))
    {
        panic!("commit composition failed: {err}");
    }
    assert_eq!(inp.state.value, "您好");
    assert_eq!(inp.state.marked_text, None);
    assert_eq!(inp.state.composition_range, None);

    inp.state.selection_start = Some(0);
    inp.state.selection_end = Some(inp.state.value.len());
    if let Err(err) = inp.apply_text_input_event(TextInputEvent::BeginComposition("abc".into())) {
        panic!("begin replacement composition failed: {err}");
    }
    assert_eq!(inp.state.value, "abc");

    if let Err(err) = inp.apply_text_input_event(TextInputEvent::CancelComposition) {
        panic!("cancel composition failed: {err}");
    }
    assert_eq!(inp.state.value, "您好");
    assert_eq!(inp.state.marked_text, None);
    assert_eq!(inp.state.composition_range, None);
}

#[test]
fn input_clipboard_operations_surface_explicit_errors() {
    let mut inp = Input::new().value("alpha beta");
    inp.state.selection_start = Some(0);
    inp.state.selection_end = Some(5);
    let mut clipboard = MemoryClipboard::new();

    match inp.copy_selection_to(&mut clipboard) {
        Ok(copied) => assert!(copied),
        Err(err) => panic!("copy failed: {err}"),
    }
    assert_eq!(clipboard.text(), "alpha");

    let outcome = match inp.cut_selection_to(&mut clipboard) {
        Ok(outcome) => outcome,
        Err(err) => panic!("cut failed: {err}"),
    };
    assert!(outcome.changed);
    assert_eq!(inp.state.value, " beta");

    if let Err(err) = inp.paste_from(&mut clipboard) {
        panic!("paste failed: {err}");
    }
    assert_eq!(inp.state.value, "alpha beta");

    let mut read_error = MemoryClipboard::with_read_error("denied");
    let error = match inp.paste_from(&mut read_error) {
        Ok(_) => panic!("clipboard read should fail"),
        Err(err) => err,
    };
    assert!(matches!(error, TextEditError::Clipboard(_)));
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
