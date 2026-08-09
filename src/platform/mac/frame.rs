//! Native frame driving for the macOS runner.
//!
//! Holds the two stages the shared [`FramePipeline`](crate::core::FramePipeline)
//! cannot supply itself: which backend-neutral events arrived this frame, and
//! which backend receives the painted scene. The stage *order* lives in
//! `FrameStage::ORDER`, not here.

use crate::core::ElementId;
use crate::core::action::route_key_event;
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{Event, FocusEvent, KeyEvent, ScrollEvent};
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::elements::element::{Element, PointerEvent};
use crate::platform::mac::app::{OrderedInputEvent, schedule_platform_redraw};
use crate::platform::window::PlatformWindow;

/// Backend-neutral events collected from one poll of the platform window.
pub(crate) struct NativeFrameEvents {
    pub viewport_changed: bool,
    pub viewport_size: Size,
    pub focus_changed: Option<bool>,
    pub close_requested: bool,
    pub automation_focused_element: Option<ElementId>,
    pub pointer_events: Vec<PointerEvent>,
    pub scroll_events: Vec<ScrollEvent>,
    pub ordered_input_events: Vec<OrderedInputEvent>,
}

/// Runs the `DispatchEvents` stage for the native runner.
///
/// Returns whether a resize was applied, so the caller can advance the size it
/// compares against on the next poll.
pub(crate) fn dispatch_native_events<E, W>(
    presenter: &mut Presenter<E>,
    context: &mut AppContext,
    window: &W,
    events: &NativeFrameEvents,
) -> bool
where
    E: Element,
    W: PlatformWindow,
{
    if events.viewport_changed {
        presenter.handle_window_event(&Event::WindowResize {
            width: events.viewport_size.width,
            height: events.viewport_size.height,
        });
    }

    if let Some(is_focused) = events.focus_changed {
        let event = if is_focused {
            Event::Focus(FocusEvent { focused: true })
        } else {
            presenter.set_focused_element(None);
            Event::Blur(FocusEvent { focused: false })
        };
        presenter.handle_window_event(&event);
        schedule_platform_redraw(window, context, RedrawSource::PlatformFocus);
    }

    if events.close_requested {
        presenter.handle_window_event(&Event::WindowClose);
        context.quit();
        schedule_platform_redraw(window, context, RedrawSource::PlatformLifecycle);
    }

    if let Some(focused) = events.automation_focused_element {
        presenter.set_focused_element(Some(focused));
    }

    for event in &events.pointer_events {
        if presenter.dispatch_pointer_event(event).redraw_requested {
            schedule_platform_redraw(window, context, RedrawSource::Element);
        }
    }

    for event in &events.scroll_events {
        let (_, redraw_requested) = presenter
            .with_event_context(|root, event_cx| root.handle_scroll_event(event_cx, event));
        if redraw_requested {
            schedule_platform_redraw(window, context, RedrawSource::Element);
        }
    }

    for event in &events.ordered_input_events {
        let (handled, redraw_requested) = match event {
            OrderedInputEvent::Key { is_down, event } => {
                dispatch_key(presenter, context, *is_down, event)
            }
            OrderedInputEvent::Text(event) => presenter
                .with_event_context(|root, event_cx| root.handle_text_input_event(event_cx, event)),
        };
        if handled || redraw_requested {
            schedule_platform_redraw(window, context, RedrawSource::PlatformInput);
        }
    }

    if context.consume_runtime_view_notification() {
        schedule_platform_redraw(window, context, RedrawSource::ViewNotification);
    }

    events.viewport_changed
}

fn dispatch_key<E>(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_key_down_events_are_forwarded_to_elements() {
        assert!(should_forward_key_event_to_tree(true));
        assert!(!should_forward_key_event_to_tree(false));
    }
}
