use super::accessibility::paint_for_accessibility_state;
use super::*;
use crate::core::event::{Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point};
use crate::elements::element::PointerEventKind;
use std::cell::Cell;
use std::rc::Rc;
use taffy::TaffyTree;

#[test]
fn scroll_view_preserves_nested_hit_origin_after_scroll() {
    let field_id = ElementId::new();
    let mut scrollable = scroll_view()
        .child(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(crate::elements::div().w(120.0).h(100.0))
                .child(
                    crate::elements::Input::new()
                        .id(field_id)
                        .value("nested")
                        .w(120.0)
                        .h(40.0),
                ),
        )
        .w(140.0)
        .h(80.0);

    let viewport = Size::new(140.0, 80.0);
    paint_for_accessibility_state(&mut scrollable, viewport);

    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Scroll so the tall container origin moves above the viewport while the
    // nested input remains visibly painted around y=50.
    assert!(scrollable.handle_scroll_event(
        &mut event_cx,
        &ScrollEvent {
            position: Point::new(8.0, 8.0),
            delta_x: 0.0,
            delta_y: 50.0,
            modifiers: Modifiers::default(),
        },
    ));

    assert!(
        scrollable.handle_pointer_event(
            &mut event_cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(8.0, 50.0),
                button: Some(MouseButton::Left),
            },
        ),
        "nested input at painted scrolled position must remain hittable"
    );
    assert_eq!(
        event_cx.focused_id(),
        Some(field_id),
        "scrolled nested origin must not shift hit-testing to the viewport origin"
    );
}

#[test]
fn scroll_view_fully_clipped_child_does_not_hit_at_origin() {
    let clicked = Rc::new(Cell::new(false));
    let clicked_ref = Rc::clone(&clicked);
    let mut scrollable = scroll_view()
        .child(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(
                    crate::elements::div()
                        .w(120.0)
                        .h(40.0)
                        .on_click(move || clicked_ref.set(true)),
                )
                .child(crate::elements::div().w(120.0).h(160.0)),
        )
        .w(140.0)
        .h(80.0);

    let viewport = Size::new(140.0, 80.0);
    paint_for_accessibility_state(&mut scrollable, viewport);

    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    assert!(scrollable.handle_scroll_event(
        &mut event_cx,
        &ScrollEvent {
            position: Point::new(8.0, 8.0),
            delta_x: 0.0,
            delta_y: 80.0,
            modifiers: Modifiers::default(),
        },
    ));

    // Top clickable child is fully above the viewport. An empty-intersection
    // Bounds::ZERO fallback would wrongly treat (0,0) as inside.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(0.0, 0.0),
            button: Some(MouseButton::Left),
        },
    );
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(0.0, 0.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !clicked.get(),
        "fully clipped child must not activate from a Bounds::ZERO origin hit"
    );
}

/// Legacy/custom Element that hit-tests with `cx.bounds().contains` only.
struct LegacyBoundsHitProbe {
    inner: crate::elements::Div,
    activated: Rc<Cell<bool>>,
}

impl Element for LegacyBoundsHitProbe {
    fn style(&self) -> &crate::core::style::Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        // Intentionally ignores contains_pointer / hit_clip.
        if !cx.bounds().contains(event.position) {
            return false;
        }
        if matches!(event.kind, PointerEventKind::Up) {
            self.activated.set(true);
        }
        true
    }
}

#[test]
fn scroll_view_enforces_hit_clip_for_bounds_contains_elements() {
    let activated = Rc::new(Cell::new(false));
    let activated_ref = Rc::clone(&activated);
    let mut scrollable = scroll_view()
        .child(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(160.0)
                .child(crate::elements::div().w(120.0).h(80.0))
                .child(LegacyBoundsHitProbe {
                    inner: crate::elements::div().w(120.0).h(80.0),
                    activated: activated_ref,
                }),
        )
        .w(140.0)
        .h(80.0)
        .disabled(true);

    let mut taffy = TaffyTree::<ElementId>::new();
    let viewport = Size::new(140.0, 80.0);
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Second row starts at the viewport max edge (y=80). Edge-only contact
    // must not activate a legacy bounds().contains Element.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(8.0, 80.0),
            button: Some(MouseButton::Left),
        },
    );
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(8.0, 80.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !activated.get(),
        "legacy bounds().contains element must not activate on clipped edge-only hit"
    );

    // Deep clipped-away point within the second row's layout bounds.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(8.0, 120.0),
            button: Some(MouseButton::Left),
        },
    );
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(8.0, 120.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !activated.get(),
        "legacy bounds().contains element must not activate outside the viewport"
    );
}

/// Parent with small layout bounds that still paints an overflowing child
/// inside the viewport (models Div→Button overflow without scene hits).
struct OverflowHost {
    inner: crate::elements::Div,
    child: AnyElement,
    overflow_height: f32,
}

