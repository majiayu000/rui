//! One winit window: its presenter, wgpu surface and frame.

use super::clipboard::WindowClipboard;
use super::ime::ImeSession;
use super::input::{InputState, file_drop_position};
use crate::core::ElementId;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::drag::FileDragEvent;
use crate::core::event::{Cursor, Event};
use crate::core::frame_pipeline::{
    FramePipeline, FramePipelineError, FramePresentation, FrameStage, IdlePolicy,
};
use crate::core::geometry::{Point, Size};
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputCommand;
use crate::core::window::{WindowId, WindowOptions};
use crate::elements::element::{Element, PointerEventKind};
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
use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{Ime, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::CursorIcon;

pub(super) const PLATFORM: &str = "winit";

/// The winit window behind the shared [`PlatformWindow`] contract.
pub(super) struct WinitWindow {
    window: Arc<winit::window::Window>,
    /// The cursor last shown, so a pointer move over the same element sets none.
    cursor: Cell<Cursor>,
    ime: Cell<ImeSession>,
    clipboard: WindowClipboard,
}

impl WinitWindow {
    /// Point the system input method at the focused field's painted caret.
    fn sync_ime<E: Element>(&self, presenter: &mut Presenter<E>) -> bool {
        let focused = presenter.focused_element().and_then(|id| {
            presenter
                .root()
                .text_input_snapshot(id)
                .map(|text| (id, text))
        });
        let redraw =
            self.release_ime_if_focus_changed(presenter, focused.as_ref().map(|(id, _)| *id));
        if let Some((id, text)) = focused {
            if matches!(self.ime.get(), ImeSession::Inactive) {
                self.ime.set(ImeSession::Waiting(id));
                self.window.set_ime_allowed(true);
            }
            if matches!(self.ime.get(), ImeSession::Active(_))
                && let Some(bounds) = text.caret_bounds()
            {
                self.window.set_ime_cursor_area(
                    LogicalPosition::new(bounds.x(), bounds.y()),
                    LogicalSize::new(bounds.width().max(1.0), bounds.height().max(1.0)),
                );
            }
        }
        redraw
    }

    fn release_ime_if_focus_changed<E: Element>(
        &self,
        presenter: &mut Presenter<E>,
        focused: Option<ElementId>,
    ) -> bool {
        let mut session = self.ime.get();
        let (released, redraw) = session.release_if_focus_changed(presenter, focused);
        if released {
            self.ime.set(session);
            self.window.set_ime_allowed(false);
        }
        redraw
    }

    /// Show the cursor the elements under the pointer asked for.
    fn show_cursor(&self, cursor: Cursor) {
        if self.cursor.replace(cursor) == cursor {
            return;
        }
        self.window.set_cursor(match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::Move => CursorIcon::Move,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::ResizeNS => CursorIcon::NsResize,
            Cursor::ResizeEW => CursorIcon::EwResize,
            Cursor::ResizeNESW => CursorIcon::NeswResize,
            Cursor::ResizeNWSE => CursorIcon::NwseResize,
            Cursor::Wait => CursorIcon::Wait,
            Cursor::Progress => CursorIcon::Progress,
        });
    }
}

impl PlatformWindow for WinitWindow {
    fn platform_name(&self) -> &'static str {
        PLATFORM
    }

    fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            clipboard: self.clipboard.is_available(),
            renderer_attachment: false,
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

    fn read_clipboard_text(&mut self) -> Result<String, PlatformWindowError> {
        self.clipboard.read_text()
    }

    fn write_clipboard_text(&mut self, text: &str) -> Result<(), PlatformWindowError> {
        self.clipboard.write_text(text)
    }
}

/// Creating a window, its surface or its renderer, or using the clipboard, failed.
pub(super) fn backend_error(message: impl std::fmt::Display) -> PlatformWindowError {
    PlatformWindowError::backend(PLATFORM, message.to_string())
}

/// One open window, behind the type the runner stores.
pub(crate) trait WinitWindowDriver {
    fn id(&self) -> WindowId;
    fn winit_id(&self) -> winit::window::WindowId;
    /// Queue `event` for the next frame.
    fn push_event(&mut self, event: &WindowEvent);
    /// Handle the queued events and draw if there is work. `context` has this
    /// window active. Returns whether the window asked to close.
    fn run_iteration(&mut self, context: &mut AppContext) -> bool;
}

/// A winit window with its surface and renderer, before its content is
/// attached.
pub(crate) struct WinitWindowParts {
    id: WindowId,
    window: Arc<winit::window::Window>,
    renderer: WgpuRenderer,
    surface: WindowSurface,
    clipboard: WindowClipboard,
    input: InputState,
    viewport_size: Size,
    /// The refresh interval of the window's monitor, when it reports one.
    frame_interval: Option<Duration>,
}

