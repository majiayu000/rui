use super::support::*;

// ==================== Padding ====================

#[test]
fn test_div_p() {
    let d = Div::new().p(20.0);
    assert_eq!(d.style.padding, Edges::all(20.0));
}

#[test]
fn test_div_px() {
    let d = Div::new().px(10.0);
    assert_eq!(d.style.padding.left, 10.0);
    assert_eq!(d.style.padding.right, 10.0);
    assert_eq!(d.style.padding.top, 0.0);
    assert_eq!(d.style.padding.bottom, 0.0);
}

#[test]
fn test_div_py() {
    let d = Div::new().py(15.0);
    assert_eq!(d.style.padding.top, 15.0);
    assert_eq!(d.style.padding.bottom, 15.0);
    assert_eq!(d.style.padding.left, 0.0);
    assert_eq!(d.style.padding.right, 0.0);
}

#[test]
fn test_div_pt() {
    let d = Div::new().pt(5.0);
    assert_eq!(d.style.padding.top, 5.0);
}

#[test]
fn test_div_pb() {
    let d = Div::new().pb(8.0);
    assert_eq!(d.style.padding.bottom, 8.0);
}

#[test]
fn test_div_pl() {
    let d = Div::new().pl(12.0);
    assert_eq!(d.style.padding.left, 12.0);
}

#[test]
fn test_div_pr() {
    let d = Div::new().pr(7.0);
    assert_eq!(d.style.padding.right, 7.0);
}

#[test]
fn test_div_padding_combined() {
    let d = Div::new().p(10.0).pt(20.0).pr(5.0);
    assert_eq!(d.style.padding.top, 20.0);
    assert_eq!(d.style.padding.right, 5.0);
    assert_eq!(d.style.padding.bottom, 10.0);
    assert_eq!(d.style.padding.left, 10.0);
}

// ==================== Margin ====================

#[test]
fn test_div_m() {
    let d = Div::new().m(16.0);
    assert_eq!(d.style.margin, Edges::all(16.0));
}

#[test]
fn test_div_mx() {
    let d = Div::new().mx(24.0);
    assert_eq!(d.style.margin.left, 24.0);
    assert_eq!(d.style.margin.right, 24.0);
    assert_eq!(d.style.margin.top, 0.0);
    assert_eq!(d.style.margin.bottom, 0.0);
}

#[test]
fn test_div_my() {
    let d = Div::new().my(32.0);
    assert_eq!(d.style.margin.top, 32.0);
    assert_eq!(d.style.margin.bottom, 32.0);
    assert_eq!(d.style.margin.left, 0.0);
    assert_eq!(d.style.margin.right, 0.0);
}

// ==================== Background ====================

#[test]
fn test_div_bg_color() {
    let d = Div::new().bg(Color::RED);
    match d.style.background {
        Background::Solid(color) => assert_eq!(color, Color::RED),
        _ => panic!("Expected solid background"),
    }
}

#[test]
fn test_div_bg_hex() {
    let d = Div::new().bg(Color::hex(0xFF00FF));
    match d.style.background {
        Background::Solid(_) => {}
        _ => panic!("Expected solid background"),
    }
}

#[test]
fn test_div_bg_gradient() {
    let d = Div::new().bg_gradient(Color::RED, Color::BLUE, 45.0);
    match d.style.background {
        Background::LinearGradient { start, end, angle } => {
            assert_eq!(start, Color::RED);
            assert_eq!(end, Color::BLUE);
            assert_eq!(angle, 45.0);
        }
        _ => panic!("Expected linear gradient background"),
    }
}

// ==================== Border ====================

#[test]
fn test_div_border() {
    let d = Div::new().border(2.0, Color::BLACK);
    assert_eq!(d.style.border.width, Edges::all(2.0));
    assert_eq!(d.style.border.color, Color::BLACK);
}

#[test]
fn test_div_border_color() {
    let d = Div::new().border_color(Color::GREEN);
    assert_eq!(d.style.border.color, Color::GREEN);
}

#[test]
fn test_div_border_width() {
    let d = Div::new().border_width(3.0);
    assert_eq!(d.style.border.width, Edges::all(3.0));
}

