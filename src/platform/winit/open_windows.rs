//! The open winit windows: which one each event is for, and running their
//! frames with each window's frame state active.

use super::window::WinitWindowDriver;
use crate::core::PendingWindow;
use crate::core::app::AppContext;
use crate::platform::window::PlatformWindowError;
use winit::event::WindowEvent;

/// The open windows, in the order they opened.
#[derive(Default)]
pub(super) struct OpenWindows {
    windows: Vec<Box<dyn WinitWindowDriver>>,
}

impl OpenWindows {
    pub(super) fn push(&mut self, window: Box<dyn WinitWindowDriver>) {
        self.windows.push(window);
    }

    /// Create the windows app code opened since the last pass with `open`.
    /// Each window is active while it opens, so its first build and its
    /// presenter start from its own viewport. A window that fails to open is
    /// closed, as if its close button had been pressed.
    pub(super) fn open_pending(
        &mut self,
        context: &mut AppContext,
        mut open: impl FnMut(
            &mut AppContext,
            PendingWindow,
        ) -> Result<Box<dyn WinitWindowDriver>, PlatformWindowError>,
    ) {
        for pending in context.take_pending_windows() {
            if !context.activate_window(pending.id) {
                continue;
            }
            let id = pending.id;
            match open(context, pending) {
                Ok(window) => self.windows.push(window),
                Err(err) => {
                    log::error!("failed to open window {id:?}: {err}");
                    context.close_window(id);
                }
            }
        }
    }

    /// Queue `event` for the window `id` names. Returns whether it is open.
    pub(super) fn push_event(&mut self, id: winit::window::WindowId, event: &WindowEvent) -> bool {
        match self
            .windows
            .iter_mut()
            .find(|window| window.winit_id() == id)
        {
            Some(window) => {
                window.push_event(event);
                true
            }
            None => false,
        }
    }

