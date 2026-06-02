//! macOS application runner

use crate::core::app::AppContext;
use crate::core::event::{Event, KeyCode, KeyEvent, Modifiers, ScrollEvent};
use crate::core::geometry::Bounds;
use crate::core::window::WindowOptions;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use crate::platform::mac::window::create_window;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEventKind, PlatformRendererTarget,
    PlatformWindow, PlatformWindowError, PlatformWindowEvent,
};
use crate::renderer::RendererError;
use crate::renderer::Scene;
use crate::renderer::metal::MetalRenderer;
use crate::renderer::text::TextMeasureCache;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use taffy::prelude::*;

struct MacAppDelegateState {
    reopen_requested: Arc<AtomicBool>,
}

define_class!(
    // SAFETY:
    // - NSObject has no extra subclassing requirements.
    // - This delegate only touches AppKit from the main thread.
    // - The delegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = MacAppDelegateState]
    struct MacAppDelegate;

    unsafe impl NSObjectProtocol for MacAppDelegate {}

    unsafe impl NSApplicationDelegate for MacAppDelegate {
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _sender: &NSApplication, has_visible_windows: bool) -> bool {
            self.ivars().reopen_requested.store(true, Ordering::Release);
            has_visible_windows
        }
    }
);

impl MacAppDelegate {
    fn new(reopen_requested: Arc<AtomicBool>, mtm: MainThreadMarker) -> Retained<Self> {
        let delegate = mtm
            .alloc()
            .set_ivars(MacAppDelegateState { reopen_requested });
        unsafe { msg_send![super(delegate), init] }
    }
}

struct FrameProfiler {
    period_start: Instant,
    report_every: u64,
    frames: u64,
    platform_events: u64,
    pointer_events: u64,
    scroll_events: u64,
    key_events: u64,
    poll_now: ProfileStats,
    active_total: ProfileStats,
    rebuild: ProfileStats,
    layout: ProfileStats,
    dispatch: ProfileStats,
    paint: ProfileStats,
    next_drawable: ProfileStats,
    render: ProfileStats,
}

impl FrameProfiler {
    fn from_env() -> Option<Self> {
        if !profile_env_enabled(env::var("RUI_PROFILE").ok().as_deref()) {
            return None;
        }

        Some(Self {
            period_start: Instant::now(),
            report_every: env::var("RUI_PROFILE_EVERY")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(30),
            frames: 0,
            platform_events: 0,
            pointer_events: 0,
            scroll_events: 0,
            key_events: 0,
            poll_now: ProfileStats::default(),
            active_total: ProfileStats::default(),
            rebuild: ProfileStats::default(),
            layout: ProfileStats::default(),
            dispatch: ProfileStats::default(),
            paint: ProfileStats::default(),
            next_drawable: ProfileStats::default(),
            render: ProfileStats::default(),
        })
    }

    fn record(&mut self, sample: FrameProfileSample) {
        self.frames += 1;
        self.platform_events += sample.platform_events as u64;
        self.pointer_events += sample.pointer_events as u64;
        self.scroll_events += sample.scroll_events as u64;
        self.key_events += sample.key_events as u64;

        if let Some(duration) = sample.poll_now {
            self.poll_now.record(duration);
        }
        self.active_total.record(sample.active_total);
        self.rebuild.record(sample.rebuild);
        self.layout.record(sample.layout);
        self.dispatch.record(sample.dispatch);
        self.paint.record(sample.paint);
        self.next_drawable.record(sample.next_drawable);
        self.render.record(sample.render);

        if self.frames >= self.report_every || self.period_start.elapsed() >= Duration::from_secs(3)
        {
            self.report();
        }
    }

    fn report(&mut self) {
        if self.frames == 0 {
            return;
        }

        eprintln!(
            "RUI_PROFILE frames={} events={} keys={} pointer={} scroll={} {} {} {} {} {} {} {} {}",
            self.frames,
            self.platform_events,
            self.key_events,
            self.pointer_events,
            self.scroll_events,
            self.active_total.summary("active"),
            self.layout.summary("layout"),
            self.dispatch.summary("dispatch"),
            self.paint.summary("paint"),
            self.next_drawable.summary("drawable"),
            self.render.summary("render"),
            self.rebuild.summary("rebuild"),
            self.poll_now.summary("poll_now"),
        );

        self.reset();
    }

