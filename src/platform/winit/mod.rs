//! Window and input through winit, frames through the wgpu backend: the runner
//! for Linux and Windows, and for macOS builds without the `metal` feature.
//!
//! It drives the shared frame pipeline for the main window and for each window
//! app code opens with [`AppContext::open_window`], every one with its own
//! presenter, surface and renderer, and copy, cut and paste go through the
//! system clipboard (arboard). IME composition goes to the focused text field.
//! File drops arrive on X11 and Windows. App commands can use an in-window
//! menu. The native accessibility bridge and system menu bar use AppKit.

mod clipboard;
mod input;
mod open_windows;
mod window;

pub(crate) use window::{WinitWindowDriver, WinitWindowParts, winit_window};

use crate::core::app::AppContext;
use crate::core::window::{WindowId, WindowOptions};
use crate::elements::element::Element;
use clipboard::WindowClipboard;
use open_windows::OpenWindows;
use std::sync::Mutex;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

/// Wakes the event loop for app handle updates posted from other threads.
struct Wake;

/// Builds the main window's first tree once its winit window exists.
type MainContent = Box<dyn FnOnce(&mut AppContext, WinitWindowParts) -> Box<dyn WinitWindowDriver>>;

/// Run the application in winit windows. `build_root` draws the main window,
/// which `App` registered in `context`. Windows that app code opens with
/// [`AppContext::open_window`] are created before the next frame.
pub fn run_app_with_options<F, E>(context: AppContext, build_root: F, options: WindowOptions)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let mut context = context;
    context.use_background_threads();
    let clipboard = clipboard::install_system_clipboard(&mut context);
    let main_window = match context.active_window() {
        Some(window) => window,
        None => context.register_window(options.clone()),
    };
    let event_loop = match EventLoop::<Wake>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(err) => panic!("failed to create the winit event loop: {err}"),
    };
    let proxy = Mutex::new(event_loop.create_proxy());
    context.update_queue().set_waker(move || {
        let sent = match proxy.lock() {
            Ok(proxy) => proxy.send_event(Wake).is_ok(),
            Err(_) => false,
        };
        if !sent {
            // The update stays queued and is applied with the next event.
            log::error!("failed to wake the winit event loop for an app handle update");
        }
    });
    let main: MainContent = Box::new(move |context: &mut AppContext, parts: WinitWindowParts| {
        winit_window(context, parts, build_root)
    });
    let mut runner = Runner {
        context,
        instance: wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        clipboard,
        main: Some((main_window, options, main)),
        windows: OpenWindows::default(),
        next_tick: None,
    };
    if let Err(err) = event_loop.run_app(&mut runner) {
        panic!("winit event loop failed: {err}");
    }
}

struct Runner {
    context: AppContext,
    instance: wgpu::Instance,
    /// The system clipboard, for the windows' `PlatformWindow` methods.
    clipboard: WindowClipboard,
    /// The main window until the event loop can create it.
    main: Option<(WindowId, WindowOptions, MainContent)>,
    windows: OpenWindows,
    /// When the next animation frame is due, while an animation asks for one.
    next_tick: Option<Instant>,
}

impl Runner {
    /// Run the frame of every open window, or only of `only`, then stop the
    /// event loop if the app quit.
    fn run_windows(&mut self, event_loop: &ActiveEventLoop, only: Option<winit::window::WindowId>) {
        self.windows.run(&mut self.context, only);
        if !self.context.is_running() {
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<Wake> for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some((id, options, main)) = self.main.take() else {
            return;
        };
        self.context.activate_window(id);
        match WinitWindowParts::create(
            event_loop,
            &self.instance,
            id,
            &options,
            self.clipboard.clone(),
        ) {
            Ok(parts) => self.windows.push(main(&mut self.context, parts)),
            Err(err) => panic!("failed to open the main window: {err}"),
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: Wake) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self.windows.push_event(id, &event) && matches!(event, WindowEvent::RedrawRequested) {
            self.run_windows(event_loop, Some(id));
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let instance = &self.instance;
        let clipboard = &self.clipboard;
        self.windows
            .open_pending(&mut self.context, |context, pending| {
                let parts = WinitWindowParts::create(
                    event_loop,
                    instance,
                    pending.id,
                    &pending.options,
                    clipboard.clone(),
                )?;
                Ok(pending.content.into_winit(context, parts))
            });
        let now = Instant::now();
        self.context.fire_due_frames(now);
        if self.context.animation_frame_requested() && self.next_tick.is_none_or(|tick| tick <= now)
        {
            self.context.begin_animation_frame();
            self.next_tick = Some(now + self.context.frame_interval());
        }
        self.run_windows(event_loop, None);
        if event_loop.exiting() {
            return;
        }

        if self.context.has_idle_work() {
            let deadline = now + self.context.idle_budget();
            self.context.run_idle_tasks(|| Instant::now() < deadline);
        }
        let control_flow = if self.context.animation_frame_requested() {
            ControlFlow::WaitUntil(*self.next_tick.get_or_insert(now))
        } else {
            self.next_tick = None;
            if self.context.has_pending_work() {
                ControlFlow::Poll
            } else if let Some(deadline) = self.context.next_frame_deadline() {
                ControlFlow::WaitUntil(deadline)
            } else {
                ControlFlow::Wait
            }
        };
        event_loop.set_control_flow(control_flow);
        if !self.context.is_running() {
            event_loop.exit();
        }
    }
}
