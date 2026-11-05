//! Prioritized work on the UI thread, and frames requested by animations.

use crate::core::app::{AppContext, RedrawSource};
use crate::core::entity::EntityId;
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

/// Order in which deferred UI-thread work runs.
///
/// Before a frame, input work runs before animation work, which runs before
/// layout work. Idle work runs only between frames, within the idle budget
/// (see [`AppContext::idle_budget`]), so it cannot delay the next frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaskPriority {
    Input,
    Animation,
    Layout,
    Idle,
}

type Task = Box<dyn FnOnce(&mut AppContext)>;

/// The display refresh interval assumed until the platform reports its own.
const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

#[derive(Default)]
pub(crate) struct ForegroundQueue {
    frame_tasks: [VecDeque<Task>; 3],
    idle_tasks: VecDeque<Task>,
}

impl ForegroundQueue {
    fn push(&mut self, priority: TaskPriority, task: Task) {
        match priority {
            TaskPriority::Input => self.frame_tasks[0].push_back(task),
            TaskPriority::Animation => self.frame_tasks[1].push_back(task),
            TaskPriority::Layout => self.frame_tasks[2].push_back(task),
            TaskPriority::Idle => self.idle_tasks.push_back(task),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.idle_tasks.is_empty() && self.frame_tasks.iter().all(VecDeque::is_empty)
    }
}

/// Who asked for the next display frame.
#[derive(Debug, Default)]
pub(crate) struct AnimationFrameRequests {
    whole_app: bool,
    entities: HashSet<EntityId>,
}

impl AnimationFrameRequests {
    fn is_empty(&self) -> bool {
        !self.whole_app && self.entities.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameTiming {
    interval: Duration,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self {
            interval: DEFAULT_FRAME_INTERVAL,
        }
    }
}

impl AppContext {
    /// Run `task` on the UI thread at `priority`.
    ///
    /// Input, animation and layout work runs at the start of the next frame, in
    /// that order; work they queue runs the frame after. Idle work runs between
    /// frames while time is left in the idle budget, and yields to pending input.
    pub fn defer(&mut self, priority: TaskPriority, task: impl FnOnce(&mut AppContext) + 'static) {
        self.foreground.push(priority, Box::new(task));
    }

    /// Render again on the next display refresh, rebuilding the whole tree.
    ///
    /// Call it from every frame that animates. When no frame asks, the display
    /// stops driving frames and the window goes idle.
    pub fn request_animation_frame(&mut self) {
        self.animation_frames.whole_app = true;
    }

    /// Render `entity` again on the next display refresh.
    pub fn request_animation_frame_for(&mut self, entity: EntityId) {
        self.animation_frames.entities.insert(entity);
    }

    /// Whether a frame asked to be redrawn on the next display refresh.
    pub fn animation_frame_requested(&self) -> bool {
        !self.animation_frames.is_empty()
    }

    /// Whether idle work is still queued.
    pub fn has_idle_work(&self) -> bool {
        !self.foreground.idle_tasks.is_empty()
    }

    /// Whether the event loop has anything to do before it can sleep.
    pub fn has_pending_work(&self) -> bool {
        self.any_window_has_frame_work() || !self.foreground.is_empty()
    }

    /// The display refresh interval frames are paced to.
    pub fn frame_interval(&self) -> Duration {
        self.frame_timing.interval
    }

    /// Time idle work may use after a frame starts: half the refresh interval, so
    /// the rest of the interval stays free for the next frame.
    pub fn idle_budget(&self) -> Duration {
        self.frame_timing.interval / 2
    }

    pub(crate) fn set_frame_interval(&mut self, interval: Duration) {
        self.frame_timing.interval = interval;
    }

    /// The display refreshed: mark everything that asked for this frame.
    ///
    /// Returns whether anything had asked.
    pub(crate) fn begin_animation_frame(&mut self) -> bool {
        let requests = std::mem::take(&mut self.animation_frames);
        if requests.is_empty() {
            return false;
        }
        if requests.whole_app {
            self.frame.needs_rebuild = true;
        }
        for entity in requests.entities {
            self.notify(entity);
        }
        self.mark_redraw_from(RedrawSource::AnimationFrame);
        true
    }

    /// Run input, then animation, then layout work queued before this call.
    pub(crate) fn run_frame_tasks(&mut self) {
        let queued = std::mem::take(&mut self.foreground.frame_tasks);
        for tasks in queued {
            for task in tasks {
                task(self);
            }
        }
    }