impl WinitWindowParts {
    /// Create the window for `options` and its surface and renderer.
    pub(super) fn create(
        event_loop: &ActiveEventLoop,
        instance: &wgpu::Instance,
        id: WindowId,
        options: &WindowOptions,
        clipboard: WindowClipboard,
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
        let frame_interval = window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map(|millihertz| Duration::from_secs_f64(1000.0 / f64::from(millihertz)));
        let device = crate::renderer::Renderer::diagnostics(&renderer).device;
        log::info!(
            "winit window presenting through {} on {}",
            device.backend,
            device.device_name
        );
        let input = InputState::new(window.scale_factor());
        let viewport_size = input.logical_size(physical);
        Ok(Self {
            id,
            window,
            clipboard,
            renderer,
            surface,
            input,
            viewport_size,
            frame_interval,
        })
    }
}

/// Attach `build_root` to a created window and build its first tree.
/// `context` has the window active.
pub(crate) fn winit_window<F, E>(
    context: &mut AppContext,
    parts: WinitWindowParts,
    mut build_root: F,
) -> Box<dyn WinitWindowDriver>
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    if let Some(interval) = parts.frame_interval {
        context.set_frame_interval(interval);
    }
    // AppContext is the single owner of viewport size. The window's frame state
    // and its presenter start from the same size, so its first build and its
    // first layout agree.
    context.set_viewport_size(parts.viewport_size);
    let root = build_root(context);
    let presenter = Presenter::with_root(parts.viewport_size, root);
    parts.window.request_redraw();
    Box::new(WindowDriver {
        id: parts.id,
        window: WinitWindow {
            window: parts.window,
            cursor: Cell::new(Cursor::Default),
            ime: Cell::new(ImeSession::Inactive),
            clipboard: parts.clipboard,
        },
        renderer: parts.renderer,
        surface: parts.surface,
        retained: RetainedTarget::new(),
        presenter,
        build_root,
        input: parts.input,
        last_viewport_size: parts.viewport_size,
        events: Vec::new(),
        file_drop: None,
    })
}

struct FileDrop {
    position: Point,
    paths: Vec<PathBuf>,
}

impl FileDrop {
    fn append(self, ordered: &mut Vec<OrderedInputEvent>) {
        let Self { position, paths } = self;
        ordered.extend([
            OrderedInputEvent::FileDrag(FileDragEvent::Entered { position, paths }),
            OrderedInputEvent::FileDrag(FileDragEvent::Dropped { position }),
        ]);
    }
}

enum QueuedEvent {
    Platform(PlatformWindowEvent),
    Ime(Ime),
    FileDrop(FileDrop),
}

struct WindowDriver<F, E> {
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
    events: Vec<QueuedEvent>,
    /// winit sends one consecutive event for each file in a native drop.
    file_drop: Option<FileDrop>,
}

