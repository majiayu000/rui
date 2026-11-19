mod frame_capture;
mod snapshot;
mod windows;

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityError, AccessibilityTree};
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RuntimeView};
use crate::core::drag::FileDragEvent;
use crate::core::entity::Entity;
use crate::core::event::{Event, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::frame_pipeline::{FramePipeline, FramePresentation, IdlePolicy};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::menu::{ContextMenuRequest, Menu};
use crate::core::presenter::{Presenter, PresenterFrame};
use crate::core::prompt::PathPrompt;
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

pub use windows::{HeadlessApp, headless_app};
pub(crate) use windows::{HeadlessWindowDriver, headless_window};

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

/// Build `build_root`'s tree for a window `viewport_size` large and draw its
/// first frame. The root is built once; the views it places render before
/// that frame lays the tree out, as in a native window.
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

/// [`mount`] with a long-lived stateful view as the root.
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
    // The first frame draws this tree instead of building it again.
    let root = FramePipeline::build_first_tree(&mut context, &mut build_root);

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

    /// The cursor the window shows, as of the last pointer move.
    pub fn cursor(&self) -> crate::core::event::Cursor {
        self.presenter.cursor()
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

    /// The window's profiler, while the profiling overlay is on.
    pub fn profiler(&self) -> Option<&crate::devtools::Profiler> {
        self.presenter.profiler()
    }

    pub fn inspector(&self) -> Option<&crate::devtools::Inspector> {
        self.presenter.inspector()
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

    /// Draw the frame the display would drive next for the animations that asked.
    ///
    /// Returns `None` without drawing when no animation asked, which is when a
    /// native window goes idle.
    pub fn animation_frame(&mut self) -> Result<Option<&HeadlessFrame>, HeadlessError> {
        if !self.context.begin_animation_frame() {
            return Ok(None);
        }
        self.frame().map(Some)
    }

    /// When the earliest frame a view asked for with
    /// [`crate::core::ViewContext::request_frame_at`] is due.
    pub fn next_frame_deadline(&self) -> Option<std::time::Instant> {
        self.context.next_frame_deadline()
    }

    /// Draw the frame a native runner wakes for when a requested frame time
    /// has passed. Returns `None` without drawing when none is due yet.
    pub fn timed_frame(&mut self) -> Result<Option<&HeadlessFrame>, HeadlessError> {
        if !self.context.fire_due_frames(std::time::Instant::now()) {
            return Ok(None);
        }
        self.frame().map(Some)
    }

    /// Run spawned tasks, background work and updates from other threads until
    /// none can make progress. Background tasks run here, on this thread, in the
    /// order they became ready; a following [`Self::frame`] draws the result.
    pub fn run_until_parked(&mut self) {
        self.context.run_until_parked();
    }

    /// Run all queued idle work, as the native runner does between frames once
    /// input and frame work are done. Returns how many tasks ran.
    pub fn run_idle_work(&mut self) -> usize {
        self.context.run_idle_tasks(|| true)
    }

    fn dispatch_frame<R>(
        &mut self,
        dispatch: impl FnOnce(&mut Presenter<E>, &mut AppContext) -> R,
    ) -> R {
        let viewport_size = self.context.viewport_size();
        let mut result = None;
        let outcome = FramePipeline::run_frame(
            &mut self.context,
            &mut self.presenter,
            &mut self.build_root,
            viewport_size,
            IdlePolicy::AlwaysDraw,
            |presenter, context| {
                result = Some(dispatch(presenter, context));
                Ok(())
            },
            |_, _| Ok(FramePresentation::Presented(None)),
        );
        match outcome {
            Ok(Some(_)) => {}
            Ok(None) => panic!("headless input frame was skipped despite AlwaysDraw"),
            Err(err) => panic!("headless input frame failed: {err}"),
        }
        match result {
            Some(result) => result,
            None => panic!("headless input frame did not run its dispatch stage"),
        }
    }

    pub fn resize(&mut self, viewport_size: Size) -> bool {
        self.context.set_viewport_size(viewport_size);
        self.context.request_window_rebuild();
        FramePipeline::prepare_frame(&mut self.context);
        self.presenter
            .rebuild_if_needed(&mut self.context, &mut self.build_root);
        self.presenter.handle_window_event(&Event::WindowResize {
            width: viewport_size.width,
            height: viewport_size.height,
        })
    }

    pub fn dispatch_pointer_event(&mut self, event: PointerEvent) -> bool {
        self.dispatch_pointer_event_with(event, Modifiers::none())
    }

    /// [`Self::dispatch_pointer_event`] with `modifiers` held, as a
    /// shift-click is.
    pub fn dispatch_pointer_event_with(
        &mut self,
        event: PointerEvent,
        modifiers: Modifiers,
    ) -> bool {
        self.dispatch_frame(move |presenter, context| {
            let dispatch = presenter.dispatch_pointer_event_with(&event, modifiers);
            if dispatch.redraw_requested {
                context.request_redraw();
            }
            dispatch.stopped
        })
    }

    /// Drive a file drag from another application, as the platform would.
    pub fn dispatch_file_drag(&mut self, event: &FileDragEvent) -> bool {
        self.dispatch_frame(|presenter, context| {
            let dispatch = presenter.dispatch_file_drag(event);
            if dispatch.redraw_requested {
                context.request_redraw();
            }
            dispatch.stopped
        })
    }

    /// Choose the item labelled `label` in `menu` or its submenus, as a click
    /// on it in a native menu does: its action is routed to the focused
    /// element, then the app's handlers. Returns false, choosing nothing, when
    /// there is no such item or it is disabled.
    pub fn choose_menu_item(&mut self, menu: &Menu, label: &str) -> bool {
        let Some(item) = menu.find(label).filter(|item| item.is_enabled()) else {
            return false;
        };
        self.context.queue_menu_action(item.action().clone());
        self.dispatch_frame(|presenter, context| {
            if presenter.dispatch_menu_actions(context) {
                context.request_redraw();
            }
        });
        true
    }

    /// The context menu the app asked to show, which a platform would show now.
    pub fn take_context_menu(&mut self) -> Option<ContextMenuRequest> {
        self.context.take_context_menu()
    }

    /// The file dialogs the app asked for since the last call, which a
    /// platform would show now. Answer each with its `answer`; one dropped
    /// unanswered fails its task with `PromptDismissed`.
    pub fn take_path_prompts(&mut self) -> Vec<PathPrompt> {
        self.context.take_path_prompts()
    }

    /// Whether a drag in this window is past its threshold.
    pub fn is_dragging(&self) -> bool {
        self.presenter.is_dragging()
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
        self.dispatch_frame(|presenter, context| {
            let (handled, redraw_requested) = presenter
                .with_event_context(|root, event_cx| root.handle_scroll_event(event_cx, event));
            if redraw_requested {
                context.request_redraw();
            }
            handled
        })
    }

    pub fn dispatch_key_event(&mut self, event: &KeyEvent) -> bool {
        self.dispatch_frame(|presenter, context| {
            let (handled, redraw_requested) = presenter.with_event_context(|root, event_cx| {
                route_key_event(root, context, event_cx, event)
            });
            if handled || redraw_requested {
                context.request_redraw();
            }
            handled
        })
    }

    pub fn request_focus(&mut self, id: Option<ElementId>) {
        self.presenter.set_focused_element(id);
        self.context.request_redraw();
    }

    /// Deliver a window event to the tree. [`Event::Blur`] and
    /// [`Event::Focus`] are the window losing and getting back key status:
    /// as in a native runner, they run in a frame of their own and blur the
    /// focused element, then focus it again. Returns whether the tree
    /// handled the event.
    pub fn dispatch_window_event(&mut self, event: &Event) -> bool {
        match event {
            Event::WindowResize { width, height } => self.resize(Size::new(*width, *height)),
            Event::Focus(_) | Event::Blur(_) => {
                let focused = matches!(event, Event::Focus(_));
                self.dispatch_frame(|presenter, _| presenter.handle_window_focus(focused))
            }
            _ => self.presenter.handle_window_event(event),
        }
    }

    /// The tree a platform bridge would publish, with each node's bounds.
    pub fn accessibility_tree(&self) -> Result<AccessibilityTree, HeadlessError> {
        Ok(self.presenter.accessibility_tree()?)
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
