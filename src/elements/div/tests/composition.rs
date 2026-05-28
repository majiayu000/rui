use super::support::*;

// ==================== Position ====================

#[test]
fn test_div_absolute() {
    let d = Div::new().absolute();
    assert_eq!(d.style.position, Position::Absolute);
}

#[test]
fn test_div_relative() {
    let d = Div::new().relative();
    assert_eq!(d.style.position, Position::Relative);
}

#[test]
fn test_div_position_default() {
    let d = Div::new();
    assert_eq!(d.style.position, Position::Relative);
}

// ==================== Children ====================

#[test]
fn test_div_child() {
    let child = Div::new().w(50.0);
    let parent = Div::new().child(child);
    assert_eq!(parent.children.len(), 1);
}

#[test]
fn test_div_multiple_children() {
    let parent = Div::new()
        .child(Div::new().w(50.0))
        .child(Div::new().w(60.0))
        .child(Div::new().w(70.0));
    assert_eq!(parent.children.len(), 3);
}

#[test]
fn test_div_children_from_iterator() {
    let child_divs = vec![Div::new().w(10.0), Div::new().w(20.0), Div::new().w(30.0)];
    let parent = Div::new().children(child_divs);
    assert_eq!(parent.children.len(), 3);
}

#[test]
fn test_div_children_empty_iterator() {
    let empty: Vec<Div> = vec![];
    let parent = Div::new().children(empty);
    assert!(parent.children.is_empty());
}

#[test]
fn test_div_children_combined() {
    let parent = Div::new()
        .child(Div::new())
        .children(vec![Div::new(), Div::new()]);
    assert_eq!(parent.children.len(), 3);
}

#[test]
fn test_div_element_trait_children() {
    let parent = Div::new().child(Div::new()).child(Div::new());
    let children = <Div as Element>::children(&parent);
    assert_eq!(children.len(), 2);
}

// ==================== Event Handlers ====================

#[test]
fn test_div_on_click() {
    let d = Div::new().on_click(|| {});
    assert!(d.on_click.is_some());
}

#[test]
fn test_div_on_hover() {
    let d = Div::new().on_hover(|_hovered| {});
    assert!(d.on_hover.is_some());
}

#[test]
fn test_div_no_event_handlers_by_default() {
    let d = Div::new();
    assert!(d.on_click.is_none());
    assert!(d.on_hover.is_none());
}

// ==================== Element Trait ====================

#[test]
fn test_div_element_trait_style() {
    let d = Div::new().w(100.0).h(50.0);
    let style = d.style();
    assert_eq!(style.width, Some(100.0));
    assert_eq!(style.height, Some(50.0));
}

// ==================== Chained Builder Pattern ====================

#[test]
fn test_div_chained_builder_comprehensive() {
    let d = div()
        .id(ElementId::from(1u64))
        .w(400.0)
        .h(300.0)
        .min_w(100.0)
        .min_h(50.0)
        .max_w(800.0)
        .max_h(600.0)
        .flex_col()
        .gap(12.0)
        .justify_center()
        .items_center()
        .p(16.0)
        .m(8.0)
        .bg(Color::WHITE)
        .border(1.0, Color::BLACK)
        .rounded(8.0)
        .shadow_md()
        .opacity(0.9)
        .overflow_hidden();

    assert_eq!(d.id, Some(ElementId::from(1u64)));
    assert_eq!(d.style.width, Some(400.0));
    assert_eq!(d.style.height, Some(300.0));
    assert_eq!(d.style.min_width, Some(100.0));
    assert_eq!(d.style.min_height, Some(50.0));
    assert_eq!(d.style.max_width, Some(800.0));
    assert_eq!(d.style.max_height, Some(600.0));
    assert_eq!(d.style.display, Display::Flex);
    assert_eq!(d.style.flex_direction, FlexDirection::Column);
    assert_eq!(d.style.gap, 12.0);
    assert_eq!(d.style.justify_content, JustifyContent::Center);
    assert_eq!(d.style.align_items, AlignItems::Center);
    assert_eq!(d.style.padding, Edges::all(16.0));
    assert_eq!(d.style.margin, Edges::all(8.0));
    assert_eq!(d.style.border.width, Edges::all(1.0));
    assert_eq!(d.style.border.color, Color::BLACK);
    assert_eq!(d.style.border.radius, Corners::all(8.0));
    assert!(d.style.shadow.is_some());
    assert_eq!(d.style.opacity, 0.9);
    assert_eq!(d.style.overflow_x, Overflow::Hidden);
    assert_eq!(d.style.overflow_y, Overflow::Hidden);
}

#[test]
fn test_div_layout_with_children() {
    let parent = div()
        .flex_row()
        .gap(8.0)
        .child(div().w(100.0).h(50.0))
        .child(div().w(100.0).h(50.0))
        .child(div().w(100.0).h(50.0));

    assert_eq!(parent.children.len(), 3);
    assert_eq!(parent.style.flex_direction, FlexDirection::Row);
    assert_eq!(parent.style.gap, 8.0);
}

#[test]
fn test_div_nested_layout() {
    let layout = div()
        .flex_col()
        .child(div().flex_row().child(div().w(50.0)).child(div().w(50.0)))
        .child(div().flex_row().child(div().w(50.0)).child(div().w(50.0)));

    assert_eq!(layout.children.len(), 2);
}

// ==================== Style Default Values ====================

#[test]
fn test_div_style_default_values() {
    let d = Div::new();
    let style = d.style();

    // Default flex shrink should be 1.0 from Style::new()
    assert_eq!(style.flex_shrink, 1.0);
    // Default opacity should be 1.0 from Style::new()
    assert_eq!(style.opacity, 1.0);
    // Default display is Flex
    assert_eq!(style.display, Display::Flex);
    // Default flex direction is Row
    assert_eq!(style.flex_direction, FlexDirection::Row);
    // Default justify content is FlexStart
    assert_eq!(style.justify_content, JustifyContent::FlexStart);
    // Default align items is Stretch
    assert_eq!(style.align_items, AlignItems::Stretch);
}
