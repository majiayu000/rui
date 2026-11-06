//! macOS application runner

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityAnnouncementKind, AccessibilityBridge,
    AccessibilityTree,
};
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::frame_pipeline::{
    FramePipeline, FramePipelineError, FramePresentation, IdlePolicy,
};
use crate::core::geometry::{Point, Size};
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::core::window::{WindowId, WindowOptions};
use crate::devtools::RenderSample;
use crate::elements::element::{Element, PointerEvent, PointerEventKind};
use crate::platform::mac::accessibility::MacAccessibilityActionRequest;
use crate::platform::mac::clipboard::MacPasteboard;
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
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererTarget, PlatformWindow, PlatformWindowError, PlatformWindowEvent,
};
use crate::renderer::metal::{MetalRenderer, PendingMetalRenderer, RetainedTarget};
use crate::renderer::{
    RendererBatchDiagnostics, RendererFramePhaseDurations, RendererResourceLimits,
    RendererTelemetryRecorder,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use std::time::Instant;

const NATIVE_DOGFOOD_INPUT_ID: ElementId = ElementId(29_001);

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
    app.activate();

    let profile_enabled = RendererTelemetryRecorder::enabled_from_env().is_some();
    let mut frame_ticker = FrameTicker::new(APP_EVENT_WINDOW_NUMBER);
    let mut held_events = HeldEvents::default();
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

/// Window events held back until the next display tick, in arrival order.
#[derive(Default)]
struct HeldEvents {
    windows: Vec<(WindowId, Instant, Vec<MacWindowEvent>)>,
}

impl HeldEvents {
    fn hold(&mut self, windows: impl Iterator<Item = WindowId>, polled: Vec<Vec<MacWindowEvent>>) {
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
    fn take(
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
const APP_EVENT_WINDOW_NUMBER: isize = 0;

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

/// What a window asked of the app loop in one iteration.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct WindowRequests {
    close: bool,
}

/// One open window, behind the type the app loop stores.
pub(crate) trait NativeWindowDriver {
    fn id(&self) -> WindowId;
    fn mac_window(&self) -> &MacWindow;
    fn mac_window_mut(&mut self) -> &mut MacWindow;
    /// Handle this window's events and draw it if it has work. `context` has
    /// this window active. `held_since` is when the first of `events` arrived,
    /// if they were held for this frame.
    fn run_iteration(
        &mut self,
        context: &mut AppContext,
        events: Vec<MacWindowEvent>,
        held_since: Option<Instant>,
    ) -> WindowRequests;
}

struct NativeWindow<F, E> {
    id: WindowId,
    window: MacWindow,
    /// Set at the first present, from `pending_renderer`.
    renderer: Option<MetalRenderer>,
    pending_renderer: Option<PendingMetalRenderer>,
    /// This window's last frame, so each frame redraws only what changed.
    retained: RetainedTarget,
    presenter: Presenter<E>,
    build_root: F,
    last_viewport_size: Size,
    profile_recorder: Option<RendererTelemetryRecorder>,
    automation: Option<NativeDogfoodAutomation>,
    ime_state: crate::platform::mac::ime_state::NativeImeState,
}

/// Attach `build_root` to a created platform window. `context` has the window
/// active, so the first build sees its viewport.
pub(crate) fn native_window<F, E>(
    context: &mut AppContext,
    parts: NativeWindowParts,
    mut build_root: F,
) -> Box<dyn NativeWindowDriver>
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    // AppContext is the single owner of viewport size; the window options only
    // provide its initial value.
    context.set_viewport_size(parts.viewport_size);
    let root = build_root(context);
    let mut presenter = Presenter::with_root(parts.viewport_size, root);
    presenter.set_accessibility_announcements_enabled(true);
    Box::new(NativeWindow {
        id: parts.id,
        window: parts.window,
        renderer: None,
        pending_renderer: Some(parts.pending_renderer),
        retained: RetainedTarget::new(),
        presenter,
        build_root,
        last_viewport_size: parts.viewport_size,
        profile_recorder: RendererTelemetryRecorder::enabled_from_env(),
        automation: parts.automation,
        ime_state: Default::default(),
    })
}

impl<F, E> NativeWindowDriver for NativeWindow<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn id(&self) -> WindowId {
        self.id
    }

    fn mac_window(&self) -> &MacWindow {
        &self.window
    }

    fn mac_window_mut(&mut self) -> &mut MacWindow {
        &mut self.window
    }

    fn run_iteration(
        &mut self,
        context: &mut AppContext,
        mut platform_events: Vec<MacWindowEvent>,
        held_since: Option<Instant>,
    ) -> WindowRequests {
        let NativeWindow {
            window,
            renderer,
            pending_renderer,
            retained,
            presenter,
            build_root,
            last_viewport_size,
            profile_recorder,
            automation: native_dogfood_automation,
            ime_state: native_ime_state,
            ..
        } = self;
        let mut requests = WindowRequests::default();
        let mut phases = RendererFramePhaseDurations::default();
        let mut viewport_size = match window.content_size() {
            Ok(size) => size,
            Err(err) => panic!("failed to read platform window size: {}", err),
        };
        let mut ordered_input_events = Vec::new();
        let mut focus_changed = None;
        let mut close_requested = false;
        let mut automation_focused_element = None;
        if let Some(automation) = native_dogfood_automation {
            automation_focused_element =
                automation.append_events(window, presenter.scene(), &mut platform_events);
        }
        let event_observed_at = if platform_events.is_empty() {
            None
        } else {
            Some(held_since.unwrap_or_else(Instant::now))
        };
        let present_profile = profile_recorder.as_ref().map(|recorder| {
            let event_observed_at = event_observed_at.map(|observed_at| {
                present_latency::media_time() - observed_at.elapsed().as_secs_f64()
            });
            (recorder.next_frame_index(), event_observed_at)
        });

        for event in platform_events {
            match event {
                MacWindowEvent::Accessibility(request) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::Accessibility(request));
                }
                MacWindowEvent::Text(command) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::Text(command));
                }
                MacWindowEvent::FileDrag(event) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::FileDrag(event));
                }
                // The app loop takes display ticks before windows see events.
                MacWindowEvent::DisplayTick => {}
                MacWindowEvent::Platform(event) => {
                    mark_platform_event_redraw(&event, context);
                    match event {
                        PlatformWindowEvent::Created | PlatformWindowEvent::RedrawRequested => {}
                        PlatformWindowEvent::Resized(size) => viewport_size = size,
                        PlatformWindowEvent::ScaleFactorChanged(_)
                        | PlatformWindowEvent::ApplicationActivated(_)
                        | PlatformWindowEvent::Minimized(_) => {}
                        PlatformWindowEvent::ReopenRequested => {
                            if let Err(err) = window.show() {
                                panic!("failed to reopen platform window: {}", err);
                            }
                        }
                        PlatformWindowEvent::FocusChanged(focused) => {
                            focus_changed = Some(focused);
                        }
                        PlatformWindowEvent::CloseRequested => {
                            close_requested = true;
                            requests.close = true;
                        }
                        PlatformWindowEvent::QuitRequested => {
                            close_requested = true;
                            context.quit();
                        }
                        PlatformWindowEvent::Input(input) => {
                            append_input_event(input, &mut ordered_input_events)
                        }
                    }
                }
            }
        }

        let viewport_changed =
            synchronize_viewport_after_platform_events(context, viewport_size, *last_viewport_size);
        if viewport_changed {
            schedule_platform_redraw(&*window, context, RedrawSource::PlatformResize);
        }

        let frame_events = NativeFrameEvents {
            viewport_changed,
            viewport_size,
            focus_changed,
            close_requested,
            automation_focused_element,
            ordered_input_events,
        };
        let mut resize_applied = false;

        // The stage order lives in FrameStage::ORDER, shared with the
        // headless runner. This runner only supplies the two stages that
        // depend on the platform: which events arrived, and which backend
        // receives the painted scene.
        let outcome = match FramePipeline::run_frame(
            context,
            presenter,
            build_root,
            viewport_size,
            IdlePolicy::SkipWhenIdle,
            |presenter, context| {
                resize_applied = dispatch_native_events(
                    presenter,
                    context,
                    window,
                    &frame_events,
                    native_ime_state,
                );
                Ok(())
            },
            |presenter, _context| {
                let renderer = match (&mut *renderer, pending_renderer.take()) {
                    (Some(renderer), _) => renderer,
                    (slot @ None, Some(pending)) => {
                        let created = match pending.wait() {
                            Ok(renderer) => renderer,
                            Err(err) => panic!("failed to create renderer: {}", err),
                        };
                        window.set_device(created.device());
                        slot.insert(created)
                    }
                    (None, None) => unreachable!("the renderer is created before it is taken"),
                };
                let drawable_wait_started_at = Instant::now();
                let metal_drawable = window.next_drawable();
                phases.drawable_wait_ns = drawable_wait_started_at.elapsed().as_nanos();

                let render_started_at = Instant::now();
                phases.event_to_render_latency_ns = event_observed_at.map(|observed_at| {
                    render_started_at
                        .saturating_duration_since(observed_at)
                        .as_nanos()
                });
                renderer.set_profiling(presenter.profiler().is_some());
                let presentation = match metal_drawable {
                    Some(metal_drawable) => {
                        if let Some((frame_index, event_observed_at)) = present_profile {
                            present_latency::track(&metal_drawable, frame_index, event_observed_at);
                        }
                        // Hold the owned retain across render+present; MetalRenderer
                        // still takes &MetalDrawableRef via Deref.
                        renderer
                            .render_retained(
                                presenter.scene(),
                                retained,
                                &metal_drawable,
                                viewport_size,
                            )
                            .map_err(|err| {
                                FramePipelineError::stage(
                                    crate::core::frame_pipeline::FrameStage::Present,
                                    err.to_string(),
                                )
                            })?;
                        FramePresentation::Presented(Some(renderer.diagnostics()))
                    }
                    None => FramePresentation::Deferred,
                };
                phases.render_ns = render_started_at.elapsed().as_nanos();
                if let Some(profiler) = presenter.profiler_mut() {
                    let stats = renderer.last_frame_stats();
                    profiler.record_render(RenderSample {
                        drawable_wait_ns: phases.drawable_wait_ns,
                        render_ns: phases.render_ns,
                        scheduled_wait_ns: renderer
                            .scheduled_wait()
                            .map_or(0, |time| time.as_nanos()),
                        draw_calls: stats.draw_calls,
                        instances: stats.instances,
                        redrawn_pixels: stats.redrawn_pixels,
                        gpu_ns: renderer.completed_gpu_time().map(|time| time.as_nanos()),
                    });
                }
                Ok(presentation)
            },
        ) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => return requests,
            Err(err) => panic!("frame failed: {err}"),
        };

        if resize_applied {
            *last_viewport_size = viewport_size;
        }
        phases.layout_ns = outcome.durations.layout_ns;
        phases.dispatch_ns = outcome.durations.dispatch_ns;
        phases.paint_ns = outcome.durations.paint_ns;
        let frame_committed = outcome.presented;

        let (ime_sync_result, ime_redraw_requested) =
            crate::platform::mac::ime_state::sync_text_input_snapshot(
                presenter,
                window,
                native_ime_state,
            );
        if let Err(err) = ime_sync_result {
            log::error!("failed to synchronize native text input state: {err}");
        }
        // Invalid-snapshot cancel can mutate the input after Paint/Present; schedule a
        // platform redraw so cleared marked text is painted before the run loop sleeps.
        if ime_redraw_requested {
            schedule_platform_redraw(&*window, context, RedrawSource::PlatformInput);
        }

        let accessibility_bridge = window.accessibility_bridge_mut();
        let tree_published = match presenter.accessibility_tree() {
            Ok(accessibility_tree) => {
                match accessibility_bridge.publish_tree(&accessibility_tree) {
                    Ok(()) => Some(accessibility_tree),
                    Err(err) => {
                        log::error!("failed to publish accessibility tree: {err}");
                        None
                    }
                }
            }
            Err(err) => {
                log::error!("failed to build accessibility tree: {err}");
                None
            }
        };
        // FocusChanged is tree-dependent and stays queued until a matching tree
        // publishes. ActionFeedback posts on the content view and is announced
        // immediately so retained-tree controls still confirm activation during
        // recovery instead of buffering a burst for the next successful publish.
        if let Some(published_tree) = tree_published.as_ref() {
            let focused = presenter.focused_element();
            for announcement in filter_retained_accessibility_announcements(
                presenter.take_accessibility_announcements(),
                published_tree,
                focused,
            ) {
                if let Err(err) = accessibility_bridge.announce(&announcement) {
                    log::error!("failed to publish accessibility announcement: {err}");
                }
            }
        } else {
            let pending = presenter.take_accessibility_announcements();
            let mut retained_focus = Vec::new();
            for announcement in pending {
                match announcement.kind() {
                    AccessibilityAnnouncementKind::ActionFeedback => {
                        if let Err(err) = accessibility_bridge.announce(&announcement) {
                            log::error!("failed to publish accessibility announcement: {err}");
                        }
                    }
                    AccessibilityAnnouncementKind::FocusChanged => {
                        retained_focus.push(announcement);
                    }
                }
            }
            presenter.retain_accessibility_announcements(retained_focus);
        }

        if frame_committed && let Some(recorder) = profile_recorder.as_mut() {
            let diagnostics = match presenter.renderer_diagnostics() {
                Some(diagnostics) => diagnostics,
                None => panic!("renderer diagnostics were not recorded for this frame"),
            };
            let telemetry = recorder.capture_telemetry(
                "metal",
                viewport_size,
                phases,
                diagnostics,
                RendererBatchDiagnostics::for_metal_scene(presenter.scene()),
            );
            eprintln!("{}", telemetry.to_json_line());
        }

        if mark_deferred_frame_for_retry(context, frame_committed) {
            request_automation_redraw(&*window);
        }
        requests
    }
}

