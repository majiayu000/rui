use super::support::*;

// ==================== display_text Tests ====================

#[test]
fn test_display_text_normal() {
    let inp = Input::new().value("hello world");
    assert_eq!(inp.display_text(), "hello world");
}

#[test]
fn test_display_text_password() {
    let inp = Input::new().value("secret123").password();
    assert_eq!(
        inp.display_text(),
        "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
    );
}

#[test]
fn test_display_text_password_empty() {
    let inp = Input::new().value("").password();
    assert_eq!(inp.display_text(), "");
}

#[test]
fn test_display_text_password_single_char() {
    let inp = Input::new().value("a").password();
    assert_eq!(inp.display_text(), "\u{2022}");
}

#[test]
fn test_display_text_email() {
    let inp = Input::new().value("user@example.com").email();
    assert_eq!(inp.display_text(), "user@example.com");
}

#[test]
fn test_display_text_number() {
    let inp = Input::new().value("12345").number();
    assert_eq!(inp.display_text(), "12345");
}

#[test]
fn test_display_text_search() {
    let inp = Input::new().value("search term").search();
    assert_eq!(inp.display_text(), "search term");
}

// ==================== display_text Table-Driven Tests ====================

#[test]
fn test_display_text_table() {
    struct TestCase {
        value: &'static str,
        input_type: InputType,
        expected: &'static str,
    }

    let test_cases = [
        TestCase {
            value: "hello",
            input_type: InputType::Text,
            expected: "hello",
        },
        TestCase {
            value: "secret",
            input_type: InputType::Password,
            expected: "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}",
        },
        TestCase {
            value: "",
            input_type: InputType::Password,
            expected: "",
        },
        TestCase {
            value: "user@test.com",
            input_type: InputType::Email,
            expected: "user@test.com",
        },
        TestCase {
            value: "42",
            input_type: InputType::Number,
            expected: "42",
        },
        TestCase {
            value: "query",
            input_type: InputType::Search,
            expected: "query",
        },
    ];

    for tc in test_cases {
        let inp = Input::new().value(tc.value).input_type(tc.input_type);
        assert_eq!(
            inp.display_text(),
            tc.expected,
            "Failed for value: {}, type: {:?}",
            tc.value,
            tc.input_type
        );
    }
}

// ==================== colors Tests ====================

#[test]
fn test_colors_default_empty() {
    let inp = Input::new();
    let (bg, text, border) = inp.colors();
    assert_eq!(bg, Color::WHITE);
    assert_eq!(text, Color::hex(0x9ca3af)); // placeholder color
    assert_eq!(border, Color::hex(0xd1d5db));
}

#[test]
fn test_colors_with_value() {
    let mut inp = Input::new().value("test");
    inp.state.value = "test".to_string();
    let (bg, text, border) = inp.colors();
    assert_eq!(bg, Color::WHITE);
    assert_eq!(text, Color::hex(0x111827)); // text color
    assert_eq!(border, Color::hex(0xd1d5db));
}

#[test]
fn test_colors_focused() {
    let mut inp = Input::new();
    inp.state.focused = true;
    let (bg, _text, border) = inp.colors();
    assert_eq!(bg, Color::WHITE);
    assert_eq!(border, Color::hex(0x6366f1)); // focus ring
}

#[test]
fn test_colors_hovered() {
    let mut inp = Input::new();
    inp.state.hovered = true;
    let (bg, _text, border) = inp.colors();
    assert_eq!(bg, Color::WHITE);
    assert_eq!(border, Color::hex(0x9ca3af)); // hover border
}

#[test]
fn test_colors_focused_takes_priority_over_hovered() {
    let mut inp = Input::new();
    inp.state.focused = true;
    inp.state.hovered = true;
    let (_bg, _text, border) = inp.colors();
    assert_eq!(border, Color::hex(0x6366f1)); // focus ring takes priority
}

// ==================== cursor Tests ====================

#[test]
fn test_cursor_type() {
    let inp = Input::new();
    assert_eq!(inp.cursor(), Cursor::Text);
}

#[test]
fn test_cursor_type_password() {
    let inp = Input::new().password();
    assert_eq!(inp.cursor(), Cursor::Text);
}

#[test]
fn test_cursor_type_all_input_types() {
    let types = [
        InputType::Text,
        InputType::Password,
        InputType::Email,
        InputType::Number,
        InputType::Search,
    ];

    for input_type in types {
        let inp = Input::new().input_type(input_type);
        assert_eq!(
            inp.cursor(),
            Cursor::Text,
            "Cursor should be Text for {:?}",
            input_type
        );
    }
}

#[test]
fn input_paints_selection_marked_text_and_layout_caret() {
    let mut inp = Input::new().value("hello");
    inp.state.focused = true;
    inp.state.selection_start = Some(1);
    inp.state.selection_end = Some(4);
    inp.state.composition_range = Some(range(2, 4));
    inp.state.marked_text = Some("ll".to_string());

    let primitives = painted_primitives(inp);
    let quads: Vec<_> = primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad {
                bounds, background, ..
            } => Some((*bounds, *background)),
            _ => None,
        })
        .collect();

    assert!(
        quads
            .iter()
            .any(|(bounds, background)| background.a == 0.22 && bounds.width() == 21.0),
        "selection paint primitive should cover the selected grapheme range"
    );
    assert!(
        quads.iter().any(|(bounds, background)| {
            *background == Color::hex(0x6366f1).to_rgba()
                && bounds.height() == INPUT_MARKED_UNDERLINE_HEIGHT
        }),
        "marked text primitive should paint an underline"
    );
    assert!(
        quads.iter().any(|(bounds, background)| {
            *background == Color::hex(0x6366f1).to_rgba()
                && bounds.width() == INPUT_CARET_WIDTH
                && bounds.height() == 20.0
        }),
        "caret primitive should come from the text edit layout"
    );
}

// ==================== Element Trait Tests ====================

#[test]
fn test_element_id_generated_by_default() {
    let inp = Input::new();
    assert!(Element::id(&inp).is_some());
}

#[test]
fn test_element_id_when_set() {
    let id = ElementId::new();
    let inp = Input::new().id(id);
    assert_eq!(Element::id(&inp), Some(id));
}

#[test]
fn test_element_style_returns_style() {
    let inp = Input::new();
    let style = Element::style(&inp);
    // Verify style has expected default values
    assert_eq!(style.border.radius, Corners::all(6.0));
}

#[test]
fn test_element_style_after_rounded() {
    let inp = Input::new().rounded(12.0);
    let style = Element::style(&inp);
    assert_eq!(style.border.radius, Corners::all(12.0));
}

#[test]
fn test_element_style_after_border_color() {
    let inp = Input::new().border_color(Color::GREEN);
    let style = Element::style(&inp);
    assert_eq!(style.border.color, Color::GREEN);
}

// ==================== Default Style Tests ====================

#[test]
fn test_default_border_radius() {
    let inp = Input::new();
    assert_eq!(inp.style.border.radius, Corners::all(6.0));
}

#[test]
fn test_default_border_color() {
    let inp = Input::new();
    assert_eq!(inp.style.border.color, Color::hex(0xd1d5db));
}

#[test]
fn test_default_border_width() {
    let inp = Input::new();
    assert_eq!(inp.style.border.width, Edges::all(1.0));
}
