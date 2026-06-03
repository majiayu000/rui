use super::*;

// ========== ScrollDirection enum tests ==========

#[test]
fn test_scroll_direction_default() {
    let direction = ScrollDirection::default();
    assert_eq!(direction, ScrollDirection::Vertical);
}

#[test]
fn test_scroll_direction_equality() {
    assert_eq!(ScrollDirection::Vertical, ScrollDirection::Vertical);
    assert_eq!(ScrollDirection::Horizontal, ScrollDirection::Horizontal);
    assert_eq!(ScrollDirection::Both, ScrollDirection::Both);
    assert_ne!(ScrollDirection::Vertical, ScrollDirection::Horizontal);
    assert_ne!(ScrollDirection::Vertical, ScrollDirection::Both);
    assert_ne!(ScrollDirection::Horizontal, ScrollDirection::Both);
}

#[test]
fn test_scroll_direction_clone() {
    let direction = ScrollDirection::Horizontal;
    let cloned = direction.clone();
    assert_eq!(direction, cloned);
}

#[test]
fn test_scroll_direction_copy() {
    let direction = ScrollDirection::Both;
    let copied: ScrollDirection = direction;
    assert_eq!(direction, copied);
}

#[test]
fn test_scroll_direction_debug() {
    let debug_v = format!("{:?}", ScrollDirection::Vertical);
    let debug_h = format!("{:?}", ScrollDirection::Horizontal);
    let debug_b = format!("{:?}", ScrollDirection::Both);
    assert!(debug_v.contains("Vertical"));
    assert!(debug_h.contains("Horizontal"));
    assert!(debug_b.contains("Both"));
}

// ========== ScrollbarVisibility enum tests ==========

#[test]
fn test_scrollbar_visibility_default() {
    let visibility = ScrollbarVisibility::default();
    assert_eq!(visibility, ScrollbarVisibility::Auto);
}

#[test]
fn test_scrollbar_visibility_equality() {
    assert_eq!(ScrollbarVisibility::Auto, ScrollbarVisibility::Auto);
    assert_eq!(ScrollbarVisibility::Always, ScrollbarVisibility::Always);
    assert_eq!(ScrollbarVisibility::Never, ScrollbarVisibility::Never);
    assert_eq!(ScrollbarVisibility::Hover, ScrollbarVisibility::Hover);
    assert_ne!(ScrollbarVisibility::Auto, ScrollbarVisibility::Always);
    assert_ne!(ScrollbarVisibility::Never, ScrollbarVisibility::Hover);
}

#[test]
fn test_scrollbar_visibility_clone() {
    let visibility = ScrollbarVisibility::Always;
    let cloned = visibility.clone();
    assert_eq!(visibility, cloned);
}

#[test]
fn test_scrollbar_visibility_copy() {
    let visibility = ScrollbarVisibility::Never;
    let copied: ScrollbarVisibility = visibility;
    assert_eq!(visibility, copied);
}

#[test]
fn test_scrollbar_visibility_debug() {
    let debug_auto = format!("{:?}", ScrollbarVisibility::Auto);
    let debug_always = format!("{:?}", ScrollbarVisibility::Always);
    let debug_never = format!("{:?}", ScrollbarVisibility::Never);
    let debug_hover = format!("{:?}", ScrollbarVisibility::Hover);
    assert!(debug_auto.contains("Auto"));
    assert!(debug_always.contains("Always"));
    assert!(debug_never.contains("Never"));
    assert!(debug_hover.contains("Hover"));
}

// ========== ScrollState tests ==========

#[test]
fn test_scroll_state_default() {
    let state = ScrollState::default();
    assert_eq!(state.offset_x, 0.0);
    assert_eq!(state.offset_y, 0.0);
    assert_eq!(state.content_size.width, 0.0);
    assert_eq!(state.content_size.height, 0.0);
    assert_eq!(state.viewport_size.width, 0.0);
    assert_eq!(state.viewport_size.height, 0.0);
    assert!(!state.is_scrolling);
    assert!(!state.scrollbar_hovered);
    assert!(!state.scrollbar_dragging);
}

