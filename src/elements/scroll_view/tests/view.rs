use super::*;
use crate::elements::text::text;

// ========== ScrollView builder tests ==========

#[test]
fn test_scroll_view_new() {
    let sv = ScrollView::new();
    assert!(sv.id.is_none());
    assert_eq!(sv.direction, ScrollDirection::Vertical);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
    assert_eq!(sv.scrollbar_width, 8.0);
    assert!(sv.children.is_empty());
    assert!(sv.on_scroll.is_none());
}

#[test]
fn test_scroll_view_default() {
    let sv = ScrollView::default();
    assert_eq!(sv.direction, ScrollDirection::Vertical);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
}

#[test]
fn test_scroll_view_id() {
    let id = ElementId::new();
    let sv = ScrollView::new().id(id);
    assert_eq!(sv.id, Some(id));
}

// ========== ScrollView direction tests ==========

struct DirectionTestCase {
    name: &'static str,
    direction: ScrollDirection,
    expected_overflow_x: Overflow,
    expected_overflow_y: Overflow,
}

#[test]
fn test_scroll_view_direction_table_driven() {
    let test_cases = vec![
        DirectionTestCase {
            name: "vertical",
            direction: ScrollDirection::Vertical,
            expected_overflow_x: Overflow::Hidden,
            expected_overflow_y: Overflow::Scroll,
        },
        DirectionTestCase {
            name: "horizontal",
            direction: ScrollDirection::Horizontal,
            expected_overflow_x: Overflow::Scroll,
            expected_overflow_y: Overflow::Hidden,
        },
        DirectionTestCase {
            name: "both",
            direction: ScrollDirection::Both,
            expected_overflow_x: Overflow::Scroll,
            expected_overflow_y: Overflow::Scroll,
        },
    ];

    for case in test_cases {
        let sv = ScrollView::new().direction(case.direction);
        assert_eq!(
            sv.direction, case.direction,
            "Test case '{}' failed for direction",
            case.name
        );
        assert_eq!(
            sv.style.overflow_x, case.expected_overflow_x,
            "Test case '{}' failed for overflow_x",
            case.name
        );
        assert_eq!(
            sv.style.overflow_y, case.expected_overflow_y,
            "Test case '{}' failed for overflow_y",
            case.name
        );
    }
}

#[test]
fn test_scroll_view_vertical() {
    let sv = ScrollView::new().vertical();
    assert_eq!(sv.direction, ScrollDirection::Vertical);
    assert_eq!(sv.style.overflow_x, Overflow::Hidden);
    assert_eq!(sv.style.overflow_y, Overflow::Scroll);
}

#[test]
fn test_scroll_view_horizontal() {
    let sv = ScrollView::new().horizontal();
    assert_eq!(sv.direction, ScrollDirection::Horizontal);
    assert_eq!(sv.style.overflow_x, Overflow::Scroll);
    assert_eq!(sv.style.overflow_y, Overflow::Hidden);
}

#[test]
fn test_scroll_view_both() {
    let sv = ScrollView::new().both();
    assert_eq!(sv.direction, ScrollDirection::Both);
    assert_eq!(sv.style.overflow_x, Overflow::Scroll);
    assert_eq!(sv.style.overflow_y, Overflow::Scroll);
}

// ========== ScrollView scrollbar visibility tests ==========

#[test]
fn test_scroll_view_scrollbar() {
    let sv = ScrollView::new().scrollbar(ScrollbarVisibility::Hover);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Hover);
}

#[test]
fn test_scroll_view_scrollbar_always() {
    let sv = ScrollView::new().scrollbar_always();
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Always);
}

#[test]
fn test_scroll_view_scrollbar_never() {
    let sv = ScrollView::new().scrollbar_never();
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Never);
}

#[test]
fn test_scroll_view_scrollbar_width() {
    let sv = ScrollView::new().scrollbar_width(12.0);
    assert_eq!(sv.scrollbar_width, 12.0);
}

// ========== ScrollView size tests ==========

#[test]
fn test_scroll_view_w() {
    let sv = ScrollView::new().w(300.0);
    assert_eq!(sv.style.width, Some(300.0));
}

