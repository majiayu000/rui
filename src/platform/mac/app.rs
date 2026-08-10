//! macOS application runner

use crate::core::ElementId;
use crate::core::accessibility::AccessibilityBridge;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::frame_pipeline::{
    FramePipeline, FramePipelineError, FramePresentation, IdlePolicy,
};
use crate::core::geometry::{Point, Size};
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::core::window::WindowOptions;
use crate::elements::element::{Element, PointerEvent, PointerEventKind};
use crate::platform::mac::accessibility::MacAccessibilityActionRequest;
use crate::platform::mac::events::MacWindowEvent;
use crate::platform::mac::frame::{NativeFrameEvents, dispatch_native_events};
use crate::platform::mac::window::create_window;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererTarget, PlatformWindow, PlatformWindowError, PlatformWindowEvent,
};
use crate::renderer::metal::MetalRenderer;
use crate::renderer::{
    RendererBatchDiagnostics, RendererError, RendererFramePhaseDurations, RendererTelemetryRecorder,
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
    let root = build_root(&mut context);

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

        let mut presenter = Presenter::with_root(options.size, root);
        presenter.set_accessibility_announcements_enabled(true);
        if !window.accessibility_bridge_mut().native_attached() {
            panic!("macOS accessibility bridge failed to attach to the content view");
        }

        // Main run loop. AppContext is the single owner of viewport size; the
        // window options only provide its initial value.
        let mut viewport_size = options.size;
        context.set_viewport_size(viewport_size);
        let mut last_viewport_size = viewport_size;
        let mut profile_recorder = RendererTelemetryRecorder::enabled_from_env();
        let mut native_dogfood_automation =
            NativeDogfoodAutomation::load_from_environment(options.size);
        let mut native_ime_state = crate::platform::mac::ime_state::NativeImeState::default();

        // Render loop (event-driven)
        loop {
            let mut phases = RendererFramePhaseDurations::default();
            viewport_size = match window.content_size() {
                Ok(size) => size,
                Err(err) => panic!("failed to read platform window size: {}", err),
            };
            let mut ordered_input_events = Vec::new();
            let mut focus_changed = None;
            let mut close_requested = false;
            let wait_for_event = !context.has_frame_work();
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
                match event {
                    MacWindowEvent::Accessibility(request) => {
                        context.mark_redraw_from(RedrawSource::PlatformInput);
                        ordered_input_events.push(OrderedInputEvent::Accessibility(request));
                    }
                    MacWindowEvent::Text(command) => {
                        context.mark_redraw_from(RedrawSource::PlatformInput);
                        ordered_input_events.push(OrderedInputEvent::Text(command));
                    }
                    MacWindowEvent::Platform(event) => {
                        mark_platform_event_redraw(&event, &mut context);
                        match event {
                            PlatformWindowEvent::Created | PlatformWindowEvent::RedrawRequested => {
                            }
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
                            PlatformWindowEvent::CloseRequested
                            | PlatformWindowEvent::QuitRequested => close_requested = true,
                            PlatformWindowEvent::Input(input) => {
                                append_input_event(input, &mut ordered_input_events)
                            }
                        }
                    }
                }
            }

            let viewport_changed = synchronize_viewport_after_platform_events(
                &mut context,
                viewport_size,
                last_viewport_size,
            );
            if viewport_changed {
                schedule_platform_redraw(&window, &mut context, RedrawSource::PlatformResize);
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
                &mut context,
                &mut presenter,
                &mut build_root,
                viewport_size,
                IdlePolicy::SkipWhenIdle,
                |presenter, context| {
                    resize_applied = dispatch_native_events(
                        presenter,
                        context,
                        &window,
                        &frame_events,
                        &mut native_ime_state,
                    );
                    Ok(())
                },
                |presenter, _context| {
                    let drawable_wait_started_at = Instant::now();
                    let metal_drawable = window.next_drawable();
                    phases.drawable_wait_ns = drawable_wait_started_at.elapsed().as_nanos();

                    let render_started_at = Instant::now();
                    phases.event_to_render_latency_ns = event_observed_at.map(|observed_at| {
                        render_started_at
                            .saturating_duration_since(observed_at)
                            .as_nanos()
                    });
                    let presentation = match metal_drawable {
                        Some(metal_drawable) => {
                            renderer
                                .render(presenter.scene(), metal_drawable, viewport_size)
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
                    Ok(presentation)
                },
            ) {
                Ok(Some(outcome)) => outcome,
                Ok(None) => {
                    if !context.is_running() {
                        break;
                    }
                    continue;
                }
                Err(err) => panic!("frame failed: {err}"),
            };

            if resize_applied {
                last_viewport_size = viewport_size;
            }
            phases.layout_ns = outcome.durations.layout_ns;
            phases.dispatch_ns = outcome.durations.dispatch_ns;
            phases.paint_ns = outcome.durations.paint_ns;
            let frame_committed = outcome.presented;

            if let Err(err) =
                crate::platform::mac::ime_state::sync_text_input_snapshot(&presenter, &window)
            {
                panic!("failed to synchronize native text input state: {err}");
            }

            let accessibility_tree = match presenter.accessibility_tree() {
                Ok(tree) => tree,
                Err(err) => panic!("failed to build accessibility tree: {err}"),
            };
            let accessibility_bridge = window.accessibility_bridge_mut();
            if let Err(err) = accessibility_bridge.publish_tree(&accessibility_tree) {
                panic!("failed to publish accessibility tree: {err}");
            }
            for announcement in presenter.take_accessibility_announcements() {
                if let Err(err) = accessibility_bridge.announce(&announcement) {
                    log::error!("failed to publish accessibility announcement: {err}");
                }
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

            if mark_deferred_frame_for_retry(&mut context, frame_committed) {
                request_automation_redraw(&window);
            }
            // Check if we should quit
            if !context.is_running() {
                break;
            }
        }
    }
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
    context.request_rebuild();
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
        events: &mut Vec<MacWindowEvent>,
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