#[test]
fn test_scroll_state_clone() {
    let mut state = ScrollState::default();
    state.offset_x = 10.0;
    state.offset_y = 20.0;
    state.is_scrolling = true;
    let cloned = state.clone();
    assert_eq!(cloned.offset_x, 10.0);
    assert_eq!(cloned.offset_y, 20.0);
    assert!(cloned.is_scrolling);
}

#[test]
fn test_scroll_state_debug() {
    let state = ScrollState::default();
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("ScrollState"));
}

// ========== ScrollState::max_scroll tests ==========

struct MaxScrollTestCase {
    name: &'static str,
    content_size: Size,
    viewport_size: Size,
    expected_max_x: f32,
    expected_max_y: f32,
}

#[test]
fn test_max_scroll_table_driven() {
    let test_cases = vec![
        MaxScrollTestCase {
            name: "content smaller than viewport",
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_max_x: 0.0,
            expected_max_y: 0.0,
        },
        MaxScrollTestCase {
            name: "content larger than viewport",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            expected_max_x: 300.0,
            expected_max_y: 500.0,
        },
        MaxScrollTestCase {
            name: "content equals viewport",
            content_size: Size::new(200.0, 200.0),
            viewport_size: Size::new(200.0, 200.0),
            expected_max_x: 0.0,
            expected_max_y: 0.0,
        },
        MaxScrollTestCase {
            name: "zero viewport",
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(0.0, 0.0),
            expected_max_x: 100.0,
            expected_max_y: 100.0,
        },
        MaxScrollTestCase {
            name: "zero content",
            content_size: Size::new(0.0, 0.0),
            viewport_size: Size::new(100.0, 100.0),
            expected_max_x: 0.0,
            expected_max_y: 0.0,
        },
    ];

    for case in test_cases {
        let mut state = ScrollState::default();
        state.content_size = case.content_size;
        state.viewport_size = case.viewport_size;

        assert_eq!(
            state.max_scroll_x(),
            case.expected_max_x,
            "Test case '{}' failed for max_scroll_x",
            case.name
        );
        assert_eq!(
            state.max_scroll_y(),
            case.expected_max_y,
            "Test case '{}' failed for max_scroll_y",
            case.name
        );
    }
}

// ========== ScrollState::scroll_to tests ==========

struct ScrollToTestCase {
    name: &'static str,
    content_size: Size,
    viewport_size: Size,
    target_x: f32,
    target_y: f32,
    expected_x: f32,
    expected_y: f32,
}

#[test]
fn test_scroll_to_table_driven() {
    let test_cases = vec![
        ScrollToTestCase {
            name: "scroll within bounds",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            target_x: 100.0,
            target_y: 200.0,
            expected_x: 100.0,
            expected_y: 200.0,
        },
        ScrollToTestCase {
            name: "scroll beyond max clamped",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            target_x: 1000.0,
            target_y: 1000.0,
            expected_x: 300.0,
            expected_y: 500.0,
        },
        ScrollToTestCase {
            name: "negative scroll clamped to zero",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            target_x: -100.0,
            target_y: -200.0,
            expected_x: 0.0,
            expected_y: 0.0,
        },
        ScrollToTestCase {
            name: "scroll to exact max",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            target_x: 300.0,
            target_y: 500.0,
            expected_x: 300.0,
            expected_y: 500.0,
        },
        ScrollToTestCase {
            name: "scroll to zero",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            target_x: 0.0,
            target_y: 0.0,
            expected_x: 0.0,
            expected_y: 0.0,
        },
    ];

    for case in test_cases {
        let mut state = ScrollState::default();
        state.content_size = case.content_size;
        state.viewport_size = case.viewport_size;
        state.scroll_to(case.target_x, case.target_y);

        assert_eq!(
            state.offset_x, case.expected_x,
            "Test case '{}' failed for offset_x",
            case.name
        );
        assert_eq!(
            state.offset_y, case.expected_y,
            "Test case '{}' failed for offset_y",
            case.name
        );
    }
}

