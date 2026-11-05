//! The open windows of an application and which one a frame runs for.
//!
//! Entities are shared: every window builds from the same [`AppContext`]. What is
//! per window is its frame state ([`WindowFrameState`]: what it needs drawn, its
//! viewport, its root view), plus the presenter, scene, focus and renderer target
//! that its runner keeps. A runner activates a window before running its frame,
//! so `cx.viewport_size()` and redraw requests refer to that window.

use crate::core::app::{AppContext, RuntimeView, WindowFrameState};
use crate::core::entity::{Entity, EntityId};
use crate::core::view::{View, ViewNotifier};
use crate::core::window::{Window, WindowId, WindowOptions};
use crate::elements::Element;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

static WINDOW_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Windows {
    open: Vec<Window>,
    active: Option<WindowId>,
    inactive_frames: HashMap<WindowId, WindowFrameState>,
    view_windows: HashMap<EntityId, WindowId>,
    pending: Vec<PendingWindow>,
    closed: Vec<WindowId>,
    quit_when_last_window_closes: bool,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            open: Vec::new(),
            active: None,
            inactive_frames: HashMap::new(),
            view_windows: HashMap::new(),
            pending: Vec::new(),
            closed: Vec::new(),
            quit_when_last_window_closes: true,
        }
    }
}

impl Windows {
    pub(crate) fn window_of_view(&self, entity: EntityId) -> Option<WindowId> {
        self.view_windows.get(&entity).copied()
    }

    pub(crate) fn any_inactive_frame_work(&self) -> bool {
        self.inactive_frames
            .values()
            .any(WindowFrameState::has_frame_work)
    }
}

/// A window opened by app code, waiting for its runner to create it.
pub(crate) struct PendingWindow {
    pub(crate) id: WindowId,
    pub(crate) options: WindowOptions,
    pub(crate) content: Box<dyn WindowContent>,
}

/// What a window renders, before a runner gives it a presenter and a surface.
pub(crate) trait WindowContent {
    /// Build the first tree, for the window active in `cx`.
    fn into_headless(
        self: Box<Self>,
        cx: &mut AppContext,
    ) -> Box<dyn crate::testing::HeadlessWindowDriver>;

    #[cfg(all(target_os = "macos", feature = "metal"))]
    fn into_native(
        self: Box<Self>,
        cx: &mut AppContext,
        parts: crate::platform::mac::NativeWindowParts,
    ) -> Box<dyn crate::platform::mac::NativeWindowDriver>;
}

struct RootContent<F, E> {
    build_root: F,
    element: PhantomData<fn() -> E>,
}

impl<F, E> WindowContent for RootContent<F, E>
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    fn into_headless(
        self: Box<Self>,
        cx: &mut AppContext,
    ) -> Box<dyn crate::testing::HeadlessWindowDriver> {
        crate::testing::headless_window(cx, self.build_root)
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    fn into_native(
        self: Box<Self>,
        cx: &mut AppContext,
        parts: crate::platform::mac::NativeWindowParts,
    ) -> Box<dyn crate::platform::mac::NativeWindowDriver> {
        crate::platform::mac::native_window(cx, parts, self.build_root)
    }
}