impl<F, E> WinitWindowDriver for WindowDriver<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn id(&self) -> WindowId {
        self.id
    }

    fn winit_id(&self) -> winit::window::WindowId {
        self.window.window.id()
    }

    fn push_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::DroppedFile(path) = event {
            if let Some(drop) = &mut self.file_drop {
                drop.paths.push(path.clone());
            } else if let Some(position) = file_drop_position(&self.window.window) {
                self.file_drop = Some(FileDrop {
                    position,
                    paths: vec![path.clone()],
                });
            } else {
                log::warn!("winit file drop did not have a native cursor position");
            }
            return;
        }
        if let Some(drop) = self.file_drop.take() {
            self.events.push(QueuedEvent::FileDrop(drop));
        }
        if let WindowEvent::Ime(event) = event {
            self.events.push(QueuedEvent::Ime(event.clone()));
        } else if let Some(event) = self.input.translate(event) {
            self.events.push(QueuedEvent::Platform(event));
        }
    }

    fn run_iteration(&mut self, context: &mut AppContext) -> bool {
        if let Some(drop) = self.file_drop.take() {
            self.events.push(QueuedEvent::FileDrop(drop));
        }
        let mut viewport_size = self.last_viewport_size;
        let mut ordered = Vec::new();
        let mut close_requested = false;
        for event in std::mem::take(&mut self.events) {
            let event = match event {
                QueuedEvent::Platform(event) => event,
                QueuedEvent::Ime(event) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    append_winit_ime(event, &mut ordered);
                    continue;
                }
                QueuedEvent::FileDrop(drop) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    drop.append(&mut ordered);
                    continue;
                }
            };
            mark_platform_event_redraw(&event, context);
            match event {
                PlatformWindowEvent::Resized(size) => viewport_size = size,
                PlatformWindowEvent::FocusChanged(focused) => {
                    ordered.push(OrderedInputEvent::WinitFocus(focused))
                }
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
                if self.window.sync_ime(&mut self.presenter) {
                    schedule_platform_redraw(&self.window, context, RedrawSource::PlatformInput);
                }
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

fn append_winit_ime(event: Ime, ordered: &mut Vec<OrderedInputEvent>) {
    // winit sends an empty preedit immediately before a commit. Keep the
    // composition intact when the two events arrive in this frame.
    if matches!(&event, Ime::Commit(_))
        && matches!(
            ordered.last(),
            Some(OrderedInputEvent::WinitIme(Ime::Preedit(text, _))) if text.is_empty()
        )
    {
        ordered.pop();
    }
    ordered.push(OrderedInputEvent::WinitIme(event));
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

/// The `DispatchEvents` stage for a winit window.
fn dispatch_events<E: Element>(
    presenter: &mut Presenter<E>,
    context: &mut AppContext,
    window: &WinitWindow,
    resized: Option<Size>,
    close_requested: bool,
    ordered: &[OrderedInputEvent],
) {
    if let Some(size) = resized {
        presenter.handle_window_event(&Event::WindowResize {
            width: size.width,
            height: size.height,
        });
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
            OrderedInputEvent::Pointer(event, modifiers) => {
                let dispatch = presenter.dispatch_pointer_event_with(event, *modifiers);
                if matches!(event.kind, PointerEventKind::Move) {
                    window.show_cursor(presenter.cursor());
                }
                (
                    dispatch.stopped,
                    dispatch.redraw_requested,
                    RedrawSource::Element,
                )
            }
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
            OrderedInputEvent::WinitFocus(focused) => {
                presenter.handle_window_focus(*focused);
                (true, false, RedrawSource::PlatformFocus)
            }
            OrderedInputEvent::WinitIme(event) => {
                let mut session = window.ime.get();
                let (handled, redraw_requested) = session.dispatch(presenter, event);
                window.ime.set(session);
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
            // Native text and accessibility from AppKit are queued by its runner.
            #[cfg(all(target_os = "macos", feature = "metal"))]
            OrderedInputEvent::Text(_) | OrderedInputEvent::Accessibility(_) => continue,
        };
        let focused = presenter.focused_element();
        let cancelled = window.release_ime_if_focus_changed(presenter, focused);
        if handled || redraw_requested || cancelled {
            schedule_platform_redraw(window, context, source);
        }
    }
    if context.consume_runtime_view_notification() {
        schedule_platform_redraw(window, context, RedrawSource::ViewNotification);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::{div, input};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn composition_updates_then_commits_once_in_the_focused_input() {
        let id = ElementId(1);
        let mut presenter =
            Presenter::with_root(Size::new(200.0, 100.0), input().id(id).value("例"));
        presenter.set_focused_element(Some(id));
        let mut session = ImeSession::Waiting(id);
        session.dispatch(&mut presenter, &Ime::Enabled);
        let mut ordered = Vec::new();
        for event in [
            Ime::Preedit("p".into(), None),
            Ime::Preedit("pin".into(), None),
            Ime::Preedit(String::new(), None),
            Ime::Commit("拼".into()),
        ] {
            append_winit_ime(event, &mut ordered);
        }
        assert_eq!(ordered.len(), 3);
        for (event, (expected, composing)) in
            ordered
                .iter()
                .zip([("例p", true), ("例pin", true), ("例拼", false)])
        {
            let OrderedInputEvent::WinitIme(event) = event else {
                panic!("composition should reach the text input");
            };
            assert!(session.dispatch(&mut presenter, event).0);
            let text = presenter
                .root()
                .text_input_snapshot(id)
                .expect("text field");
            assert_eq!(text.text(), expected);
            assert_eq!(text.composition().is_some(), composing);
        }
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit("音".into(), None))
                .0
        );
        assert!(
            session
                .dispatch(&mut presenter, &Ime::Preedit(String::new(), None))
                .0
        );
        let text = presenter
            .root()
            .text_input_snapshot(id)
            .expect("text field");
        assert_eq!(text.text(), "例拼");
        assert!(text.composition().is_none());
    }

    #[test]
    fn a_native_drop_delivers_all_paths_together_to_the_target_at_its_position() {
        let paths = vec![PathBuf::from("first.txt"), PathBuf::from("second.txt")];
        let output = Rc::new(RefCell::new(Vec::new()));
        let root = div()
            .w(200.0)
            .h(100.0)
            .flex_row()
            .child(div().w(100.0).h(100.0).on_drop_files(|_| {
                panic!("a drop on the second target must not reach the first target")
            }))
            .child(div().w(100.0).h(100.0).on_drop_files({
                let output = Rc::clone(&output);
                move |paths| output.borrow_mut().push(paths.to_vec())
            }));
        let size = Size::new(200.0, 100.0);
        let mut presenter = Presenter::with_root(size, root);
        presenter.layout(size).expect("drop targets lay out");
        presenter.paint();
        let mut ordered = Vec::new();
        FileDrop {
            position: Point::new(150.0, 50.0),
            paths: paths.clone(),
        }
        .append(&mut ordered);
        for event in ordered {
            let OrderedInputEvent::FileDrag(event) = event else {
                panic!("the file drop should reach the presenter");
            };
            presenter.dispatch_file_drag(&event);
        }
        assert_eq!(*output.borrow(), [paths]);
    }
}