// ========== ScrollState::scroll_by tests ==========

#[test]
fn test_scroll_by() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 800.0);
    state.viewport_size = Size::new(200.0, 300.0);
    state.offset_x = 50.0;
    state.offset_y = 100.0;

    state.scroll_by(25.0, 50.0);
    assert_eq!(state.offset_x, 75.0);
    assert_eq!(state.offset_y, 150.0);
}

#[test]
fn test_scroll_by_negative() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 800.0);
    state.viewport_size = Size::new(200.0, 300.0);
    state.offset_x = 50.0;
    state.offset_y = 100.0;

    state.scroll_by(-25.0, -50.0);
    assert_eq!(state.offset_x, 25.0);
    assert_eq!(state.offset_y, 50.0);
}

#[test]
fn test_scroll_by_clamped() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 800.0);
    state.viewport_size = Size::new(200.0, 300.0);
    state.offset_x = 50.0;
    state.offset_y = 100.0;

    state.scroll_by(-100.0, -200.0); // Would go negative
    assert_eq!(state.offset_x, 0.0);
    assert_eq!(state.offset_y, 0.0);
}

// ========== ScrollState::scroll_to_top/bottom tests ==========

#[test]
fn test_scroll_to_top() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 800.0);
    state.viewport_size = Size::new(200.0, 300.0);
    state.offset_y = 250.0;

    state.scroll_to_top();
    assert_eq!(state.offset_y, 0.0);
}

#[test]
fn test_scroll_to_bottom() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(500.0, 800.0);
    state.viewport_size = Size::new(200.0, 300.0);
    state.offset_y = 0.0;

    state.scroll_to_bottom();
    assert_eq!(state.offset_y, 500.0);
}

#[test]
fn test_scroll_to_bottom_content_smaller_than_viewport() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(100.0, 100.0);
    state.viewport_size = Size::new(200.0, 300.0);

    state.scroll_to_bottom();
    assert_eq!(state.offset_y, 0.0); // max_scroll_y is 0
}

// ========== ScrollState::can_scroll tests ==========

struct CanScrollTestCase {
    name: &'static str,
    content_size: Size,
    viewport_size: Size,
    offset_x: f32,
    offset_y: f32,
    can_up: bool,
    can_down: bool,
    can_left: bool,
    can_right: bool,
}

#[test]
fn test_can_scroll_table_driven() {
    let test_cases = vec![
        CanScrollTestCase {
            name: "at top-left corner",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            offset_x: 0.0,
            offset_y: 0.0,
            can_up: false,
            can_down: true,
            can_left: false,
            can_right: true,
        },
        CanScrollTestCase {
            name: "at bottom-right corner",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            offset_x: 300.0,
            offset_y: 500.0,
            can_up: true,
            can_down: false,
            can_left: true,
            can_right: false,
        },
        CanScrollTestCase {
            name: "in middle",
            content_size: Size::new(500.0, 800.0),
            viewport_size: Size::new(200.0, 300.0),
            offset_x: 150.0,
            offset_y: 250.0,
            can_up: true,
            can_down: true,
            can_left: true,
            can_right: true,
        },
        CanScrollTestCase {
            name: "content fits viewport",
            content_size: Size::new(100.0, 100.0),
            viewport_size: Size::new(200.0, 300.0),
            offset_x: 0.0,
            offset_y: 0.0,
            can_up: false,
            can_down: false,
            can_left: false,
            can_right: false,
        },
    ];

    for case in test_cases {
        let mut state = ScrollState::default();
        state.content_size = case.content_size;
        state.viewport_size = case.viewport_size;
        state.offset_x = case.offset_x;
        state.offset_y = case.offset_y;

        assert_eq!(
            state.can_scroll_up(),
            case.can_up,
            "Test case '{}' failed for can_scroll_up",
            case.name
        );
        assert_eq!(
            state.can_scroll_down(),
            case.can_down,
            "Test case '{}' failed for can_scroll_down",
            case.name
        );
        assert_eq!(
            state.can_scroll_left(),
            case.can_left,
            "Test case '{}' failed for can_scroll_left",
            case.name
        );
        assert_eq!(
            state.can_scroll_right(),
            case.can_right,
            "Test case '{}' failed for can_scroll_right",
            case.name
        );
    }
}

