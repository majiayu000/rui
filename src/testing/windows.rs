//! Headless runner for applications with more than one window.

use super::HeadlessError;
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityTree};
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::KeyEvent;
use crate::core::event::MouseButton;
use crate::core::frame_pipeline::{FramePipeline, FramePresentation, IdlePolicy};
use crate::core::geometry::{Point, Size};
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::core::view::View;
use crate::core::window::{WindowId, WindowOptions};
use crate::elements::element::{Element, PointerEvent, PointerEventKind};
use crate::renderer::Scene;

/// One window's presenter and build function, behind the type the runner stores.
pub(crate) trait HeadlessWindowDriver {
    /// Run one frame for the active window. Returns whether it was drawn.
    fn frame(
        &mut self,
        cx: &mut AppContext,
        idle_policy: IdlePolicy,
        pointer: Option<PointerEvent>,
    ) -> Result<bool, HeadlessError>;
    fn dispatch_key_event(
        &mut self,
        cx: &mut AppContext,
        event: &KeyEvent,
    ) -> Result<bool, HeadlessError>;
    fn dispatch_text_input_event(
        &mut self,
        cx: &mut AppContext,
        event: &TextInputEvent,
    ) -> Result<bool, HeadlessError>;
    fn accessibility_tree(&self) -> Result<AccessibilityTree, HeadlessError>;
    fn scene(&self) -> &Scene;
    fn focused_element(&self) -> Option<ElementId>;
}

struct HeadlessWindow<F, E> {
    build_root: F,
    presenter: Presenter<E>,
}

pub(crate) fn headless_window<F, E>(
    cx: &mut AppContext,
    mut build_root: F,
) -> Box<dyn HeadlessWindowDriver>
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let root = build_root(cx);
    // The first frame draws this tree instead of building it again.
    cx.frame.needs_rebuild = false;
    cx.frame.dirty = true;
    Box::new(HeadlessWindow {
        build_root,
        presenter: Presenter::with_root(cx.viewport_size(), root),
    })
}

impl<F, E> HeadlessWindow<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn dispatch_frame(
        &mut self,
        cx: &mut AppContext,
        dispatch: impl FnOnce(&mut Presenter<E>, &mut AppContext) -> bool,
    ) -> Result<bool, HeadlessError> {
        let mut handled = false;
        let viewport = cx.viewport_size();
        FramePipeline::run_frame(
            cx,
            &mut self.presenter,
            &mut self.build_root,
            viewport,
            IdlePolicy::AlwaysDraw,
            |presenter, cx| {
                handled = dispatch(presenter, cx);
                if cx.consume_runtime_view_notification() {
                    cx.mark_redraw_from(RedrawSource::ViewNotification);
                }
                Ok(())
            },
            |_, _| Ok(FramePresentation::Presented(None)),
        )
        .map_err(|err| HeadlessError::Layout {
            message: err.to_string(),
        })?;
        Ok(handled)
    }
}

