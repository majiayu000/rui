//! Async tasks: on the UI thread with [`AppContext::spawn`], and off it with
//! [`AppContext::background_spawn`].
//!
//! Foreground tasks are polled at the start of a frame, before input, animation
//! and layout work. Their wakers go through the same queue as [`AppHandle`]
//! updates, so a task woken from any thread wakes the event loop.
//!
//! Background tasks run on a pool of worker threads in the native runner. A
//! headless application runs them on the test thread instead, in the order they
//! became ready, when the test calls `run_until_parked`, so tests are
//! deterministic.
//!
//! [`AppHandle`]: crate::core::AppHandle

use crate::core::app::AppContext;
use crate::core::app_handle::{AppUpdateQueue, lock};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::{Rc, Weak as RcWeak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::task::{Context, Poll, Wake, Waker};

/// Rounds `run_until_parked` runs before deciding that tasks never park, so a
/// task that wakes itself forever fails the test instead of hanging it.
const MAX_PARK_ROUNDS: usize = 10_000;

type LocalFuture = Pin<Box<dyn Future<Output = ()>>>;
type SendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

thread_local! {
    /// The application whose foreground tasks are being polled on this thread.
    static POLLING_APP: Cell<*mut AppContext> = const { Cell::new(std::ptr::null_mut()) };
}

/// Restore the previous poll context on both normal return and unwinding.
struct PollingAppScope(*mut AppContext);

impl PollingAppScope {
    fn enter(app: *mut AppContext) -> Self {
        Self(POLLING_APP.with(|polling| polling.replace(app)))
    }
}

impl Drop for PollingAppScope {
    fn drop(&mut self) {
        POLLING_APP.with(|polling| polling.set(self.0));
    }
}

/// The result of a task, shared between the task and its handle.
struct Completion<T> {
    value: Option<T>,
    waiter: Option<Waker>,
}

impl<T> Default for Completion<T> {
    fn default() -> Self {
        Self {
            value: None,
            waiter: None,
        }
    }
}

impl<T> Completion<T> {
    fn complete(&mut self, value: T) {
        self.value = Some(value);
        if let Some(waiter) = self.waiter.take() {
            waiter.wake();
        }
    }

    fn poll_value(&mut self, cx: &mut Context<'_>) -> Poll<T> {
        match self.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                self.waiter = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct ForegroundEntry {
    future: Option<LocalFuture>,
    cancelled: Rc<Cell<bool>>,
}

#[derive(Default)]
struct ForegroundTasks {
    next_id: u64,
    entries: HashMap<u64, ForegroundEntry>,
}

/// Tasks spawned with [`AppContext::spawn`].
#[derive(Default)]
pub(crate) struct ForegroundExecutor {
    tasks: Rc<RefCell<ForegroundTasks>>,
}

struct TaskWaker {
    task: u64,
    queue: Weak<AppUpdateQueue>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(queue) = self.queue.upgrade() {
            queue.schedule_task(self.task);
        }
    }
}

/// A task running on the UI thread. Awaiting it gives its output.
///
/// Dropping a `Task` cancels it; [`Task::detach`] lets it run to completion.
#[must_use = "dropping a Task cancels it; call detach() to let it run"]
pub struct Task<T> {
    completion: Rc<RefCell<Completion<T>>>,
    cancelled: Rc<Cell<bool>>,
    task: u64,
    tasks: RcWeak<RefCell<ForegroundTasks>>,
    detached: bool,
}

impl<T> Task<T> {
    /// Keep the task running without waiting for it.
    pub fn detach(mut self) {
        self.detached = true;
    }
}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        self.completion.borrow_mut().poll_value(cx)
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        if self.detached {
            return;
        }
        self.cancelled.set(true);
        if let Some(tasks) = self.tasks.upgrade()
            && let Ok(mut tasks) = tasks.try_borrow_mut()
        {
            tasks.entries.remove(&self.task);
        }
    }
}