    /// Run the frame of every open window, or only of `only`, each with its
    /// frame state active. Then close the windows that asked to and drop every
    /// window that closed, including those app code closed.
    pub(super) fn run(&mut self, context: &mut AppContext, only: Option<winit::window::WindowId>) {
        let mut close_requested = Vec::new();
        for window in &mut self.windows {
            if only.is_some_and(|id| id != window.winit_id())
                || !context.activate_window(window.id())
            {
                continue;
            }
            if window.run_iteration(context) {
                close_requested.push(window.id());
            }
        }
        for window in close_requested {
            context.close_window(window);
        }
        let closed = context.take_closed_windows();
        self.windows.retain(|window| !closed.contains(&window.id()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::core::window::{Window, WindowId, WindowOptions};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// What a window saw when its frame ran.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Frame {
        window: WindowId,
        active: Option<WindowId>,
        viewport: Size,
        events: usize,
    }

    type Frames = Rc<RefCell<Vec<Frame>>>;

    /// A window without a winit window or a renderer: it records its frames
    /// and asks to close after a close request, as the real driver does.
    struct FakeWindow {
        id: WindowId,
        winit_id: winit::window::WindowId,
        events: usize,
        close_requested: bool,
        frames: Frames,
    }

    impl WinitWindowDriver for FakeWindow {
        fn id(&self) -> WindowId {
            self.id
        }

        fn winit_id(&self) -> winit::window::WindowId {
            self.winit_id
        }

        fn push_event(&mut self, event: &WindowEvent) {
            self.events += 1;
            self.close_requested |= matches!(event, WindowEvent::CloseRequested);
        }

        fn run_iteration(&mut self, context: &mut AppContext) -> bool {
            self.frames.borrow_mut().push(Frame {
                window: self.id,
                active: context.active_window(),
                viewport: context.viewport_size(),
                events: std::mem::take(&mut self.events),
            });
            std::mem::take(&mut self.close_requested)
        }
    }

    fn fake(id: WindowId, winit_id: u64, frames: &Frames) -> Box<dyn WinitWindowDriver> {
        Box::new(FakeWindow {
            id,
            winit_id: winit_id.into(),
            events: 0,
            close_requested: false,
            frames: Rc::clone(frames),
        })
    }

    /// A frame of `window` run with its own frame state active.
    fn frame(window: WindowId, viewport: Size, events: usize) -> Frame {
        Frame {
            window,
            active: Some(window),
            viewport,
            events,
        }
    }

    /// A running app with a 320x240 first window and a 640x480 second one,
    /// winit ids 1 and 2.
    fn two_windows() -> (AppContext, OpenWindows, [WindowId; 2], Frames) {
        let mut context = AppContext::new();
        context.running = true;
        let first = context.register_window(WindowOptions::new().size(320.0, 240.0));
        let second = context.register_window(WindowOptions::new().size(640.0, 480.0));
        let frames = Frames::default();
        let mut windows = OpenWindows::default();
        windows.push(fake(first, 1, &frames));
        windows.push(fake(second, 2, &frames));
        (context, windows, [first, second], frames)
    }

    fn open_ids(context: &AppContext) -> Vec<WindowId> {
        context.windows().iter().map(Window::id).collect()
    }

    #[test]
    fn every_window_runs_with_its_own_frame_state_active() {
        let (mut context, mut windows, [first, second], frames) = two_windows();

        windows.run(&mut context, None);

        assert_eq!(
            *frames.borrow(),
            [
                frame(first, Size::new(320.0, 240.0), 0),
                frame(second, Size::new(640.0, 480.0), 0),
            ]
        );
    }

    #[test]
    fn events_reach_only_the_window_they_are_for() {
        let (mut context, mut windows, [_, second], frames) = two_windows();

        assert!(windows.push_event(2.into(), &WindowEvent::RedrawRequested));
        assert!(!windows.push_event(3.into(), &WindowEvent::RedrawRequested));
        windows.run(&mut context, Some(2.into()));

        assert_eq!(
            *frames.borrow(),
            [frame(second, Size::new(640.0, 480.0), 1)]
        );
    }

    #[test]
    fn closing_one_window_keeps_the_app_running_until_the_last() {
        let (mut context, mut windows, [_, second], frames) = two_windows();

        windows.push_event(1.into(), &WindowEvent::CloseRequested);
        windows.run(&mut context, None);
        assert_eq!(open_ids(&context), [second]);
        assert!(context.is_running());
        assert!(
            !windows.push_event(1.into(), &WindowEvent::RedrawRequested),
            "the closed window's driver is dropped"
        );

        frames.borrow_mut().clear();
        windows.run(&mut context, None);
        assert_eq!(
            *frames.borrow(),
            [frame(second, Size::new(640.0, 480.0), 0)]
        );

        windows.push_event(2.into(), &WindowEvent::CloseRequested);
        windows.run(&mut context, None);
        assert!(open_ids(&context).is_empty());
        assert!(!context.is_running());
    }

    #[test]
    fn windows_app_code_closes_are_dropped() {
        let (mut context, mut windows, [first, second], frames) = two_windows();

        assert!(context.close_window(second));
        windows.run(&mut context, None);

        assert_eq!(*frames.borrow(), [frame(first, Size::new(320.0, 240.0), 0)]);
        assert!(!windows.push_event(2.into(), &WindowEvent::RedrawRequested));
        assert!(context.is_running());
    }

    #[test]
    fn opened_windows_are_created_with_their_frame_state_active() {
        let (mut context, mut windows, [first, second], frames) = two_windows();
        let third = context.open_window(WindowOptions::new().size(500.0, 300.0), |_| {
            crate::elements::div()
        });
        assert_eq!(context.active_window(), Some(first));

        let mut opened = Vec::new();
        windows.open_pending(&mut context, |context, pending| {
            opened.push((pending.id, context.active_window(), context.viewport_size()));
            Ok(fake(pending.id, 3, &frames))
        });
        assert_eq!(opened, [(third, Some(third), Size::new(500.0, 300.0))]);

        windows.run(&mut context, None);
        assert_eq!(
            *frames.borrow(),
            [
                frame(first, Size::new(320.0, 240.0), 0),
                frame(second, Size::new(640.0, 480.0), 0),
                frame(third, Size::new(500.0, 300.0), 0),
            ]
        );
    }

    #[test]
    fn a_window_that_fails_to_open_is_closed() {
        let (mut context, mut windows, [first, second], _) = two_windows();
        context.open_window(WindowOptions::new(), |_| crate::elements::div());

        windows.open_pending(&mut context, |_, _| {
            Err(PlatformWindowError::backend("winit", "no adapter"))
        });

        assert_eq!(open_ids(&context), [first, second]);
        assert!(context.is_running());
    }
}
