use crate::core::ElementId;
use crate::core::app::AppContext;
use crate::core::event::Event;
use crate::core::frame_pipeline::{FramePipeline, FramePipelineError};
use crate::core::geometry::{Bounds, Point, Size};
use crate::elements::Element;
use crate::elements::element::{EventContext, PointerEvent, PointerEventKind};
use crate::renderer::RendererDiagnostics;
use crate::renderer::Scene;
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::TaffyTree;

#[derive(Debug, Clone, PartialEq)]
pub struct PresenterFrame {
    pub viewport_size: Size,
    pub root_bounds: Bounds,
    pub primitive_count: usize,
}

/// Outcome of dispatching one pointer event through the presented tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerDispatch {
    /// An element stopped propagation.
    pub stopped: bool,
    /// An element asked for a redraw while handling the event.
    pub redraw_requested: bool,
}

/// Per-window presentation state.
///
/// The presenter owns the element tree it presents together with everything
/// derived from it: layout tree, text measurement cache, scene, root bounds,
/// focus, hit-test tracking, pointer capture and the last renderer diagnostics
/// snapshot. Native window handles and renderer backend resources stay outside.
///
/// Authoritative viewport size is deliberately *not* stored here. [`AppContext`]
/// is its single owner; the presenter only records which viewport produced its
/// latest layout and painted frame.
pub struct Presenter<E = ()> {
    root: E,
    legacy_viewport_size: Option<Size>,
    taffy: TaffyTree<ElementId>,
    scene: Scene,
    text_measurer: TextMeasureCache,
    root_bounds: Bounds,
    focused_element: Option<ElementId>,
    last_pointer_hit_target: Option<ElementId>,
    pointer_capture_target: Option<ElementId>,
    renderer_diagnostics: Option<RendererDiagnostics>,
    laid_out_viewport: Size,
    prepared_frame: PresenterFrame,
    last_frame: Option<PresenterFrame>,
}