/// The application, reachable from a task spawned with [`AppContext::spawn`].
///
/// It stays on the UI thread. [`AsyncApp::update`] runs while the task is
/// polled, so between two `await`s the task sees and changes app state directly.
#[derive(Clone)]
pub struct AsyncApp {
    owner: RcWeak<RefCell<ForegroundTasks>>,
    background: BackgroundExecutor,
    not_send: PhantomData<*const ()>,
}

impl AsyncApp {
    /// Run `update` with the application.
    ///
    /// # Panics
    ///
    /// Outside the poll of a task spawned with [`AppContext::spawn`] on this
    /// application, or inside another `update`.
    pub fn update<R>(&self, update: impl FnOnce(&mut AppContext) -> R) -> R {
        let restore = PollingAppScope::enter(std::ptr::null_mut());
        let app = restore.0;
        if app.is_null() {
            panic!(
                "AsyncApp::update runs only while its task is polled, and not inside another update"
            );
        }
        // SAFETY: the poll scope keeps this pointer valid, including during
        // unwinding. Clearing TLS above prevents reentrant mutable access.
        let owner = unsafe { &(*app).foreground_tasks.tasks };
        assert!(
            self.owner.ptr_eq(&Rc::downgrade(owner)),
            "AsyncApp::update cannot access another application"
        );
        // SAFETY: `app` was set by `poll_foreground_tasks` from its `&mut
        // AppContext`, which it does not use while the task is polled, and is
        // cleared while this update runs so no second `&mut` can be made.
        update(unsafe { &mut *app })
    }

    /// Run `future` off the UI thread; see [`AppContext::background_spawn`].
    pub fn background_spawn<F>(&self, future: F) -> BackgroundTask<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.background.spawn(future)
    }
}

impl AppContext {
    /// Run `task` on the UI thread. It starts at the next frame.
    ///
    /// ```ignore
    /// cx.spawn(move |cx| async move {
    ///     let reply = cx.background_spawn(async move { fetch() }).await;
    ///     cx.update(|cx| {
    ///         *cx.get_mut(reply_entity).unwrap() = reply;
    ///         cx.notify(reply_entity.id());
    ///     });
    /// })
    /// .detach();
    /// ```
    pub fn spawn<F, Fut>(&mut self, task: F) -> Task<Fut::Output>
    where
        F: FnOnce(AsyncApp) -> Fut,
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        let future = task(AsyncApp {
            owner: Rc::downgrade(&self.foreground_tasks.tasks),
            background: self.background.clone(),
            not_send: PhantomData,
        });
        let completion = Rc::new(RefCell::new(Completion::default()));
        let cancelled = Rc::new(Cell::new(false));
        let output = Rc::clone(&completion);
        let wrapped: LocalFuture = Box::pin(async move {
            let value = future.await;
            output.borrow_mut().complete(value);
        });

        let mut tasks = self.foreground_tasks.tasks.borrow_mut();
        let id = tasks.next_id;
        tasks.next_id += 1;
        tasks.entries.insert(
            id,
            ForegroundEntry {
                future: Some(wrapped),
                cancelled: Rc::clone(&cancelled),
            },
        );
        drop(tasks);
        self.update_queue().schedule_task(id);