impl Element for OverflowHost {
    fn style(&self) -> &crate::core::style::Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let layout = cx.layout_bounds();
        let overflow_bounds =
            Bounds::from_xywh(layout.x(), layout.y(), layout.width(), self.overflow_height);
        let mut child_cx = cx.with_bounds(overflow_bounds);
        self.child.handle_pointer_event(&mut child_cx, event)
    }
}

#[test]
fn scroll_view_forwards_pointer_to_visible_overflow_descendant() {
    let activated = Rc::new(Cell::new(false));
    let activated_ref = Rc::clone(&activated);
    let mut scrollable = scroll_view()
        .child(OverflowHost {
            inner: crate::elements::div().w(120.0).h(40.0),
            overflow_height: 80.0,
            child: AnyElement::new(LegacyBoundsHitProbe {
                inner: crate::elements::div().w(120.0).h(80.0),
                activated: activated_ref,
            }),
        })
        .w(140.0)
        .h(80.0)
        .disabled(true);

    let mut taffy = TaffyTree::<ElementId>::new();
    let viewport = Size::new(140.0, 80.0);
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Click in the overflow region: outside the host's 40px layout bounds,
    // but still inside the viewport and the descendant's painted area.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(8.0, 60.0),
            button: Some(MouseButton::Left),
        },
    );
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(8.0, 60.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        activated.get(),
        "visible overflow descendant inside hit_clip must remain clickable"
    );
}

#[test]
fn scroll_view_forwards_through_fully_clipped_overflow_host() {
    let activated = Rc::new(Cell::new(false));
    let activated_ref = Rc::clone(&activated);

    /// Host whose layout is fully below the viewport, but forwards to a
    /// child that overflows upward back into the visible clip.
    struct UpwardOverflowHost {
        inner: crate::elements::Div,
        child: AnyElement,
        child_y_offset: f32,
        child_height: f32,
    }

    impl Element for UpwardOverflowHost {
        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            let layout = cx.layout_bounds();
            let overflow_bounds = Bounds::from_xywh(
                layout.x(),
                layout.y() + self.child_y_offset,
                layout.width(),
                self.child_height,
            );
            let mut child_cx = cx.with_bounds(overflow_bounds);
            self.child.handle_pointer_event(&mut child_cx, event)
        }
    }

    // Spacer pushes the host to y=80 (fully clipped); child overflows up.
    let mut scrollable = scroll_view()
        .child(
            crate::elements::div()
                .flex_col()
                .w(120.0)
                .h(200.0)
                .child(crate::elements::div().w(120.0).h(80.0))
                .child(UpwardOverflowHost {
                    inner: crate::elements::div().w(120.0).h(40.0),
                    child_y_offset: -40.0,
                    child_height: 80.0,
                    child: AnyElement::new(LegacyBoundsHitProbe {
                        inner: crate::elements::div().w(120.0).h(80.0),
                        activated: activated_ref,
                    }),
                }),
        )
        .w(140.0)
        .h(80.0)
        .disabled(true);

    let mut taffy = TaffyTree::<ElementId>::new();
    let viewport = Size::new(140.0, 80.0);
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Click inside the viewport over the upward-overflow paint of a fully
    // clipped host (host layout starts at y=80).
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(8.0, 50.0),
            button: Some(MouseButton::Left),
        },
    );
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(8.0, 50.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        activated.get(),
        "fully clipped overflow host must still forward in-clip descendant hits"
    );
}

#[test]
fn scroll_view_with_bounds_recomputes_clip_for_concrete_legacy_child() {
    // TabsRoot/ScrollView-style composites dispatch via with_bounds to a
    // concrete child (not AnyElement). Outside Up cleanup must recompute
    // clip-derived hit_suppressed so legacy bounds().contains cannot
    // activate when the release is outside the viewport.
    let activated = Rc::new(Cell::new(false));
    let activated_ref = Rc::clone(&activated);

    struct ConcreteOverflowHost {
        inner: crate::elements::Div,
        child: LegacyBoundsHitProbe,
        overflow_height: f32,
    }

    impl Element for ConcreteOverflowHost {
        fn style(&self) -> &crate::core::style::Style {
            self.inner.style()
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            self.inner.layout(cx)
        }

        fn paint(&mut self, cx: &mut PaintContext) {
            self.inner.paint(cx);
        }

        fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
            let layout = cx.layout_bounds();
            let overflow_bounds =
                Bounds::from_xywh(layout.x(), layout.y(), layout.width(), self.overflow_height);
            let mut child_cx = cx.with_bounds(overflow_bounds);
            self.child.handle_pointer_event(&mut child_cx, event)
        }
    }

    let mut scrollable = scroll_view()
        .child(ConcreteOverflowHost {
            inner: crate::elements::div().w(120.0).h(40.0),
            overflow_height: 200.0,
            child: LegacyBoundsHitProbe {
                inner: crate::elements::div().w(120.0).h(200.0),
                activated: activated_ref,
            },
        })
        .w(140.0)
        .h(80.0)
        .disabled(true);

    let mut taffy = TaffyTree::<ElementId>::new();
    let viewport = Size::new(140.0, 80.0);
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = scrollable.layout(&mut layout_cx);
    if let Err(err) = taffy.compute_layout(
        node,
        taffy::Size {
            width: taffy::prelude::AvailableSpace::Definite(viewport.width),
            height: taffy::prelude::AvailableSpace::Definite(viewport.height),
        },
    ) {
        panic!("layout should compute: {err}");
    }

    let mut focused = None;
    let mut event_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height),
        &taffy,
        &mut focused,
    );

    // Press inside the visible overflow region.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Down,
            position: Point::new(8.0, 60.0),
            button: Some(MouseButton::Left),
        },
    );
    // Release outside the viewport but still inside the oversized child
    // layout bounds forwarded via concrete with_bounds.
    let _ = scrollable.handle_pointer_event(
        &mut event_cx,
        &PointerEvent {
            kind: PointerEventKind::Up,
            position: Point::new(8.0, 150.0),
            button: Some(MouseButton::Left),
        },
    );
    assert!(
        !activated.get(),
        "concrete with_bounds child must not activate on outside-viewport Up"
    );
}

