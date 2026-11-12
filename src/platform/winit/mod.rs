//! Window and input through winit, frames through the wgpu backend: the runner
//! for Linux and Windows, and for macOS builds without the `metal` feature.
//!
//! It drives the shared frame pipeline for the main window. IME composition,
//! accessibility, the system clipboard, menus, file drops and further windows
//! are macOS-only for now.

mod input;
mod window;

use crate::core::app::AppContext;
use crate::core::window::WindowOptions;
use crate::elements::element::Element;
use std::sync::Mutex;
use std::time::Instant;
use window::WindowDriver;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};

/// Wakes the event loop for app handle updates posted from other threads.
struct Wake;

/// Run the application in a winit window. `build_root` draws the main window,
/// which `App` registered in `context`.
pub fn run_app_with_options<F, E>(context: AppContext, build_root: F, options: WindowOptions)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let mut context = context;
    context.use_background_threads();
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
    let mut runner = Runner {
        context,
        instance: wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        main: Some((main_window, options, build_root)),
        window: None,
        next_tick: None,
    };
    if let Err(err) = event_loop.run_app(&mut runner) {
        panic!("winit event loop failed: {err}");
    }
}

struct Runner<F, E> {
    context: AppContext,
    instance: wgpu::Instance,
    /// The main window until the event loop can create it.
    main: Option<(crate::core::window::WindowId, WindowOptions, F)>,
    window: Option<WindowDriver<F, E>>,
    /// When the next animation frame is due, while an animation asks for one.
    next_tick: Option<Instant>,
}

impl<F, E> Runner<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn run_window(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_mut() else {
            return;
        };
        if !self.context.activate_window(window.id()) {
            return;
        }
        if window.run_iteration(&mut self.context) {
            self.context.close_window(window.id());
        }
        for closed in self.context.take_closed_windows() {
            if self
                .window
                .as_ref()
                .is_some_and(|window| window.id() == closed)
            {
                self.window = None;
            }
        }
        if !self.context.is_running() {
            event_loop.exit();
        }
    }
}

impl<F, E> ApplicationHandler<Wake> for Runner<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some((id, options, build_root)) = self.main.take() else {
            return;
        };
        self.context.activate_window(id);
        match WindowDriver::open(
            event_loop,
            &self.instance,
            &mut self.context,
            id,
            &options,
            build_root,
        ) {
            Ok(window) => self.window = Some(window),
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
        let Some(window) = self
            .window
            .as_mut()
            .filter(|window| window.winit_id() == id)
        else {
            return;
        };
        window.push_event(&event);
        if matches!(event, WindowEvent::RedrawRequested) {
            self.run_window(event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        for pending in self.context.take_pending_windows() {
            log::error!(
                "the winit runner shows only the main window; closing {:?}",
                pending.id
            );
            self.context.close_window(pending.id);
        }
        let now = Instant::now();
        if self.context.animation_frame_requested() && self.next_tick.is_none_or(|tick| tick <= now)
        {
            self.context.begin_animation_frame();
            self.next_tick = Some(now + self.context.frame_interval());
        }
        self.run_window(event_loop);
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
