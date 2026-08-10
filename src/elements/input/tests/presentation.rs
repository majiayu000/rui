use super::support::*;

#[test]
fn refresh_text_geometry_rebuilds_a_layout_invalidated_by_input() {
    let mut input = Input::new().value("a");
    let _ = layout_input(&mut input, Size::new(240.0, 56.0));
    assert_eq!(
        input.current_text_layout().map(|layout| layout.text()),
        Some("a")
    );

    input
        .apply_text_input_event(TextInputEvent::InsertText("b".to_string()))
        .expect("input mutation should apply");
    assert!(input.current_text_layout().is_none());

    let mut text_measurer = crate::renderer::text::TextMeasureCache::new();
    Element::refresh_text_geometry(&mut input, &mut text_measurer);
    assert_eq!(
        input.current_text_layout().map(|layout| layout.text()),
        Some("ab")
    );
}

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
    use crate::renderer::text::{TextMeasureCache, TextRequest};

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
    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new("hello", 14.0, 400, None, 1.0))
        .expect("test text should shape");
    let expected_selection_width: f32 = plan
        .clusters()
        .iter()
        .filter(|cluster| cluster.byte_end > 1 && cluster.byte_start < 4)
        .map(|cluster| cluster.advance_width)
        .sum();

    assert!(
        quads.iter().any(|(bounds, background)| {
            background.a == 0.22 && (bounds.width() - expected_selection_width).abs() < 0.01
        }),
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

#[test]
fn input_caret_uses_the_same_proportional_shape_plan_as_rendered_text() {
    use crate::renderer::text::{TextMeasureCache, TextRequest};

    let mut inp = Input::new().value("Wi");
    inp.state.focused = true;
    inp.state.cursor_position = 1;

    let primitives = painted_primitives(inp);
    let caret_x = primitives
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Quad {
                bounds, background, ..
            } if *background == Color::hex(0x6366f1).to_rgba()
                && bounds.width() == INPUT_CARET_WIDTH
                && bounds.height() == 20.0 =>
            {
                Some(bounds.x())
            }
            _ => None,
        })
        .expect("focused input should paint a caret");
    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new("Wi", 14.0, 400, None, 1.0))
        .expect("test text should shape");
    let expected = INPUT_HORIZONTAL_PADDING + plan.clusters()[0].advance_width;

    assert!((caret_x - expected).abs() < 0.01);
}

#[test]
fn input_uses_shaped_visual_order_for_rtl_arrow_navigation() {
    let id = ElementId::new();
    let mut inp = Input::new().id(id).value("שלום");
    inp.state.focused = true;
    let (taffy, bounds) = layout_input(&mut inp, Size::new(240.0, 56.0));
    let before = inp.state.cursor_position;
    let mut focused = Some(id);
    let mut cx = EventContext::new(bounds, &taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::ArrowRight)));
    assert!(inp.state.cursor_position < before);
    let layout = inp.text_layout.as_ref().expect("layout should be retained");
    let before_x = layout
        .caret_for_offset(before)
        .expect("old caret should be valid")
        .position
        .x;
    let after_x = layout
        .caret_for_offset(inp.state.cursor_position)
        .expect("new caret should be valid")
        .position
        .x;
    assert!(after_x > before_x);
}

#[test]
fn input_mixed_bidi_navigation_and_hit_testing_preserve_caret_affinity() {
    use crate::renderer::text::{TextDirection, TextMeasureCache, TextRequest};
    use unicode_segmentation::UnicodeSegmentation;

    let text = "abc שלום";
    let id = ElementId::new();
    let mut inp = Input::new().id(id).value(text).w(240.0);
    inp.state.focused = true;
    let (taffy, bounds) = layout_input(&mut inp, Size::new(280.0, 56.0));
    let mut focused = Some(id);
    let mut cx = EventContext::new(bounds, &taffy, &mut focused);

    assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::Home)));
    let mut visited = vec![inp.visual_caret.expect("Home should set visual affinity")];
    for _ in 0..text.graphemes(true).count() {
        assert!(inp.handle_key_event(&mut cx, &key_event(KeyCode::ArrowRight)));
        visited.push(
            inp.visual_caret
                .expect("visual navigation should retain affinity"),
        );
    }
    assert!(visited.windows(2).all(|pair| pair[0].x() < pair[1].x()));
    let mut offsets = std::collections::HashSet::new();
    assert!(visited.iter().any(|caret| !offsets.insert(caret.offset())));

    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new(text, 14.0, 400, None, 1.0))
        .expect("mixed bidi text should shape");
    let rtl = plan
        .clusters()
        .iter()
        .find(|cluster| cluster.direction == TextDirection::RightToLeft)
        .expect("mixed bidi plan should include RTL text");
    let boundary = rtl.byte_start;
    let upstream = plan
        .clusters()
        .iter()
        .find(|cluster| cluster.byte_end == boundary)
        .map(|cluster| cluster.x_offset + cluster.advance_width)
        .expect("LTR side of bidi boundary should exist");
    let downstream = rtl.x_offset + rtl.advance_width;
    let origin_x = bounds.x() + INPUT_HORIZONTAL_PADDING;

    for expected_x in [upstream, downstream] {
        let event = PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(origin_x + expected_x, bounds.center().y),
            button: Some(crate::core::event::MouseButton::Left),
        };
        assert!(
            inp.handle_pointer_event(&mut cx, &event),
            "pointer {:?} should be inside {bounds:?}",
            event.position
        );
        assert_eq!(inp.state.cursor_position, boundary);
        let caret = inp
            .visual_caret
            .expect("pointer hit should retain affinity");
        assert!((caret.x() - expected_x).abs() < 0.01);

        let mut scene = Scene::new();
        let mut paint_cx = PaintContext::new(&mut scene, bounds, &taffy);
        inp.paint(&mut paint_cx);
        let painted = inp
            .caret_bounds
            .expect("focused input should paint a caret");
        assert!((painted.x() - (origin_x + expected_x)).abs() < 0.01);

        let geometry = inp
            .native_text_input_geometry()
            .expect("painted input should expose native caret geometry");
        let (_, native) = geometry
            .first_bounds_for_range(range(boundary, boundary))
            .expect("native geometry query should succeed")
            .expect("collapsed selection should have caret geometry");
        assert!((native.x() - painted.x()).abs() < 0.01);
    }
}

#[test]
fn input_pointer_hit_testing_uses_shaped_cluster_advances() {
    use crate::renderer::text::{TextMeasureCache, TextRequest};

    let id = ElementId::new();
    let mut inp = Input::new().id(id).value("Wi");
    let (taffy, bounds) = layout_input(&mut inp, Size::new(240.0, 56.0));
    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new("Wi", 14.0, 400, None, 1.0))
        .expect("test text should shape");
    let mut focused = None;
    let mut cx = EventContext::new(bounds, &taffy, &mut focused);
    let event = PointerEvent {
        kind: PointerEventKind::Down,
        position: Point::new(
            bounds.x() + INPUT_HORIZONTAL_PADDING + plan.clusters()[0].advance_width,
            bounds.center().y,
        ),
        button: Some(crate::core::event::MouseButton::Left),
    };

    assert!(inp.handle_pointer_event(&mut cx, &event));
    assert_eq!(inp.state.cursor_position, 1);
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
