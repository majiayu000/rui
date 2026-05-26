use super::*;
use taffy::prelude::{AvailableSpace, TaffyTree};

// ==================== TextAlign Tests ====================

#[test]
fn test_text_align_default() {
    let align = TextAlign::default();
    assert_eq!(align, TextAlign::Left);
}

#[test]
fn test_text_align_variants() {
    let cases = [
        (TextAlign::Left, "Left"),
        (TextAlign::Center, "Center"),
        (TextAlign::Right, "Right"),
    ];

    for (align, expected_debug) in cases {
        assert!(format!("{:?}", align).contains(expected_debug));
    }
}

#[test]
fn test_text_align_equality() {
    assert_eq!(TextAlign::Left, TextAlign::Left);
    assert_eq!(TextAlign::Center, TextAlign::Center);
    assert_eq!(TextAlign::Right, TextAlign::Right);
    assert_ne!(TextAlign::Left, TextAlign::Center);
    assert_ne!(TextAlign::Left, TextAlign::Right);
    assert_ne!(TextAlign::Center, TextAlign::Right);
}

#[test]
fn test_text_align_clone() {
    let align = TextAlign::Center;
    let cloned = align.clone();
    assert_eq!(align, cloned);
}

#[test]
fn test_text_align_copy() {
    let align = TextAlign::Right;
    let copied: TextAlign = align;
    assert_eq!(align, copied);
}

// ==================== FontWeight Tests ====================

#[test]
fn test_font_weight_default() {
    let weight = FontWeight::default();
    assert_eq!(weight, FontWeight::Regular);
}

#[test]
fn test_font_weight_to_value() {
    let cases = [
        (FontWeight::Thin, 100_u16),
        (FontWeight::Light, 300),
        (FontWeight::Regular, 400),
        (FontWeight::Medium, 500),
        (FontWeight::Semibold, 600),
        (FontWeight::Bold, 700),
        (FontWeight::Black, 900),
    ];

    for (weight, expected_value) in cases {
        assert_eq!(
            weight.to_value(),
            expected_value,
            "FontWeight::{:?} should have value {}",
            weight,
            expected_value
        );
    }
}

#[test]
fn test_font_weight_equality() {
    assert_eq!(FontWeight::Thin, FontWeight::Thin);
    assert_eq!(FontWeight::Bold, FontWeight::Bold);
    assert_ne!(FontWeight::Thin, FontWeight::Bold);
    assert_ne!(FontWeight::Light, FontWeight::Regular);
}

#[test]
fn test_font_weight_clone() {
    let weight = FontWeight::Bold;
    let cloned = weight.clone();
    assert_eq!(weight, cloned);
}

#[test]
fn test_font_weight_copy() {
    let weight = FontWeight::Semibold;
    let copied: FontWeight = weight;
    assert_eq!(weight, copied);
}

#[test]
fn test_font_weight_debug() {
    let weights = [
        (FontWeight::Thin, "Thin"),
        (FontWeight::Light, "Light"),
        (FontWeight::Regular, "Regular"),
        (FontWeight::Medium, "Medium"),
        (FontWeight::Semibold, "Semibold"),
        (FontWeight::Bold, "Bold"),
        (FontWeight::Black, "Black"),
    ];

    for (weight, expected_debug) in weights {
        assert!(
            format!("{:?}", weight).contains(expected_debug),
            "FontWeight::{:?} should contain '{}'",
            weight,
            expected_debug
        );
    }
}

// ==================== Text::new Tests ====================

#[test]
fn test_text_new_with_string() {
    let t = Text::new("Hello".to_string());
    assert_eq!(t.content, "Hello");
}

#[test]
fn test_text_new_with_str() {
    let t = Text::new("World");
    assert_eq!(t.content, "World");
}

#[test]
fn test_text_new_empty_string() {
    let t = Text::new("");
    assert_eq!(t.content, "");
}