    fn reset(&mut self) {
        self.period_start = Instant::now();
        self.frames = 0;
        self.platform_events = 0;
        self.pointer_events = 0;
        self.scroll_events = 0;
        self.key_events = 0;
        self.poll_now.clear();
        self.active_total.clear();
        self.rebuild.clear();
        self.layout.clear();
        self.dispatch.clear();
        self.paint.clear();
        self.next_drawable.clear();
        self.render.clear();
    }
}

impl Drop for FrameProfiler {
    fn drop(&mut self) {
        self.report();
    }
}

#[derive(Default)]
struct FrameProfileSample {
    platform_events: usize,
    pointer_events: usize,
    scroll_events: usize,
    key_events: usize,
    poll_now: Option<Duration>,
    active_total: Duration,
    rebuild: Duration,
    layout: Duration,
    dispatch: Duration,
    paint: Duration,
    next_drawable: Duration,
    render: Duration,
}

#[derive(Default)]
struct ProfileStats {
    samples_us: Vec<u128>,
    total_us: u128,
    max_us: u128,
}

impl ProfileStats {
    fn record(&mut self, duration: Duration) {
        let micros = duration.as_micros();
        self.samples_us.push(micros);
        self.total_us += micros;
        self.max_us = self.max_us.max(micros);
    }

    fn clear(&mut self) {
        self.samples_us.clear();
        self.total_us = 0;
        self.max_us = 0;
    }

    fn summary(&self, label: &str) -> String {
        if self.samples_us.is_empty() {
            return format!("{label}=n/a");
        }

        let avg = self.total_us as f64 / self.samples_us.len() as f64;
        format!(
            "{label}:avg={:.2}ms,p50={:.2}ms,p95={:.2}ms,max={:.2}ms",
            micros_to_millis(avg),
            micros_to_millis(self.percentile_us(50) as f64),
            micros_to_millis(self.percentile_us(95) as f64),
            micros_to_millis(self.max_us as f64),
        )
    }

    fn percentile_us(&self, percentile: usize) -> u128 {
        debug_assert!(!self.samples_us.is_empty());
        let mut samples = self.samples_us.clone();
        samples.sort_unstable();
        let index = ((samples.len() - 1) * percentile).div_ceil(100);
        samples[index]
    }
}

fn profile_env_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

fn profile_mark(enabled: bool) -> Option<Instant> {
    enabled.then(Instant::now)
}

fn elapsed_since(mark: Option<Instant>) -> Duration {
    mark.map(|instant| instant.elapsed()).unwrap_or_default()
}

fn micros_to_millis(micros: f64) -> f64 {
    micros / 1_000.0
}

/// Run the application with default window options
pub fn run_app<F, E>(context: AppContext, build_root: F)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    run_app_with_options(context, build_root, WindowOptions::default());
}

/// Run the application with custom window options
pub fn run_app_with_options<F, E>(context: AppContext, build_root: F, options: WindowOptions)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    run_app_with_renderer_factory(context, build_root, options, MetalRenderer::new);
}

