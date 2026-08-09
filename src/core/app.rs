//! Application context and lifecycle

use crate::core::action::{ActionHandler, ActionId, ActionOutcome, Keymap};
use crate::core::entity::{Entity, EntityId, EntityStore};
use crate::core::geometry::Size;
use crate::core::view::{View, ViewContext, ViewNotifier};
use crate::core::window::{Window, WindowId, WindowOptions};
use crate::elements::Element;
use crate::platform::window::{PlatformWindowError, PlatformWindowFeature};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

static WINDOW_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The main application context - owns all state
pub struct AppContext {
    pub(crate) entities: EntityStore,
    pub(crate) windows: Vec<Window>,
    pub(crate) pending_updates: HashSet<EntityId>,
    pub(crate) running: bool,
    pub(crate) needs_rebuild: bool,
    pub(crate) dirty: bool,
    redraw_scheduler: RedrawScheduler,
    keymap: Keymap,
    app_action_handlers: Vec<Box<dyn ActionHandler>>,
    viewport_size: Size,
    runtime_view_notifier: Option<(EntityId, ViewNotifier)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedrawSource {
    Explicit,
    ViewNotification,
    Element,
    PlatformLifecycle,
    PlatformResize,
    PlatformScaleFactor,
    PlatformFocus,
    PlatformInput,
    PlatformRedraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RedrawSourceCounts {
    pub explicit: u64,
    pub view_notification: u64,
    pub element: u64,
    pub platform_lifecycle: u64,
    pub platform_resize: u64,
    pub platform_scale_factor: u64,
    pub platform_focus: u64,
    pub platform_input: u64,
    pub platform_redraw: u64,
}

impl RedrawSourceCounts {
    fn increment(&mut self, source: RedrawSource) {
        match source {
            RedrawSource::Explicit => self.explicit += 1,
            RedrawSource::ViewNotification => self.view_notification += 1,
            RedrawSource::Element => self.element += 1,
            RedrawSource::PlatformLifecycle => self.platform_lifecycle += 1,
            RedrawSource::PlatformResize => self.platform_resize += 1,
            RedrawSource::PlatformScaleFactor => self.platform_scale_factor += 1,
            RedrawSource::PlatformFocus => self.platform_focus += 1,
            RedrawSource::PlatformInput => self.platform_input += 1,
            RedrawSource::PlatformRedraw => self.platform_redraw += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RedrawScheduler {
    counts: RedrawSourceCounts,
    platform_redraw_pending: bool,
}

impl RedrawScheduler {
    fn mark_redraw(&mut self, source: RedrawSource) {
        self.counts.increment(source);
    }

    fn request_platform_redraw(&mut self, source: RedrawSource) -> bool {
        self.mark_redraw(source);
        if self.platform_redraw_pending {
            return false;
        }
        self.platform_redraw_pending = true;
        true
    }

    fn complete_frame(&mut self) {
        self.platform_redraw_pending = false;
    }
}

impl AppContext {
    pub fn new() -> Self {
        let keymap = match Keymap::with_standard_bindings() {
            Ok(keymap) => keymap,
            Err(err) => panic!("failed to initialize standard keymap: {err}"),
        };

        Self {
            entities: EntityStore::new(),
            windows: Vec::new(),
            pending_updates: HashSet::new(),
            running: false,
            needs_rebuild: true,
            dirty: true,
            redraw_scheduler: RedrawScheduler::default(),
            keymap,
            app_action_handlers: Vec::new(),
            viewport_size: Size::ZERO,
            runtime_view_notifier: None,
        }
    }

    /// Create a new entity
    pub fn create<T: 'static>(&mut self, state: T) -> Entity<T> {
        let id = self.entities.insert(state);
        Entity::new(id)
    }

    /// Get an entity by ID
    pub fn get<T: 'static>(&self, entity: Entity<T>) -> Option<std::cell::Ref<'_, T>> {
        self.entities.get::<T>(entity.id())
    }

    /// Get a mutable reference to an entity
    pub fn get_mut<T: 'static>(&self, entity: Entity<T>) -> Option<std::cell::RefMut<'_, T>> {
        self.entities.get_mut::<T>(entity.id())
    }

    /// Mark an entity as needing re-render
    pub fn notify(&mut self, entity_id: EntityId) {
        self.pending_updates.insert(entity_id);
        self.needs_rebuild = true;
        self.mark_redraw_from(RedrawSource::ViewNotification);
    }

    /// Request a full rebuild of the UI tree
    pub fn request_rebuild(&mut self) {
        self.needs_rebuild = true;
        self.mark_redraw_from(RedrawSource::Explicit);
    }

    /// Request a redraw without rebuilding the UI tree
    pub fn request_redraw(&mut self) {
        self.mark_redraw_from(RedrawSource::Explicit);
    }

    pub(crate) fn mark_redraw_from(&mut self, source: RedrawSource) {
        self.dirty = true;
        self.redraw_scheduler.mark_redraw(source);
    }

    pub(crate) fn request_platform_redraw_from(&mut self, source: RedrawSource) -> bool {
        self.dirty = true;
        self.redraw_scheduler.request_platform_redraw(source)
    }

    pub(crate) fn complete_redraw_frame(&mut self) {
        self.redraw_scheduler.complete_frame();
        if !self.needs_rebuild && self.pending_updates.is_empty() {
            self.dirty = false;
        }
    }

    pub(crate) fn preserve_frame_work(&mut self) {
        self.dirty = true;
    }

    /// Whether anything is pending that a frame would render.
    ///
    /// The single definition of "idle" for the frame pipeline; runners must not
    /// re-derive it from the individual flags.
    pub fn has_frame_work(&self) -> bool {
        self.dirty || self.needs_rebuild || !self.pending_updates.is_empty()
    }

    pub fn redraw_source_counts(&self) -> RedrawSourceCounts {
        self.redraw_scheduler.counts
    }

    pub fn platform_redraw_pending(&self) -> bool {
        self.redraw_scheduler.platform_redraw_pending
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    pub fn add_action_handler(&mut self, handler: impl ActionHandler + 'static) {
        self.app_action_handlers.push(Box::new(handler));
    }

    pub(crate) fn dispatch_app_action(&mut self, action: &ActionId) -> ActionOutcome {
        for handler in &mut self.app_action_handlers {
            if action.requires_enabled() && !handler.action_handler_enabled() {
                continue;
            }

            let outcome = handler.run_action(action);
            if outcome.is_handled() {
                return outcome;
            }
        }

        ActionOutcome::Ignored
    }

    /// Current drawable viewport size for the main window.
    pub fn viewport_size(&self) -> Size {
        self.viewport_size
    }

    pub(crate) fn set_viewport_size(&mut self, size: Size) {
        self.viewport_size = size;
    }

    pub(crate) fn set_runtime_view_notifier(
        &mut self,
        entity_id: EntityId,
        notifier: ViewNotifier,
    ) {
        self.runtime_view_notifier = Some((entity_id, notifier));
    }

    pub(crate) fn consume_runtime_view_notification(&mut self) -> bool {
        let Some((entity_id, notifier)) = &self.runtime_view_notifier else {
            return false;
        };
        let entity_id = *entity_id;

        if !notifier.take_notified() {
            return false;
        }

        self.notify(entity_id);
        true
    }

    /// Open a new window
    pub fn open_window(&mut self, options: WindowOptions) -> Result<WindowId, PlatformWindowError> {
        if !self.windows.is_empty() {
            return Err(PlatformWindowError::unsupported(
                std::env::consts::OS,
                PlatformWindowFeature::MultiWindow,
            ));
        }

        let id = WindowId::new(WINDOW_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        self.viewport_size = options.size;
        let window = Window::new(id, options);
        self.windows.push(window);
        Ok(id)
    }

    /// Check if the application is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Request the application to quit
    pub fn quit(&mut self) {
        self.running = false;
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct RuntimeView<V: View> {
    entity: Entity<V>,
    notifier: ViewNotifier,
    view: V,
}

impl<V: View> RuntimeView<V> {
    pub(crate) fn new(entity: Entity<V>, notifier: ViewNotifier, view: V) -> Self {
        Self {
            entity,
            notifier,
            view,
        }
    }

    pub(crate) fn render(&mut self, app: &mut AppContext) -> V::Element {
        let mut view_cx = ViewContext::with_notifier(app, self.entity, self.notifier.clone());
        self.view.render(&mut view_cx)
    }
}

/// Application builder and runner
pub struct App {
    context: AppContext,
}

impl App {
    pub fn new() -> Self {
        // Initialize logging
        let _ = env_logger::try_init();

        Self {
            context: AppContext::new(),
        }
    }

    /// Run the application with a root view builder
    #[cfg_attr(
        not(all(target_os = "macos", feature = "metal")),
        allow(unused_variables)
    )]
    pub fn run<F, E>(mut self, build_root: F)
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        #[cfg(all(target_os = "macos", feature = "metal"))]
        {
            self.context.running = true;

            // Create the main window
            let window_options = WindowOptions::default().title("RUI Application");
            let _window_id = open_main_window(&mut self.context, window_options);

            // Start the platform-specific event loop
            crate::platform::mac::run_app(self.context, build_root);
        }

        #[cfg(not(all(target_os = "macos", feature = "metal")))]
        {
            self.context.running = true;
            panic!("{}", unsupported_platform_error());
        }
    }

    /// Run the application with a long-lived stateful view.
    pub fn run_view<V>(self, view: V)
    where
        V: View,
        V::Element: Element + 'static,
    {
        let window_options = WindowOptions::default().title("RUI Application");
        self.run_view_with_options(window_options, view);
    }

    /// Run with custom window options
    #[cfg_attr(
        not(all(target_os = "macos", feature = "metal")),
        allow(unused_variables)
    )]
    pub fn run_with_options<F, E>(mut self, options: WindowOptions, build_root: F)
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        #[cfg(all(target_os = "macos", feature = "metal"))]
        {
            self.context.running = true;

            let _window_id = open_main_window(&mut self.context, options.clone());

            crate::platform::mac::run_app_with_options(self.context, build_root, options);
        }

        #[cfg(not(all(target_os = "macos", feature = "metal")))]
        {
            self.context.running = true;
            panic!("{}", unsupported_platform_error());
        }
    }

    /// Run a long-lived stateful view with custom window options.
    #[cfg_attr(
        not(all(target_os = "macos", feature = "metal")),
        allow(unused_variables)
    )]
    pub fn run_view_with_options<V>(mut self, options: WindowOptions, view: V)
    where
        V: View,
        V::Element: Element + 'static,
    {
        #[cfg(all(target_os = "macos", feature = "metal"))]
        {
            self.context.running = true;

            let _window_id = open_main_window(&mut self.context, options.clone());
            let view_marker = self.context.create(());
            let view_entity = Entity::<V>::new(view_marker.id());
            let notifier = ViewNotifier::new();
            self.context
                .set_runtime_view_notifier(view_entity.id(), notifier.clone());

            let mut runtime_view = RuntimeView::new(view_entity, notifier, view);

            crate::platform::mac::run_app_with_options(
                self.context,
                move |context| runtime_view.render(context),
                options,
            );
        }

        #[cfg(not(all(target_os = "macos", feature = "metal")))]
        {
            self.context.running = true;
            panic!("{}", unsupported_platform_error());
        }
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn open_main_window(context: &mut AppContext, options: WindowOptions) -> WindowId {
    match context.open_window(options) {
        Ok(window_id) => window_id,
        Err(err) => panic!("failed to open main window: {err}"),
    }
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
fn unsupported_platform_error() -> crate::platform::window::PlatformWindowError {
    crate::platform::window::PlatformWindowError::unsupported(
        std::env::consts::OS,
        crate::platform::window::PlatformWindowFeature::Lifecycle,
    )
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{Text, text};
    use std::cell::Cell;
    use std::rc::Rc;

    struct CountingView {
        renders: Rc<Cell<u32>>,
        notify_during_render: bool,
    }

    impl View for CountingView {
        type Element = Text;

        fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
            self.renders.set(self.renders.get() + 1);
            if self.notify_during_render {
                cx.notify();
            }
            text(format!("renders: {}", self.renders.get()))
        }
    }

    fn runtime_view(context: &mut AppContext, view: CountingView) -> RuntimeView<CountingView> {
        let marker = context.create(());
        let entity = Entity::<CountingView>::new(marker.id());
        let notifier = ViewNotifier::new();
        context.set_runtime_view_notifier(entity.id(), notifier.clone());
        RuntimeView::new(entity, notifier, view)
    }

    #[test]
    fn runtime_view_render_invokes_view_once() {
        let renders = Rc::new(Cell::new(0));
        let mut context = AppContext::new();
        let view = CountingView {
            renders: Rc::clone(&renders),
            notify_during_render: false,
        };
        let mut runtime = runtime_view(&mut context, view);

        let _rendered = runtime.render(&mut context);

        assert_eq!(renders.get(), 1);
    }

    #[test]
    fn view_context_notify_marks_current_view_pending() {
        let mut context = AppContext::new();
        let view = CountingView {
            renders: Rc::new(Cell::new(0)),
            notify_during_render: true,
        };
        let mut runtime = runtime_view(&mut context, view);
        let entity_id = runtime.entity.id();

        let _rendered = runtime.render(&mut context);

        assert!(context.pending_updates.contains(&entity_id));
        assert!(context.needs_rebuild);
        assert!(context.dirty);
    }

    #[test]
    fn runtime_view_notifier_schedules_rebuild() {
        let mut context = AppContext::new();
        let notifier = ViewNotifier::new();
        let marker = context.create(());

        context.set_runtime_view_notifier(marker.id(), notifier.clone());
        notifier.notify();

        assert!(context.consume_runtime_view_notification());
        assert!(context.pending_updates.contains(&marker.id()));
        assert!(context.needs_rebuild);
        assert!(context.dirty);
    }

    #[test]
    fn viewport_size_tracks_open_window_and_updates() {
        let mut context = AppContext::new();

        assert_eq!(context.viewport_size(), Size::ZERO);

        context.open_window(WindowOptions::new().size(320.0, 240.0));
        assert_eq!(context.viewport_size(), Size::new(320.0, 240.0));

        context.set_viewport_size(Size::new(640.0, 480.0));
        assert_eq!(context.viewport_size(), Size::new(640.0, 480.0));
    }

    #[test]
    fn runtime_view_renders_again_after_notification() {
        let renders = Rc::new(Cell::new(0));
        let mut context = AppContext::new();
        let view = CountingView {
            renders: Rc::clone(&renders),
            notify_during_render: false,
        };
        let mut runtime = runtime_view(&mut context, view);
        let notifier = runtime.notifier.clone();

        let _first_render = runtime.render(&mut context);
        context.pending_updates.clear();
        context.needs_rebuild = false;
        context.dirty = false;

        notifier.notify();
        assert!(context.consume_runtime_view_notification());
        let _second_render = runtime.render(&mut context);

        assert_eq!(renders.get(), 2);
    }

    #[test]
    fn notify_during_render_survives_processed_pending_clear() {
        let mut context = AppContext::new();
        let view = CountingView {
            renders: Rc::new(Cell::new(0)),
            notify_during_render: true,
        };
        let mut runtime = runtime_view(&mut context, view);
        let entity_id = runtime.entity.id();

        context.notify(entity_id);
        context.pending_updates.clear();
        context.needs_rebuild = false;
        let _rendered = runtime.render(&mut context);
        if !context.pending_updates.is_empty() {
            context.needs_rebuild = true;
        }

        assert!(context.pending_updates.contains(&entity_id));
        assert!(context.needs_rebuild);
    }

    #[test]
    fn redraw_scheduler_counts_sources_and_coalesces_platform_requests() {
        let mut context = AppContext::new();
        context.pending_updates.clear();
        context.needs_rebuild = false;
        context.dirty = false;

        assert!(context.request_platform_redraw_from(RedrawSource::Element));
        assert!(!context.request_platform_redraw_from(RedrawSource::Element));
        assert!(!context.request_platform_redraw_from(RedrawSource::ViewNotification));

        let counts = context.redraw_source_counts();
        assert_eq!(counts.element, 2);
        assert_eq!(counts.view_notification, 1);
        assert!(context.dirty);
        assert!(context.platform_redraw_pending());

        context.complete_redraw_frame();
        assert!(!context.dirty);
        assert!(!context.platform_redraw_pending());
        assert!(context.request_platform_redraw_from(RedrawSource::PlatformLifecycle));
    }

    #[test]
    fn second_window_returns_explicit_unsupported_error() {
        let mut context = AppContext::new();
        match context.open_window(WindowOptions::default()) {
            Ok(_) => {}
            Err(err) => panic!("first window should open: {err}"),
        }

        let err = match context.open_window(WindowOptions::default()) {
            Ok(_) => panic!("second window should be unsupported"),
            Err(err) => err,
        };

        assert_eq!(
            err,
            PlatformWindowError::unsupported(
                std::env::consts::OS,
                PlatformWindowFeature::MultiWindow
            )
        );
    }
}
