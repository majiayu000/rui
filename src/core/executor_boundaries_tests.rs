use super::*;

#[test]
fn regression_async_app_rejects_other_owner_and_survives_move() {
    let mut a = AppContext::new();
    let mut b = AppContext::new();
    let a_value = a.create(10_u32);
    let b_value = b.create(20_u32);
    let saved = Rc::new(RefCell::new(None));
    let slot = saved.clone();
    a.spawn(move |cx| async move { *slot.borrow_mut() = Some(cx) })
        .detach();
    a.run_until_parked();
    let cx = saved.borrow().as_ref().unwrap().clone();
    let called = Rc::new(Cell::new(false));
    let flag = called.clone();
    b.spawn(move |_| async move {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cx.update(|app| {
                    flag.set(true);
                    *app.get_mut(a_value).unwrap() = 99;
                });
            }))
            .is_err()
        );
    })
    .detach();
    b.run_until_parked();
    assert!(!called.get());
    assert_eq!(*a.get(a_value).unwrap(), 10);
    assert_eq!(*b.get(b_value).unwrap(), 20);
    let mut moved = Box::new(a);
    let cx = saved.borrow().as_ref().unwrap().clone();
    moved
        .spawn(move |_| async move {
            cx.update(|app| *app.get_mut(a_value).unwrap() = 11);
        })
        .detach();
    moved.run_until_parked();
    assert_eq!(*moved.get(a_value).unwrap(), 11);
}

#[test]
fn regression_poll_panic_restores_context() {
    let mut app = AppContext::new();
    let saved = Rc::new(RefCell::new(None));
    let slot = saved.clone();
    app.spawn(move |cx| async move {
        *slot.borrow_mut() = Some(cx);
        panic!("task panic");
    })
    .detach();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run_until_parked())).is_err()
    );
    let cx = saved.borrow().as_ref().unwrap().clone();
    let called = Cell::new(false);
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cx.update(|_| called.set(true))
    }))
    .is_err();
    // Clear the baseline's stale pointer before asserting, so a red test cannot leak TLS.
    POLLING_APP.with(|polling| polling.set(std::ptr::null_mut()));
    assert!(rejected);
    assert!(!called.get());
    let value = app.create(0_u32);
    app.spawn(move |cx| async move {
        cx.update(|app| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cx.update(|_| {})))
                    .is_err()
            );
            *app.get_mut(value).unwrap() = 7;
        });
    })
    .detach();
    app.run_until_parked();
    assert_eq!(*app.get(value).unwrap(), 7);
}

struct SelfWake {
    polls: Arc<std::sync::atomic::AtomicUsize>,
    limit: usize,
}
impl Future for SelfWake {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let count = self.polls.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.limit {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[test]
fn regression_inline_batch_is_bounded() {
    let mut app = AppContext::new();
    let polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    app.background_spawn(SelfWake {
        polls: polls.clone(),
        limit: 3,
    })
    .detach();
    assert_eq!(app.background.run_inline_tasks(), 1);
    assert_eq!(polls.load(Ordering::Relaxed), 1);
    app.run_until_parked();
    assert_eq!(polls.load(Ordering::Relaxed), 3);
}

struct RetainedWaker {
    waker: Arc<Mutex<Option<Waker>>>,
    dropped: Arc<AtomicBool>,
}
impl Future for RetainedWaker {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        *lock(&self.waker) = Some(cx.waker().clone());
        Poll::Pending
    }
}
impl Drop for RetainedWaker {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[test]
fn regression_cancel_releases_future_with_retained_waker() {
    let mut app = AppContext::new();
    let waker = Arc::new(Mutex::new(None));
    let dropped = Arc::new(AtomicBool::new(false));
    let task = app.background_spawn(RetainedWaker {
        waker: waker.clone(),
        dropped: dropped.clone(),
    });
    app.run_until_parked();
    assert!(lock(&waker).is_some());
    assert!(!dropped.load(Ordering::Acquire));
    drop(task);
    app.run_until_parked();
    assert!(
        dropped.load(Ordering::Acquire),
        "cancel must dispose of the future without waking the retained waker"
    );
    assert!(lock(&waker).is_some());
}

#[test]
#[should_panic(expected = "tasks were still waking each other after 10000 rounds")]
fn endless_inline_self_waker_reaches_round_limit() {
    let mut app = AppContext::new();
    app.background_spawn(SelfWake {
        polls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        limit: usize::MAX,
    })
    .detach();
    app.run_until_parked();
}

#[test]
fn background_cancel_before_poll_and_detach() {
    let mut app = AppContext::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let waker = Arc::new(Mutex::new(None));
    drop(app.background_spawn(RetainedWaker {
        waker: waker.clone(),
        dropped: dropped.clone(),
    }));
    app.run_until_parked();
    assert!(dropped.load(Ordering::Acquire));
    assert!(lock(&waker).is_none());
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    app.background_spawn(async move {
        flag.store(true, Ordering::Release);
    })
    .detach();
    app.run_until_parked();
    assert!(ran.load(Ordering::Acquire));
}

#[test]
fn threaded_cancel_during_poll_is_nonblocking_and_releases_future() {
    use std::sync::mpsc;
    use std::time::Duration;
    let mut app = AppContext::new();
    app.use_background_threads();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    struct OnDrop(mpsc::Sender<()>);
    impl Drop for OnDrop {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let guard = OnDrop(dropped_tx);
    let waker = Arc::new(Mutex::new(None));
    let saved = waker.clone();
    let task = app.background_spawn(std::future::poll_fn(move |cx| {
        let _ = &guard;
        *lock(&saved) = Some(cx.waker().clone());
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        Poll::<()>::Pending
    }));
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let (cancelled_tx, cancelled_rx) = mpsc::channel();
    let cancel_thread = std::thread::spawn(move || {
        drop(task);
        cancelled_tx.send(()).unwrap();
    });
    let cancelled = cancelled_rx.recv_timeout(Duration::from_secs(1));
    // Release the worker even if cancellation wrongly waits for its poll.
    release_tx.send(()).unwrap();
    assert!(
        cancelled.is_ok(),
        "dropping the handle must not wait for the running poll"
    );
    cancel_thread.join().unwrap();
    dropped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(lock(&waker).is_some());
}