#[test]
fn test_text_new_default_values() {
    let t = Text::new("Test");

    assert_eq!(t.id, None);
    assert_eq!(t.content, "Test");
    assert_eq!(t.color, Color::BLACK);
    assert_eq!(t.font_size, 14.0);
    assert_eq!(t.font_weight, FontWeight::Regular);
    assert_eq!(t.font_family, None);
    assert_eq!(t.line_height, 1.4);
    assert_eq!(t.align, TextAlign::Left);
    assert_eq!(t.layout_node, None);
}

// ==================== Text Builder Method Tests ====================

#[test]
fn test_text_id() {
    let t = Text::new("Test").id(ElementId::from(42u64));
    assert_eq!(t.id, Some(ElementId::from(42u64)));
}

#[test]
fn test_text_color() {
    let t = Text::new("Test").color(Color::RED);
    assert_eq!(t.color, Color::RED);
}

#[test]
fn test_text_color_with_rgba() {
    let rgba = crate::core::color::Rgba::new(0.5, 0.5, 0.5, 1.0);
    let t = Text::new("Test").color(rgba);
    assert_eq!(t.color, Color::Rgba(rgba));
}

#[test]
fn test_text_color_with_hex() {
    let t = Text::new("Test").color(Color::hex(0xFF0000));
    let rgba = t.color.to_rgba();
    assert!((rgba.r - 1.0).abs() < 0.01);
    assert!(rgba.g.abs() < 0.01);
    assert!(rgba.b.abs() < 0.01);
}

#[test]
fn test_text_size() {
    let t = Text::new("Test").size(24.0);
    assert_eq!(t.font_size, 24.0);
}

#[test]
fn test_text_size_small() {
    let t = Text::new("Test").size(8.0);
    assert_eq!(t.font_size, 8.0);
}

#[test]
fn test_text_size_large() {
    let t = Text::new("Test").size(72.0);
    assert_eq!(t.font_size, 72.0);
}

#[test]
fn test_text_weight() {
    let t = Text::new("Test").weight(FontWeight::Bold);
    assert_eq!(t.font_weight, FontWeight::Bold);
}

#[test]
fn test_text_weight_all_variants() {
    let weights = [
        FontWeight::Thin,
        FontWeight::Light,
        FontWeight::Regular,
        FontWeight::Medium,
        FontWeight::Semibold,
        FontWeight::Bold,
        FontWeight::Black,
    ];

    for weight in weights {
        let t = Text::new("Test").weight(weight);
        assert_eq!(t.font_weight, weight);
    }
}

#[test]
fn test_text_bold() {
    let t = Text::new("Test").bold();
    assert_eq!(t.font_weight, FontWeight::Bold);
}

#[test]
fn test_text_semibold() {
    let t = Text::new("Test").semibold();
    assert_eq!(t.font_weight, FontWeight::Semibold);
}

#[test]
fn test_text_medium() {
    let t = Text::new("Test").medium();
    assert_eq!(t.font_weight, FontWeight::Medium);
}

#[test]
fn test_text_light() {
    let t = Text::new("Test").light();
    assert_eq!(t.font_weight, FontWeight::Light);
}

#[test]
fn test_text_font() {
    let t = Text::new("Test").font("Arial");
    assert_eq!(t.font_family, Some("Arial".to_string()));
}

#[test]
fn test_text_font_with_string() {
    let t = Text::new("Test").font("Helvetica Neue".to_string());
    assert_eq!(t.font_family, Some("Helvetica Neue".to_string()));
}

#[test]
fn test_text_line_height() {
    let t = Text::new("Test").line_height(1.6);
    assert_eq!(t.line_height, 1.6);
}

#[test]
fn test_text_line_height_small() {
    let t = Text::new("Test").line_height(1.0);
    assert_eq!(t.line_height, 1.0);
}

#[test]
fn test_text_line_height_large() {
    let t = Text::new("Test").line_height(2.5);
    assert_eq!(t.line_height, 2.5);
}

