use super::support::*;

// ==================== Div::new() and Default ====================

#[test]
fn test_div_new() {
    let d = Div::new();
    assert!(d.id.is_none());
    assert!(d.children.is_empty());
    assert!(d.on_click.is_none());
    assert!(d.on_hover.is_none());
    assert!(d.layout_node.is_none());
}

#[test]
fn test_div_default() {
    let d = Div::default();
    assert!(d.id.is_none());
    assert!(d.children.is_empty());
}

#[test]
fn test_div_function() {
    let d = div();
    assert!(d.id.is_none());
    assert!(d.children.is_empty());
}

// ==================== Identity ====================

#[test]
fn test_div_id() {
    let d = Div::new().id(ElementId::from(123u64));
    assert_eq!(d.id, Some(ElementId::from(123u64)));
}

#[test]
fn test_div_element_trait_id() {
    let d = Div::new().id(ElementId::from(456u64));
    assert_eq!(Element::id(&d), Some(ElementId::from(456u64)));
}

#[test]
fn test_div_element_trait_id_none() {
    let d = Div::new();
    assert_eq!(Element::id(&d), None);
}

// ==================== Size Methods ====================

struct SizeTestCase {
    name: &'static str,
    width: Option<f32>,
    height: Option<f32>,
}

#[test]
fn test_div_size_methods() {
    let test_cases = [
        SizeTestCase {
            name: "w",
            width: Some(100.0),
            height: None,
        },
        SizeTestCase {
            name: "h",
            width: None,
            height: Some(50.0),
        },
    ];

    for tc in test_cases {
        let mut d = Div::new();
        if let Some(w) = tc.width {
            d = d.w(w);
        }
        if let Some(h) = tc.height {
            d = d.h(h);
        }
        assert_eq!(d.style.width, tc.width, "failed for case: {}", tc.name);
        assert_eq!(d.style.height, tc.height, "failed for case: {}", tc.name);
    }
}

#[test]
fn test_div_size() {
    let d = Div::new().size((200.0, 100.0));
    assert_eq!(d.style.width, Some(200.0));
    assert_eq!(d.style.height, Some(100.0));
}

#[test]
fn test_div_size_from_size_struct() {
    let d = Div::new().size(Size::new(150.0, 75.0));
    assert_eq!(d.style.width, Some(150.0));
    assert_eq!(d.style.height, Some(75.0));
}

#[test]
fn test_div_w() {
    let d = Div::new().w(250.0);
    assert_eq!(d.style.width, Some(250.0));
    assert_eq!(d.style.height, None);
}

#[test]
fn test_div_h() {
    let d = Div::new().h(125.0);
    assert_eq!(d.style.width, None);
    assert_eq!(d.style.height, Some(125.0));
}

#[test]
fn test_div_w_full() {
    let d = Div::new().w_full();
    assert_eq!(d.style.width, None);
    assert_eq!(d.style.dimensions.width, Some(Dimension::Fill));
    assert_eq!(d.style.flex_grow, 1.0);
}

#[test]
fn test_div_h_full() {
    let d = Div::new().h_full();
    assert_eq!(d.style.height, None);
    assert_eq!(d.style.dimensions.height, Some(Dimension::Fill));
    assert_eq!(d.style.flex_grow, 1.0);
}

#[test]
fn test_div_percent_and_auto_dimensions() {
    let d = Div::new().w_percent(50.0).h_auto();

    assert_eq!(d.style.width, None);
    assert_eq!(d.style.height, None);
    assert_eq!(d.style.dimensions.width, Some(Dimension::Percent(50.0)));
    assert_eq!(d.style.dimensions.height, Some(Dimension::Auto));
}

#[test]
fn test_div_min_w() {
    let d = Div::new().min_w(50.0);
    assert_eq!(d.style.min_width, Some(50.0));
}

#[test]
fn test_div_min_h() {
    let d = Div::new().min_h(30.0);
    assert_eq!(d.style.min_height, Some(30.0));
}

#[test]
fn test_div_max_w() {
    let d = Div::new().max_w(800.0);
    assert_eq!(d.style.max_width, Some(800.0));
}

#[test]
fn test_div_max_h() {
    let d = Div::new().max_h(600.0);
    assert_eq!(d.style.max_height, Some(600.0));
}

// ==================== Flex Properties ====================

#[test]
fn test_div_flex() {
    let d = Div::new().flex();
    assert_eq!(d.style.display, Display::Flex);
}

