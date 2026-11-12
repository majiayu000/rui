//! macOS application runner

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityAnnouncementKind, AccessibilityBridge,
    AccessibilityTree,
};
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton};
use crate::core::frame_pipeline::{
    FramePipeline, FramePipelineError, FramePresentation, IdlePolicy,
};
use crate::core::geometry::{Point, Size};
use crate::core::presenter::Presenter;
use crate::core::window::{WindowId, WindowOptions};
use crate::devtools::RenderSample;
use crate::elements::element::Element;
use crate::platform::frame_driver::*;
use crate::platform::mac::clipboard::MacPasteboard;
use crate::platform::mac::menus::MacMenus;
use crate::platform::mac::prompts::run_path_prompts;
use crate::platform::mac::events::{
    MAC_REDRAW_EVENT_DATA, MacWindowEvent, post_application_event_from_any_thread,
};
use crate::platform::mac::frame::{NativeFrameEvents, dispatch_native_events};
use crate::platform::mac::frame_ticker::FrameTicker;
use crate::platform::mac::present_latency;
use crate::platform::mac::window::{
    MacAppEvent, MacWindow, create_window, has_pending_event, poll_windows,
};
use crate::platform::window::{
    PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind, PlatformRendererTarget,
    PlatformWindow, PlatformWindowError, PlatformWindowEvent,
};
use crate::renderer::metal::{MetalRenderer, PendingMetalRenderer, RetainedTarget};
use crate::renderer::{
    RendererBatchDiagnostics, RendererFramePhaseDurations, RendererResourceLimits,
    RendererTelemetryRecorder,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use std::time::Instant;

mod dogfood_automation;
mod event_loop;
mod window_driver;

use dogfood_automation::*;
use event_loop::*;
pub(crate) use crate::platform::frame_driver::{OrderedInputEvent, schedule_platform_redraw};
pub(crate) use window_driver::{NativeWindowDriver, native_window};

/// Run the application with default window options
pub fn run_app<F, E>(context: AppContext, build_root: F)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    run_app_with_options(context, build_root, WindowOptions::default());
}

/// Run the application with custom window options
///
/// `build_root` draws the main window, which `App` registered in `context`.
/// Windows that app code opens with [`AppContext::open_window`] are created
/// before the next frame; each has its own presenter, scene, focus, viewport and
/// renderer target, and all of them share `context`'s entities.
pub fn run_app_with_options<F, E>(context: AppContext, build_root: F, options: WindowOptions)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let mut context = context;
    context.use_background_threads();
    context.set_clipboard(MacPasteboard);
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let main_window = match context.active_window() {
        Some(window) => window,
        None => context.register_window(options.clone()),
    };
    // Wakes carry no window number: they are for the app, whichever windows exist.
    context.update_queue().set_waker(move || {
        if let Err(err) =
            post_application_event_from_any_thread(APP_EVENT_WINDOW_NUMBER, MAC_REDRAW_EVENT_DATA)
        {
            // The update stays queued and is applied with the next event.
            log::error!("failed to wake the event loop for an app handle update: {err}");
        }
    });

    // Each window's renderer is created on another thread while the window opens
    // and its first frame is built and laid out; the first present waits for it.
    let parts = create_window_parts(main_window, &options, &[], mtm);
    // The first open window's delegate is also the application's.
    parts.window.install_application_delegate(&app);
    let mut windows = vec![native_window(&mut context, parts, build_root)];
    // Our custom event loop does not call NSApplication::run, which normally
    // completes launch and makes the app available to accessibility clients.
    app.finishLaunching();
    app.activate();

    let profile_enabled = RendererTelemetryRecorder::enabled_from_env().is_some();
    let mut frame_ticker = FrameTicker::new(APP_EVENT_WINDOW_NUMBER);
    let mut held_events = HeldEvents::default();
    let mut menus = MacMenus::new(mtm);
    loop {
        open_pending_windows(&mut context, &mut windows, mtm);

        // While the display paces frames, the next tick is at most a refresh
        // away and its frame does whatever work is pending.
        let display_paced = frame_ticker.source().is_some();
        let wait_for_event = display_paced || !context.has_pending_work();
        let polled = {
            let mut mac_windows: Vec<&mut MacWindow> = windows
                .iter_mut()
                .map(|window| window.mac_window_mut())
                .collect();
            match poll_windows(&app, &mut mac_windows, wait_for_event) {
                Ok(polled) => polled,
                Err(err) => panic!("failed to poll platform window events: {}", err),
            }
        };
        let iteration_started_at = Instant::now();
        let mut ticked = false;
        for event in polled.app {
            match event {
                MacAppEvent::DisplayTick => {
                    frame_ticker.tick_handled();
                    context.begin_animation_frame();
                    ticked = true;
                }
                MacAppEvent::Wake => {}
            }
        }

        // Between ticks of a display-paced animation, events wait for the next
        // tick's frame. A frame drawn between ticks would reach the screen with
        // the tick's frame and one of them would be thrown away.
        if display_paced && !ticked {
            held_events.hold(windows.iter().map(|window| window.id()), polled.windows);
            run_between_frames(
                &mut context,
                &mut frame_ticker,
                windows.first().map(|window| window.mac_window()),
                &app,
                iteration_started_at,
                profile_enabled,
            );
            if !context.is_running() {
                break;
            }
            continue;
        }

        let mut close_requested = Vec::new();
        for (window, new_events) in windows.iter_mut().zip(polled.windows) {
            if !context.activate_window(window.id()) {
                continue;
            }
            let (events, held_since) = held_events.take(window.id(), new_events);
            if window.run_iteration(&mut context, events, held_since).close {
                close_requested.push(window.id());
            }
        }
        for window in close_requested {
            context.close_window(window);
        }
        for closed in context.take_closed_windows() {
            held_events.take(closed, Vec::new());
            if let Some(index) = windows.iter().position(|window| window.id() == closed) {
                let mut window = windows.remove(index);
                if let Err(err) = window.mac_window_mut().close() {
                    log::error!("failed to close platform window: {err}");
                }
                // NSApplication holds its delegate weakly; hand it to a window
                // that stays open before this one's delegate is released.
                if index == 0 {
                    match windows.first() {
                        Some(next) => next.mac_window().install_application_delegate(&app),
                        None => app.setDelegate(None),
                    }
                }
            }
        }

        if !context.is_running() {
            break;
        }
        menus.sync(
            &app,
            &mut context,
            windows.iter().map(|window| (window.id(), window.mac_window())),
        );
        run_path_prompts(&mut context);
        run_between_frames(
            &mut context,
            &mut frame_ticker,
            windows.first().map(|window| window.mac_window()),
            &app,
            iteration_started_at,
            profile_enabled,
        );
        // Idle work may have quit.
        if !context.is_running() {
            break;
        }
    }
}