impl<F, E> HeadlessWindowDriver for HeadlessWindow<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn frame(
        &mut self,
        cx: &mut AppContext,
        idle_policy: IdlePolicy,
        pointer: Option<PointerEvent>,
    ) -> Result<bool, HeadlessError> {
        let viewport_size = cx.viewport_size();
        let outcome = FramePipeline::run_frame(
            cx,
            &mut self.presenter,
            &mut self.build_root,
            viewport_size,
            idle_policy,
            |presenter, cx| {
                if let Some(event) = pointer
                    && presenter.dispatch_pointer_event(&event).redraw_requested
                {
                    cx.mark_redraw_from(RedrawSource::Element);
                }
                if cx.consume_runtime_view_notification() {
                    cx.mark_redraw_from(RedrawSource::ViewNotification);
                }
                Ok(())
            },
            |_, _| Ok(FramePresentation::Presented(None)),
        )
        .map_err(|err| HeadlessError::Layout {
            message: err.to_string(),
        })?;
        Ok(outcome.is_some())
    }

    fn dispatch_key_event(
        &mut self,
        cx: &mut AppContext,
        event: &KeyEvent,
    ) -> Result<bool, HeadlessError> {
        self.dispatch_frame(cx, |presenter, cx| {
            let (handled, redraw) = presenter
                .with_event_context(|root, event_cx| route_key_event(root, cx, event_cx, event));
            if handled || redraw {
                cx.mark_redraw_from(RedrawSource::PlatformInput);
            }
            handled
        })
    }

    fn dispatch_text_input_event(
        &mut self,
        cx: &mut AppContext,
        event: &TextInputEvent,
    ) -> Result<bool, HeadlessError> {
        self.dispatch_frame(cx, |presenter, cx| {
            let (handled, redraw) = presenter
                .with_event_context(|root, event_cx| root.handle_text_input_event(event_cx, event));
            if handled || redraw {
                cx.mark_redraw_from(RedrawSource::PlatformInput);
            }
            handled
        })
    }

    fn accessibility_tree(&self) -> Result<AccessibilityTree, HeadlessError> {
        let nodes = self
            .presenter
            .root()
            .accessibility_nodes(&AccessibilityContext::new(self.presenter.focused_element()))?;
        Ok(AccessibilityTree::new(nodes))
    }

    fn scene(&self) -> &Scene {
        self.presenter.scene()
    }

    fn focused_element(&self) -> Option<ElementId> {
        self.presenter.focused_element()
    }
}

struct OpenWindow {
    id: WindowId,
    driver: Box<dyn HeadlessWindowDriver>,
    frames_drawn: u64,
}

/// A headless application with any number of windows sharing one [`AppContext`].
///
/// [`Self::frame`] runs one pass of the native event loop: each window draws only
/// when it has something to draw.
pub struct HeadlessApp {
    context: AppContext,
    windows: Vec<OpenWindow>,
}

/// Start a headless application with no windows yet.
pub fn headless_app() -> HeadlessApp {
    let mut context = AppContext::new();
    context.running = true;
    HeadlessApp {
        context,
        windows: Vec::new(),
    }
}

impl HeadlessApp {
    pub fn app_context(&self) -> &AppContext {
        &self.context
    }

    pub fn app_context_mut(&mut self) -> &mut AppContext {
        &mut self.context
    }