#[test]
fn test_div_flex_row() {
    let d = Div::new().flex_row();
    assert_eq!(d.style.display, Display::Flex);
    assert_eq!(d.style.flex_direction, FlexDirection::Row);
}

#[test]
fn test_div_flex_col() {
    let d = Div::new().flex_col();
    assert_eq!(d.style.display, Display::Flex);
    assert_eq!(d.style.flex_direction, FlexDirection::Column);
}

#[test]
fn test_div_flex_grow() {
    let d = Div::new().flex_grow(2.5);
    assert_eq!(d.style.flex_grow, 2.5);
}

#[test]
fn test_div_flex_shrink() {
    let d = Div::new().flex_shrink(0.5);
    assert_eq!(d.style.flex_shrink, 0.5);
}

#[test]
fn test_div_gap() {
    let d = Div::new().gap(16.0);
    assert_eq!(d.style.gap, 16.0);
}

// ==================== Justify Content ====================

struct JustifyContentTestCase {
    name: &'static str,
    expected: JustifyContent,
}

#[test]
fn test_div_justify_content_methods() {
    let test_cases = [
        JustifyContentTestCase {
            name: "justify_start",
            expected: JustifyContent::FlexStart,
        },
        JustifyContentTestCase {
            name: "justify_end",
            expected: JustifyContent::FlexEnd,
        },
        JustifyContentTestCase {
            name: "justify_center",
            expected: JustifyContent::Center,
        },
        JustifyContentTestCase {
            name: "justify_between",
            expected: JustifyContent::SpaceBetween,
        },
        JustifyContentTestCase {
            name: "justify_around",
            expected: JustifyContent::SpaceAround,
        },
    ];

    for tc in test_cases {
        let d = match tc.name {
            "justify_start" => Div::new().justify_start(),
            "justify_end" => Div::new().justify_end(),
            "justify_center" => Div::new().justify_center(),
            "justify_between" => Div::new().justify_between(),
            "justify_around" => Div::new().justify_around(),
            _ => unreachable!(),
        };
        assert_eq!(
            d.style.justify_content, tc.expected,
            "failed for case: {}",
            tc.name
        );
    }
}

#[test]
fn test_div_justify_start() {
    let d = Div::new().justify_start();
    assert_eq!(d.style.justify_content, JustifyContent::FlexStart);
}

#[test]
fn test_div_justify_end() {
    let d = Div::new().justify_end();
    assert_eq!(d.style.justify_content, JustifyContent::FlexEnd);
}

#[test]
fn test_div_justify_center() {
    let d = Div::new().justify_center();
    assert_eq!(d.style.justify_content, JustifyContent::Center);
}

#[test]
fn test_div_justify_between() {
    let d = Div::new().justify_between();
    assert_eq!(d.style.justify_content, JustifyContent::SpaceBetween);
}

#[test]
fn test_div_justify_around() {
    let d = Div::new().justify_around();
    assert_eq!(d.style.justify_content, JustifyContent::SpaceAround);
}

// ==================== Align Items ====================

struct AlignItemsTestCase {
    name: &'static str,
    expected: AlignItems,
}

#[test]
fn test_div_align_items_methods() {
    let test_cases = [
        AlignItemsTestCase {
            name: "items_start",
            expected: AlignItems::FlexStart,
        },
        AlignItemsTestCase {
            name: "items_end",
            expected: AlignItems::FlexEnd,
        },
        AlignItemsTestCase {
            name: "items_center",
            expected: AlignItems::Center,
        },
        AlignItemsTestCase {
            name: "items_stretch",
            expected: AlignItems::Stretch,
        },
    ];

    for tc in test_cases {
        let d = match tc.name {
            "items_start" => Div::new().items_start(),
            "items_end" => Div::new().items_end(),
            "items_center" => Div::new().items_center(),
            "items_stretch" => Div::new().items_stretch(),
            _ => unreachable!(),
        };
        assert_eq!(
            d.style.align_items, tc.expected,
            "failed for case: {}",
            tc.name
        );
    }
}

#[test]
fn test_div_items_start() {
    let d = Div::new().items_start();
    assert_eq!(d.style.align_items, AlignItems::FlexStart);
}

#[test]
fn test_div_items_end() {
    let d = Div::new().items_end();
    assert_eq!(d.style.align_items, AlignItems::FlexEnd);
}

#[test]
fn test_div_items_center() {
    let d = Div::new().items_center();
    assert_eq!(d.style.align_items, AlignItems::Center);
}

#[test]
fn test_div_items_stretch() {
    let d = Div::new().items_stretch();
    assert_eq!(d.style.align_items, AlignItems::Stretch);
}