pub(crate) fn run_app_with_renderer_factory<F, E>(
    context: AppContext,
    mut build_root: F,
    options: WindowOptions,
    create_renderer: impl FnOnce() -> Result<MetalRenderer, RendererError>,
) where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let mut context = context;
    let mut root = build_root(&mut context);
    let mut profiler = FrameProfiler::from_env();
    let profiling = profiler.is_some();

    // Get main thread marker
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");

    unsafe {
        // Initialize NSApplication
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let (app_delegate, reopen_requested) = install_app_delegate(&app, mtm);
        app.finishLaunching();

        // Create the renderer
        let mut renderer = match create_renderer() {
            Ok(renderer) => renderer,
            Err(err) => panic!("failed to create renderer: {}", err),
        };

        // Create the window with Metal layer
        let mut window = match create_window(&options, renderer.device(), mtm) {
            Ok(window) => window,
            Err(err) => panic!("failed to create platform window: {}", err),
        };
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

        // Activate the application
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        // Create layout engine
        let mut taffy: TaffyTree<crate::core::ElementId> = TaffyTree::new();
        let mut text_measurer = TextMeasureCache::new();

        // Create scene
        let mut scene = Scene::new();

        // Main run loop
        let mut viewport_size = options.size;
        let mut last_viewport_size = viewport_size;
        context.set_viewport_size(viewport_size);
        let mut focused_element: Option<crate::core::ElementId> = None;
        let mut last_pointer_hit_target: Option<crate::core::ElementId> = None;
        let mut pointer_capture_target: Option<crate::core::ElementId> = None;
        let mut last_app_active = app.isActive();

        // Render loop (event-driven)
        loop {
            viewport_size = match window.content_size() {
                Ok(size) => size,
                Err(err) => panic!("failed to read platform window size: {}", err),
            };

            let mut pointer_events = Vec::new();
            let mut scroll_events = Vec::new();
            let mut key_events = Vec::new();
            let mut focus_changed = None;
            let mut close_requested = false;
            let wait_for_event =
                !context.dirty && !context.needs_rebuild && context.pending_updates.is_empty();
            let poll_start = profile_mark(profiling);
            let platform_events = match window.poll_events_for_app(&app, wait_for_event) {
                Ok(events) => events,
                Err(err) => panic!("failed to poll platform window events: {}", err),
            };
            let poll_duration = elapsed_since(poll_start);
            let active_start = profile_mark(profiling);
            let platform_event_count = platform_events.len();

            let app_active = app.isActive();
            if should_restore_window_for_app_activation(
                last_app_active,
                app_active,
                window.is_visible(),
                window.is_minimized(),
            ) {
                if let Err(err) = window.show() {
                    panic!("failed to restore platform window: {}", err);
                }
                context.request_redraw();
            }
            last_app_active = app_active;

            if should_restore_window_for_app_reopen(
                reopen_requested.swap(false, Ordering::AcqRel),
                window.is_visible(),
                window.is_minimized(),
            ) {
                if let Err(err) = window.show() {
                    panic!("failed to reopen platform window: {}", err);
                }
                context.request_redraw();
            }

            for event in platform_events {
                if event.requests_redraw() {
                    schedule_platform_redraw(&window, &mut context);
                }

                match event {
                    PlatformWindowEvent::Created | PlatformWindowEvent::RedrawRequested => {
                        context.request_redraw();
                    }
                    PlatformWindowEvent::Resized(size) => {
                        viewport_size = size;
                    }
                    PlatformWindowEvent::ScaleFactorChanged(_) => {}
                    PlatformWindowEvent::FocusChanged(focused) => {
                        focus_changed = Some(focused);
                    }
                    PlatformWindowEvent::CloseRequested => {
                        close_requested = true;
                        context.request_redraw();
                    }
                    PlatformWindowEvent::Input(input) => append_input_event(
                        input,
                        &mut pointer_events,
                        &mut scroll_events,
                        &mut key_events,
                    ),
                }
            }

            if viewport_size != last_viewport_size {
                context.set_viewport_size(viewport_size);
                context.request_rebuild();
                schedule_platform_redraw(&window, &mut context);
            }

            let mut profile_sample = FrameProfileSample {
                platform_events: platform_event_count,
                poll_now: (!wait_for_event).then_some(poll_duration),
                ..FrameProfileSample::default()
            };

            context.consume_runtime_view_notification();

            if !context.dirty && !context.needs_rebuild && context.pending_updates.is_empty() {
                if !context.is_running() {
                    break;
                }
                continue;
            }

            let rebuild_start = profile_mark(profiling);
            if context.needs_rebuild || !context.pending_updates.is_empty() {
                context.pending_updates.clear();
                context.needs_rebuild = false;
                root = build_root(&mut context);
                if !context.pending_updates.is_empty() {
                    context.needs_rebuild = true;
                }
            }
            profile_sample.rebuild = elapsed_since(rebuild_start);

            // Rebuild layout tree each frame we render to avoid unbounded growth
            taffy.clear();

            let layout_start = profile_mark(profiling);
            // Layout phase
            let mut layout_cx =
                LayoutContext::with_text_measurer(&mut taffy, viewport_size, &mut text_measurer);
            let root_node = root.layout(&mut layout_cx);

            // Compute layout
            taffy
                .compute_layout(
                    root_node,
                    taffy::Size {
                        width: AvailableSpace::Definite(viewport_size.width),
                        height: AvailableSpace::Definite(viewport_size.height),
                    },
                )
                .expect("Layout failed");

            // Get computed bounds
            let layout = taffy.layout(root_node).expect("No layout");
            let root_bounds = Bounds::from_xywh(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            );
            profile_sample.layout = elapsed_since(layout_start);

            let dispatch_start = profile_mark(profiling);
            if viewport_size != last_viewport_size {
                root.handle_window_event(&Event::WindowResize {
                    width: viewport_size.width,
                    height: viewport_size.height,
                });
                last_viewport_size = viewport_size;
            }

            if let Some(is_focused) = focus_changed {
                let evt = if is_focused {
                    Event::Focus(crate::core::event::FocusEvent { focused: true })
                } else {
                    focused_element = None;
                    Event::Blur(crate::core::event::FocusEvent { focused: false })
                };
                root.handle_window_event(&evt);
                schedule_platform_redraw(&window, &mut context);
            }

            if close_requested {
                root.handle_window_event(&Event::WindowClose);
                context.quit();
                schedule_platform_redraw(&window, &mut context);
            }

            let mut event_cx = EventContext::new(root_bounds, &taffy, &mut focused_element);

            for event in &pointer_events {
                let hit_target = scene.hit_test(event.position);
                let dispatch_target = pointer_capture_target.or(hit_target);
                let previous_target = if matches!(event.kind, PointerEventKind::Move) {
                    last_pointer_hit_target
                } else {
                    None
                };

                event_cx.set_hit_target(dispatch_target);
                event_cx.set_previous_hit_target(previous_target);

                let result = root.dispatch_pointer_event(&mut event_cx, event);

                match event.kind {
                    PointerEventKind::Down if result.is_stopped() => {
                        pointer_capture_target = dispatch_target;
                    }
                    PointerEventKind::Up => {
                        pointer_capture_target = None;
                    }
                    PointerEventKind::Move => {
                        last_pointer_hit_target = hit_target;
                    }
                    PointerEventKind::Down => {}
                }
            }

            if event_cx.redraw_requested() {
                schedule_platform_redraw(&window, &mut context);
            }

            for event in &scroll_events {
                root.handle_scroll_event(&mut event_cx, event);
            }

            for (is_down, event) in &key_events {
                if should_quit_app_from_key_event(*is_down, event) {
                    context.quit();
                    schedule_platform_redraw(&window, &mut context);
                    continue;
                }
                if should_forward_key_event_to_tree(*is_down) {
                    root.handle_key_event(&mut event_cx, event);
                }
            }
            profile_sample.pointer_events = pointer_events.len();
            profile_sample.scroll_events = scroll_events.len();
            profile_sample.key_events = key_events.len();
            profile_sample.dispatch = elapsed_since(dispatch_start);

            if context.consume_runtime_view_notification() {
                schedule_platform_redraw(&window, &mut context);
            }

            let paint_start = profile_mark(profiling);
            // Paint phase
            scene.clear();
            let mut paint_cx = PaintContext::new(&mut scene, root_bounds, &taffy);
            root.paint(&mut paint_cx);
            scene.finish();
            profile_sample.paint = elapsed_since(paint_start);

            // Get next drawable from the platform window renderer attachment
            let drawable_start = profile_mark(profiling);
            if let Some(metal_drawable) = window.next_drawable() {
                profile_sample.next_drawable = elapsed_since(drawable_start);
                let render_start = profile_mark(profiling);
                if let Err(err) = renderer.render(&scene, metal_drawable, viewport_size) {
                    panic!("renderer failed: {}", err);
                }
                profile_sample.render = elapsed_since(render_start);
            } else {
                profile_sample.next_drawable = elapsed_since(drawable_start);
            }

            if !context.needs_rebuild && context.pending_updates.is_empty() {
                context.dirty = false;
            }

            profile_sample.active_total = elapsed_since(active_start);
            if let Some(profiler) = profiler.as_mut() {
                profiler.record(profile_sample);
            }

            // Check if we should quit
            if !context.is_running() {
                break;
            }
        }

        app.setDelegate(None);
        drop(app_delegate);
    }
}

