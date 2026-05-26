//! Application context and lifecycle

use crate::core::entity::{Entity, EntityId, EntityStore};
use crate::core::view::{View, ViewContext, ViewNotifier};
use crate::core::window::{Window, WindowId, WindowOptions};
use crate::elements::Element;
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
    runtime_view_notifier: Option<(EntityId, ViewNotifier)>,
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            entities: EntityStore::new(),
            windows: Vec::new(),
            pending_updates: HashSet::new(),
            running: false,
            needs_rebuild: true,
            dirty: true,
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
        self.dirty = true;
    }

    /// Request a full rebuild of the UI tree
    pub fn request_rebuild(&mut self) {
        self.needs_rebuild = true;
        self.dirty = true;
    }

    /// Request a redraw without rebuilding the UI tree
    pub fn request_redraw(&mut self) {
        self.dirty = true;
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
    pub fn open_window(&mut self, options: WindowOptions) -> WindowId {
        let id = WindowId::new(WINDOW_ID_COUNTER.fetch_add(1, Ordering::Relaxed));
        let window = Window::new(id, options);
        self.windows.push(window);
        id
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
    pub fn run<F, E>(mut self, build_root: F)
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        self.context.running = true;

        // Create the main window
        let window_options = WindowOptions::default().title("RUI Application");
        let _window_id = self.context.open_window(window_options);

        // Start the platform-specific event loop
        #[cfg(target_os = "macos")]
        {
            crate::platform::mac::run_app(self.context, build_root);
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::error!("Platform not supported");
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
    pub fn run_with_options<F, E>(mut self, options: WindowOptions, build_root: F)
    where
        F: FnMut(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        self.context.running = true;

        let _window_id = self.context.open_window(options.clone());

        #[cfg(target_os = "macos")]
        {
            crate::platform::mac::run_app_with_options(self.context, build_root, options);
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::error!("Platform not supported");
        }
    }

    /// Run a long-lived stateful view with custom window options.
    pub fn run_view_with_options<V>(mut self, options: WindowOptions, view: V)
    where
        V: View,
        V::Element: Element + 'static,
    {
        self.context.running = true;

        let _window_id = self.context.open_window(options.clone());
        let view_marker = self.context.create(());
        let view_entity = Entity::<V>::new(view_marker.id());
        let notifier = ViewNotifier::new();
        self.context
            .set_runtime_view_notifier(view_entity.id(), notifier.clone());

        let mut runtime_view = RuntimeView::new(view_entity, notifier, view);

        #[cfg(target_os = "macos")]
        {
            crate::platform::mac::run_app_with_options(
                self.context,
                move |context| runtime_view.render(context),
                options,
            );
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::error!("Platform not supported");
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{text, Text};
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
}