#[test]
fn test_text_align() {
    let t = Text::new("Test").align(TextAlign::Center);
    assert_eq!(t.align, TextAlign::Center);
}

#[test]
fn test_text_align_all_variants() {
    let aligns = [TextAlign::Left, TextAlign::Center, TextAlign::Right];

    for align in aligns {
        let t = Text::new("Test").align(align);
        assert_eq!(t.align, align);
    }
}

#[test]
fn test_text_center() {
    let t = Text::new("Test").center();
    assert_eq!(t.align, TextAlign::Center);
}

#[test]
fn test_text_right() {
    let t = Text::new("Test").right();
    assert_eq!(t.align, TextAlign::Right);
}

// ==================== Chained Builder Tests ====================

#[test]
fn test_text_chained_builder_full() {
    let t = Text::new("Hello World")
        .id(ElementId::from(1u64))
        .color(Color::BLUE)
        .size(18.0)
        .weight(FontWeight::Bold)
        .font("Roboto")
        .line_height(1.5)
        .align(TextAlign::Center);

    assert_eq!(t.id, Some(ElementId::from(1u64)));
    assert_eq!(t.content, "Hello World");
    assert_eq!(t.color, Color::BLUE);
    assert_eq!(t.font_size, 18.0);
    assert_eq!(t.font_weight, FontWeight::Bold);
    assert_eq!(t.font_family, Some("Roboto".to_string()));
    assert_eq!(t.line_height, 1.5);
    assert_eq!(t.align, TextAlign::Center);
}

#[test]
fn test_text_chained_builder_partial() {
    let t = Text::new("Partial").size(20.0).bold();

    assert_eq!(t.content, "Partial");
    assert_eq!(t.font_size, 20.0);
    assert_eq!(t.font_weight, FontWeight::Bold);
    // Default values should remain
    assert_eq!(t.id, None);
    assert_eq!(t.color, Color::BLACK);
    assert_eq!(t.font_family, None);
    assert_eq!(t.line_height, 1.4);
    assert_eq!(t.align, TextAlign::Left);
}

#[test]
fn test_text_chained_convenience_methods() {
    let t = Text::new("Styled").bold().center();

    assert_eq!(t.font_weight, FontWeight::Bold);
    assert_eq!(t.align, TextAlign::Center);
}

#[test]
fn test_text_overwrite_values() {
    let t = Text::new("Test").size(10.0).size(20.0).size(30.0);

    assert_eq!(t.font_size, 30.0);
}

#[test]
fn test_text_overwrite_weight() {
    let t = Text::new("Test").bold().light().semibold();

    assert_eq!(t.font_weight, FontWeight::Semibold);
}

#[test]
fn test_text_overwrite_align() {
    let t = Text::new("Test").center().right().align(TextAlign::Left);

    assert_eq!(t.align, TextAlign::Left);
}

// ==================== estimate_width Tests ====================

#[test]
fn test_text_estimate_width_empty() {
    let t = Text::new("");
    assert_eq!(t.estimate_width(), 0.0);
}

#[test]
fn test_empty_text_layout_is_empty_and_paint_emits_no_primitive() {
    let mut t = Text::new("");
    let viewport = crate::core::geometry::Size::new(100.0, 40.0);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = t.layout(&mut layout_cx);

    let result = taffy.compute_layout(
        node,
        taffy::Size {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        },
    );
    assert!(result.is_ok());

    let layout = match taffy.layout(node) {
        Ok(layout) => layout,
        Err(err) => panic!("text layout missing after compute: {:?}", err),
    };
    assert_eq!(layout.size.width, 0.0);
    assert_eq!(layout.size.height, 0.0);

    let mut scene = crate::renderer::Scene::new();
    let bounds = crate::core::geometry::Bounds::from_xywh(0.0, 0.0, 0.0, 0.0);
    let mut paint_cx = PaintContext::new(&mut scene, bounds, &taffy);
    t.paint(&mut paint_cx);
    assert!(scene.primitives().is_empty());
}