        Task {
            completion,
            cancelled,
            task: id,
            tasks: Rc::downgrade(&self.foreground_tasks.tasks),
            detached: false,
        }
    }

    /// Run `future` off the UI thread and give its output to whoever awaits the
    /// returned task, typically a task spawned with [`Self::spawn`].
    pub fn background_spawn<F>(&self, future: F) -> BackgroundTask<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.background.spawn(future)
    }

    /// Poll each foreground task woken since the last call once. Tasks woken
    /// while these run are polled on the next call.
    pub(crate) fn poll_foreground_tasks(&mut self) -> usize {
        let ready = self.update_queue().take_ready_tasks();
        let mut polled = 0;
        for id in ready {
            let Some((mut future, cancelled)) = self.take_foreground_future(id) else {
                continue;
            };
            let waker = Waker::from(Arc::new(TaskWaker {
                task: id,
                queue: Arc::downgrade(self.update_queue()),
            }));
            let app: *mut AppContext = self;
            let scope = PollingAppScope::enter(app);
            let poll = future.as_mut().poll(&mut Context::from_waker(&waker));
            drop(scope);
            polled += 1;

            let mut tasks = self.foreground_tasks.tasks.borrow_mut();
            match poll {
                Poll::Ready(()) => {
                    tasks.entries.remove(&id);
                }
                Poll::Pending if cancelled.get() => {
                    tasks.entries.remove(&id);
                }
                Poll::Pending => {
                    if let Some(entry) = tasks.entries.get_mut(&id) {
                        entry.future = Some(future);
                    }
                }
            }
        }
        polled
    }

    fn take_foreground_future(&mut self, id: u64) -> Option<(LocalFuture, Rc<Cell<bool>>)> {
        let mut tasks = self.foreground_tasks.tasks.borrow_mut();
        let entry = tasks.entries.get_mut(&id)?;
        if entry.cancelled.get() {
            tasks.entries.remove(&id);
            return None;
        }
        // A task woken twice before its poll is in the ready list twice; the
        // second entry finds the future already taken or finished.
        let future = entry.future.take()?;
        Some((future, Rc::clone(&entry.cancelled)))
    }

    /// Whether a foreground task is waiting to be polled.
    pub(crate) fn has_ready_tasks(&self) -> bool {
        self.update_queue().has_ready_tasks()
    }

    /// Run foreground tasks, inline background tasks and updates from other
    /// threads until none of them can make progress.
    ///
    /// # Panics
    ///
    /// When tasks keep waking each other for `MAX_PARK_ROUNDS` rounds.
    pub(crate) fn run_until_parked(&mut self) {
        for _ in 0..MAX_PARK_ROUNDS {
            self.apply_handle_updates();
            let foreground = self.poll_foreground_tasks();
            let background = self.background.run_inline_tasks();
            if foreground == 0 && background == 0 && !self.has_ready_tasks() {
                return;
            }
        }
        panic!("tasks were still waking each other after {MAX_PARK_ROUNDS} rounds");
    }

    /// Run background tasks on worker threads; the native runner calls this
    /// before the app first renders.
    pub(crate) fn use_background_threads(&mut self) {
        self.background = BackgroundExecutor::threaded();
    }
}

/// A future running off the UI thread. Awaiting it gives its output.
///
/// Dropping it requests cancellation and schedules disposal of its future. A
/// currently running poll must return first. [`BackgroundTask::detach`] lets it
/// run to completion.
#[must_use = "dropping a BackgroundTask cancels it; call detach() to let it run"]
pub struct BackgroundTask<T> {
    completion: Arc<Mutex<Completion<T>>>,
    task: Arc<BackgroundRun>,
    detached: bool,
}

impl<T> BackgroundTask<T> {
    /// Keep the task running without waiting for it.
    pub fn detach(mut self) {
        self.detached = true;
    }
}

impl<T> Future for BackgroundTask<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        lock(&self.completion).poll_value(cx)
    }
}

impl<T> Drop for BackgroundTask<T> {
    fn drop(&mut self) {
        if !self.detached {
            self.task.cancelled.store(true, Ordering::Release);
            Arc::clone(&self.task).wake();
        }
    }
}

