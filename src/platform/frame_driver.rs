//! Frame-driver pieces shared by the native runners: viewport sync, redraw
//! scheduling, input ordering and key routing. Nothing here calls a platform
//! API; each runner supplies its window through [`PlatformWindow`].

use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{KeyEvent, ScrollEvent};
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::elements::element::{Element, PointerEvent, PointerEventKind};
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEventKind, PlatformWindow,
    PlatformWindowEvent,
};

/// Propagates a platform-reported size change to the single viewport-size owner.
pub(crate) fn synchronize_viewport_after_platform_events(
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

pub(crate) fn mark_platform_event_redraw(event: &PlatformWindowEvent, context: &mut AppContext) {
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

pub(crate) fn append_input_event(
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
    Key {
        is_down: bool,
        event: KeyEvent,
    },
    Text(crate::core::text_editing::TextInputCommand),
    #[cfg(all(target_os = "macos", feature = "metal"))]
    Accessibility(crate::platform::mac::MacAccessibilityActionRequest),
    #[cfg(all(target_os = "macos", feature = "metal"))]
    FileDrag(crate::core::drag::FileDragEvent),
}

pub(crate) fn mark_deferred_frame_for_retry(context: &mut AppContext, presented: bool) -> bool {
    !presented && context.request_platform_redraw_from(RedrawSource::PlatformRedraw)
}

pub(crate) fn dispatch_key<E>(
    presenter: &mut Presenter<E>,
    context: &mut AppContext,
    is_down: bool,
    event: &KeyEvent,
) -> (bool, bool)
where
    E: Element,
{
    if !should_forward_key_event_to_tree(is_down) {
        return (false, false);
    }
    presenter.with_event_context(|root, event_cx| route_key_event(root, context, event_cx, event))
}

/// Only key-down events reach elements; key-up is consumed by the platform layer.
pub(crate) fn should_forward_key_event_to_tree(is_down: bool) -> bool {
    is_down
}
