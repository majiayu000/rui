//! Reaching the application from other threads.

use crate::core::app::AppContext;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, Weak};

type AppUpdate = Box<dyn FnOnce(&mut AppContext) + Send>;
type Waker = Box<dyn Fn() + Send + Sync>;

/// Updates sent from other threads, applied on the UI thread before the next frame.
#[derive(Default)]
pub(crate) struct AppUpdateQueue {
    updates: Mutex<Vec<AppUpdate>>,
    waker: Mutex<Option<Waker>>,
}

impl AppUpdateQueue {
    /// Set what wakes the UI thread's event loop. Headless runners leave it unset
    /// and apply updates on their next frame.
    pub(crate) fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *lock(&self.waker) = Some(Box::new(waker));
    }

    pub(crate) fn take(&self) -> Vec<AppUpdate> {
        std::mem::take(&mut *lock(&self.updates))
    }

    fn push(&self, update: AppUpdate) {
        lock(&self.updates).push(update);
        if let Some(wake) = lock(&self.waker).as_ref() {
            wake();
        }
    }
}

/// An update closure panicking on the UI thread must not wedge every later sender.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// A `Send` handle to the application, for background threads and tasks.
///
/// ```ignore
/// let handle = cx.app_handle();
/// let reply = cx.app_mut().create(String::new());
/// std::thread::spawn(move || {
///     let text = fetch();
///     handle.update(move |cx| {
///         *cx.get_mut(reply).unwrap() = text;
///         cx.request_rebuild();
///     })
/// });
/// ```
#[derive(Clone)]
pub struct AppHandle {
    queue: Weak<AppUpdateQueue>,
}

impl AppHandle {
    pub(crate) fn new(queue: &Arc<AppUpdateQueue>) -> Self {
        Self {
            queue: Arc::downgrade(queue),
        }
    }

    /// Run `update` on the UI thread before the next frame, waking the event loop.
    ///
    /// Fails once the application has been dropped; `update` is not run then.
    pub fn update(
        &self,
        update: impl FnOnce(&mut AppContext) + Send + 'static,
    ) -> Result<(), AppClosed> {
        let queue = self.queue.upgrade().ok_or(AppClosed)?;
        queue.push(Box::new(update));
        Ok(())
    }
}

impl fmt::Debug for AppHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppHandle")
            .field("open", &(self.queue.strong_count() > 0))
            .finish()
    }
}

/// The application an [`AppHandle`] points to no longer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppClosed;

impl fmt::Display for AppClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the application has closed")
    }
}

impl Error for AppClosed {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn each_update_wakes_the_event_loop_and_keeps_its_order() {
        let queue = Arc::new(AppUpdateQueue::default());
        let wakes = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&wakes);
        queue.set_waker(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });

        let handle = AppHandle::new(&queue);
        let order = Arc::new(Mutex::new(Vec::new()));
        for step in 0..3 {
            let order = Arc::clone(&order);
            handle
                .update(move |_| lock(&order).push(step))
                .expect("queue is alive");
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 3);

        let mut app = AppContext::new();
        for update in queue.take() {
            update(&mut app);
        }
        assert_eq!(*lock(&order), [0, 1, 2]);
    }
}