/// Where a background task is queued when it is ready to run.
#[derive(Clone)]
enum Schedule {
    Pool(&'static WorkerPool),
    Inline(Weak<Mutex<VecDeque<Arc<BackgroundRun>>>>),
}

struct BackgroundRun {
    future: Mutex<Option<SendFuture>>,
    queued: AtomicBool,
    cancelled: AtomicBool,
    schedule: Schedule,
}

impl BackgroundRun {
    fn run(self: Arc<Self>) {
        self.queued.store(false, Ordering::Release);
        let mut slot = lock(&self.future);
        if self.cancelled.load(Ordering::Acquire) {
            *slot = None;
            return;
        }
        let Some(future) = slot.as_mut() else {
            return;
        };
        let waker = Waker::from(Arc::clone(&self));
        if future
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
        {
            *slot = None;
        }
    }
}

impl Wake for BackgroundRun {
    fn wake(self: Arc<Self>) {
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        match &self.schedule {
            Schedule::Pool(pool) => pool.push(Arc::clone(&self)),
            Schedule::Inline(queue) => {
                if let Some(queue) = queue.upgrade() {
                    lock(&queue).push_back(Arc::clone(&self));
                }
            }
        }
    }
}

/// Runs background tasks: on worker threads, or inline for headless tests.
#[derive(Clone)]
pub struct BackgroundExecutor {
    inline: Option<Arc<Mutex<VecDeque<Arc<BackgroundRun>>>>>,
}

impl Default for BackgroundExecutor {
    /// Inline: tasks run when the headless runner calls `run_until_parked`.
    fn default() -> Self {
        Self {
            inline: Some(Arc::default()),
        }
    }
}

impl BackgroundExecutor {
    fn threaded() -> Self {
        Self { inline: None }
    }

    /// Run `future` off the UI thread; see [`AppContext::background_spawn`].
    pub fn spawn<F>(&self, future: F) -> BackgroundTask<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let completion = Arc::new(Mutex::new(Completion::default()));
        let output = Arc::clone(&completion);
        let future: SendFuture = Box::pin(async move {
            let value = future.await;
            lock(&output).complete(value);
        });
        let schedule = match &self.inline {
            Some(queue) => Schedule::Inline(Arc::downgrade(queue)),
            None => Schedule::Pool(WorkerPool::shared()),
        };
        let task = Arc::new(BackgroundRun {
            future: Mutex::new(Some(future)),
            queued: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            schedule,
        });
        Arc::clone(&task).wake();
        BackgroundTask {
            completion,
            task,
            detached: false,
        }
    }

    /// Run the ready batch present at entry, in order. New wakes wait for
    /// the next round so self-waking tasks cannot monopolize this drain.
    fn run_inline_tasks(&self) -> usize {
        let Some(queue) = &self.inline else {
            return 0;
        };
        let ready = lock(queue).len();
        let mut ran = 0;
        for _ in 0..ready {
            let Some(task) = lock(queue).pop_front() else {
                break;
            };
            task.run();
            ran += 1;
        }
        ran
    }
}

/// Worker threads shared by every application in the process.
struct WorkerPool {
    queue: Mutex<VecDeque<Arc<BackgroundRun>>>,
    ready: Condvar,
}