impl<E> Presenter<E> {
    /// `viewport_size` only seeds the initial root bounds; every later frame
    /// recomputes them from the size passed to [`Presenter::layout`].
    pub fn with_root(viewport_size: Size, root: E) -> Self {
        let root_bounds = Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height);
        Self {
            root,
            legacy_viewport_size: None,
            taffy: TaffyTree::new(),
            scene: Scene::new(),
            text_measurer: TextMeasureCache::new(),
            root_bounds,
            focused_element: None,
            last_pointer_hit_target: None,
            pointer_capture_target: None,
            renderer_diagnostics: None,
            laid_out_viewport: viewport_size,
            prepared_frame: PresenterFrame {
                viewport_size,
                root_bounds,
                primitive_count: 0,
            },
            last_frame: None,
        }
    }

    pub fn root(&self) -> &E {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut E {
        &mut self.root
    }

    pub fn root_bounds(&self) -> Bounds {
        self.root_bounds
    }

    pub fn set_root_bounds(&mut self, root_bounds: Bounds) {
        self.root_bounds = root_bounds;
    }

    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    pub fn set_focused_element(&mut self, focused_element: Option<ElementId>) {
        self.focused_element = focused_element;
    }

    pub fn focused_element_mut(&mut self) -> &mut Option<ElementId> {
        &mut self.focused_element
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn taffy(&self) -> &TaffyTree<ElementId> {
        &self.taffy
    }

    /// Latest renderer diagnostics snapshot recorded for this window, if the
    /// backend produced one. Headless presentation leaves this empty.
    pub fn renderer_diagnostics(&self) -> Option<&RendererDiagnostics> {
        self.renderer_diagnostics.as_ref()
    }

    pub fn frame_surfaces_mut(&mut self) -> (&mut TaffyTree<ElementId>, &mut Scene) {
        (&mut self.taffy, &mut self.scene)
    }

    /// Layout inputs owned by the presenter. The text measurement cache is
    /// handed out by reference so its metrics survive across frames.
    pub fn layout_surfaces_mut(&mut self) -> (&mut TaffyTree<ElementId>, &mut TextMeasureCache) {
        (&mut self.taffy, &mut self.text_measurer)
    }

    pub fn paint_surfaces_mut(&mut self) -> (&TaffyTree<ElementId>, &mut Scene) {
        (&self.taffy, &mut self.scene)
    }

    pub fn event_context_parts_mut(
        &mut self,
    ) -> (Bounds, &TaffyTree<ElementId>, &mut Option<ElementId>) {
        (self.root_bounds, &self.taffy, &mut self.focused_element)
    }

    pub fn hit_test(&self, position: Point) -> Option<ElementId> {
        self.scene.hit_test(position)
    }

    pub(crate) fn prepared_scene(&self) -> (&Scene, &PresenterFrame) {
        (&self.scene, &self.prepared_frame)
    }

    pub fn complete_presented_frame(&mut self, diagnostics: Option<RendererDiagnostics>) {
        self.last_frame = Some(self.prepared_frame.clone());
        self.renderer_diagnostics = diagnostics;
    }

    pub fn last_frame(&self) -> Option<&PresenterFrame> {
        self.last_frame.as_ref()
    }

    pub fn pointer_dispatch_target(&self, hit_target: Option<ElementId>) -> Option<ElementId> {
        self.pointer_capture_target.or(hit_target)
    }

    pub fn previous_pointer_target(&self, kind: PointerEventKind) -> Option<ElementId> {
        if matches!(kind, PointerEventKind::Move) {
            self.last_pointer_hit_target
        } else {
            None
        }
    }

    pub fn update_pointer_tracking(
        &mut self,
        kind: PointerEventKind,
        stopped: bool,
        dispatch_target: Option<ElementId>,
        hit_target: Option<ElementId>,
    ) {
        match kind {
            PointerEventKind::Down if stopped => {
                self.pointer_capture_target = dispatch_target;
            }
            PointerEventKind::Up => {
                self.pointer_capture_target = None;
            }
            PointerEventKind::Move => {
                self.last_pointer_hit_target = hit_target;
            }
            PointerEventKind::Down => {}
        }
    }
}

impl Presenter<()> {
    /// Constructs the pre-root-ownership presenter surface for source
    /// compatibility. Runtime code that transfers root ownership uses
    /// [`Presenter::with_root`].
    pub fn new(viewport_size: Size) -> Self {
        let mut presenter = Self::with_root(viewport_size, ());
        presenter.legacy_viewport_size = Some(viewport_size);
        presenter
    }

    pub fn viewport_size(&self) -> Size {
        match self.legacy_viewport_size {
            Some(viewport_size) => viewport_size,
            None => Size::new(self.root_bounds.width(), self.root_bounds.height()),
        }
    }

    pub fn set_viewport_size(&mut self, viewport_size: Size) {
        self.legacy_viewport_size = Some(viewport_size);
    }

    pub fn complete_frame(&mut self) {
        self.prepared_frame = PresenterFrame {
            viewport_size: self.viewport_size(),
            root_bounds: self.root_bounds,
            primitive_count: self.scene.len(),
        };
        self.complete_presented_frame(None);
    }
}

impl<E> Presenter<E>
where
    E: Element,
{
    pub fn rebuild_if_needed<F>(&mut self, context: &mut AppContext, build_root: &mut F)
    where
        F: FnMut(&mut AppContext) -> E,
    {
        FramePipeline::rebuild_if_needed(context, &mut self.root, build_root);
    }

    /// Lays the presented tree out and stores the resulting root bounds.
    pub fn layout(&mut self, viewport_size: Size) -> Result<Bounds, FramePipelineError> {
        let root_bounds = FramePipeline::layout_root_with_text_measurer(
            &mut self.root,
            &mut self.taffy,
            &mut self.text_measurer,
            viewport_size,
        )?;
        self.root_bounds = root_bounds;
        self.laid_out_viewport = viewport_size;
        Ok(root_bounds)
    }

    /// Paints the presented tree into the scene using the stored root bounds.
    pub fn paint(&mut self) {
        FramePipeline::paint_root(
            &mut self.root,
            &self.taffy,
            &mut self.scene,
            self.root_bounds,
        );
        self.prepared_frame = PresenterFrame {
            viewport_size: self.laid_out_viewport,
            root_bounds: self.root_bounds,
            primitive_count: self.scene.len(),
        };
    }

    pub fn build_frame<F>(
        &mut self,
        context: &mut AppContext,
        build_root: &mut F,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        F: FnMut(&mut AppContext) -> E,
    {
        let root_bounds = FramePipeline::build_frame_with_text_measurer(
            context,
            &mut self.root,
            build_root,
            &mut self.taffy,
            &mut self.scene,
            &mut self.text_measurer,
            viewport_size,
        )?;
        self.root_bounds = root_bounds;
        self.laid_out_viewport = viewport_size;
        self.prepared_frame = PresenterFrame {
            viewport_size,
            root_bounds,
            primitive_count: self.scene.len(),
        };
        Ok(root_bounds)
    }

    /// Runs `dispatch` against the presented tree with an event context built
    /// from the presenter's own root bounds, layout tree and focus, and reports
    /// whether the tree asked for a redraw.
    pub fn with_event_context<R>(
        &mut self,
        dispatch: impl FnOnce(&mut E, &mut EventContext<'_>) -> R,
    ) -> (R, bool) {
        let mut event_cx =
            EventContext::new(self.root_bounds, &self.taffy, &mut self.focused_element);
        let value = dispatch(&mut self.root, &mut event_cx);
        (value, event_cx.redraw_requested())
    }

    pub fn dispatch_pointer_event(&mut self, event: &PointerEvent) -> PointerDispatch {
        let hit_target = self.hit_test(event.position);
        let dispatch_target = self.pointer_dispatch_target(hit_target);
        let previous_target = self.previous_pointer_target(event.kind);

        let (stopped, redraw_requested) = self.with_event_context(|root, event_cx| {
            event_cx.set_hit_target(dispatch_target);
            event_cx.set_previous_hit_target(previous_target);
            root.dispatch_pointer_event(event_cx, event).is_stopped()
        });

        self.update_pointer_tracking(event.kind, stopped, dispatch_target, hit_target);

        PointerDispatch {
            stopped,
            redraw_requested,
        }
    }

    pub fn handle_window_event(&mut self, event: &Event) -> bool {
        self.root.handle_window_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::elements::div;
    use crate::elements::element::{LayoutContext, PaintContext};
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::prelude::{Dimension, NodeId};

    /// Test element that registers a hit region and claims pointer presses, so
    /// the presenter's capture bookkeeping can be exercised. No shipped element
    /// registers hit regions today, so a probe is the only way to reach this
    /// path from a unit test.
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
            let mut style = taffy::Style::default();
            style.size = taffy::Size {
                width: Dimension::Length(self.size.width),
                height: Dimension::Length(self.size.height),
            };
            match cx.taffy.new_leaf(style) {
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
}
