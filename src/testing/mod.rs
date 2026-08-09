mod frame_capture;
mod snapshot;

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityError, AccessibilityTree};
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RuntimeView};
use crate::core::entity::Entity;
use crate::core::event::{Event, KeyEvent, MouseButton, ScrollEvent};
use crate::core::frame_pipeline::{FramePipeline, FramePresentation, IdlePolicy};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::presenter::{Presenter, PresenterFrame};
use crate::core::view::{View, ViewNotifier};
use crate::elements::element::{Element, PointerEvent, PointerEventKind};
use crate::renderer::{RecordedScene, RecordingRenderer, Renderer, RendererError, Scene};
use std::error::Error;
use std::fmt;

#[cfg(all(target_os = "macos", feature = "metal"))]
pub use frame_capture::MetalFrameCaptureBackend;
pub use frame_capture::{
    CapturedFrame, FrameCaptureBackend, MissingFrameCaptureBackend, capture_frame_with_backend,
};
pub use snapshot::{
    PrimitiveSnapshot, PrimitiveSnapshotError, assert_primitive_snapshot_file,
    assert_primitive_snapshot_text, primitive_snapshot,
};

pub type HeadlessFrame = PresenterFrame;

#[derive(Debug, Clone, PartialEq)]
pub enum HeadlessError {
    Layout { message: String },
    Accessibility(AccessibilityError),
    Renderer(RendererError),
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout { message } => write!(f, "headless layout failed: {message}"),
            Self::Accessibility(err) => write!(f, "{err}"),
            Self::Renderer(err) => write!(f, "{err}"),
        }
    }
}

impl Error for HeadlessError {}

impl From<AccessibilityError> for HeadlessError {
    fn from(value: AccessibilityError) -> Self {
        Self::Accessibility(value)
    }
}

impl From<RendererError> for HeadlessError {
    fn from(value: RendererError) -> Self {
        Self::Renderer(value)
    }
}

pub struct HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    context: AppContext,
    build_root: F,
    presenter: Presenter<E>,
}

pub type HeadlessViewBuilder<V> = Box<dyn FnMut(&mut AppContext) -> <V as View>::Element>;
pub type HeadlessViewSession<V> = HeadlessSession<HeadlessViewBuilder<V>, <V as View>::Element>;

pub fn mount<F, E>(
    viewport_size: Size,
    build_root: F,
) -> Result<HeadlessSession<F, E>, HeadlessError>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    let mut context = AppContext::new();
    context.running = true;
    mount_with_context(context, viewport_size, build_root)
}

pub fn mount_view<V>(viewport_size: Size, view: V) -> Result<HeadlessViewSession<V>, HeadlessError>
where
    V: View,
    V::Element: Element,
{
    let mut context = AppContext::new();
    context.running = true;

    let view_marker = context.create(());
    let view_entity = Entity::<V>::new(view_marker.id());
    let notifier = ViewNotifier::new();
    context.set_runtime_view_notifier(view_entity.id(), notifier.clone());

    let mut runtime_view = RuntimeView::new(view_entity, notifier, view);
    let build_root: HeadlessViewBuilder<V> = Box::new(move |context| runtime_view.render(context));
    mount_with_context(context, viewport_size, build_root)
}

fn mount_with_context<F, E>(
    mut context: AppContext,
    viewport_size: Size,
    mut build_root: F,
) -> Result<HeadlessSession<F, E>, HeadlessError>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    context.set_viewport_size(viewport_size);
    let root = build_root(&mut context);
    context.needs_rebuild = false;
    context.dirty = true;

    let mut session = HeadlessSession {
        context,
        build_root,
        presenter: Presenter::with_root(viewport_size, root),
    };
    session.frame()?;
    Ok(session)
}