#[test]
fn test_div_rounded() {
    let d = Div::new().rounded(8.0);
    assert_eq!(d.style.border.radius, Corners::all(8.0));
}

#[test]
fn test_div_rounded_t() {
    let d = Div::new().rounded_t(12.0);
    assert_eq!(d.style.border.radius.top_left, 12.0);
    assert_eq!(d.style.border.radius.top_right, 12.0);
    assert_eq!(d.style.border.radius.bottom_left, 0.0);
    assert_eq!(d.style.border.radius.bottom_right, 0.0);
}

#[test]
fn test_div_rounded_b() {
    let d = Div::new().rounded_b(10.0);
    assert_eq!(d.style.border.radius.bottom_left, 10.0);
    assert_eq!(d.style.border.radius.bottom_right, 10.0);
    assert_eq!(d.style.border.radius.top_left, 0.0);
    assert_eq!(d.style.border.radius.top_right, 0.0);
}

#[test]
fn test_div_rounded_full() {
    let d = Div::new().rounded_full();
    assert_eq!(d.style.border.radius, Corners::all(9999.0));
}

// ==================== Shadow ====================

#[test]
fn test_div_shadow() {
    let shadow = Shadow::new(2.0, 4.0, 8.0, Color::rgba(0.0, 0.0, 0.0, 0.2));
    let d = Div::new().shadow(shadow);
    assert!(d.style.shadow.is_some());
    let s = d.style.shadow.unwrap();
    assert_eq!(s.offset_x, 2.0);
    assert_eq!(s.offset_y, 4.0);
    assert_eq!(s.blur_radius, 8.0);
}

#[test]
fn test_div_shadow_sm() {
    let d = Div::new().shadow_sm();
    assert!(d.style.shadow.is_some());
    let s = d.style.shadow.unwrap();
    assert_eq!(s.offset_x, 0.0);
    assert_eq!(s.offset_y, 1.0);
    assert_eq!(s.blur_radius, 2.0);
}

#[test]
fn test_div_shadow_md() {
    let d = Div::new().shadow_md();
    assert!(d.style.shadow.is_some());
    let s = d.style.shadow.unwrap();
    assert_eq!(s.offset_x, 0.0);
    assert_eq!(s.offset_y, 4.0);
    assert_eq!(s.blur_radius, 6.0);
}

#[test]
fn test_div_shadow_lg() {
    let d = Div::new().shadow_lg();
    assert!(d.style.shadow.is_some());
    let s = d.style.shadow.unwrap();
    assert_eq!(s.offset_x, 0.0);
    assert_eq!(s.offset_y, 10.0);
    assert_eq!(s.blur_radius, 15.0);
}

#[test]
fn test_div_no_shadow_by_default() {
    let d = Div::new();
    assert!(d.style.shadow.is_none());
}

// ==================== Opacity ====================

#[test]
fn test_div_opacity() {
    let d = Div::new().opacity(0.5);
    assert_eq!(d.style.opacity, 0.5);
}

#[test]
fn test_div_opacity_full() {
    let d = Div::new().opacity(1.0);
    assert_eq!(d.style.opacity, 1.0);
}

#[test]
fn test_div_opacity_zero() {
    let d = Div::new().opacity(0.0);
    assert_eq!(d.style.opacity, 0.0);
}

// ==================== Overflow ====================

#[test]
fn test_div_overflow_hidden() {
    let d = Div::new().overflow_hidden();
    assert_eq!(d.style.overflow_x, Overflow::Hidden);
    assert_eq!(d.style.overflow_y, Overflow::Hidden);
}

#[test]
fn test_div_overflow_scroll() {
    let d = Div::new().overflow_scroll();
    assert_eq!(d.style.overflow_x, Overflow::Scroll);
    assert_eq!(d.style.overflow_y, Overflow::Scroll);
}

#[test]
fn test_div_overflow_default() {
    let d = Div::new();
    assert_eq!(d.style.overflow_x, Overflow::Visible);
    assert_eq!(d.style.overflow_y, Overflow::Visible);
}
