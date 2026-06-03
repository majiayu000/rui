mod frame_capture;
mod snapshot;

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityError, AccessibilityTree};
use crate::core::app::{AppContext, RuntimeView};
use crate::core::entity::Entity;
use crate::core::event::{Event, KeyEvent, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::view::{View, ViewNotifier};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use crate::renderer::{RecordedScene, RecordingRenderer, Renderer, RendererError, Scene};
use std::error::Error;
use std::fmt;
use taffy::prelude::{AvailableSpace, TaffyTree};

#[cfg(target_os = "macos")]
pub use frame_capture::MetalFrameCaptureBackend;
pub use frame_capture::{
    CapturedFrame, FrameCaptureBackend, MissingFrameCaptureBackend, capture_frame_with_backend,
};
pub use snapshot::{
    PrimitiveSnapshot, PrimitiveSnapshotError, assert_primitive_snapshot_file,
    assert_primitive_snapshot_text, primitive_snapshot,
};

#[derive(Debug, Clone, PartialEq)]
pub struct HeadlessFrame {
    pub viewport_size: Size,
    pub root_bounds: Bounds,
    pub primitive_count: usize,
}

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
    viewport_size: Size,
    taffy: TaffyTree<ElementId>,
    scene: Scene,
    root_bounds: Bounds,
    focused_element: Option<ElementId>,
    last_pointer_hit_target: Option<ElementId>,
    pointer_capture_target: Option<ElementId>,
    last_frame: Option<HeadlessFrame>,
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
    let root = build_root(&mut context);
    context.needs_rebuild = false;
    context.dirty = true;

    let mut session = HeadlessSession {
        context,
        build_root,
        root,
        viewport_size,
        taffy: TaffyTree::new(),
        scene: Scene::new(),
        root_bounds: Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height),
        focused_element: None,
        last_pointer_hit_target: None,
        pointer_capture_target: None,
        last_frame: None,
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
        self.viewport_size
    }

    pub fn root_bounds(&self) -> Bounds {
        self.root_bounds
    }

    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    pub fn last_frame(&self) -> Option<&HeadlessFrame> {
        self.last_frame.as_ref()
    }

    pub fn primitives(&self) -> &[crate::renderer::Primitive] {
        self.scene.primitives()
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn frame(&mut self) -> Result<&HeadlessFrame, HeadlessError> {
        if self.context.consume_runtime_view_notification() {
            self.context.request_redraw();
        }

        if self.context.needs_rebuild || !self.context.pending_updates.is_empty() {
            self.context.pending_updates.clear();
            self.context.needs_rebuild = false;
            self.root = (self.build_root)(&mut self.context);
            if !self.context.pending_updates.is_empty() {
                self.context.needs_rebuild = true;
            }
        }

        self.taffy.clear();
        let mut layout_cx = LayoutContext::new(&mut self.taffy, self.viewport_size);
        let root_node = self.root.layout(&mut layout_cx);

        self.taffy
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(self.viewport_size.width),
                    height: AvailableSpace::Definite(self.viewport_size.height),
                },
            )
            .map_err(|err| HeadlessError::Layout {
                message: err.to_string(),
            })?;

        let layout = self
            .taffy
            .layout(root_node)
            .map_err(|err| HeadlessError::Layout {
                message: err.to_string(),
            })?;
        self.root_bounds = Bounds::from_xywh(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        );

        self.scene.clear();
        let mut paint_cx = PaintContext::new(&mut self.scene, self.root_bounds, &self.taffy);
        self.root.paint(&mut paint_cx);
        self.scene.finish();

        self.context.complete_redraw_frame();

        self.last_frame = Some(HeadlessFrame {
            viewport_size: self.viewport_size,
            root_bounds: self.root_bounds,
            primitive_count: self.scene.len(),
        });

        match &self.last_frame {
            Some(frame) => Ok(frame),
            None => Err(HeadlessError::Layout {
                message: String::from("headless frame was not recorded"),
            }),
        }
    }

    pub fn resize(&mut self, viewport_size: Size) -> bool {
        self.viewport_size = viewport_size;
        let handled = self.root.handle_window_event(&Event::WindowResize {
            width: viewport_size.width,
            height: viewport_size.height,
        });
        self.context.request_redraw();
        handled
    }

    pub fn dispatch_pointer_event(&mut self, event: PointerEvent) -> bool {
        let hit_target = self.scene.hit_test(event.position);
        let dispatch_target = self.pointer_capture_target.or(hit_target);
        let previous_target = if matches!(event.kind, PointerEventKind::Move) {
            self.last_pointer_hit_target
        } else {
            None
        };

        let mut event_cx =
            EventContext::new(self.root_bounds, &self.taffy, &mut self.focused_element);
        event_cx.set_hit_target(dispatch_target);
        event_cx.set_previous_hit_target(previous_target);

        let result = self.root.dispatch_pointer_event(&mut event_cx, &event);

        match event.kind {
            PointerEventKind::Down if result.is_stopped() => {
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

        if event_cx.redraw_requested() {
            self.context.request_redraw();
        }

        result.is_stopped()
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
        let mut event_cx =
            EventContext::new(self.root_bounds, &self.taffy, &mut self.focused_element);
        let handled = self.root.handle_scroll_event(&mut event_cx, event);
        if event_cx.redraw_requested() {
            self.context.request_redraw();
        }
        handled
    }

    pub fn dispatch_key_event(&mut self, event: &KeyEvent) -> bool {
        let mut event_cx =
            EventContext::new(self.root_bounds, &self.taffy, &mut self.focused_element);
        let handled = self.root.handle_key_event(&mut event_cx, event);
        if event_cx.redraw_requested() {
            self.context.request_redraw();
        }
        handled
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
            .accessibility_nodes(&AccessibilityContext::new(self.focused_element))?;
        Ok(AccessibilityTree::new(nodes))
    }

    pub fn primitive_snapshot(&self) -> Result<PrimitiveSnapshot, PrimitiveSnapshotError> {
        primitive_snapshot(self.scene.primitives())
    }

    pub fn record_frame(&self) -> Result<RecordedScene, HeadlessError> {
        let mut renderer = RecordingRenderer::new();
        renderer.render(&self.scene, &(), self.viewport_size)?;
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
            &self.scene,
            self.viewport_size,
        )?)
    }
}