#[test]
fn test_scroll_view_h() {
    let sv = ScrollView::new().h(400.0);
    assert_eq!(sv.style.height, Some(400.0));
}

#[test]
fn test_scroll_view_size() {
    let sv = ScrollView::new().size(Size::new(300.0, 400.0));
    assert_eq!(sv.style.width, Some(300.0));
    assert_eq!(sv.style.height, Some(400.0));
}

#[test]
fn test_scroll_view_size_from_tuple() {
    let sv = ScrollView::new().size((250.0, 350.0));
    assert_eq!(sv.style.width, Some(250.0));
    assert_eq!(sv.style.height, Some(350.0));
}

// ========== ScrollView background tests ==========

#[test]
fn test_scroll_view_bg() {
    let sv = ScrollView::new().bg(Color::RED);
    match sv.style.background {
        crate::core::style::Background::Solid(color) => {
            assert_eq!(color, Color::RED);
        }
        _ => panic!("Expected solid background"),
    }
}

#[test]
fn test_scroll_view_bg_hex() {
    let sv = ScrollView::new().bg(Color::hex(0xFF00FF));
    match sv.style.background {
        crate::core::style::Background::Solid(_) => {}
        _ => panic!("Expected solid background"),
    }
}

// ========== ScrollView children tests ==========

#[test]
fn test_scroll_view_child() {
    let sv = ScrollView::new().child(text("Test"));
    assert_eq!(sv.children.len(), 1);
}

#[test]
fn test_scroll_view_multiple_children() {
    let sv = ScrollView::new()
        .child(text("Child 1"))
        .child(text("Child 2"))
        .child(text("Child 3"));
    assert_eq!(sv.children.len(), 3);
}

#[test]
fn test_scroll_view_children_method() {
    let texts = vec![text("A"), text("B"), text("C"), text("D")];
    let sv = ScrollView::new().children(texts);
    assert_eq!(sv.children.len(), 4);
}

#[test]
fn test_scroll_view_children_trait() {
    let sv = ScrollView::new().child(text("First")).child(text("Second"));
    assert_eq!(Element::children(&sv).len(), 2);
}

// ========== ScrollView on_scroll handler tests ==========

#[test]
fn test_scroll_view_on_scroll() {
    let sv = ScrollView::new().on_scroll(|_x, _y| {});
    assert!(sv.on_scroll.is_some());
}

// ========== ScrollView should_show_scrollbar tests ==========

struct ShowScrollbarTestCase {
    name: &'static str,
    direction: ScrollDirection,
    visibility: ScrollbarVisibility,
    content_size: Size,
    viewport_size: Size,
    expected_show_x: bool,
    expected_show_y: bool,
}

