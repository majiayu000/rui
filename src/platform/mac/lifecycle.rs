//! AppKit delegate-backed lifecycle signals.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSApplicationTerminateReply, NSWindow, NSWindowDelegate,
};
use objc2_foundation::{MainThreadMarker, NSNotification, NSObject, NSObjectProtocol};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacLifecycleEvent {
    CloseRequested,
    QuitRequested,
    ReopenRequested,
    ApplicationActivated(bool),
    FocusChanged(bool),
    Resized,
    Miniaturized(bool),
}

#[derive(Debug, Default)]
pub(crate) struct MacLifecycleDelegateIvars {
    events: RefCell<MacLifecycleEventQueue>,
}

#[derive(Debug, Default)]
pub(crate) struct MacLifecycleEventQueue {
    events: Vec<MacLifecycleEvent>,
    close_requested: bool,
}

impl MacLifecycleEventQueue {
    fn push(&mut self, event: MacLifecycleEvent) {
        if matches!(event, MacLifecycleEvent::CloseRequested) {
            if self.close_requested {
                return;
            }
            self.close_requested = true;
        }

        if self.events.last() == Some(&event) {
            return;
        }
        self.events.push(event);
    }

    fn drain(&mut self) -> Vec<MacLifecycleEvent> {
        self.events.drain(..).collect()
    }
}

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have subclassing requirements.
    // - MacLifecycleDelegate has no custom Drop implementation.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MacLifecycleDelegateIvars]
    pub(crate) struct MacLifecycleDelegate;

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for MacLifecycleDelegate {}

    // SAFETY: NSApplicationDelegate methods record main-thread AppKit signals.
    unsafe impl NSApplicationDelegate for MacLifecycleDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        fn application_should_terminate(
            &self,
            _sender: &NSApplication,
        ) -> NSApplicationTerminateReply {
            self.push_event(MacLifecycleEvent::QuitRequested);
            NSApplicationTerminateReply::TerminateCancel
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn application_should_handle_reopen(
            &self,
            _sender: &NSApplication,
            _has_visible_windows: bool,
        ) -> bool {
            self.push_event(MacLifecycleEvent::ReopenRequested);
            true
        }

        #[unsafe(method(applicationDidBecomeActive:))]
        fn application_did_become_active(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::ApplicationActivated(true));
        }

        #[unsafe(method(applicationDidResignActive:))]
        fn application_did_resign_active(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::ApplicationActivated(false));
        }
    }

    // SAFETY: NSWindowDelegate methods only enqueue lifecycle signals.
    unsafe impl NSWindowDelegate for MacLifecycleDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            self.push_event(MacLifecycleEvent::CloseRequested);
            true
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::CloseRequested);
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::Resized);
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::FocusChanged(true));
        }

        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::FocusChanged(false));
        }

        #[unsafe(method(windowDidMiniaturize:))]
        fn window_did_miniaturize(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::Miniaturized(true));
        }

        #[unsafe(method(windowDidDeminiaturize:))]
        fn window_did_deminiaturize(&self, _notification: &NSNotification) {
            self.push_event(MacLifecycleEvent::Miniaturized(false));
        }
    }
);

impl MacLifecycleDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MacLifecycleDelegateIvars::default());
        // SAFETY: The NSObject init selector has the standard init signature.
        unsafe { msg_send![super(this), init] }
    }

    pub(crate) fn install_as_app_delegate(&self, app: &NSApplication) {
        app.setDelegate(Some(ProtocolObject::from_ref(self)));
    }

    pub(crate) fn install_as_window_delegate(&self, window: &NSWindow) {
        window.setDelegate(Some(ProtocolObject::from_ref(self)));
    }

    pub(crate) fn drain_events(&self) -> Vec<MacLifecycleEvent> {
        self.ivars().events.borrow_mut().drain()
    }

    fn push_event(&self, event: MacLifecycleEvent) {
        self.ivars().events.borrow_mut().push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_event_queue_coalesces_repeated_delegate_events() {
        let mut queue = MacLifecycleEventQueue::default();

        queue.push(MacLifecycleEvent::Resized);
        queue.push(MacLifecycleEvent::Resized);
        queue.push(MacLifecycleEvent::FocusChanged(true));
        queue.push(MacLifecycleEvent::FocusChanged(true));
        queue.push(MacLifecycleEvent::FocusChanged(false));

        assert_eq!(
            queue.drain(),
            vec![
                MacLifecycleEvent::Resized,
                MacLifecycleEvent::FocusChanged(true),
                MacLifecycleEvent::FocusChanged(false),
            ]
        );
    }

    #[test]
    fn lifecycle_event_queue_deduplicates_close_requests() {
        let mut queue = MacLifecycleEventQueue::default();

        queue.push(MacLifecycleEvent::CloseRequested);
        queue.push(MacLifecycleEvent::FocusChanged(false));
        queue.push(MacLifecycleEvent::CloseRequested);

        assert_eq!(
            queue.drain(),
            vec![
                MacLifecycleEvent::CloseRequested,
                MacLifecycleEvent::FocusChanged(false),
            ]
        );
    }
}