fn install_app_delegate(
    app: &NSApplication,
    mtm: MainThreadMarker,
) -> (Retained<MacAppDelegate>, Arc<AtomicBool>) {
    let reopen_requested = Arc::new(AtomicBool::new(false));
    let delegate = MacAppDelegate::new(Arc::clone(&reopen_requested), mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    (delegate, reopen_requested)
}

fn schedule_platform_redraw<W: PlatformWindow>(_window: &W, context: &mut AppContext) {
    context.request_redraw();
}

fn append_input_event(
    input: PlatformInputEvent,
    pointer_events: &mut Vec<PointerEvent>,
    scroll_events: &mut Vec<ScrollEvent>,
    key_events: &mut Vec<(bool, KeyEvent)>,
) {
    match input {
        PlatformInputEvent::KeyDown(event) => key_events.push((true, event)),
        PlatformInputEvent::KeyUp(event) => key_events.push((false, event)),
        PlatformInputEvent::Ime(PlatformImeEvent::Commit(text)) => {
            append_committed_text_events(&text, key_events);
        }
        PlatformInputEvent::Mouse(event) => {
            let kind = match event.kind {
                PlatformMouseEventKind::Down => PointerEventKind::Down,
                PlatformMouseEventKind::Up => PointerEventKind::Up,
                PlatformMouseEventKind::Move => PointerEventKind::Move,
            };
            pointer_events.push(PointerEvent {
                kind,
                position: event.position,
                button: event.button,
            });
        }
        PlatformInputEvent::Scroll(event) => scroll_events.push(event),
    }
}

fn append_committed_text_events(text: &str, key_events: &mut Vec<(bool, KeyEvent)>) {
    for ch in text.chars().filter(|ch| !ch.is_control()) {
        key_events.push((
            true,
            KeyEvent::new(KeyCode::Unknown(0), Modifiers::none()).with_char(ch),
        ));
    }
}

fn should_forward_key_event_to_tree(is_down: bool) -> bool {
    is_down
}

fn should_quit_app_from_key_event(is_down: bool, event: &KeyEvent) -> bool {
    is_down && event.modifiers.meta && event.key == KeyCode::Q
}

fn should_restore_window_for_app_activation(
    was_app_active: bool,
    app_active: bool,
    window_visible: bool,
    window_minimized: bool,
) -> bool {
    !was_app_active && app_active && !window_visible && window_minimized
}

fn should_restore_window_for_app_reopen(
    reopen_requested: bool,
    window_visible: bool,
    window_minimized: bool,
) -> bool {
    reopen_requested && (!window_visible || window_minimized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_key_down_events_are_forwarded_to_elements() {
        assert!(should_forward_key_event_to_tree(true));
        assert!(!should_forward_key_event_to_tree(false));
    }

    #[test]
    fn command_q_requests_app_quit() {
        assert!(should_quit_app_from_key_event(
            true,
            &KeyEvent::new(KeyCode::Q, Modifiers::meta())
        ));
        assert!(!should_quit_app_from_key_event(
            false,
            &KeyEvent::new(KeyCode::Q, Modifiers::meta())
        ));
        assert!(!should_quit_app_from_key_event(
            true,
            &KeyEvent::new(KeyCode::Q, Modifiers::none())
        ));
    }

    #[test]
    fn app_activation_restores_minimized_hidden_window() {
        assert!(should_restore_window_for_app_activation(
            false, true, false, true
        ));
        assert!(!should_restore_window_for_app_activation(
            true, true, false, true
        ));
        assert!(!should_restore_window_for_app_activation(
            false, false, false, true
        ));
        assert!(!should_restore_window_for_app_activation(
            false, true, true, true
        ));
        assert!(!should_restore_window_for_app_activation(
            false, true, false, false
        ));
    }

    #[test]
    fn app_reopen_restores_hidden_or_minimized_window() {
        assert!(should_restore_window_for_app_reopen(true, false, false));
        assert!(should_restore_window_for_app_reopen(true, false, true));
        assert!(should_restore_window_for_app_reopen(true, true, true));
        assert!(!should_restore_window_for_app_reopen(false, false, true));
        assert!(!should_restore_window_for_app_reopen(true, true, false));
    }

    #[test]
    fn profile_env_accepts_explicit_truthy_values() {
        assert!(profile_env_enabled(Some("1")));
        assert!(profile_env_enabled(Some("true")));
        assert!(profile_env_enabled(Some("yes")));
        assert!(!profile_env_enabled(None));
        assert!(!profile_env_enabled(Some("")));
        assert!(!profile_env_enabled(Some("0")));
        assert!(!profile_env_enabled(Some("false")));
    }

    #[test]
    fn profile_stats_report_percentiles() {
        let mut stats = ProfileStats::default();
        stats.record(Duration::from_micros(100));
        stats.record(Duration::from_micros(200));
        stats.record(Duration::from_micros(300));

        assert_eq!(stats.percentile_us(50), 200);
        assert_eq!(stats.percentile_us(95), 300);
        assert!(stats.summary("stage").contains("stage:avg=0.20ms"));
    }

    #[test]
    fn ime_commit_events_are_forwarded_as_text_key_events() {
        let mut pointer_events = Vec::new();
        let mut scroll_events = Vec::new();
        let mut key_events = Vec::new();

        append_input_event(
            PlatformInputEvent::Ime(PlatformImeEvent::Commit("你好".to_string())),
            &mut pointer_events,
            &mut scroll_events,
            &mut key_events,
        );

        assert!(pointer_events.is_empty());
        assert!(scroll_events.is_empty());
        assert_eq!(key_events.len(), 2);
        assert!(key_events.iter().all(|(is_down, _)| *is_down));
        assert_eq!(key_events[0].1.char, Some('你'));
        assert_eq!(key_events[1].1.char, Some('好'));
    }
}