impl AppContext {
    /// Open another window showing `build_root`. Its runner creates it before
    /// the next frame.
    pub fn open_window<F, E>(&mut self, options: WindowOptions, build_root: F) -> WindowId
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        let id = self.register_window(options.clone());
        self.windows.pending.push(PendingWindow {
            id,
            options,
            content: Box::new(RootContent {
                build_root,
                element: PhantomData,
            }),
        });
        id
    }

    /// Open another window showing a long-lived stateful view.
    pub fn open_view_window<V>(&mut self, options: WindowOptions, view: V) -> WindowId
    where
        V: View,
        V::Element: Element + 'static,
    {
        let marker = self.create(());
        let entity = Entity::<V>::new(marker.id());
        let notifier = ViewNotifier::new();
        let mut runtime_view = RuntimeView::new(entity, notifier.clone(), view);
        let id = self.open_window(options, move |cx| runtime_view.render(cx));
        self.attach_root_view(id, entity.id(), notifier);
        id
    }

    /// Close `window`. Returns whether it was open.
    ///
    /// When the last window closes the application quits, unless
    /// [`Self::set_quit_when_last_window_closes`] turned that off.
    pub fn close_window(&mut self, window: WindowId) -> bool {
        let Some(index) = self.windows.open.iter().position(|open| open.id == window) else {
            return false;
        };
        self.windows.open.remove(index);
        self.windows.inactive_frames.remove(&window);
        self.windows
            .view_windows
            .retain(|_, owner| *owner != window);
        self.windows.pending.retain(|pending| pending.id != window);
        self.windows.closed.push(window);
        if self.windows.active == Some(window) {
            self.windows.active = None;
            self.frame = WindowFrameState::new(crate::core::geometry::Size::ZERO);
            if let Some(next) = self.windows.open.first().map(Window::id) {
                self.activate_window(next);
            }
        }
        if self.windows.open.is_empty() && self.windows.quit_when_last_window_closes {
            self.quit();
        }
        true
    }

    /// Whether closing the last window quits the application (the default).
    pub fn set_quit_when_last_window_closes(&mut self, quit: bool) {
        self.windows.quit_when_last_window_closes = quit;
    }

    /// The open windows, in the order they were opened.
    pub fn windows(&self) -> &[Window] {
        &self.windows.open
    }

    /// The window whose frame runs now.
    pub fn active_window(&self) -> Option<WindowId> {
        self.windows.active
    }

    /// Record a window whose runner supplies its content. The first window
    /// becomes active and keeps the frame state built so far.
    pub(crate) fn register_window(&mut self, options: WindowOptions) -> WindowId {
        let id = WindowId::new(WINDOW_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        let frame = WindowFrameState::new(options.size);
        self.windows.open.push(Window::new(id, options));
        if self.windows.active.is_none() {
            self.windows.active = Some(id);
            self.frame.viewport_size = frame.viewport_size;
        } else {
            self.windows.inactive_frames.insert(id, frame);
        }
        id
    }

    /// Mark `entity` as `window`'s root view, which `notifier` wakes.
    pub(crate) fn attach_root_view(
        &mut self,
        window: WindowId,
        entity: EntityId,
        notifier: ViewNotifier,
    ) {
        self.windows.view_windows.insert(entity, window);
        self.window_frame_mut(window).runtime_view_notifier = Some((entity, notifier));
    }

    /// Make `window`'s frame state the current one. Returns whether it is open.
    pub(crate) fn activate_window(&mut self, window: WindowId) -> bool {
        if self.windows.active == Some(window) {
            return true;
        }
        let Some(mut frame) = self.windows.inactive_frames.remove(&window) else {
            return false;
        };
        std::mem::swap(&mut self.frame, &mut frame);
        if let Some(previous) = self.windows.active.replace(window) {
            self.windows.inactive_frames.insert(previous, frame);
        }
        true
    }

    pub(crate) fn take_pending_windows(&mut self) -> Vec<PendingWindow> {
        std::mem::take(&mut self.windows.pending)
    }

    pub(crate) fn take_closed_windows(&mut self) -> Vec<WindowId> {
        std::mem::take(&mut self.windows.closed)
    }

    pub(crate) fn window_frame_mut(&mut self, window: WindowId) -> &mut WindowFrameState {
        if self.windows.active == Some(window) || self.windows.active.is_none() {
            return &mut self.frame;
        }
        match self.windows.inactive_frames.get_mut(&window) {
            Some(frame) => frame,
            // A root view outlives its closed window only until close_window
            // removes it from `view_windows`; fall back to the current window.
            None => &mut self.frame,
        }
    }

    pub(crate) fn for_each_window_frame(&mut self, mut apply: impl FnMut(&mut WindowFrameState)) {
        apply(&mut self.frame);
        for frame in self.windows.inactive_frames.values_mut() {
            apply(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;

    fn settle(context: &mut AppContext) {
        context.for_each_window_frame(|frame| {
            frame.needs_rebuild = false;
            frame.dirty = false;
            frame.pending_updates.clear();
        });
    }

    #[test]
    fn each_window_keeps_its_own_viewport() {
        let mut context = AppContext::new();
        let first = context.register_window(WindowOptions::new().size(320.0, 240.0));
        let second = context.register_window(WindowOptions::new().size(640.0, 480.0));

        assert_eq!(context.active_window(), Some(first));
        assert_eq!(context.viewport_size(), Size::new(320.0, 240.0));
        assert!(context.activate_window(second));
        assert_eq!(context.viewport_size(), Size::new(640.0, 480.0));
        context.set_viewport_size(Size::new(800.0, 600.0));
        assert!(context.activate_window(first));
        assert_eq!(context.viewport_size(), Size::new(320.0, 240.0));
        assert!(context.activate_window(second));
        assert_eq!(context.viewport_size(), Size::new(800.0, 600.0));
    }

    #[test]
    fn shared_entities_redraw_every_window_and_root_views_only_their_own() {
        let mut context = AppContext::new();
        let first = context.register_window(WindowOptions::new());
        let second = context.register_window(WindowOptions::new());
        let view = context.create(());
        context.attach_root_view(second, view.id(), ViewNotifier::new());
        let model = context.create(0_u32);
        settle(&mut context);

        context.notify(view.id());
        assert!(
            !context.frame.has_frame_work(),
            "the first window is untouched"
        );
        assert!(context.windows.any_inactive_frame_work());
        context.activate_window(second);
        assert!(context.frame.pending_updates.contains(&view.id()));

        settle(&mut context);
        context.notify(model.id());
        assert!(context.frame.has_frame_work());
        context.activate_window(first);
        assert!(context.frame.has_frame_work());
    }

    #[test]
    fn input_redraws_only_the_active_window() {
        let mut context = AppContext::new();
        let _first = context.register_window(WindowOptions::new());
        let _second = context.register_window(WindowOptions::new());
        settle(&mut context);

        context.mark_redraw_from(crate::core::app::RedrawSource::PlatformInput);
        context.request_window_rebuild();

        assert!(context.has_frame_work());
        assert!(!context.windows.any_inactive_frame_work());
    }

    #[test]
    fn closing_windows_quits_only_after_the_last_unless_configured() {
        let mut context = AppContext::new();
        context.running = true;
        let first = context.register_window(WindowOptions::new().size(100.0, 100.0));
        let second = context.register_window(WindowOptions::new().size(200.0, 200.0));

        assert!(context.close_window(first));
        assert!(context.is_running());
        assert_eq!(context.active_window(), Some(second));
        assert_eq!(context.viewport_size(), Size::new(200.0, 200.0));
        assert!(!context.close_window(first));
        assert_eq!(context.take_closed_windows(), [first]);

        context.set_quit_when_last_window_closes(false);
        assert!(context.close_window(second));
        assert!(context.is_running());
        assert!(context.windows().is_empty());

        let third = context.register_window(WindowOptions::new());
        context.set_quit_when_last_window_closes(true);
        assert!(context.close_window(third));
        assert!(!context.is_running());
    }
}