#[test]
fn test_text_estimate_width_single_char() {
    let t = Text::new("A");
    assert!(t.estimate_width() > 0.0);
}

#[test]
fn test_text_estimate_width_with_custom_size() {
    let small = Text::new("Hello").size(10.0);
    let large = Text::new("Hello").size(20.0);
    assert!(large.estimate_width() > small.estimate_width());
}

#[test]
fn test_text_estimate_width_longer_text() {
    let short = Text::new("Hello");
    let long = Text::new("Hello World");
    assert!(long.estimate_width() > short.estimate_width());
}

// ==================== estimate_height Tests ====================

#[test]
fn test_text_estimate_height_default() {
    let t = Text::new("Test");
    // height = 14.0 * 1.4 = 19.6
    assert!((t.estimate_height() - 19.6).abs() < 0.01);
}

#[test]
fn test_text_estimate_height_custom_size() {
    let t = Text::new("Test").size(20.0);
    // height = 20.0 * 1.4 = 28.0
    assert_eq!(t.estimate_height(), 28.0);
}

#[test]
fn test_text_estimate_height_custom_line_height() {
    let t = Text::new("Test").line_height(2.0);
    // height = 14.0 * 2.0 = 28.0
    assert_eq!(t.estimate_height(), 28.0);
}

#[test]
fn test_text_estimate_height_custom_size_and_line_height() {
    let t = Text::new("Test").size(24.0).line_height(1.5);
    // height = 24.0 * 1.5 = 36.0
    assert_eq!(t.estimate_height(), 36.0);
}

// ==================== Element Trait Tests ====================

#[test]
fn test_text_element_id_none() {
    let t = Text::new("Test");
    assert_eq!(Element::id(&t), None);
}

#[test]
fn test_text_element_id_some() {
    let t = Text::new("Test").id(ElementId::from(123u64));
    assert_eq!(Element::id(&t), Some(ElementId::from(123u64)));
}

#[test]
fn test_text_element_style() {
    let t = Text::new("Test");
    let _ = Element::style(&t);
    // Just verify it doesn't panic
}

// ==================== text() Function Tests ====================

#[test]
fn test_text_function() {
    let t = text("Hello");
    assert_eq!(t.content, "Hello");
}

#[test]
fn test_text_function_with_string() {
    let t = text("World".to_string());
    assert_eq!(t.content, "World");
}

#[test]
fn test_text_function_chained() {
    let t = text("Chained").size(16.0).bold().center();

    assert_eq!(t.content, "Chained");
    assert_eq!(t.font_size, 16.0);
    assert_eq!(t.font_weight, FontWeight::Bold);
    assert_eq!(t.align, TextAlign::Center);
}

#[test]
fn test_text_function_empty() {
    let t = text("");
    assert_eq!(t.content, "");
}

// ==================== Unicode and Special Characters Tests ====================

#[test]
fn test_text_unicode_content() {
    let t = Text::new("Hello, World!");
    assert_eq!(t.content, "Hello, World!");
}

#[test]
fn test_text_chinese_characters() {
    let t = Text::new("Chinese characters");
    assert_eq!(t.content, "Chinese characters");
}

#[test]
fn test_text_emoji() {
    let t = Text::new("Test");
    assert_eq!(t.content, "Test");
}

#[test]
fn test_text_multiline_content() {
    let t = Text::new("Line1\nLine2\nLine3");
    assert_eq!(t.content, "Line1\nLine2\nLine3");
}

#[test]
fn test_text_whitespace_content() {
    let t = Text::new("   spaces   ");
    assert_eq!(t.content, "   spaces   ");
}

#[test]
fn test_text_tab_content() {
    let t = Text::new("tab\there");
    assert_eq!(t.content, "tab\there");
}

// ==================== Edge Cases Tests ====================

#[test]
fn test_text_size_zero() {
    let t = Text::new("Test").size(0.0);
    assert_eq!(t.font_size, 0.0);
    assert_eq!(t.estimate_width(), 0.0);
    assert_eq!(t.estimate_height(), 0.0);
}