impl WorkerPool {
    fn shared() -> &'static WorkerPool {
        static POOL: OnceLock<&'static WorkerPool> = OnceLock::new();
        POOL.get_or_init(|| {
            let pool: &'static WorkerPool = Box::leak(Box::new(WorkerPool {
                queue: Mutex::new(VecDeque::new()),
                ready: Condvar::new(),
            }));
            let workers = std::thread::available_parallelism().map_or(1, |count| count.get());
            for index in 0..workers {
                if let Err(err) = std::thread::Builder::new()
                    .name(format!("rui-background-{index}"))
                    .spawn(move || pool.work())
                {
                    panic!("failed to start a background worker thread: {err}");
                }
            }
            pool
        })
    }

    fn push(&self, task: Arc<BackgroundRun>) {
        lock(&self.queue).push_back(task);
        self.ready.notify_one();
    }

    fn work(&self) {
        loop {
            let task = {
                let mut queue = lock(&self.queue);
                loop {
                    if let Some(task) = queue.pop_front() {
                        break task;
                    }
                    queue = match self.ready.wait(queue) {
                        Ok(queue) => queue,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                }
            };
            task.run();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A future that is pending until `ready` is set, then wakes on demand.
    struct Gate {
        ready: Rc<Cell<bool>>,
        waker: Rc<RefCell<Option<Waker>>>,
    }

    impl Future for Gate {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.ready.get() {
                Poll::Ready(())
            } else {
                *self.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    #[test]
    fn foreground_tasks_reach_the_app_between_awaits() {
        let mut app = AppContext::new();
        let counter = app.create(0_u32);
        let ready = Rc::new(Cell::new(false));
        let waker = Rc::new(RefCell::new(None));
        let gate = Gate {
            ready: Rc::clone(&ready),
            waker: Rc::clone(&waker),
        };

        let task = app.spawn(move |cx| async move {
            cx.update(|cx| *cx.get_mut(counter).unwrap() += 1);
            gate.await;
            cx.update(|cx| *cx.get_mut(counter).unwrap() += 1);
            "done"
        });
        assert_eq!(
            *app.get(counter).unwrap(),
            0,
            "a task starts at the next poll"
        );

        app.run_until_parked();
        assert_eq!(*app.get(counter).unwrap(), 1);

        ready.set(true);
        waker.borrow_mut().take().unwrap().wake();
        assert!(app.has_ready_tasks());
        let output = app.spawn(move |_| task);
        app.run_until_parked();
        assert_eq!(*app.get(counter).unwrap(), 2);

        let result = Rc::new(Cell::new(None));
        let seen = Rc::clone(&result);
        app.spawn(move |_| async move { seen.set(Some(output.await)) })
            .detach();
        app.run_until_parked();
        assert_eq!(result.get(), Some("done"));
    }

    #[test]
    fn dropping_a_task_cancels_it() {
        let mut app = AppContext::new();
        let ran = Rc::new(Cell::new(false));
        let flag = Rc::clone(&ran);
        drop(app.spawn(move |_| async move { flag.set(true) }));

        app.run_until_parked();

        assert!(!ran.get());
        assert!(app.foreground_tasks.tasks.borrow().entries.is_empty());
    }

    #[test]
    fn inline_background_tasks_run_in_order_when_parked() {
        let mut app = AppContext::new();
        let order = Arc::new(Mutex::new(Vec::new()));
        let tasks: Vec<_> = (0..3)
            .map(|step| {
                let order = Arc::clone(&order);
                app.background_spawn(async move {
                    lock(&order).push(step);
                    step * 10
                })
            })
            .collect();
        assert!(lock(&order).is_empty(), "inline work waits for the runner");

        let sum = Rc::new(Cell::new(0));
        let total = Rc::clone(&sum);
        app.spawn(move |_| async move {
            for task in tasks {
                total.set(total.get() + task.await);
            }
        })
        .detach();
        app.run_until_parked();

        assert_eq!(*lock(&order), [0, 1, 2]);
        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn threaded_background_tasks_run_off_the_ui_thread() {
        let mut app = AppContext::new();
        app.use_background_threads();
        let ui_thread = std::thread::current().id();
        let task = app.background_spawn(async move { std::thread::current().id() });

        let worker = Rc::new(Cell::new(None));
        let seen = Rc::clone(&worker);
        app.spawn(move |_| async move { seen.set(Some(task.await)) })
            .detach();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while worker.get().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "background task finished"
            );
            app.run_until_parked();
            std::thread::yield_now();
        }

        assert_ne!(worker.get(), Some(ui_thread));
    }

    #[test]
    #[should_panic(expected = "AsyncApp::update runs only while its task is polled")]
    fn async_app_update_outside_a_poll_fails_clearly() {
        let mut app = AppContext::new();
        let stolen = Rc::new(RefCell::new(None));
        let slot = Rc::clone(&stolen);
        app.spawn(move |cx| async move { *slot.borrow_mut() = Some(cx) })
            .detach();
        app.run_until_parked();

        let cx = stolen.borrow_mut().take().unwrap();
        cx.update(|_| {});
    }
}

#[cfg(test)]
#[path = "executor_boundaries_tests.rs"]
mod boundary_tests;