// ========== ScrollState::scrollbar_y tests ==========

struct ScrollbarYTestCase {
    name: &'static str,
    content_height: f32,
    viewport_height: f32,
    offset_y: f32,
    expected_pos: f32,
    expected_size: f32,
}

#[test]
fn test_scrollbar_y_table_driven() {
    let test_cases = vec![
        ScrollbarYTestCase {
            name: "content fits viewport",
            content_height: 100.0,
            viewport_height: 200.0,
            offset_y: 0.0,
            expected_pos: 0.0,
            expected_size: 1.0,
        },
        ScrollbarYTestCase {
            name: "at top",
            content_height: 1000.0,
            viewport_height: 200.0,
            offset_y: 0.0,
            expected_pos: 0.0,
            expected_size: 0.2,
        },
        ScrollbarYTestCase {
            name: "at bottom",
            content_height: 1000.0,
            viewport_height: 200.0,
            offset_y: 800.0,
            expected_pos: 0.8,
            expected_size: 0.2,
        },
        ScrollbarYTestCase {
            name: "in middle",
            content_height: 1000.0,
            viewport_height: 200.0,
            offset_y: 400.0,
            expected_pos: 0.4,
            expected_size: 0.2,
        },
    ];

    for case in test_cases {
        let mut state = ScrollState::default();
        state.content_size = Size::new(100.0, case.content_height);
        state.viewport_size = Size::new(100.0, case.viewport_height);
        state.offset_y = case.offset_y;

        let (pos, size) = state.scrollbar_y();
        assert!(
            (pos - case.expected_pos).abs() < 0.001,
            "Test case '{}' failed for scrollbar_y pos: expected {}, got {}",
            case.name,
            case.expected_pos,
            pos
        );
        assert!(
            (size - case.expected_size).abs() < 0.001,
            "Test case '{}' failed for scrollbar_y size: expected {}, got {}",
            case.name,
            case.expected_size,
            size
        );
    }
}

// ========== ScrollState::scrollbar_x tests ==========

#[test]
fn test_scrollbar_x_content_fits() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(100.0, 100.0);
    state.viewport_size = Size::new(200.0, 100.0);

    let (pos, size) = state.scrollbar_x();
    assert_eq!(pos, 0.0);
    assert_eq!(size, 1.0);
}

#[test]
fn test_scrollbar_x_at_start() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(1000.0, 100.0);
    state.viewport_size = Size::new(200.0, 100.0);
    state.offset_x = 0.0;

    let (pos, size) = state.scrollbar_x();
    assert_eq!(pos, 0.0);
    assert!((size - 0.2).abs() < 0.001);
}

#[test]
fn test_scrollbar_x_at_end() {
    let mut state = ScrollState::default();
    state.content_size = Size::new(1000.0, 100.0);
    state.viewport_size = Size::new(200.0, 100.0);
    state.offset_x = 800.0;

    let (pos, size) = state.scrollbar_x();
    assert!((pos - 0.8).abs() < 0.001);
    assert!((size - 0.2).abs() < 0.001);
}

#[test]
fn test_scrollbar_thumb_size_minimum() {
    // When content is much larger than viewport, thumb size should be clamped to 0.1
    let mut state = ScrollState::default();
    state.content_size = Size::new(100.0, 100000.0); // Very large content
    state.viewport_size = Size::new(100.0, 100.0);

    let (_, size) = state.scrollbar_y();
    assert!((size - 0.1).abs() < 0.001); // Clamped to minimum 0.1
}
