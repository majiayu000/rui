//! What the app loop does between and around frames: held events, idle work,
//! viewport sync, redraw scheduling and input ordering.

use super::*;

/// Window events held back until the next display tick, in arrival order.
#[derive(Default)]
pub(super) struct HeldEvents {
    windows: Vec<(WindowId, Instant, Vec<MacWindowEvent>)>,
}

impl HeldEvents {
    pub(super) fn hold(
        &mut self,
        windows: impl Iterator<Item = WindowId>,
        polled: Vec<Vec<MacWindowEvent>>,
    ) {
        for (window, events) in windows.zip(polled) {
            if events.is_empty() {
                continue;
            }
            match self.windows.iter_mut().find(|(id, ..)| *id == window) {
                Some((.., held)) => held.extend(events),
                None => self.windows.push((window, Instant::now(), events)),
            }
        }
    }

    /// The window's held events followed by `new_events`, and when the first
    /// held one arrived.
    pub(super) fn take(
        &mut self,
        window: WindowId,
        new_events: Vec<MacWindowEvent>,
    ) -> (Vec<MacWindowEvent>, Option<Instant>) {
        match self.windows.iter().position(|(id, ..)| *id == window) {
            Some(index) => {
                let (_, held_since, mut events) = self.windows.swap_remove(index);
                events.extend(new_events);
                (events, Some(held_since))
            }
            None => (new_events, None),
        }
    }
}

/// Window number of application-defined events meant for the app, not a window.
pub(super) const APP_EVENT_WINDOW_NUMBER: isize = 0;

/// Keep frame ticks running exactly while an animation asks for frames, then
/// spend what is left of the idle budget on idle work.
///
/// Idle work stops at the budget, measured from when this loop iteration
/// started, and as soon as an event is waiting, so input and the next frame
/// tick are never queued behind it.
pub(super) fn run_between_frames(
    context: &mut AppContext,
    frame_ticker: &mut FrameTicker,
    window: Option<&MacWindow>,
    app: &NSApplication,
    iteration_started_at: Instant,
    report_frame_ticks: bool,
) {
    let animating = context.animation_frame_requested();
    let source_before = frame_ticker.source();
    let display = window.and_then(MacWindow::display_id);
    if let Err(err) = frame_ticker.update(animating, display, context.frame_interval()) {
        panic!("failed to drive animation frames: {err}");
    }
    if let Some(interval) = frame_ticker.refresh_interval() {
        context.set_frame_interval(interval);
    }
    let source = frame_ticker.source();
    if report_frame_ticks && source != source_before {
        let source = match source {
            Some(source) => format!("\"{}\"", source.name()),
            None => String::from("null"),
        };
        eprintln!(
            "{{\"schema\":\"rui.display_link.v1\",\"running\":{},\"source\":{},\"refresh_interval_ns\":{}}}",
            animating,
            source,
            context.frame_interval().as_nanos()
        );
    }

    if context.has_idle_work() {
        let deadline = iteration_started_at + context.idle_budget();
        context.run_idle_tasks(|| Instant::now() < deadline && !has_pending_event(app));
    }
}

/// Drop obsolete focus announcements retained across failed publish frames.
///
/// Action feedback is kept in place. Matching FocusChanged entries are coalesced
/// to the latest one that still matches the published tree's focused node (and
/// the presenter's current focus), emitted at that latest entry's original index
/// so focus stays ahead of later ActionFeedback instead of being appended after
/// every action. Clearing focus discards all pending FocusChanged.
pub(crate) fn filter_retained_accessibility_announcements(
    announcements: Vec<AccessibilityAnnouncement>,
    published_tree: &AccessibilityTree,
    focused: Option<ElementId>,
) -> Vec<AccessibilityAnnouncement> {
    let published_focus = focused.filter(|id| {
        published_tree
            .find(*id)
            .is_some_and(|node| node.a11y_focused())
    });

    let latest_matching_focus_idx = announcements.iter().enumerate().rev().find_map(|(idx, a)| {
        (a.kind() == AccessibilityAnnouncementKind::FocusChanged
            && Some(a.node_id()) == published_focus)
            .then_some(idx)
    });

    let mut filtered = Vec::with_capacity(announcements.len());
    for (idx, announcement) in announcements.into_iter().enumerate() {
        match announcement.kind() {
            AccessibilityAnnouncementKind::FocusChanged => {
                if Some(idx) == latest_matching_focus_idx {
                    filtered.push(announcement);
                }
            }
            AccessibilityAnnouncementKind::ActionFeedback => filtered.push(announcement),
        }
    }
    filtered
}