    /// Run idle work in the order it was queued while `should_continue` allows.
    ///
    /// `should_continue` is asked before each task; runners stop when the idle
    /// budget is spent or input is waiting. Returns how many tasks ran.
    pub(crate) fn run_idle_tasks(&mut self, mut should_continue: impl FnMut() -> bool) -> usize {
        let mut ran = 0;
        while !self.foreground.idle_tasks.is_empty() && should_continue() {
            let Some(task) = self.foreground.idle_tasks.pop_front() else {
                break;
            };
            task(self);
            ran += 1;
        }
        ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn recorder() -> (Rc<RefCell<Vec<&'static str>>>, AppContext) {
        let mut context = AppContext::new();
        context.frame.needs_rebuild = false;
        context.frame.dirty = false;
        (Rc::new(RefCell::new(Vec::new())), context)
    }

    fn record(
        log: &Rc<RefCell<Vec<&'static str>>>,
        label: &'static str,
    ) -> impl FnOnce(&mut AppContext) + 'static {
        let log = Rc::clone(log);
        move |_| log.borrow_mut().push(label)
    }

    #[test]
    fn frame_tasks_run_by_priority_then_queue_order() {
        let (log, mut context) = recorder();
        context.defer(TaskPriority::Layout, record(&log, "layout"));
        context.defer(TaskPriority::Idle, record(&log, "idle"));
        context.defer(TaskPriority::Animation, record(&log, "animation 1"));
        context.defer(TaskPriority::Input, record(&log, "input"));
        context.defer(TaskPriority::Animation, record(&log, "animation 2"));

        context.run_frame_tasks();

        assert_eq!(
            *log.borrow(),
            ["input", "animation 1", "animation 2", "layout"]
        );
        assert!(context.has_idle_work());
    }

    #[test]
    fn work_queued_by_a_frame_task_waits_for_the_next_frame() {
        let (log, mut context) = recorder();
        let inner = record(&log, "queued during frame");
        context.defer(TaskPriority::Layout, move |cx| {
            cx.defer(TaskPriority::Input, inner);
        });

        context.run_frame_tasks();
        assert!(log.borrow().is_empty());
        assert!(context.has_pending_work());

        context.run_frame_tasks();
        assert_eq!(*log.borrow(), ["queued during frame"]);
        assert!(!context.has_pending_work());
    }

    #[test]
    fn idle_tasks_stop_when_the_budget_says_so() {
        let (log, mut context) = recorder();
        for label in ["idle 1", "idle 2", "idle 3"] {
            context.defer(TaskPriority::Idle, record(&log, label));
        }

        let mut budget = 2_u32;
        let ran = context.run_idle_tasks(|| {
            let allowed = budget > 0;
            budget = budget.saturating_sub(1);
            allowed
        });

        assert_eq!(ran, 2);
        assert_eq!(*log.borrow(), ["idle 1", "idle 2"]);
        assert!(context.has_idle_work());
        assert_eq!(context.run_idle_tasks(|| true), 1);
        assert!(!context.has_idle_work());
    }

    #[test]
    fn idle_budget_is_half_the_frame_interval() {
        let mut context = AppContext::new();
        assert_eq!(context.frame_interval(), DEFAULT_FRAME_INTERVAL);
        context.set_frame_interval(Duration::from_micros(8_333));
        assert_eq!(context.idle_budget(), Duration::from_nanos(4_166_500));
    }

    #[test]
    fn animation_frame_notifies_only_the_entities_that_asked() {
        let (_, mut context) = recorder();
        let animating = context.create(());
        let still = context.create(());

        assert!(!context.begin_animation_frame());
        assert!(!context.has_frame_work());

        context.request_animation_frame_for(animating.id());
        // Asking for a frame does not draw one until the display refreshes.
        assert!(context.animation_frame_requested());
        assert!(!context.has_frame_work());

        assert!(context.begin_animation_frame());
        assert!(context.frame.pending_updates.contains(&animating.id()));
        assert!(!context.frame.pending_updates.contains(&still.id()));
        assert!(context.has_frame_work());
        assert!(!context.animation_frame_requested());
        assert_eq!(context.redraw_source_counts().animation_frame, 1);
    }

    #[test]
    fn whole_app_animation_frame_requests_a_rebuild() {
        let (_, mut context) = recorder();
        context.request_animation_frame();

        assert!(context.begin_animation_frame());
        assert!(context.frame.needs_rebuild);
    }
}