    /// Open a window showing `build_root` and draw its first frame.
    pub fn open_window<F, E>(
        &mut self,
        options: WindowOptions,
        build_root: F,
    ) -> Result<WindowId, HeadlessError>
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        let id = self.context.open_window(options, build_root);
        self.frame()?;
        Ok(id)
    }

    /// Open a window showing a long-lived stateful view and draw its first frame.
    pub fn open_view_window<V>(
        &mut self,
        options: WindowOptions,
        view: V,
    ) -> Result<WindowId, HeadlessError>
    where
        V: View,
        V::Element: Element + 'static,
    {
        let id = self.context.open_view_window(options, view);
        self.frame()?;
        Ok(id)
    }

    /// Run spawned tasks, background work and updates from other threads until
    /// none can make progress; see `HeadlessSession::run_until_parked`.
    pub fn run_until_parked(&mut self) {
        self.context.run_until_parked();
    }

    /// Close a window, as its close button would. Returns whether it was open.
    pub fn close_window(&mut self, window: WindowId) -> bool {
        let closed = self.context.close_window(window);
        self.sync_windows();
        closed
    }

    /// The open windows, in the order they were opened.
    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    /// Run one pass of the event loop: create windows app code opened, drop
    /// closed ones, and draw each window that has work. Returns the windows drawn.
    pub fn frame(&mut self) -> Result<Vec<WindowId>, HeadlessError> {
        self.sync_windows();
        let mut drawn = Vec::new();
        for index in 0..self.windows.len() {
            let id = self.windows[index].id;
            if !self.context.activate_window(id) {
                continue;
            }
            if self.windows[index].driver.frame(
                &mut self.context,
                IdlePolicy::SkipWhenIdle,
                None,
            )? {
                self.windows[index].frames_drawn += 1;
                drawn.push(id);
            }
        }
        self.sync_windows();
        Ok(drawn)
    }

    /// Press and release the left button at `position` in `window`, one frame each.
    pub fn click(&mut self, window: WindowId, position: Point) -> Result<(), HeadlessError> {
        for kind in [PointerEventKind::Down, PointerEventKind::Up] {
            self.pointer_event(
                window,
                PointerEvent {
                    kind,
                    position,
                    button: Some(MouseButton::Left),
                },
            )?;
        }
        Ok(())
    }

    /// Deliver a pointer event to `window` in a frame of its own.
    pub fn pointer_event(
        &mut self,
        window: WindowId,
        event: PointerEvent,
    ) -> Result<(), HeadlessError> {
        let index = self.activate_window(window)?;
        self.windows[index]
            .driver
            .frame(&mut self.context, IdlePolicy::AlwaysDraw, Some(event))?;
        self.windows[index].frames_drawn += 1;
        Ok(())
    }

    /// Route a key through the specified window in a frame of its own.
    pub fn dispatch_key_event(
        &mut self,
        window: WindowId,
        event: &KeyEvent,
    ) -> Result<bool, HeadlessError> {
        let index = self.activate_window(window)?;
        let handled = self.windows[index]
            .driver
            .dispatch_key_event(&mut self.context, event)?;
        self.windows[index].frames_drawn += 1;
        Ok(handled)
    }

    /// Deliver text to the specified window's focused input in a frame of its own.
    pub fn dispatch_text_input_event(
        &mut self,
        window: WindowId,
        event: &TextInputEvent,
    ) -> Result<bool, HeadlessError> {
        let index = self.activate_window(window)?;
        let handled = self.windows[index]
            .driver
            .dispatch_text_input_event(&mut self.context, event)?;
        self.windows[index].frames_drawn += 1;
        Ok(handled)
    }

    /// Resize `window`; it redraws on the next [`Self::frame`].
    pub fn resize(&mut self, window: WindowId, size: Size) -> Result<(), HeadlessError> {
        self.activate_window(window)?;
        self.context.set_viewport_size(size);
        self.context.request_window_rebuild();
        Ok(())
    }

    /// How many frames `window` has drawn.
    pub fn frames_drawn(&self, window: WindowId) -> Result<u64, HeadlessError> {
        Ok(self.windows[self.index_of(window)?].frames_drawn)
    }

    pub fn accessibility_tree(&self, window: WindowId) -> Result<AccessibilityTree, HeadlessError> {
        self.windows[self.index_of(window)?]
            .driver
            .accessibility_tree()
    }

    pub fn scene(&self, window: WindowId) -> Result<&Scene, HeadlessError> {
        Ok(self.windows[self.index_of(window)?].driver.scene())
    }

    pub fn focused_element(&self, window: WindowId) -> Result<Option<ElementId>, HeadlessError> {
        Ok(self.windows[self.index_of(window)?]
            .driver
            .focused_element())
    }

    fn activate_window(&mut self, window: WindowId) -> Result<usize, HeadlessError> {
        // App code can close windows directly through app_context_mut().
        self.sync_windows();
        let index = self.index_of(window)?;
        self.context.activate_window(window);
        Ok(index)
    }

    fn index_of(&self, window: WindowId) -> Result<usize, HeadlessError> {
        self.windows
            .iter()
            .position(|open| open.id == window)
            .ok_or_else(|| HeadlessError::Layout {
                message: format!("{window:?} is not open"),
            })
    }

    fn sync_windows(&mut self) {
        for closed in self.context.take_closed_windows() {
            self.windows.retain(|open| open.id != closed);
        }
        for pending in self.context.take_pending_windows() {
            if !self.context.activate_window(pending.id) {
                continue;
            }
            let driver = pending.content.into_headless(&mut self.context);
            self.windows.push(OpenWindow {
                id: pending.id,
                driver,
                frames_drawn: 0,
            });
        }
    }
}