#[test]
fn test_should_show_scrollbar_table_driven() {
    let test_cases = vec![
        ShowScrollbarTestCase {
            name: "vertical auto no overflow",
            direction: ScrollDirection::Vertical,
            visibility: ScrollbarVisibility::Auto,
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: false,
        },
        ShowScrollbarTestCase {
            name: "vertical auto with overflow",
            direction: ScrollDirection::Vertical,
            visibility: ScrollbarVisibility::Auto,
            content_size: Size::new(100.0, 500.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: true,
        },
        ShowScrollbarTestCase {
            name: "horizontal auto with overflow",
            direction: ScrollDirection::Horizontal,
            visibility: ScrollbarVisibility::Auto,
            content_size: Size::new(500.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: true,
            expected_show_y: false,
        },
        ShowScrollbarTestCase {
            name: "both auto with overflow both",
            direction: ScrollDirection::Both,
            visibility: ScrollbarVisibility::Auto,
            content_size: Size::new(500.0, 500.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: true,
            expected_show_y: true,
        },
        ShowScrollbarTestCase {
            name: "vertical always",
            direction: ScrollDirection::Vertical,
            visibility: ScrollbarVisibility::Always,
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: true,
        },
        ShowScrollbarTestCase {
            name: "horizontal always",
            direction: ScrollDirection::Horizontal,
            visibility: ScrollbarVisibility::Always,
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: true,
            expected_show_y: false,
        },
        ShowScrollbarTestCase {
            name: "both always",
            direction: ScrollDirection::Both,
            visibility: ScrollbarVisibility::Always,
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: true,
            expected_show_y: true,
        },
        ShowScrollbarTestCase {
            name: "vertical never",
            direction: ScrollDirection::Vertical,
            visibility: ScrollbarVisibility::Never,
            content_size: Size::new(100.0, 500.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: false,
        },
        ShowScrollbarTestCase {
            name: "both never",
            direction: ScrollDirection::Both,
            visibility: ScrollbarVisibility::Never,
            content_size: Size::new(500.0, 500.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: false,
        },
        ShowScrollbarTestCase {
            name: "vertical hover with overflow",
            direction: ScrollDirection::Vertical,
            visibility: ScrollbarVisibility::Hover,
            content_size: Size::new(100.0, 500.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_show_x: false,
            expected_show_y: true,
        },
    ];

    for case in test_cases {
        let mut sv = ScrollView::new()
            .direction(case.direction)
            .scrollbar(case.visibility);
        sv.state.content_size = case.content_size;
        sv.state.viewport_size = case.viewport_size;

        let (show_x, show_y) = sv.should_show_scrollbar();
        assert_eq!(
            show_x, case.expected_show_x,
            "Test case '{}' failed for show_x",
            case.name
        );
        assert_eq!(
            show_y, case.expected_show_y,
            "Test case '{}' failed for show_y",
            case.name
        );
    }
}

// ========== Element trait tests ==========

#[test]
fn test_scroll_view_element_id() {
    let id = ElementId::new();
    let sv = ScrollView::new().id(id);
    assert_eq!(Element::id(&sv), Some(id));
}

#[test]
fn test_scroll_view_element_id_none() {
    let sv = ScrollView::new();
    assert_eq!(Element::id(&sv), None);
}

#[test]
fn test_scroll_view_element_style() {
    let sv = ScrollView::new();
    let _style = Element::style(&sv);
    // Just verify we can access it
}

// ========== Helper function tests ==========

#[test]
fn test_scroll_view_helper() {
    let sv = scroll_view();
    assert_eq!(sv.direction, ScrollDirection::Vertical);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
}

// ========== Chained builder tests ==========

#[test]
fn test_full_builder_chain() {
    let id = ElementId::new();
    let sv = ScrollView::new()
        .id(id)
        .direction(ScrollDirection::Both)
        .scrollbar(ScrollbarVisibility::Always)
        .scrollbar_width(10.0)
        .w(400.0)
        .h(600.0)
        .bg(Color::WHITE)
        .child(text("Content 1"))
        .child(text("Content 2"));

    assert_eq!(sv.id, Some(id));
    assert_eq!(sv.direction, ScrollDirection::Both);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Always);
    assert_eq!(sv.scrollbar_width, 10.0);
    assert_eq!(sv.style.width, Some(400.0));
    assert_eq!(sv.style.height, Some(600.0));
    assert_eq!(sv.children.len(), 2);
}

#[test]
fn test_builder_chain_with_helper() {
    let sv = scroll_view()
        .vertical()
        .scrollbar_never()
        .size((300.0, 400.0));

    assert_eq!(sv.direction, ScrollDirection::Vertical);
    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Never);
    assert_eq!(sv.style.width, Some(300.0));
    assert_eq!(sv.style.height, Some(400.0));
}

#[test]
fn test_direction_override() {
    let sv = ScrollView::new().vertical().horizontal().both();

    assert_eq!(sv.direction, ScrollDirection::Both);
    assert_eq!(sv.style.overflow_x, Overflow::Scroll);
    assert_eq!(sv.style.overflow_y, Overflow::Scroll);
}

#[test]
fn test_scrollbar_override() {
    let sv = ScrollView::new()
        .scrollbar_always()
        .scrollbar_never()
        .scrollbar(ScrollbarVisibility::Hover);

    assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Hover);
}

// ========== Default values verification ==========

#[test]
fn test_default_scrollbar_width() {
    let sv = ScrollView::new();
    assert_eq!(sv.scrollbar_width, 8.0);
}

#[test]
fn test_default_scroll_state() {
    let sv = ScrollView::new();
    assert_eq!(sv.state.offset_x, 0.0);
    assert_eq!(sv.state.offset_y, 0.0);
    assert!(!sv.state.is_scrolling);
    assert!(!sv.state.scrollbar_hovered);
    assert!(!sv.state.scrollbar_dragging);
}

#[test]
fn test_default_overflow_settings() {
    let sv = ScrollView::new();
    // Default is Vertical direction
    assert_eq!(sv.style.overflow_x, Overflow::Hidden);
    assert_eq!(sv.style.overflow_y, Overflow::Scroll);
}

// ========== Edge cases ==========

#[test]
fn test_scroll_state_zero_content() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(0.0, 0.0);
    state.viewport_size = Size::new(100.0, 100.0);

    assert_eq!(state.max_scroll_x(), 0.0);
    assert_eq!(state.max_scroll_y(), 0.0);
    assert!(!state.can_scroll_up());
    assert!(!state.can_scroll_down());
    assert!(!state.can_scroll_left());
    assert!(!state.can_scroll_right());
}

#[test]
fn test_scroll_state_zero_viewport() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(100.0, 100.0);
    state.viewport_size = Size::new(0.0, 0.0);

    assert_eq!(state.max_scroll_x(), 100.0);
    assert_eq!(state.max_scroll_y(), 100.0);
}

#[test]
fn test_scroll_state_equal_sizes() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(200.0, 300.0);
    state.viewport_size = Size::new(200.0, 300.0);

    assert_eq!(state.max_scroll_x(), 0.0);
    assert_eq!(state.max_scroll_y(), 0.0);
    assert!(!state.can_scroll_up());
    assert!(!state.can_scroll_down());
    assert!(!state.can_scroll_left());
    assert!(!state.can_scroll_right());
}