/// Captured legacy probe that activates on Up via bounds().contains only.
pub(super) struct CapturedLegacyBoundsProbe {
    pub(super) id: ElementId,
    pub(super) inner: crate::elements::Div,
    pub(super) activated: Rc<Cell<bool>>,
    pub(super) pressed: Rc<Cell<bool>>,
}

impl Element for CapturedLegacyBoundsProbe {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &crate::core::style::Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        // Intentionally ignores contains_pointer / hit_clip.
        if !cx.bounds().contains(event.position) {
            if matches!(event.kind, PointerEventKind::Move | PointerEventKind::Up)
                && self.pressed.get()
            {
                self.pressed.set(false);
                return true;
            }
            return false;
        }
        match event.kind {
            PointerEventKind::Down => {
                self.pressed.set(true);
                true
            }
            PointerEventKind::Up => {
                if self.pressed.get() {
                    self.pressed.set(false);
                    self.activated.set(true);
                }
                true
            }
            PointerEventKind::Move => true,
        }
    }
}

#[test]
fn event_context_contains_pointer_rejects_edge_only_hit_clip_intersection() {
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut root = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        &taffy,
        &mut focused,
    );
    // Child starts exactly at the viewport's max y edge: inclusive contains
    // on both rects would accept y=80, but the intersection has zero area.
    {
        let cx = root.with_bounds_and_hit_clip(
            Bounds::from_xywh(0.0, 80.0, 120.0, 80.0),
            Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        );
        assert!(
            !cx.contains_pointer(Point::new(8.0, 80.0)),
            "edge-only bounds∩hit_clip must not count as a pointer hit"
        );
        assert!(
            !cx.contains_pointer(Point::new(8.0, 100.0)),
            "fully clipped layout area must not count as a pointer hit"
        );
    }

    let visible = root.with_bounds_and_hit_clip(
        Bounds::from_xywh(0.0, 0.0, 120.0, 160.0),
        Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
    );
    assert!(
        visible.contains_pointer(Point::new(8.0, 40.0)),
        "positive-area visible intersection must still accept interior points"
    );
}

#[test]
fn event_context_pointer_outside_hit_region_allows_overflow_inside_clip() {
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut root = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
        &taffy,
        &mut focused,
    );
    let small_ancestor = root.with_bounds_and_hit_clip(
        Bounds::from_xywh(0.0, 0.0, 120.0, 40.0),
        Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
    );
    assert!(
        !small_ancestor.pointer_outside_hit_region(Point::new(8.0, 60.0)),
        "point outside ancestor bounds but inside hit_clip must still dispatch"
    );
    assert!(
        small_ancestor.pointer_outside_hit_region(Point::new(8.0, 100.0)),
        "point outside hit_clip must remain suppressed"
    );

    let fully_clipped = root.with_bounds_and_hit_clip(
        Bounds::from_xywh(0.0, 80.0, 120.0, 80.0),
        Bounds::from_xywh(0.0, 0.0, 140.0, 80.0),
    );
    assert!(
        !fully_clipped.pointer_outside_hit_region(Point::new(8.0, 40.0)),
        "fully clipped ancestors must still forward points inside the viewport clip"
    );
    assert!(
        !fully_clipped.pointer_outside_hit_region(Point::new(8.0, 80.0)),
        "edge points inside the inclusive clip must still reach overflow hosts"
    );
    assert!(
        fully_clipped.pointer_outside_hit_region(Point::new(8.0, 100.0)),
        "points outside the viewport clip must remain suppressed"
    );
}
