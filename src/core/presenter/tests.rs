use super::*;
use crate::core::event::MouseButton;
use crate::elements::div;
use crate::elements::element::{LayoutContext, PaintContext};
use std::cell::RefCell;
use std::rc::Rc;
use taffy::prelude::{Dimension, NodeId};

/// Test element that registers a hit region and claims pointer presses, so
/// the presenter's capture bookkeeping can be exercised independently of control behavior.
struct CaptureProbe {
    id: ElementId,
    size: Size,
    style: crate::core::style::Style,
    /// Dispatch target the probe saw for each pointer event it received.
    observed_targets: Rc<RefCell<Vec<Option<ElementId>>>>,
}

impl CaptureProbe {
    fn new(id: ElementId, size: Size) -> (Self, Rc<RefCell<Vec<Option<ElementId>>>>) {
        let observed_targets = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                id,
                size,
                style: crate::core::style::Style::default(),
                observed_targets: Rc::clone(&observed_targets),
            },
            observed_targets,
        )
    }
}

impl Element for CaptureProbe {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &crate::core::style::Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let style = taffy::Style {
            size: taffy::Size {
                width: Dimension::Length(self.size.width),
                height: Dimension::Length(self.size.height),
            },
            ..Default::default()
        };
        match cx.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("capture probe layout failed: {err}"),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        self.observed_targets.borrow_mut().push(cx.hit_target());
        matches!(event.kind, PointerEventKind::Down)
    }
}

#[allow(clippy::type_complexity)]
fn presenter_with_probe() -> (
    Presenter<CaptureProbe>,
    ElementId,
    Rc<RefCell<Vec<Option<ElementId>>>>,
) {
    let id = ElementId::from(7);
    let viewport_size = Size::new(100.0, 100.0);
    let (probe, observed_targets) = CaptureProbe::new(id, Size::new(50.0, 50.0));
    let mut presenter = Presenter::with_root(viewport_size, probe);
    match presenter.layout(viewport_size) {
        Ok(_) => {}
        Err(err) => panic!("layout failed: {err}"),
    }
    presenter.paint();
    (presenter, id, observed_targets)
}

#[test]
fn presenter_owns_the_root_it_presents() {
    let viewport_size = Size::new(64.0, 32.0);
    let mut presenter = Presenter::with_root(viewport_size, div().w(64.0).h(32.0));

    let root_bounds = match presenter.layout(viewport_size) {
        Ok(bounds) => bounds,
        Err(err) => panic!("layout failed: {err}"),
    };

    assert_eq!(root_bounds, presenter.root_bounds());
    assert_eq!(presenter.root_bounds().width(), 64.0);
}

#[test]
fn completed_frame_records_the_viewport_it_was_given() {
    let viewport_size = Size::new(64.0, 32.0);
    let mut presenter = Presenter::with_root(viewport_size, div().w_full().h_full());
    match presenter.layout(viewport_size) {
        Ok(_) => {}
        Err(err) => panic!("layout failed: {err}"),
    }
    let painted_root_bounds = presenter.root_bounds();
    presenter.paint();

    let resized = Size::new(128.0, 96.0);
    match presenter.layout(resized) {
        Ok(_) => {}
        Err(err) => panic!("resized layout failed: {err}"),
    }
    assert_ne!(presenter.root_bounds(), painted_root_bounds);
    presenter.complete_presented_frame(None);

    match presenter.last_frame() {
        Some(frame) => {
            assert_eq!(
                frame.viewport_size, viewport_size,
                "layout alone must not relabel the scene painted for the previous viewport"
            );
            assert_eq!(
                frame.root_bounds, painted_root_bounds,
                "layout alone must not relabel the scene with new root bounds"
            );
        }
        None => panic!("expected a recorded frame"),
    }

    presenter.paint();
    presenter.complete_presented_frame(None);

    match presenter.last_frame() {
        Some(frame) => assert_eq!(frame.viewport_size, resized),
        None => panic!("expected a recorded frame"),
    }
}

#[test]
fn pointer_capture_routes_later_events_until_release() {
    let (mut presenter, id, observed_targets) = presenter_with_probe();
    let inside = Point::new(10.0, 10.0);
    let outside = Point::new(90.0, 90.0);

    assert_eq!(presenter.hit_test(inside), Some(id));
    assert_eq!(presenter.hit_test(outside), None);

    let down = presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Down,
        position: inside,
        button: Some(MouseButton::Left),
    });
    assert!(down.stopped, "the probe claims pointer presses");

    // Captured: a move outside the probe still targets it.
    presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Move,
        position: outside,
        button: None,
    });

    presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Up,
        position: outside,
        button: Some(MouseButton::Left),
    });

    // Released: the same move now has no target.
    presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Move,
        position: outside,
        button: None,
    });

    assert_eq!(
        observed_targets.borrow().as_slice(),
        [Some(id), Some(id), Some(id), None],
        "capture should hold the target from press until release"
    );
}

#[test]
fn focus_is_owned_by_the_presenter() {
    let (mut presenter, id, _) = presenter_with_probe();
    assert_eq!(presenter.focused_element(), None);

    presenter.set_focused_element(Some(id));
    assert_eq!(presenter.focused_element(), Some(id));

    let (focused_during_dispatch, _) =
        presenter.with_event_context(|_, event_cx| event_cx.focused_id());
    assert_eq!(focused_during_dispatch, Some(id));
}

#[test]
fn renderer_diagnostics_start_empty() {
    let presenter = Presenter::with_root(Size::new(8.0, 8.0), div());
    assert!(presenter.renderer_diagnostics().is_none());
}

#[test]
fn explicit_accessibility_bounds_are_not_overwritten_by_scene_regions() {
    let id = ElementId::from(77);
    let explicit = Bounds::from_xywh(1.0, 2.0, 30.0, 40.0);
    let mut node = crate::core::accessibility::AccessibilityNode::new(
        id,
        crate::core::accessibility::AccessibilityRole::ScrollArea,
    )
    .with_bounds(explicit);
    let bounds_by_id = HashMap::from([(id, Bounds::from_xywh(9.0, 9.0, 9.0, 9.0))]);

    apply_accessibility_bounds(&mut node, Bounds::ZERO, &bounds_by_id);

    assert_eq!(node.a11y_bounds(), Some(explicit));
}
