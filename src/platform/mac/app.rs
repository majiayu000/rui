//! macOS application runner

use crate::core::ElementId;
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{Event, KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point};
use crate::core::text_editing::TextInputEvent;
use crate::core::window::WindowOptions;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use crate::platform::mac::window::create_window;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererTarget, PlatformWindow, PlatformWindowError, PlatformWindowEvent,
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
        let mut native_dogfood_automation =
            NativeDogfoodAutomation::load_from_environment(options.size);

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
            let mut text_input_events = Vec::new();
            let mut focus_changed = None;
            let mut close_requested = false;
            let wait_for_event =
                !context.dirty && !context.needs_rebuild && context.pending_updates.is_empty();
            let mut platform_events = match window.poll_events_for_app(&app, wait_for_event) {
                Ok(events) => events,
                Err(err) => panic!("failed to poll platform window events: {}", err),
            };
            let mut automation_focused_element = None;
            if let Some(automation) = &mut native_dogfood_automation {
                automation_focused_element =
                    automation.append_events(&window, &mut platform_events);
            }
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
                        &mut text_input_events,
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

            if let Some(focused) = automation_focused_element {
                focused_element = Some(focused);
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
                    if route_key_event(&mut root, &mut context, &mut event_cx, event) {
                        schedule_platform_redraw(
                            &window,
                            &mut context,
                            RedrawSource::PlatformInput,
                        );
                    }
                }
            }

            for event in &text_input_events {
                if root.handle_text_input_event(&mut event_cx, event) {
                    schedule_platform_redraw(&window, &mut context, RedrawSource::PlatformInput);
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
    text_input_events: &mut Vec<TextInputEvent>,
) {
    match input {
        PlatformInputEvent::KeyDown(event) => key_events.push((true, event)),
        PlatformInputEvent::KeyUp(event) => key_events.push((false, event)),
        PlatformInputEvent::Ime(PlatformImeEvent::Commit(text)) => {
            text_input_events.push(TextInputEvent::CommitComposition(text));
        }
        PlatformInputEvent::Ime(event) => {
            text_input_events.push(event.into_text_input_event());
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
    click_point: Point,
}

impl NativeDogfoodAutomation {
    fn load_from_environment(viewport_size: crate::core::Size) -> Option<Self> {
        if std::env::var_os("RUI_NATIVE_DOGFOOD_AUTOMATION").is_none() {
            return None;
        }

        let text = std::env::var("RUI_NATIVE_DOGFOOD_TEXT")
            .unwrap_or_else(|_| String::from("rui-native-dogfood"));
        Some(Self {
            phase: NativeDogfoodAutomationPhase::Warmup,
            text,
            click_point: Point::new(90.0, (viewport_size.height * 0.48).round()),
        })
    }

    fn append_events(
        &mut self,
        window: &crate::platform::mac::window::MacWindow,
        events: &mut Vec<PlatformWindowEvent>,
    ) -> Option<ElementId> {
        let focused_element = match self.phase {
            NativeDogfoodAutomationPhase::Warmup => {
                self.phase = NativeDogfoodAutomationPhase::Interact;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Interact => {
                append_pointer_click(events, self.click_point);
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
                window.set_minimized(false);
                self.phase = NativeDogfoodAutomationPhase::Submit;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Submit => {
                append_pointer_click(events, self.click_point);
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

fn append_pointer_click(events: &mut Vec<PlatformWindowEvent>, position: Point) {
    events.push(PlatformWindowEvent::Input(PlatformInputEvent::Mouse(
        PlatformMouseEvent {
            kind: PlatformMouseEventKind::Down,
            position,
            button: Some(MouseButton::Left),
        },
    )));
    events.push(PlatformWindowEvent::Input(PlatformInputEvent::Mouse(
        PlatformMouseEvent {
            kind: PlatformMouseEventKind::Up,
            position,
            button: Some(MouseButton::Left),
        },
    )));
}

fn append_text_keys(events: &mut Vec<PlatformWindowEvent>, text: &str) {
    for ch in text.chars() {
        let key = key_code_for_dogfood_char(ch);
        append_key_pair(events, KeyEvent::new(key, Modifiers::none()).with_char(ch));
    }
}

fn append_key_pair(events: &mut Vec<PlatformWindowEvent>, event: KeyEvent) {
    events.push(PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
        event.clone(),
    )));
    events.push(PlatformWindowEvent::Input(PlatformInputEvent::KeyUp(event)));
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
    fn ime_commit_events_are_forwarded_as_text_input_events() {
        let mut pointer_events = Vec::new();
        let mut scroll_events = Vec::new();
        let mut key_events = Vec::new();
        let mut text_input_events = Vec::new();

        append_input_event(
            PlatformInputEvent::Ime(PlatformImeEvent::Commit("你好".to_string())),
            &mut pointer_events,
            &mut scroll_events,
            &mut key_events,
            &mut text_input_events,
        );

        assert!(pointer_events.is_empty());
        assert!(scroll_events.is_empty());
        assert!(key_events.is_empty());
        assert_eq!(
            text_input_events,
            [TextInputEvent::CommitComposition("你好".to_string())]
        );
    }
}
