use super::support::*;

// ==================== Builder Pattern Tests ====================

#[test]
fn test_builder_id() {
    let id = ElementId::new();
    let inp = Input::new().id(id);
    assert_eq!(inp.id, Some(id));
}

#[test]
fn test_builder_placeholder() {
    let inp = Input::new().placeholder("Enter your name");
    assert_eq!(inp.placeholder, "Enter your name");
}

#[test]
fn test_builder_placeholder_with_string() {
    let inp = Input::new().placeholder(String::from("Type here..."));
    assert_eq!(inp.placeholder, "Type here...");
}

#[test]
fn test_builder_value() {
    let inp = Input::new().value("hello");
    assert_eq!(inp.state.value, "hello");
    assert_eq!(inp.state.cursor_position, 5);
}

#[test]
fn test_builder_value_with_string() {
    let inp = Input::new().value(String::from("world"));
    assert_eq!(inp.state.value, "world");
}

#[test]
fn test_builder_value_empty() {
    let inp = Input::new().value("");
    assert_eq!(inp.state.value, "");
    assert_eq!(inp.state.cursor_position, 0);
}

#[test]
fn test_builder_value_unicode() {
    let inp = Input::new().value("Hello, World!");
    assert_eq!(inp.state.value, "Hello, World!");
}

#[test]
fn test_builder_value_sets_cursor_to_end() {
    let inp = Input::new().value("testing");
    assert_eq!(inp.state.cursor_position, 7);
}

#[test]
fn test_builder_input_type() {
    let inp = Input::new().input_type(InputType::Password);
    assert_eq!(inp.input_type, InputType::Password);
}

#[test]
fn test_builder_password() {
    let inp = Input::new().password();
    assert_eq!(inp.input_type, InputType::Password);
}

#[test]
fn test_builder_email() {
    let inp = Input::new().email();
    assert_eq!(inp.input_type, InputType::Email);
}

#[test]
fn test_builder_number() {
    let inp = Input::new().number();
    assert_eq!(inp.input_type, InputType::Number);
}

#[test]
fn test_builder_search() {
    let inp = Input::new().search();
    assert_eq!(inp.input_type, InputType::Search);
}

#[test]
fn test_builder_width() {
    let inp = Input::new().w(200.0);
    assert_eq!(inp.width, Some(200.0));
}

#[test]
fn test_builder_rounded() {
    let inp = Input::new().rounded(10.0);
    assert_eq!(inp.style.border.radius, Corners::all(10.0));
}

#[test]
fn test_builder_border_color() {
    let inp = Input::new().border_color(Color::RED);
    assert_eq!(inp.style.border.color, Color::RED);
}

#[test]
fn test_builder_border_color_hex() {
    let inp = Input::new().border_color(Color::hex(0xFF00FF));
    let _ = inp.style.border.color; // Just verify no panic
}

// ==================== Builder Chain Tests ====================

#[test]
fn test_builder_chain() {
    let id = ElementId::new();
    let inp = Input::new()
        .id(id)
        .value("test")
        .placeholder("Enter text")
        .password()
        .w(300.0)
        .rounded(8.0)
        .border_color(Color::BLUE);

    assert_eq!(inp.id, Some(id));
    assert_eq!(inp.state.value, "test");
    assert_eq!(inp.placeholder, "Enter text");
    assert_eq!(inp.input_type, InputType::Password);
    assert_eq!(inp.width, Some(300.0));
    assert_eq!(inp.style.border.radius, Corners::all(8.0));
    assert_eq!(inp.style.border.color, Color::BLUE);
}

#[test]
fn test_builder_chain_all_input_types() {
    // Test that we can switch input types multiple times
    let inp = Input::new()
        .password()
        .email()
        .number()
        .search()
        .input_type(InputType::Text);

    assert_eq!(inp.input_type, InputType::Text);
}

// ==================== Input Type Table-Driven Builder Tests ====================

#[test]
fn test_input_type_shortcut_methods_table() {
    struct TestCase {
        name: &'static str,
        build: fn(Input) -> Input,
        expected_type: InputType,
    }

    let test_cases = [
        TestCase {
            name: "password",
            build: |i| i.password(),
            expected_type: InputType::Password,
        },
        TestCase {
            name: "email",
            build: |i| i.email(),
            expected_type: InputType::Email,
        },
        TestCase {
            name: "number",
            build: |i| i.number(),
            expected_type: InputType::Number,
        },
        TestCase {
            name: "search",
            build: |i| i.search(),
            expected_type: InputType::Search,
        },
    ];

    for tc in test_cases {
        let inp = (tc.build)(Input::new());
        assert_eq!(
            inp.input_type, tc.expected_type,
            "Failed for input type shortcut: {}",
            tc.name
        );
    }
}