impl<F, E> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    pub fn app_context(&self) -> &AppContext {
        &self.context
    }

    pub fn app_context_mut(&mut self) -> &mut AppContext {
        &mut self.context
    }

    pub fn viewport_size(&self) -> Size {
        self.context.viewport_size()
    }

    pub fn root_bounds(&self) -> Bounds {
        self.presenter.root_bounds()
    }

    pub fn focused_element(&self) -> Option<ElementId> {
        self.presenter.focused_element()
    }

    pub fn last_frame(&self) -> Option<&HeadlessFrame> {
        self.presenter.last_frame()
    }

    pub fn primitives(&self) -> &[crate::renderer::Primitive] {
        self.presenter.scene().primitives()
    }

    pub fn scene(&self) -> &Scene {
        self.presenter.scene()
    }

    pub fn frame(&mut self) -> Result<&HeadlessFrame, HeadlessError> {
        let viewport_size = self.context.viewport_size();
        // Headless supplies no events and no backend, but runs the same stage
        // order as the native runner.
        let outcome = FramePipeline::run_frame(
            &mut self.context,
            &mut self.presenter,
            &mut self.build_root,
            viewport_size,
            IdlePolicy::AlwaysDraw,
            |_, _| Ok(()),
            |_, _| Ok(FramePresentation::Presented(None)),
        )
        .map_err(|err| HeadlessError::Layout {
            message: err.to_string(),
        })?;
        if outcome.is_none() {
            return Err(HeadlessError::Layout {
                message: String::from("headless frame was skipped despite AlwaysDraw"),
            });
        }

        match self.presenter.last_frame() {
            Some(frame) => Ok(frame),
            None => Err(HeadlessError::Layout {
                message: String::from("headless frame was not recorded"),
            }),
        }
    }

    pub fn resize(&mut self, viewport_size: Size) -> bool {
        self.context.set_viewport_size(viewport_size);
        self.context.request_rebuild();
        FramePipeline::prepare_frame(&mut self.context);
        self.presenter
            .rebuild_if_needed(&mut self.context, &mut self.build_root);
        self.presenter.handle_window_event(&Event::WindowResize {
            width: viewport_size.width,
            height: viewport_size.height,
        })
    }

    pub fn dispatch_pointer_event(&mut self, event: PointerEvent) -> bool {
        let dispatch = self.presenter.dispatch_pointer_event(&event);
        if dispatch.redraw_requested {
            self.context.request_redraw();
        }
        dispatch.stopped
    }

    pub fn pointer_move(&mut self, position: Point) -> bool {
        self.dispatch_pointer_event(PointerEvent {
            kind: PointerEventKind::Move,
            position,
            button: None,
        })
    }

    pub fn pointer_down(&mut self, position: Point) -> bool {
        self.dispatch_pointer_event(PointerEvent {
            kind: PointerEventKind::Down,
            position,
            button: Some(MouseButton::Left),
        })
    }

    pub fn pointer_up(&mut self, position: Point) -> bool {
        self.dispatch_pointer_event(PointerEvent {
            kind: PointerEventKind::Up,
            position,
            button: Some(MouseButton::Left),
        })
    }

    pub fn dispatch_scroll_event(&mut self, event: &ScrollEvent) -> bool {
        let (handled, redraw_requested) = self
            .presenter
            .with_event_context(|root, event_cx| root.handle_scroll_event(event_cx, event));
        if redraw_requested {
            self.context.request_redraw();
        }
        handled
    }

    pub fn dispatch_key_event(&mut self, event: &KeyEvent) -> bool {
        let context = &mut self.context;
        let (handled, redraw_requested) = self
            .presenter
            .with_event_context(|root, event_cx| route_key_event(root, context, event_cx, event));
        if handled || redraw_requested {
            self.context.request_redraw();
        }
        handled
    }

    pub fn request_focus(&mut self, id: Option<ElementId>) {
        self.presenter.set_focused_element(id);
        self.context.request_redraw();
    }

    pub fn dispatch_window_event(&mut self, event: &Event) -> bool {
        match event {
            Event::WindowResize { width, height } => self.resize(Size::new(*width, *height)),
            _ => self.presenter.handle_window_event(event),
        }
    }

    pub fn accessibility_tree(&self) -> Result<AccessibilityTree, HeadlessError> {
        let nodes = self
            .presenter
            .root()
            .accessibility_nodes(&AccessibilityContext::new(self.presenter.focused_element()))?;
        Ok(AccessibilityTree::new(nodes))
    }

    pub fn primitive_snapshot(&self) -> Result<PrimitiveSnapshot, PrimitiveSnapshotError> {
        primitive_snapshot(self.presenter.scene().primitives())
    }

    pub fn record_frame(&self) -> Result<RecordedScene, HeadlessError> {
        let mut renderer = RecordingRenderer::new();
        let (scene, prepared_frame) = self.presenter.prepared_scene();
        renderer.render(scene, &(), prepared_frame.viewport_size)?;
        match renderer.frames().first() {
            Some(frame) => Ok(frame.clone()),
            None => Err(HeadlessError::Renderer(RendererError::render_failed(
                "recording renderer did not capture a frame",
            ))),
        }
    }

    pub fn capture_current_frame(&self) -> Result<CapturedFrame, HeadlessError> {
        let mut backend = MissingFrameCaptureBackend;
        let (scene, prepared_frame) = self.presenter.prepared_scene();
        Ok(capture_frame_with_backend(
            &mut backend,
            scene,
            prepared_frame.viewport_size,
        )?)
    }
}
