//! macOS application runner

use crate::core::app::{AppContext, RedrawSource};
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
use crate::renderer::metal::MetalRenderer;
use crate::renderer::{
    RendererBatchDiagnostics, RendererError, RendererFramePhaseDurations,
    RendererTelemetryRecorder, Scene,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use std::time::Instant;
use taffy::prelude::*;

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

    // Get main thread marker
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");

    unsafe {
        // Initialize NSApplication
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

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
        window.install_application_delegate(&app);
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
        app.activate();

        // Create layout engine
        let mut taffy: TaffyTree<crate::core::ElementId> = TaffyTree::new();

        // Create scene
        let mut scene = Scene::new();

        // Main run loop
        let mut viewport_size = options.size;
        let mut last_viewport_size = viewport_size;
        let mut focused_element: Option<crate::core::ElementId> = None;
        let mut last_pointer_hit_target: Option<crate::core::ElementId> = None;
        let mut pointer_capture_target: Option<crate::core::ElementId> = None;
        let mut profile_recorder = RendererTelemetryRecorder::enabled_from_env();

        // Render loop (event-driven)
        loop {
            let mut phases = RendererFramePhaseDurations::default();
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
            let platform_events = match window.poll_events_for_app(&app, wait_for_event) {
                Ok(events) => events,
                Err(err) => panic!("failed to poll platform window events: {}", err),
            };
            let event_observed_at = if platform_events.is_empty() {
                None
            } else {
                Some(Instant::now())
            };

            for event in platform_events {
                mark_platform_event_redraw(&event, &mut context);

                match event {
                    PlatformWindowEvent::Created | PlatformWindowEvent::RedrawRequested => {}
                    PlatformWindowEvent::Resized(size) => {
                        viewport_size = size;
                    }
                    PlatformWindowEvent::ScaleFactorChanged(_) => {}
                    PlatformWindowEvent::ApplicationActivated(_) => {}
                    PlatformWindowEvent::Minimized(_) => {}
                    PlatformWindowEvent::ReopenRequested => {
                        if let Err(err) = window.show() {
                            panic!("failed to reopen platform window: {}", err);
                        }
                    }
                    PlatformWindowEvent::FocusChanged(focused) => {
                        focus_changed = Some(focused);
                    }
                    PlatformWindowEvent::CloseRequested | PlatformWindowEvent::QuitRequested => {
                        close_requested = true;
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
                schedule_platform_redraw(&window, &mut context, RedrawSource::PlatformResize);
            }

            context.consume_runtime_view_notification();

            if !context.dirty && !context.needs_rebuild && context.pending_updates.is_empty() {
                if !context.is_running() {
                    break;
                }
                continue;
            }

            if context.needs_rebuild || !context.pending_updates.is_empty() {
                context.pending_updates.clear();
                context.needs_rebuild = false;
                root = build_root(&mut context);
                if !context.pending_updates.is_empty() {
                    context.needs_rebuild = true;
                }
            }

            // Rebuild layout tree each frame we render to avoid unbounded growth
            let layout_started_at = Instant::now();
            taffy.clear();

            // Layout phase
            let mut layout_cx = LayoutContext::new(&mut taffy, viewport_size);
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
            phases.layout_ns = layout_started_at.elapsed().as_nanos();

            let dispatch_started_at = Instant::now();
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
                schedule_platform_redraw(&window, &mut context, RedrawSource::PlatformFocus);
            }

            if close_requested {
                root.handle_window_event(&Event::WindowClose);
                context.quit();
                schedule_platform_redraw(&window, &mut context, RedrawSource::PlatformLifecycle);
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
                schedule_platform_redraw(&window, &mut context, RedrawSource::Element);
            }

            for event in &scroll_events {
                root.handle_scroll_event(&mut event_cx, event);
            }

            for (is_down, event) in &key_events {
                if should_forward_key_event_to_tree(*is_down) {
                    root.handle_key_event(&mut event_cx, event);
                }
            }

            if context.consume_runtime_view_notification() {
                schedule_platform_redraw(&window, &mut context, RedrawSource::ViewNotification);
            }
            phases.dispatch_ns = dispatch_started_at.elapsed().as_nanos();

            // Paint phase
            let paint_started_at = Instant::now();
            scene.clear();
            let mut paint_cx = PaintContext::new(&mut scene, root_bounds, &taffy);
            root.paint(&mut paint_cx);
            scene.finish();
            phases.paint_ns = paint_started_at.elapsed().as_nanos();

            // Get next drawable from the platform window renderer attachment
            let drawable_wait_started_at = Instant::now();
            let metal_drawable = window.next_drawable();
            phases.drawable_wait_ns = drawable_wait_started_at.elapsed().as_nanos();

            let render_started_at = Instant::now();
            phases.event_to_render_latency_ns = event_observed_at.map(|observed_at| {
                render_started_at
                    .saturating_duration_since(observed_at)
                    .as_nanos()
            });
            if let Some(metal_drawable) = metal_drawable
                && let Err(err) = renderer.render(&scene, metal_drawable, viewport_size)
            {
                panic!("renderer failed: {}", err);
            }
            phases.render_ns = render_started_at.elapsed().as_nanos();

            if let Some(recorder) = profile_recorder.as_mut() {
                let diagnostics = renderer.diagnostics();
                let telemetry = recorder.capture_telemetry(
                    "metal",
                    viewport_size,
                    phases,
                    &diagnostics,
                    RendererBatchDiagnostics::for_metal_scene(&scene),
                );
                eprintln!("{}", telemetry.to_json_line());
            }

            context.complete_redraw_frame();

            // Check if we should quit
            if !context.is_running() {
                break;
            }
        }
    }
}

fn mark_platform_event_redraw(event: &PlatformWindowEvent, context: &mut AppContext) {
    let source = match event {
        PlatformWindowEvent::Created
        | PlatformWindowEvent::Minimized(false)
        | PlatformWindowEvent::ReopenRequested => RedrawSource::PlatformLifecycle,
        PlatformWindowEvent::CloseRequested
        | PlatformWindowEvent::Minimized(true)
        | PlatformWindowEvent::QuitRequested => return,
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

fn schedule_platform_redraw<W: PlatformWindow>(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_key_down_events_are_forwarded_to_elements() {
        assert!(should_forward_key_event_to_tree(true));
        assert!(!should_forward_key_event_to_tree(false));
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
