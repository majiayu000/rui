//! Application context and lifecycle

use crate::core::entity::{Entity, EntityId, EntityStore};
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
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            entities: EntityStore::new(),
            windows: Vec::new(),
            pending_updates: HashSet::new(),
            running: false,
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
        F: FnOnce(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        self.context.running = true;

        // Build the root element
        let root_element = build_root(&mut self.context);

        // Create the main window
        let window_options = WindowOptions::default().title("RUI Application");
        let window_id = self.context.open_window(window_options);

        // Start the platform-specific event loop
        #[cfg(target_os = "macos")]
        {
            crate::platform::mac::run_app(self.context, root_element);
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::error!("Platform not supported");
        }
    }

    /// Run with custom window options
    pub fn run_with_options<F, E>(mut self, options: WindowOptions, build_root: F)
    where
        F: FnOnce(&mut AppContext) -> E + 'static,
        E: Element + 'static,
    {
        self.context.running = true;

        let root_element = build_root(&mut self.context);
        let _window_id = self.context.open_window(options.clone());

        #[cfg(target_os = "macos")]
        {
            crate::platform::mac::run_app_with_options(self.context, root_element, options);
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
