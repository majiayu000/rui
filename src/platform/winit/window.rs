//! One winit window: its presenter, wgpu surface and frame.

use super::input::InputState;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{Event, FocusEvent};
use crate::core::frame_pipeline::{
    FramePipeline, FramePipelineError, FramePresentation, FrameStage, IdlePolicy,
};
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputCommand;
use crate::core::window::{WindowId, WindowOptions};
use crate::elements::element::Element;
use crate::platform::frame_driver::{
    OrderedInputEvent, append_input_event, dispatch_key, mark_deferred_frame_for_retry,
    mark_platform_event_redraw, schedule_platform_redraw,
    synchronize_viewport_after_platform_events,
};
use crate::platform::window::{
    PlatformWindow, PlatformWindowError, PlatformWindowEvent, PlatformWindowFeatures,
    PlatformWindowState,
};
use crate::renderer::RendererResourceLimits;
use crate::renderer::wgpu::{RetainedTarget, WgpuRenderer, WindowSurface};
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;

const PLATFORM: &str = "winit";

/// The winit window behind the shared [`PlatformWindow`] contract.
pub(super) struct WinitWindow {
    window: Arc<winit::window::Window>,
}

impl PlatformWindow for WinitWindow {
    fn platform_name(&self) -> &'static str {
        PLATFORM
    }

    fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            clipboard: false,
            renderer_attachment: false,
            multi_window: false,
            ..PlatformWindowFeatures::supported()
        }
    }

    fn state(&self) -> Result<PlatformWindowState, PlatformWindowError> {
        let scale_factor = self.window.scale_factor();
        let size = self.window.inner_size().to_logical::<f32>(scale_factor);
        Ok(PlatformWindowState {
            size: Size::new(size.width, size.height),
            scale_factor: scale_factor as f32,
            focused: self.window.has_focus(),
            visible: self.window.is_visible().unwrap_or(true),
            renderer_attached: true,
        })
    }

    fn set_title(&mut self, title: &str) -> Result<(), PlatformWindowError> {
        self.window.set_title(title);
        Ok(())
    }

    fn request_redraw(&self) -> Result<(), PlatformWindowError> {
        self.window.request_redraw();
        Ok(())
    }
}

/// Creating a window, its surface or its renderer failed.
fn backend_error(message: impl std::fmt::Display) -> PlatformWindowError {
    PlatformWindowError::backend(PLATFORM, message.to_string())
}

pub(super) struct WindowDriver<F, E> {
    id: WindowId,
    window: WinitWindow,
    renderer: WgpuRenderer,
    surface: WindowSurface,
    /// This window's last frame, so each frame redraws only what changed.
    retained: RetainedTarget,
    presenter: Presenter<E>,
    build_root: F,
    input: InputState,
    last_viewport_size: Size,
    events: Vec<PlatformWindowEvent>,
}

impl<F, E> WindowDriver<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    /// Open a window for `options` and build its first tree. `context` has the
    /// window active.
    pub(super) fn open(
        event_loop: &ActiveEventLoop,
        instance: &wgpu::Instance,
        context: &mut AppContext,
        id: WindowId,
        options: &WindowOptions,
        mut build_root: F,
    ) -> Result<Self, PlatformWindowError> {
        let window = Arc::new(
            event_loop
                .create_window(window_attributes(options))
                .map_err(backend_error)?,
        );
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(backend_error)?;
        let physical = window.inner_size();
        let (renderer, surface) = WgpuRenderer::with_surface(
            instance,
            surface,
            (physical.width, physical.height),
            RendererResourceLimits::default(),
        )?;
        if let Some(millihertz) = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
        {
            context.set_frame_interval(std::time::Duration::from_secs_f64(
                1000.0 / f64::from(millihertz),
            ));
        }
        let device = crate::renderer::Renderer::diagnostics(&renderer).device;
        log::info!(
            "winit window presenting through {} on {}",
            device.backend,
            device.device_name
        );
        let input = InputState::new(window.scale_factor());
        // AppContext is the single owner of viewport size.
        let viewport_size = input.logical_size(physical);
        context.set_viewport_size(viewport_size);
        let root = build_root(context);
        let presenter = Presenter::with_root(viewport_size, root);
        window.request_redraw();
        Ok(Self {
            id,
            window: WinitWindow { window },
            renderer,
            surface,
            retained: RetainedTarget::new(),
            presenter,
            build_root,
            input,
            last_viewport_size: viewport_size,
            events: Vec::new(),
        })
    }

    pub(super) fn id(&self) -> WindowId {
        self.id
    }

    pub(super) fn winit_id(&self) -> winit::window::WindowId {
        self.window.window.id()
    }

    /// Queue `event` for the next frame.
    pub(super) fn push_event(&mut self, event: &winit::event::WindowEvent) {
        if let Some(event) = self.input.translate(event) {
            self.events.push(event);
        }
    }

    /// Handle the queued events and draw if there is work. `context` has this
    /// window active. Returns whether the window asked to close.
    pub(super) fn run_iteration(&mut self, context: &mut AppContext) -> bool {
        let mut viewport_size = self.last_viewport_size;
        let mut ordered = Vec::new();
        let mut focus_changed = None;
        let mut close_requested = false;
        for event in std::mem::take(&mut self.events) {
            mark_platform_event_redraw(&event, context);
            match event {
                PlatformWindowEvent::Resized(size) => viewport_size = size,
                PlatformWindowEvent::FocusChanged(focused) => focus_changed = Some(focused),
                PlatformWindowEvent::CloseRequested => close_requested = true,
                PlatformWindowEvent::Input(input) => append_input_event(input, &mut ordered),
                _ => {}
            }
        }

        let viewport_changed = synchronize_viewport_after_platform_events(
            context,
            viewport_size,
            self.last_viewport_size,
        );
        if viewport_changed {
            let physical = self.window.window.inner_size();
            self.surface
                .resize(&self.renderer, (physical.width, physical.height));
            schedule_platform_redraw(&self.window, context, RedrawSource::PlatformResize);
        }

        let Self {
            window,
            renderer,
            surface,
            retained,
            presenter,
            build_root,
            ..
        } = self;
        let outcome = FramePipeline::run_frame(
            context,
            presenter,
            build_root,
            viewport_size,
            IdlePolicy::SkipWhenIdle,
            |presenter, context| {
                dispatch_events(
                    presenter,
                    context,
                    window,
                    viewport_changed.then_some(viewport_size),
                    focus_changed,
                    close_requested,
                    &ordered,
                );
                Ok(())
            },
            |presenter, _context| {
                let presented = surface
                    .present(renderer, presenter.scene(), retained, viewport_size)
                    .map_err(|err| {
                        FramePipelineError::stage(FrameStage::Present, err.to_string())
                    })?;
                Ok(if presented {
                    FramePresentation::Presented(Some(crate::renderer::Renderer::diagnostics(
                        renderer,
                    )))
                } else {
                    FramePresentation::Deferred
                })
            },
        );
        match outcome {
            Ok(Some(outcome)) => {
                self.last_viewport_size = viewport_size;
                if mark_deferred_frame_for_retry(context, outcome.presented) {
                    self.window.window.request_redraw();
                }
            }
            Ok(None) => {}
            Err(err) => panic!("frame failed: {err}"),
        }
        close_requested
    }
}