/// Create the platform windows for windows app code opened since the last pass.
fn open_pending_windows(
    context: &mut AppContext,
    windows: &mut Vec<Box<dyn NativeWindowDriver>>,
    mtm: MainThreadMarker,
) {
    for pending in context.take_pending_windows() {
        if !context.activate_window(pending.id) {
            continue;
        }
        let open: Vec<&MacWindow> = windows.iter().map(|window| window.mac_window()).collect();
        let parts = create_window_parts(pending.id, &pending.options, &open, mtm);
        windows.push(pending.content.into_native(context, parts));
    }
}

/// A platform window with its renderer, before its content is attached.
pub(crate) struct NativeWindowParts {
    id: WindowId,
    window: MacWindow,
    pending_renderer: PendingMetalRenderer,
    viewport_size: Size,
    automation: Option<NativeDogfoodAutomation>,
}

fn create_window_parts(
    id: WindowId,
    options: &WindowOptions,
    open_windows: &[&MacWindow],
    mtm: MainThreadMarker,
) -> NativeWindowParts {
    let pending_renderer = MetalRenderer::spawn(RendererResourceLimits::default());
    // SAFETY: called on the main thread.
    let mut window = match unsafe { create_window(options, mtm) } {
        Ok(window) => window,
        Err(err) => panic!("failed to create platform window: {}", err),
    };
    if let Some(last) = open_windows.last() {
        window.cascade_after(last);
    }
    let attachment = match window.renderer_attachment() {
        Ok(attachment) => attachment,
        Err(err) => panic!("failed to attach renderer to platform window: {}", err),
    };
    if attachment.target != PlatformRendererTarget::MetalLayer {
        panic!(
            "{}",
            PlatformWindowError::backend(
                "macos",
                "Metal renderer requires a Metal layer attachment"
            )
        );
    }
    if let Err(err) = window.show() {
        panic!("failed to show platform window: {}", err);
    }
    if !window.accessibility_bridge_mut().native_attached() {
        panic!("macOS accessibility bridge failed to attach to the content view");
    }
    NativeWindowParts {
        id,
        window,
        pending_renderer,
        viewport_size: options.size,
        // The dogfood script drives the first window only.
        automation: if open_windows.is_empty() {
            NativeDogfoodAutomation::load_from_environment()
        } else {
            None
        },
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