/// Keep frame ticks running exactly while an animation asks for frames, then
/// spend what is left of the idle budget on idle work.
///
/// Idle work stops at the budget, measured from when this loop iteration
/// started, and as soon as an event is waiting, so input and the next frame
/// tick are never queued behind it.
fn run_between_frames(
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

/// Propagates a platform-reported size change to the single viewport-size owner.
fn synchronize_viewport_after_platform_events(
    context: &mut AppContext,
    viewport_size: Size,
    last_viewport_size: Size,
) -> bool {
    if viewport_size == last_viewport_size {
        return false;
    }

    context.set_viewport_size(viewport_size);
    context.request_window_rebuild();
    true
}

fn mark_platform_event_redraw(event: &PlatformWindowEvent, context: &mut AppContext) {
    let source = match event {
        PlatformWindowEvent::Created
        | PlatformWindowEvent::Minimized(false)
        | PlatformWindowEvent::ReopenRequested => RedrawSource::PlatformLifecycle,
        PlatformWindowEvent::CloseRequested | PlatformWindowEvent::QuitRequested => {
            RedrawSource::PlatformLifecycle
        }
        PlatformWindowEvent::Minimized(true) => return,
        PlatformWindowEvent::Resized(_) => RedrawSource::PlatformResize,
        PlatformWindowEvent::ScaleFactorChanged(_) => RedrawSource::PlatformScaleFactor,
        PlatformWindowEvent::FocusChanged(_) | PlatformWindowEvent::ApplicationActivated(_) => {
            RedrawSource::PlatformFocus
        }
        PlatformWindowEvent::RedrawRequested => RedrawSource::PlatformRedraw,
        PlatformWindowEvent::Input(_) => RedrawSource::PlatformInput,
    };
    context.mark_redraw_from(source);
}

pub(crate) fn schedule_platform_redraw<W: PlatformWindow>(
    window: &W,
    context: &mut AppContext,
    source: RedrawSource,
) {
    if context.request_platform_redraw_from(source)
        && let Err(err) = window.request_redraw()
    {
        panic!("failed to request platform redraw: {}", err);
    }
}

fn append_input_event(
    input: PlatformInputEvent,
    ordered_input_events: &mut Vec<OrderedInputEvent>,
) {
    match input {
        PlatformInputEvent::KeyDown(event) => ordered_input_events.push(OrderedInputEvent::Key {
            is_down: true,
            event,
        }),
        PlatformInputEvent::KeyUp(event) => ordered_input_events.push(OrderedInputEvent::Key {
            is_down: false,
            event,
        }),
        PlatformInputEvent::Ime(PlatformImeEvent::Commit(text)) => {
            ordered_input_events.push(OrderedInputEvent::Text(
                TextInputEvent::CommitComposition(text).into(),
            ));
        }
        PlatformInputEvent::Ime(event) => {
            ordered_input_events.push(OrderedInputEvent::Text(
                event.into_text_input_event().into(),
            ));
        }
        PlatformInputEvent::Mouse(event) => {
            let kind = match event.kind {
                PlatformMouseEventKind::Down => PointerEventKind::Down,
                PlatformMouseEventKind::Up => PointerEventKind::Up,
                PlatformMouseEventKind::Move => PointerEventKind::Move,
            };
            ordered_input_events.push(OrderedInputEvent::Pointer(PointerEvent {
                kind,
                position: event.position,
                button: event.button,
            }));
        }
        PlatformInputEvent::Scroll(event) => {
            ordered_input_events.push(OrderedInputEvent::Scroll(event))
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum OrderedInputEvent {
    Pointer(PointerEvent),
    Scroll(ScrollEvent),
    Key { is_down: bool, event: KeyEvent },
    Text(crate::core::text_editing::TextInputCommand),
    Accessibility(MacAccessibilityActionRequest),
    FileDrag(crate::core::drag::FileDragEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeDogfoodAutomationPhase {
    Warmup,
    Interact,
    Minimize,
    Reopen,
    Submit,
    Done,
}

struct NativeDogfoodAutomation {
    phase: NativeDogfoodAutomationPhase,
    text: String,
}

impl NativeDogfoodAutomation {
    fn load_from_environment() -> Option<Self> {
        if std::env::var_os("RUI_NATIVE_DOGFOOD_AUTOMATION").is_none() {
            return None;
        }

        let text = std::env::var("RUI_NATIVE_DOGFOOD_TEXT")
            .unwrap_or_else(|_| String::from("rui-native-dogfood"));
        Some(Self {
            phase: NativeDogfoodAutomationPhase::Warmup,
            text,
        })
    }

    fn append_events(
        &mut self,
        window: &crate::platform::mac::window::MacWindow,
        scene: &crate::renderer::Scene,
        events: &mut Vec<MacWindowEvent>,
    ) -> Option<ElementId> {
        let click_point = || {
            scene
                .layers()
                .iter()
                .flat_map(|layer| layer.hit_regions())
                .find(|region| region.id == NATIVE_DOGFOOD_INPUT_ID)
                .expect("native dogfood input must have a visible hit region")
                .visible_bounds
                .center()
        };
        let focused_element = match self.phase {
            NativeDogfoodAutomationPhase::Warmup => {
                self.phase = NativeDogfoodAutomationPhase::Interact;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Interact => {
                append_pointer_click(events, click_point());
                append_text_keys(events, &self.text);
                self.phase = NativeDogfoodAutomationPhase::Minimize;
                request_automation_redraw(window);
                Some(NATIVE_DOGFOOD_INPUT_ID)
            }
            NativeDogfoodAutomationPhase::Minimize => {
                window.set_minimized(true);
                self.phase = NativeDogfoodAutomationPhase::Reopen;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Reopen => {
                // AppKit can animate minimization asynchronously. Do not count a
                // request as a completed transition; the script owns the timeout.
                if !window.is_miniaturized() {
                    request_automation_redraw(window);
                    return None;
                }
                eprintln!("native dogfood: AppKit minimized=true");
                window.set_minimized(false);
                self.phase = NativeDogfoodAutomationPhase::Submit;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Submit => {
                if window.is_miniaturized() || !window.is_visible() {
                    request_automation_redraw(window);
                    return None;
                }
                eprintln!("native dogfood: AppKit restored=true");
                append_pointer_click(events, click_point());
                append_key_pair(
                    events,
                    KeyEvent::new(KeyCode::Enter, Modifiers::none()).with_char('\n'),
                );
                self.phase = NativeDogfoodAutomationPhase::Done;
                request_automation_redraw(window);
                Some(NATIVE_DOGFOOD_INPUT_ID)
            }
            NativeDogfoodAutomationPhase::Done => None,
        };
        focused_element
    }
}

fn append_pointer_click(events: &mut Vec<MacWindowEvent>, position: Point) {
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind: PlatformMouseEventKind::Down,
            position,
            button: Some(MouseButton::Left),
        }),
    )));
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind: PlatformMouseEventKind::Up,
            position,
            button: Some(MouseButton::Left),
        }),
    )));
}