fn window_attributes(options: &WindowOptions) -> winit::window::WindowAttributes {
    let size = |size: Size| winit::dpi::LogicalSize::new(size.width, size.height);
    let mut attributes = winit::window::Window::default_attributes()
        .with_title(options.title.clone())
        .with_inner_size(size(options.size))
        .with_resizable(options.resizable)
        .with_decorations(options.decorations)
        .with_transparent(options.transparent);
    if let Some(min_size) = options.min_size {
        attributes = attributes.with_min_inner_size(size(min_size));
    }
    if let Some(max_size) = options.max_size {
        attributes = attributes.with_max_inner_size(size(max_size));
    }
    attributes
}

/// The `DispatchEvents` stage: the portable part of what the macOS runner
/// dispatches, without IME composition, accessibility or file drops.
fn dispatch_events<E: Element>(
    presenter: &mut Presenter<E>,
    context: &mut AppContext,
    window: &WinitWindow,
    resized: Option<Size>,
    focus_changed: Option<bool>,
    close_requested: bool,
    ordered: &[OrderedInputEvent],
) {
    if let Some(size) = resized {
        presenter.handle_window_event(&Event::WindowResize {
            width: size.width,
            height: size.height,
        });
    }
    if let Some(focused) = focus_changed {
        let event = if focused {
            Event::Focus(FocusEvent { focused: true })
        } else {
            presenter.set_focused_element(None);
            Event::Blur(FocusEvent { focused: false })
        };
        presenter.handle_window_event(&event);
        schedule_platform_redraw(window, context, RedrawSource::PlatformFocus);
    }
    if close_requested {
        presenter.handle_window_event(&Event::WindowClose);
        schedule_platform_redraw(window, context, RedrawSource::PlatformLifecycle);
    }
    if presenter.dispatch_menu_actions(context) {
        schedule_platform_redraw(window, context, RedrawSource::PlatformInput);
    }
    for event in ordered {
        let (handled, redraw_requested, source) = match event {
            OrderedInputEvent::Pointer(event) => {
                let dispatch = presenter.dispatch_pointer_event(event);
                (
                    dispatch.stopped,
                    dispatch.redraw_requested,
                    RedrawSource::Element,
                )
            }
            #[cfg(all(target_os = "macos", feature = "metal"))]
            OrderedInputEvent::FileDrag(event) => {
                let dispatch = presenter.dispatch_file_drag(event);
                (
                    dispatch.stopped,
                    dispatch.redraw_requested,
                    RedrawSource::Element,
                )
            }
            OrderedInputEvent::Scroll(event) => {
                let (handled, redraw_requested) = presenter
                    .with_event_context(|root, event_cx| root.handle_scroll_event(event_cx, event));
                (handled, redraw_requested, RedrawSource::Element)
            }
            OrderedInputEvent::Key { is_down, event } => {
                // As on macOS, a typed character goes to a focused text input
                // as text instead of as a key press.
                let text = event
                    .char
                    .filter(|_| *is_down && !event.modifiers.ctrl && !event.modifiers.meta)
                    .filter(|_| {
                        presenter.focused_element().is_some_and(|focused| {
                            presenter.root().text_input_snapshot(focused).is_some()
                        })
                    });
                let (handled, redraw_requested) = match text {
                    Some(ch) => presenter.with_event_context(|root, event_cx| {
                        root.handle_text_input_command(
                            event_cx,
                            &TextInputCommand::InsertText(ch.to_string()),
                        )
                    }),
                    None => dispatch_key(presenter, context, *is_down, event),
                };
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
            OrderedInputEvent::Text(command) => {
                let (handled, redraw_requested) = presenter.with_event_context(|root, event_cx| {
                    root.handle_text_input_command(event_cx, command)
                });
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
            // Only the AppKit accessibility bridge produces these.
            #[cfg(all(target_os = "macos", feature = "metal"))]
            OrderedInputEvent::Accessibility(_) => continue,
        };
        if handled || redraw_requested {
            schedule_platform_redraw(window, context, source);
        }
    }
    if context.consume_runtime_view_notification() {
        schedule_platform_redraw(window, context, RedrawSource::ViewNotification);
    }
}