#[test]
fn test_empty_scroll_view() {
    let sv = ScrollView::new();
    assert!(sv.children.is_empty());
    assert_eq!(Element::children(&sv).len(), 0);
}

#[test]
fn test_scrollbar_width_zero() {
    let sv = ScrollView::new().scrollbar_width(0.0);
    assert_eq!(sv.scrollbar_width, 0.0);
}

#[test]
fn test_scrollbar_width_large() {
    let sv = ScrollView::new().scrollbar_width(50.0);
    assert_eq!(sv.scrollbar_width, 50.0);
}

// ========== Negative value edge cases ==========

#[test]
fn test_scroll_to_with_negative_max() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(50.0, 50.0);
    state.viewport_size = Size::new(100.0, 100.0);

    // max_scroll should be 0, not negative
    assert_eq!(state.max_scroll_x(), 0.0);
    assert_eq!(state.max_scroll_y(), 0.0);

    // scroll_to should clamp to 0
    state.scroll_to(100.0, 100.0);
    assert_eq!(state.offset_x, 0.0);
    assert_eq!(state.offset_y, 0.0);
}

// ========== Complex workflow tests ==========

#[test]
fn test_scroll_state_workflow() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 1000.0);
    state.viewport_size = Size::new(200.0, 300.0);

    // Start at top
    assert!(!state.can_scroll_up());
    assert!(state.can_scroll_down());

    // Scroll down
    state.scroll_by(0.0, 100.0);
    assert!(state.can_scroll_up());
    assert!(state.can_scroll_down());

    // Scroll to bottom
    state.scroll_to_bottom();
    assert!(state.can_scroll_up());
    assert!(!state.can_scroll_down());

    // Scroll to top
    state.scroll_to_top();
    assert!(!state.can_scroll_up());
    assert!(state.can_scroll_down());
    assert_eq!(state.offset_y, 0.0);
}

#[test]
fn test_scrollbar_position_workflow() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(200.0, 1000.0);
    state.viewport_size = Size::new(200.0, 200.0);

    // At top: thumb position should be 0
    let (pos, _) = state.scrollbar_y();
    assert_eq!(pos, 0.0);

    // Scroll to middle
    state.scroll_to(0.0, 400.0);
    let (pos, _) = state.scrollbar_y();
    assert!(pos > 0.0 && pos < 1.0);

    // Scroll to bottom
    state.scroll_to_bottom();
    let (pos, size) = state.scrollbar_y();
    // At bottom, pos + size should equal 1.0
    assert!((pos + size - 1.0).abs() < 0.001);
}
