use super::support::*;

// ==================== InputType Enum Tests ====================

#[test]
fn test_input_type_default() {
    let input_type = InputType::default();
    assert_eq!(input_type, InputType::Text);
}

#[test]
fn test_input_type_text() {
    assert_eq!(InputType::Text, InputType::Text);
}

#[test]
fn test_input_type_password() {
    assert_eq!(InputType::Password, InputType::Password);
}

#[test]
fn test_input_type_email() {
    assert_eq!(InputType::Email, InputType::Email);
}

#[test]
fn test_input_type_number() {
    assert_eq!(InputType::Number, InputType::Number);
}

#[test]
fn test_input_type_search() {
    assert_eq!(InputType::Search, InputType::Search);
}

#[test]
fn test_input_type_inequality() {
    assert_ne!(InputType::Text, InputType::Password);
    assert_ne!(InputType::Email, InputType::Number);
    assert_ne!(InputType::Search, InputType::Text);
}

#[test]
fn test_input_type_clone() {
    let input_type = InputType::Password;
    let cloned = input_type.clone();
    assert_eq!(input_type, cloned);
}

#[test]
fn test_input_type_copy() {
    let input_type = InputType::Email;
    let copied: InputType = input_type; // Copy
    assert_eq!(input_type, copied);
}

#[test]
fn test_input_type_debug() {
    let debug_str = format!("{:?}", InputType::Text);
    assert_eq!(debug_str, "Text");

    let debug_str = format!("{:?}", InputType::Password);
    assert_eq!(debug_str, "Password");

    let debug_str = format!("{:?}", InputType::Email);
    assert_eq!(debug_str, "Email");

    let debug_str = format!("{:?}", InputType::Number);
    assert_eq!(debug_str, "Number");

    let debug_str = format!("{:?}", InputType::Search);
    assert_eq!(debug_str, "Search");
}

// ==================== InputType Table-Driven Tests ====================

#[test]
fn test_input_type_variants_table() {
    struct TestCase {
        input_type: InputType,
        expected_debug: &'static str,
    }

    let test_cases = [
        TestCase {
            input_type: InputType::Text,
            expected_debug: "Text",
        },
        TestCase {
            input_type: InputType::Password,
            expected_debug: "Password",
        },
        TestCase {
            input_type: InputType::Email,
            expected_debug: "Email",
        },
        TestCase {
            input_type: InputType::Number,
            expected_debug: "Number",
        },
        TestCase {
            input_type: InputType::Search,
            expected_debug: "Search",
        },
    ];

    for tc in test_cases {
        assert_eq!(format!("{:?}", tc.input_type), tc.expected_debug);
    }
}

// ==================== InputState Tests ====================

#[test]
fn test_input_state_default() {
    let state = InputState::default();
    assert_eq!(state.value, "");
    assert_eq!(state.cursor_position, 0);
    assert_eq!(state.selection_start, None);
    assert_eq!(state.selection_end, None);
    assert!(!state.focused);
    assert!(!state.hovered);
}

#[test]
fn test_input_state_clone() {
    let mut state = InputState::default();
    state.value = "test".to_string();
    state.cursor_position = 4;
    state.focused = true;
    state.hovered = true;
    state.selection_start = Some(1);
    state.selection_end = Some(3);

    let cloned = state.clone();
    assert_eq!(cloned.value, "test");
    assert_eq!(cloned.cursor_position, 4);
    assert!(cloned.focused);
    assert!(cloned.hovered);
    assert_eq!(cloned.selection_start, Some(1));
    assert_eq!(cloned.selection_end, Some(3));
}

#[test]
fn test_input_state_debug() {
    let state = InputState::default();
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("InputState"));
    assert!(debug_str.contains("value"));
    assert!(debug_str.contains("cursor_position"));
}

// ==================== Input Creation Tests ====================

#[test]
fn test_input_new() {
    let input = Input::new();
    assert_eq!(input.placeholder, "");
    assert_eq!(input.input_type, InputType::Text);
    assert_eq!(input.state.value, "");
    assert!(!input.state.focused);
    assert!(!input.state.hovered);
    assert!(input.id.is_none());
    assert!(input.width.is_none());
    assert!(input.on_change.is_none());
    assert!(input.on_submit.is_none());
    assert!(input.on_focus.is_none());
    assert!(input.on_blur.is_none());
}

#[test]
fn test_input_default() {
    let input = Input::default();
    assert_eq!(input.placeholder, "");
    assert_eq!(input.input_type, InputType::Text);
    assert_eq!(input.state.value, "");
}

#[test]
fn test_input_helper_function() {
    let inp = input();
    assert_eq!(inp.placeholder, "");
    assert_eq!(inp.input_type, InputType::Text);
}
