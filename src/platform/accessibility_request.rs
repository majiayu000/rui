//! Accessibility requests from a screen reader, as every runner dispatches
//! them: the AppKit bridge on macOS and AccessKit in the winit runner.

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityAction, AccessibilityNode, AccessibilityTree};
use crate::core::action::{ActionId, StandardAction};
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::elements::element::Element;

/// What an assistive technology asked one element to do.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityRequest {
    Action {
        action: AccessibilityAction,
        value: Option<String>,
    },
    Focus(bool),
}

/// Dispatch `request` for element `id` to the presenter's tree. The element
/// must be in `retained_tree`, the tree the platform last published, or in a
/// fresh tree when none was published, and must offer the action. `platform`
/// names the runner in the logs. Returns whether the request was handled and
/// whether it asked for a redraw.
pub(crate) fn dispatch_accessibility_request<E>(
    presenter: &mut Presenter<E>,
    id: ElementId,
    request: &AccessibilityRequest,
    retained_tree: Option<&AccessibilityTree>,
    platform: &str,
) -> (bool, bool)
where
    E: Element,
{
    let Some(node) = current_accessibility_node(presenter, id, retained_tree, platform) else {
        if presenter.focused_element() == Some(id) {
            presenter.set_focused_element(None);
            sync_accessibility_focus(presenter);
        }
        log::error!("discarded stale {platform} accessibility request for element {id:?}");
        return (false, false);
    };
    let (action, value) = match request {
        AccessibilityRequest::Action { action, value } => (action, value),
        AccessibilityRequest::Focus(focused) => {
            if *focused {
                presenter.set_focused_element(Some(id));
            } else if presenter.focused_element() == Some(id) {
                presenter.set_focused_element(None);
            }
            let redraw_requested = sync_accessibility_focus(presenter);
            return (true, redraw_requested);
        }
    };
    if !node.a11y_actions().contains(action) {
        log::error!(
            "discarded stale {platform} accessibility action {action:?} for element {id:?}"
        );
        return (false, false);
    }
    presenter.set_focused_element(Some(id));
    let focus_redraw = sync_accessibility_focus(presenter);
    match action {
        AccessibilityAction::Activate => {
            let (outcome, redraw_requested) = presenter.with_event_context(|root, event_cx| {
                root.dispatch_action(event_cx, &ActionId::Standard(StandardAction::Activate))
            });
            (outcome.is_handled(), focus_redraw || redraw_requested)
        }
        AccessibilityAction::SetValue => {
            let Some(value) = value else {
                log::error!("{platform} accessibility SetValue request for {id:?} had no value");
                return (false, false);
            };
            let composition_active = node.a11y_text_composition().is_some();
            let (handled, redraw_requested) = presenter.with_event_context(|root, event_cx| {
                let composition_cancelled = !composition_active
                    || root.handle_text_input_event(event_cx, &TextInputEvent::CancelComposition);
                let selection =
                    root.dispatch_action(event_cx, &ActionId::Standard(StandardAction::SelectAll));
                let inserted = root
                    .handle_text_input_event(event_cx, &TextInputEvent::InsertText(value.clone()));
                composition_cancelled && selection.is_handled() && inserted
            });
            (handled, focus_redraw || redraw_requested)
        }
        AccessibilityAction::ScrollForward
        | AccessibilityAction::ScrollBackward
        | AccessibilityAction::Increment
        | AccessibilityAction::Decrement => {
            let action = match action {
                AccessibilityAction::ScrollForward => {
                    crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION
                }
                AccessibilityAction::ScrollBackward => {
                    crate::core::action::ACCESSIBILITY_SCROLL_BACKWARD_ACTION
                }
                AccessibilityAction::Increment => {
                    crate::core::action::ACCESSIBILITY_INCREMENT_ACTION
                }
                _ => crate::core::action::ACCESSIBILITY_DECREMENT_ACTION,
            };
            let (outcome, redraw_requested) = presenter.with_event_context(|root, event_cx| {
                root.dispatch_action(event_cx, &ActionId::custom(action))
            });
            (outcome.is_handled(), focus_redraw || redraw_requested)
        }
    }
}

fn current_accessibility_node<E>(
    presenter: &Presenter<E>,
    id: ElementId,
    retained_tree: Option<&AccessibilityTree>,
    platform: &str,
) -> Option<AccessibilityNode>
where
    E: Element,
{
    // Screen reader requests name elements of the tree the platform holds.
    // Prefer that retained tree whenever present so a request is never
    // validated against a rebuild that the platform did not receive (a build
    // that succeeded here but failed to publish), whose node actions or
    // membership may disagree.
    if let Some(tree) = retained_tree {
        return tree.find(id).cloned();
    }

    match presenter.accessibility_tree() {
        Ok(tree) => tree.find(id).cloned(),
        Err(err) => {
            log::error!(
                "failed to rebuild {platform} accessibility tree for request validation: {err}"
            );
            None
        }
    }
}

fn sync_accessibility_focus<E>(presenter: &mut Presenter<E>) -> bool
where
    E: Element,
{
    let (_, redraw_requested) = presenter.with_event_context(|root, event_cx| {
        root.dispatch_action(
            event_cx,
            &ActionId::custom(crate::core::action::SYNC_ACCESSIBILITY_FOCUS_ACTION),
        )
    });
    redraw_requested
}