fn append_text_keys(events: &mut Vec<MacWindowEvent>, text: &str) {
    for ch in text.chars() {
        let key = key_code_for_dogfood_char(ch);
        append_key_pair(events, KeyEvent::new(key, Modifiers::none()).with_char(ch));
    }
}

fn append_key_pair(events: &mut Vec<MacWindowEvent>, event: KeyEvent) {
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::KeyDown(event.clone()),
    )));
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::KeyUp(event),
    )));
}

fn key_code_for_dogfood_char(ch: char) -> KeyCode {
    match ch {
        'a' | 'A' => KeyCode::A,
        'b' | 'B' => KeyCode::B,
        'c' | 'C' => KeyCode::C,
        'd' | 'D' => KeyCode::D,
        'e' | 'E' => KeyCode::E,
        'f' | 'F' => KeyCode::F,
        'g' | 'G' => KeyCode::G,
        'h' | 'H' => KeyCode::H,
        'i' | 'I' => KeyCode::I,
        'j' | 'J' => KeyCode::J,
        'k' | 'K' => KeyCode::K,
        'l' | 'L' => KeyCode::L,
        'm' | 'M' => KeyCode::M,
        'n' | 'N' => KeyCode::N,
        'o' | 'O' => KeyCode::O,
        'p' | 'P' => KeyCode::P,
        'q' | 'Q' => KeyCode::Q,
        'r' | 'R' => KeyCode::R,
        's' | 'S' => KeyCode::S,
        't' | 'T' => KeyCode::T,
        'u' | 'U' => KeyCode::U,
        'v' | 'V' => KeyCode::V,
        'w' | 'W' => KeyCode::W,
        'x' | 'X' => KeyCode::X,
        'y' | 'Y' => KeyCode::Y,
        'z' | 'Z' => KeyCode::Z,
        '0' => KeyCode::Key0,
        '1' => KeyCode::Key1,
        '2' => KeyCode::Key2,
        '3' => KeyCode::Key3,
        '4' => KeyCode::Key4,
        '5' => KeyCode::Key5,
        '6' => KeyCode::Key6,
        '7' => KeyCode::Key7,
        '8' => KeyCode::Key8,
        '9' => KeyCode::Key9,
        '-' => KeyCode::Minus,
        '_' => KeyCode::Minus,
        ' ' => KeyCode::Space,
        other => KeyCode::Unknown(other as u32),
    }
}

fn request_automation_redraw<W: PlatformWindow>(window: &W) {
    if let Err(err) = window.request_redraw() {
        panic!("native dogfood automation failed to request redraw: {err}");
    }
}

fn mark_deferred_frame_for_retry(context: &mut AppContext, presented: bool) -> bool {
    !presented && context.request_platform_redraw_from(RedrawSource::PlatformRedraw)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
