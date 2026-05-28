use super::support::*;

// ==================== Edge Cases ====================

#[test]
fn test_div_zero_dimensions() {
    let d = Div::new().w(0.0).h(0.0);
    assert_eq!(d.style.width, Some(0.0));
    assert_eq!(d.style.height, Some(0.0));
}

#[test]
fn test_div_negative_dimensions() {
    // Note: The API allows negative values; validation should happen at layout time
    let d = Div::new().w(-10.0).h(-20.0);
    assert_eq!(d.style.width, Some(-10.0));
    assert_eq!(d.style.height, Some(-20.0));
}

#[test]
fn test_div_large_dimensions() {
    let d = Div::new().w(10000.0).h(10000.0);
    assert_eq!(d.style.width, Some(10000.0));
    assert_eq!(d.style.height, Some(10000.0));
}

#[test]
fn test_div_overwrite_style() {
    let d = Div::new()
        .w(100.0)
        .w(200.0) // Overwrite
        .h(50.0)
        .h(100.0); // Overwrite

    assert_eq!(d.style.width, Some(200.0));
    assert_eq!(d.style.height, Some(100.0));
}

#[test]
fn test_div_flex_grow_after_w_full() {
    let d = Div::new().w_full().flex_grow(2.0);
    // flex_grow should be overwritten
    assert_eq!(d.style.flex_grow, 2.0);
}

#[test]
fn test_div_flex_direction_after_flex_row() {
    let d = Div::new().flex_row().flex_col();
    // Should end up as column
    assert_eq!(d.style.flex_direction, FlexDirection::Column);
}

// ==================== Padding and Margin Table-Driven Tests ====================

struct PaddingTestCase {
    name: &'static str,
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

#[test]
fn test_div_padding_individual() {
    let test_cases = [
        PaddingTestCase {
            name: "pt only",
            top: 10.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        },
        PaddingTestCase {
            name: "pr only",
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 0.0,
        },
        PaddingTestCase {
            name: "pb only",
            top: 0.0,
            right: 0.0,
            bottom: 10.0,
            left: 0.0,
        },
        PaddingTestCase {
            name: "pl only",
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 10.0,
        },
    ];

    for tc in test_cases {
        let mut d = Div::new();
        if tc.top > 0.0 {
            d = d.pt(tc.top);
        }
        if tc.right > 0.0 {
            d = d.pr(tc.right);
        }
        if tc.bottom > 0.0 {
            d = d.pb(tc.bottom);
        }
        if tc.left > 0.0 {
            d = d.pl(tc.left);
        }

        assert_eq!(d.style.padding.top, tc.top, "failed for case: {}", tc.name);
        assert_eq!(
            d.style.padding.right, tc.right,
            "failed for case: {}",
            tc.name
        );
        assert_eq!(
            d.style.padding.bottom, tc.bottom,
            "failed for case: {}",
            tc.name
        );
        assert_eq!(
            d.style.padding.left, tc.left,
            "failed for case: {}",
            tc.name
        );
    }
}

// ==================== Shadow Table-Driven Tests ====================

struct ShadowTestCase {
    name: &'static str,
    offset_y: f32,
    blur_radius: f32,
}

#[test]
fn test_div_shadow_presets() {
    let test_cases = [
        ShadowTestCase {
            name: "sm",
            offset_y: 1.0,
            blur_radius: 2.0,
        },
        ShadowTestCase {
            name: "md",
            offset_y: 4.0,
            blur_radius: 6.0,
        },
        ShadowTestCase {
            name: "lg",
            offset_y: 10.0,
            blur_radius: 15.0,
        },
    ];

    for tc in test_cases {
        let d = match tc.name {
            "sm" => Div::new().shadow_sm(),
            "md" => Div::new().shadow_md(),
            "lg" => Div::new().shadow_lg(),
            _ => unreachable!(),
        };

        let shadow = d
            .style
            .shadow
            .expect(&format!("shadow should be set for {}", tc.name));
        assert_eq!(shadow.offset_x, 0.0, "offset_x for {}", tc.name);
        assert_eq!(shadow.offset_y, tc.offset_y, "offset_y for {}", tc.name);
        assert_eq!(
            shadow.blur_radius, tc.blur_radius,
            "blur_radius for {}",
            tc.name
        );
    }
}

// ==================== Border Radius Table-Driven Tests ====================

struct BorderRadiusTestCase {
    name: &'static str,
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

#[test]
fn test_div_border_radius_variants() {
    let test_cases = [
        BorderRadiusTestCase {
            name: "rounded_all",
            top_left: 8.0,
            top_right: 8.0,
            bottom_right: 8.0,
            bottom_left: 8.0,
        },
        BorderRadiusTestCase {
            name: "rounded_t",
            top_left: 12.0,
            top_right: 12.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        },
        BorderRadiusTestCase {
            name: "rounded_b",
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 10.0,
            bottom_left: 10.0,
        },
        BorderRadiusTestCase {
            name: "rounded_full",
            top_left: 9999.0,
            top_right: 9999.0,
            bottom_right: 9999.0,
            bottom_left: 9999.0,
        },
    ];

    for tc in test_cases {
        let d = match tc.name {
            "rounded_all" => Div::new().rounded(8.0),
            "rounded_t" => Div::new().rounded_t(12.0),
            "rounded_b" => Div::new().rounded_b(10.0),
            "rounded_full" => Div::new().rounded_full(),
            _ => unreachable!(),
        };

        assert_eq!(
            d.style.border.radius.top_left, tc.top_left,
            "top_left for {}",
            tc.name
        );
        assert_eq!(
            d.style.border.radius.top_right, tc.top_right,
            "top_right for {}",
            tc.name
        );
        assert_eq!(
            d.style.border.radius.bottom_right, tc.bottom_right,
            "bottom_right for {}",
            tc.name
        );
        assert_eq!(
            d.style.border.radius.bottom_left, tc.bottom_left,
            "bottom_left for {}",
            tc.name
        );
    }
}
