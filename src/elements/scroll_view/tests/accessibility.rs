use super::*;
use crate::advanced_ui::container;
use crate::core::event::{Modifiers, ScrollEvent};
use crate::core::geometry::{Bounds, Point};
use crate::core::presenter::Presenter;
use crate::renderer::Scene;
use taffy::TaffyTree;

pub(super) fn paint_for_accessibility_state(scrollable: &mut ScrollView, size: Size) {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, size);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(size.width),
            height: taffy::prelude::AvailableSpace::Definite(size.height),
        },
    ) {
        panic!("layout should compute: {}", err);
    }

    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(
        &mut scene,
        Bounds::from_xywh(0.0, 0.0, size.width, size.height),
        &taffy,
    );
    scrollable.paint(&mut paint_cx);
}

pub(super) fn accessibility_node(scrollable: &ScrollView) -> AccessibilityNode {
    match scrollable.accessibility(&AccessibilityContext::default()) {
        Ok(Some(node)) => node,
        Ok(None) => panic!("scrollable should expose an accessibility node"),
        Err(err) => panic!("accessibility failed: {}", err),
    }
}

#[test]
fn scroll_view_accessibility_actions_follow_scroll_range() {
    let mut overflowing = scroll_view().child(container().w(100.0).h(300.0)).h(100.0);
    let before_paint = match overflowing.accessibility(&AccessibilityContext::default()) {
        Ok(Some(node)) => node,
        Ok(None) => panic!("scrollable should expose an accessibility node"),
        Err(err) => panic!("accessibility failed: {}", err),
    };
    assert!(before_paint.a11y_actions().is_empty());

    paint_for_accessibility_state(&mut overflowing, Size::new(100.0, 100.0));
    let after_paint = match overflowing.accessibility(&AccessibilityContext::default()) {
        Ok(Some(node)) => node,
        Ok(None) => panic!("scrollable should expose an accessibility node"),
        Err(err) => panic!("accessibility failed: {}", err),
    };
    assert_eq!(
        after_paint.a11y_actions(),
        [AccessibilityAction::ScrollForward]
    );

    let mut empty = scroll_view().h(100.0);
    paint_for_accessibility_state(&mut empty, Size::new(100.0, 100.0));
    let empty_node = match empty.accessibility(&AccessibilityContext::default()) {
        Ok(Some(node)) => node,
        Ok(None) => panic!("scrollable should expose an accessibility node"),
        Err(err) => panic!("accessibility failed: {}", err),
    };
    assert!(empty_node.a11y_actions().is_empty());
}

#[test]
fn scroll_view_accessibility_scroll_position_tracks_scroll_state() {
    let mut scrollable = scroll_view().child(container().w(100.0).h(300.0)).h(100.0);
    assert_eq!(accessibility_node(&scrollable).a11y_scroll_position(), None);

    paint_for_accessibility_state(&mut scrollable, Size::new(100.0, 100.0));
    let before_scroll = accessibility_node(&scrollable);
    assert_eq!(
        before_scroll.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 0.0, 0.0, 200.0))
    );
    assert_eq!(
        before_scroll.a11y_actions(),
        [AccessibilityAction::ScrollForward]
    );

    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
        &taffy,
        &mut focused,
    );
    assert!(scrollable.handle_scroll_event(
        &mut event_cx,
        &ScrollEvent {
            position: Point::new(4.0, 4.0),
            delta_x: 0.0,
            delta_y: 24.0,
            modifiers: Modifiers::default(),
        },
    ));

    let after_scroll = accessibility_node(&scrollable);
    assert_eq!(
        after_scroll.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
    );
    assert_eq!(
        after_scroll.a11y_actions(),
        [
            AccessibilityAction::ScrollForward,
            AccessibilityAction::ScrollBackward
        ]
    );

    paint_for_accessibility_state(&mut scrollable, Size::new(100.0, 100.0));
    assert_eq!(
        accessibility_node(&scrollable).a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 24.0, 0.0, 200.0))
    );
}

#[test]
fn accessibility_scroll_action_uses_the_horizontal_axis() {
    let id = ElementId::new();
    let scrollable = scroll_view()
        .child(crate::elements::div().w(300.0).h(40.0).flex_shrink(0.0))
        .id(id)
        .horizontal()
        .w(100.0)
        .h(40.0)
        .accessibility_label("Columns");
    let mut presenter = Presenter::with_root(Size::new(100.0, 40.0), scrollable);
    presenter
        .layout(Size::new(100.0, 40.0))
        .expect("horizontal scrollable should lay out");
    presenter.paint();
    presenter.set_focused_element(Some(id));

    let (outcome, redraw) = presenter.with_event_context(|root, cx| {
        root.dispatch_action(
            cx,
            &crate::core::action::ActionId::custom(
                crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION,
            ),
        )
    });

    assert!(outcome.is_handled());
    assert!(redraw);
    let tree = presenter
        .accessibility_tree()
        .expect("horizontal accessibility tree should build");
    assert_eq!(
        tree.roots()[0].a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(40.0, 0.0, 200.0, 0.0))
    );
}

#[test]
fn accessibility_scroll_action_routes_to_the_nested_target() {
    let outer_id = ElementId::new();
    let inner_id = ElementId::new();
    let inner = scroll_view()
        .child(container().w(100.0).h(300.0))
        .id(inner_id)
        .vertical()
        .w(100.0)
        .h(150.0)
        .accessibility_label("Inner");
    let outer = scroll_view()
        .child(inner)
        .id(outer_id)
        .vertical()
        .w(100.0)
        .h(100.0)
        .accessibility_label("Outer");
    let mut presenter = Presenter::with_root(Size::new(100.0, 100.0), outer);
    presenter
        .layout(Size::new(100.0, 100.0))
        .expect("nested scrollables should lay out");
    presenter.paint();
    presenter.set_focused_element(Some(inner_id));

    let (outcome, redraw) = presenter.with_event_context(|root, cx| {
        root.dispatch_action(
            cx,
            &crate::core::action::ActionId::custom(
                crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION,
            ),
        )
    });

    assert!(outcome.is_handled());
    assert!(redraw);
    let tree = presenter
        .accessibility_tree()
        .expect("nested accessibility tree should build");
    let outer = &tree.roots()[0];
    let inner = &outer.a11y_children()[0];
    assert_eq!(
        outer.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 0.0, 0.0, 50.0))
    );
    assert_eq!(
        inner.a11y_scroll_position(),
        Some(AccessibilityScrollPosition::new(0.0, 40.0, 0.0, 150.0))
    );
}

#[test]
#[should_panic(expected = "scroll view accessibility label must not be empty")]
fn scroll_view_rejects_empty_accessibility_label() {
    drop(scroll_view().accessibility_label(" "));
}
