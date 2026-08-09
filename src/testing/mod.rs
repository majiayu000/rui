mod frame_capture;
mod snapshot;

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityError, AccessibilityTree};
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RuntimeView};
use crate::core::entity::Entity;
use crate::core::event::{Event, KeyEvent, MouseButton, ScrollEvent};
use crate::core::frame_pipeline::FramePipeline;
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::presenter::{Presenter, PresenterFrame};
use crate::core::view::{View, ViewNotifier};
use crate::elements::element::{Element, EventContext, PointerEvent, PointerEventKind};
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
    root: E,
    presenter: Presenter,
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
        root,
        presenter: Presenter::new(viewport_size),
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
        self.presenter.viewport_size()
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
        let viewport_size = self.presenter.viewport_size();
        let (taffy, scene, text_measurer) = self.presenter.frame_surfaces_mut();
        let root_bounds = FramePipeline::build_frame(
            &mut self.context,
            &mut self.root,
            &mut self.build_root,
            taffy,
            scene,
            text_measurer,
            viewport_size,
        )
        .map_err(|err| HeadlessError::Layout {
            message: err.to_string(),
        })?;
        self.presenter.set_root_bounds(root_bounds);
        self.presenter.complete_frame();

        match self.presenter.last_frame() {
            Some(frame) => Ok(frame),
            None => Err(HeadlessError::Layout {
                message: String::from("headless frame was not recorded"),
            }),
        }
    }

    pub fn resize(&mut self, viewport_size: Size) -> bool {
        self.presenter.set_viewport_size(viewport_size);
        self.context.set_viewport_size(viewport_size);
        self.context.request_rebuild();
        FramePipeline::prepare_frame(&mut self.context);
        FramePipeline::rebuild_if_needed(&mut self.context, &mut self.root, &mut self.build_root);
        let handled = self.root.handle_window_event(&Event::WindowResize {
            width: viewport_size.width,
            height: viewport_size.height,
        });
        handled
    }

    pub fn dispatch_pointer_event(&mut self, event: PointerEvent) -> bool {
        let hit_target = self.presenter.hit_test(event.position);
        let dispatch_target = self.presenter.pointer_dispatch_target(hit_target);
        let previous_target = self.presenter.previous_pointer_target(event.kind);

        let (stopped, redraw_requested) = {
            let (root_bounds, taffy, focused_element) = self.presenter.event_context_parts_mut();
            let mut event_cx = EventContext::new(root_bounds, taffy, focused_element);
            event_cx.set_hit_target(dispatch_target);
            event_cx.set_previous_hit_target(previous_target);

            let result = self.root.dispatch_pointer_event(&mut event_cx, &event);
            (result.is_stopped(), event_cx.redraw_requested())
        };

        self.presenter
            .update_pointer_tracking(event.kind, stopped, dispatch_target, hit_target);

        if redraw_requested {
            self.context.request_redraw();
        }

        stopped
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
        let (handled, redraw_requested) = {
            let (root_bounds, taffy, focused_element) = self.presenter.event_context_parts_mut();
            let mut event_cx = EventContext::new(root_bounds, taffy, focused_element);
            let handled = self.root.handle_scroll_event(&mut event_cx, event);
            (handled, event_cx.redraw_requested())
        };
        if redraw_requested {
            self.context.request_redraw();
        }
        handled
    }

    pub fn dispatch_key_event(&mut self, event: &KeyEvent) -> bool {
        let (handled, redraw_requested) = {
            let (root_bounds, taffy, focused_element) = self.presenter.event_context_parts_mut();
            let mut event_cx = EventContext::new(root_bounds, taffy, focused_element);
            let handled = route_key_event(&mut self.root, &mut self.context, &mut event_cx, event);
            (handled, event_cx.redraw_requested())
        };
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
            _ => self.root.handle_window_event(event),
        }
    }

    pub fn accessibility_tree(&self) -> Result<AccessibilityTree, HeadlessError> {
        let nodes = self
            .root
            .accessibility_nodes(&AccessibilityContext::new(self.presenter.focused_element()))?;
        Ok(AccessibilityTree::new(nodes))
    }

    pub fn primitive_snapshot(&self) -> Result<PrimitiveSnapshot, PrimitiveSnapshotError> {
        primitive_snapshot(self.presenter.scene().primitives())
    }

    pub fn record_frame(&self) -> Result<RecordedScene, HeadlessError> {
        let mut renderer = RecordingRenderer::new();
        renderer.render(self.presenter.scene(), &(), self.presenter.viewport_size())?;
        match renderer.frames().first() {
            Some(frame) => Ok(frame.clone()),
            None => Err(HeadlessError::Renderer(RendererError::render_failed(
                "recording renderer did not capture a frame",
            ))),
        }
    }

    pub fn capture_current_frame(&self) -> Result<CapturedFrame, HeadlessError> {
        let mut backend = MissingFrameCaptureBackend;
        Ok(capture_frame_with_backend(
            &mut backend,
            self.presenter.scene(),
            self.presenter.viewport_size(),
        )?)
    }
}