#[test]
fn test_text_line_height_zero() {
    let t = Text::new("Test").line_height(0.0);
    assert_eq!(t.line_height, 0.0);
    assert_eq!(t.estimate_height(), 0.0);
}

#[test]
fn test_text_very_long_content() {
    let long_text = "a".repeat(1000);
    let t = Text::new(long_text.clone());
    assert_eq!(t.content, long_text);
    assert!(t.estimate_width() > Text::new("a").estimate_width());
}

#[test]
fn test_text_font_family_empty() {
    let t = Text::new("Test").font("");
    assert_eq!(t.font_family, Some("".to_string()));
}

#[test]
fn test_text_font_family_with_spaces() {
    let t = Text::new("Test").font("San Francisco Display");
    assert_eq!(t.font_family, Some("San Francisco Display".to_string()));
}

// ==================== Table-Driven Tests ====================

#[test]
fn test_font_weight_values_table() {
    struct TestCase {
        weight: FontWeight,
        expected_value: u16,
    }

    let test_cases = [
        TestCase {
            weight: FontWeight::Thin,
            expected_value: 100,
        },
        TestCase {
            weight: FontWeight::Light,
            expected_value: 300,
        },
        TestCase {
            weight: FontWeight::Regular,
            expected_value: 400,
        },
        TestCase {
            weight: FontWeight::Medium,
            expected_value: 500,
        },
        TestCase {
            weight: FontWeight::Semibold,
            expected_value: 600,
        },
        TestCase {
            weight: FontWeight::Bold,
            expected_value: 700,
        },
        TestCase {
            weight: FontWeight::Black,
            expected_value: 900,
        },
    ];

    for tc in test_cases {
        assert_eq!(
            tc.weight.to_value(),
            tc.expected_value,
            "FontWeight::{:?}.to_value() should be {}",
            tc.weight,
            tc.expected_value
        );
    }
}

#[test]
fn test_text_builder_methods_return_self() {
    // Verify all builder methods return Self for chaining
    let t = text("test")
        .id(ElementId::from(1u64))
        .color(Color::RED)
        .size(12.0)
        .weight(FontWeight::Bold)
        .bold()
        .semibold()
        .medium()
        .light()
        .font("Arial")
        .line_height(1.5)
        .align(TextAlign::Center)
        .center()
        .right();

    // If we get here, all methods successfully returned Self
    assert_eq!(t.align, TextAlign::Right);
}

#[test]
fn test_estimate_dimensions_table() {
    struct TestCase {
        content: &'static str,
        font_size: f32,
        line_height: f32,
        expected_width: Option<f32>,
        expected_height: f32,
    }

    let test_cases = [
        TestCase {
            content: "Test",
            font_size: 14.0,
            line_height: 1.4,
            expected_width: None,
            expected_height: 19.6, // 14.0 * 1.4
        },
        TestCase {
            content: "Hello",
            font_size: 20.0,
            line_height: 1.0,
            expected_width: None,
            expected_height: 20.0, // 20.0 * 1.0
        },
        TestCase {
            content: "",
            font_size: 16.0,
            line_height: 1.5,
            expected_width: Some(0.0),
            expected_height: 0.0,
        },
    ];

    for tc in test_cases {
        let t = Text::new(tc.content)
            .size(tc.font_size)
            .line_height(tc.line_height);

        if let Some(expected_width) = tc.expected_width {
            assert!((t.estimate_width() - expected_width).abs() < 0.01);
        } else {
            assert!(t.estimate_width() > 0.0);
        }

        assert!(
            (t.estimate_height() - tc.expected_height).abs() < 0.01,
            "Content '{}' with size {} and line_height {} should have height {}, got {}",
            tc.content,
            tc.font_size,
            tc.line_height,
            tc.expected_height,
            t.estimate_height()
        );
    }
}
