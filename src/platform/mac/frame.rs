//! Native frame driving for the macOS runner.
//!
//! Holds the two stages the shared [`FramePipeline`](crate::core::FramePipeline)
//! cannot supply itself: which backend-neutral events arrived this frame, and
//! which backend receives the painted scene. The stage *order* lives in
//! `FrameStage::ORDER`, not here.

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityAction, AccessibilityNode};
use crate::core::action::route_key_event;
use crate::core::action::{ActionId, StandardAction};
use crate::core::app::{AppContext, RedrawSource};
use crate::core::event::{Event, FocusEvent, KeyEvent};
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::core::text_editing::TextInputEvent;
use crate::elements::element::Element;
use crate::platform::mac::accessibility::MacAccessibilityActionRequest;
use crate::platform::mac::accessibility::MacAccessibilityRequest;
use crate::platform::mac::app::{OrderedInputEvent, schedule_platform_redraw};
use crate::platform::mac::window::MacWindow;

/// Backend-neutral events collected from one poll of the platform window.
pub(crate) struct NativeFrameEvents {
    pub viewport_changed: bool,
    pub viewport_size: Size,
    pub focus_changed: Option<bool>,
    pub close_requested: bool,
    pub automation_focused_element: Option<ElementId>,
    pub ordered_input_events: Vec<OrderedInputEvent>,
}

#[derive(Debug, Default)]
pub(crate) struct NativeImeState {
    composition_owner: Option<ElementId>,
}

impl NativeImeState {
    fn target_for_event(
        &mut self,
        event: &TextInputEvent,
        focused: Option<ElementId>,
    ) -> Option<ElementId> {
        match event {
            TextInputEvent::InsertText(_) | TextInputEvent::InsertTextReplacing { .. } => focused,
            TextInputEvent::BeginComposition(_)
            | TextInputEvent::BeginCompositionReplacing { .. } => {
                self.composition_owner = focused;
                focused
            }
            TextInputEvent::UpdateComposition(_)
            | TextInputEvent::UpdateCompositionReplacing { .. } => self.composition_owner,
            TextInputEvent::CommitComposition(_)
            | TextInputEvent::CommitCompositionReplacing { .. }
            | TextInputEvent::CancelComposition => self.composition_owner.take(),
        }
    }

    fn cancel_owner_after_focus_change(&mut self, focused: Option<ElementId>) -> Option<ElementId> {
        if self.composition_owner.is_some() && self.composition_owner != focused {
            self.composition_owner.take()
        } else {
            None
        }
    }
}

/// Runs the `DispatchEvents` stage for the native runner.
///
/// Returns whether a resize was applied, so the caller can advance the size it
/// compares against on the next poll.
pub(crate) fn dispatch_native_events<E>(
    presenter: &mut Presenter<E>,
    context: &mut AppContext,
    window: &MacWindow,
    events: &NativeFrameEvents,
    ime_state: &mut NativeImeState,
) -> bool
where
    E: Element,
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
        cancel_composition_if_owner_lost(presenter, window, ime_state);
        schedule_platform_redraw(window, context, RedrawSource::PlatformFocus);
    }

    if events.close_requested {
        presenter.handle_window_event(&Event::WindowClose);
        context.quit();
        schedule_platform_redraw(window, context, RedrawSource::PlatformLifecycle);
    }

    if let Some(focused) = events.automation_focused_element {
        presenter.set_focused_element(Some(focused));
        if cancel_composition_if_owner_lost(presenter, window, ime_state) {
            schedule_platform_redraw(window, context, RedrawSource::PlatformInput);
        }
    }

    for event in &events.ordered_input_events {
        let (handled, redraw_requested, redraw_source) = match event {
            OrderedInputEvent::Pointer(event) => {
                let dispatch = presenter.dispatch_pointer_event(event);
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
                let (handled, redraw_requested) = dispatch_key(presenter, context, *is_down, event);
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
            OrderedInputEvent::Text(event) => {
                let (handled, redraw_requested) =
                    dispatch_text_input_event(presenter, ime_state, event);
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
            OrderedInputEvent::Accessibility(request) => {
                let (handled, redraw_requested) = dispatch_accessibility_action(presenter, request);
                (handled, redraw_requested, RedrawSource::PlatformInput)
            }
        };
        let focus_cancelled = cancel_composition_if_owner_lost(presenter, window, ime_state);
        if handled || redraw_requested || focus_cancelled {
            schedule_platform_redraw(window, context, redraw_source);
        }
    }

    if context.consume_runtime_view_notification() {
        schedule_platform_redraw(window, context, RedrawSource::ViewNotification);
    }

    events.viewport_changed
}

fn dispatch_text_input_event<E>(
    presenter: &mut Presenter<E>,
    ime_state: &mut NativeImeState,
    event: &TextInputEvent,
) -> (bool, bool)
where
    E: Element,
{
    let Some(target) = ime_state.target_for_event(event, presenter.focused_element()) else {
        log::error!("discarded macOS text input event without a focused composition owner");
        return (false, false);
    };
    dispatch_text_input_event_to(presenter, target, event)
}

fn dispatch_text_input_event_to<E>(
    presenter: &mut Presenter<E>,
    target: ElementId,
    event: &TextInputEvent,
) -> (bool, bool)
where
    E: Element,
{
    let focused = presenter.focused_element();
    *presenter.focused_element_mut() = Some(target);
    let result = presenter
        .with_event_context(|root, event_cx| root.handle_text_input_event(event_cx, event));
    *presenter.focused_element_mut() = focused;
    result
}

fn cancel_composition_if_owner_lost<E>(
    presenter: &mut Presenter<E>,
    window: &MacWindow,
    ime_state: &mut NativeImeState,
) -> bool
where
    E: Element,
{
    let Some(owner) = ime_state.cancel_owner_after_focus_change(presenter.focused_element()) else {
        return false;
    };
    let (handled, redraw_requested) =
        dispatch_text_input_event_to(presenter, owner, &TextInputEvent::CancelComposition);
    window.discard_marked_text();
    if !handled {
        log::error!("failed to cancel macOS composition for its previous focused owner");
    }
    handled || redraw_requested
}

fn dispatch_accessibility_action<E>(
    presenter: &mut Presenter<E>,
    request: &MacAccessibilityActionRequest,
) -> (bool, bool)
where
    E: Element,
{
    let Some(node) = current_accessibility_node(presenter, request.id) else {
        if presenter.focused_element() == Some(request.id) {
            presenter.set_focused_element(None);
            sync_accessibility_focus(presenter);
        }
        log::error!(
            "discarded stale macOS accessibility request for element {:?}",
            request.id
        );
        return (false, false);
    };
    let (action, value) = match &request.request {
        MacAccessibilityRequest::Action { action, value } => (action, value),
        MacAccessibilityRequest::Focus(focused) => {
            if *focused {
                presenter.set_focused_element(Some(request.id));
            } else if presenter.focused_element() == Some(request.id) {
                presenter.set_focused_element(None);
            }
            let redraw_requested = sync_accessibility_focus(presenter);
            return (true, redraw_requested);
        }
    };
    if !node.a11y_actions().contains(action) {
        log::error!(
            "discarded stale macOS accessibility action {:?} for element {:?}",
            action,
            request.id
        );
        return (false, false);
    }
    presenter.set_focused_element(Some(request.id));
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
                log::error!(
                    "macOS accessibility SetValue request for {:?} had no value",
                    request.id
                );
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
        AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward => {
            let action = if *action == AccessibilityAction::ScrollForward {
                crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION
            } else {
                crate::core::action::ACCESSIBILITY_SCROLL_BACKWARD_ACTION
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
) -> Option<AccessibilityNode>
where
    E: Element,
{
    let tree = match presenter.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => {
            log::error!("failed to validate macOS accessibility request: {err}");
            return None;
        }
    };
    find_accessibility_node(tree.roots(), id).cloned()
}

fn find_accessibility_node(
    nodes: &[AccessibilityNode],
    id: ElementId,
) -> Option<&AccessibilityNode> {
    nodes.iter().find_map(|node| {
        (node.a11y_id() == id)
            .then_some(node)
            .or_else(|| find_accessibility_node(node.a11y_children(), id))
    })
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
    use crate::advanced_ui::{
        DataList, DataListItem, DataTableCell, DataTableRow, DataTree, DataTreeItem, Menu,
        SegmentedControl, Tab, TabList,
    };
    use crate::core::accessibility::{AccessibilityContext, AccessibilityNode, AccessibilityRole};
    use crate::core::action::ActionOutcome;
    use crate::core::geometry::Bounds;
    use crate::core::style::Style;
    use crate::elements::Input;
    use crate::elements::element::{EventContext, LayoutContext, PaintContext};
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::prelude::NodeId;

    #[derive(Debug, Clone, PartialEq)]
    enum ObservedAccessibilityEvent {
        Action(ActionId),
        Text(TextInputEvent),
    }

    struct AccessibilityActionProbe {
        id: ElementId,
        style: Style,
        observed: Rc<RefCell<Vec<ObservedAccessibilityEvent>>>,
        actions: Vec<AccessibilityAction>,
    }

    impl Element for AccessibilityActionProbe {
        fn id(&self) -> Option<ElementId> {
            Some(self.id)
        }

        fn style(&self) -> &Style {
            &self.style
        }

        fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
            match cx.taffy.new_leaf(taffy::Style::default()) {
                Ok(node) => node,
                Err(err) => panic!("accessibility action probe layout failed: {err}"),
            }
        }

        fn paint(&mut self, _cx: &mut PaintContext) {}

        fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
            if matches!(action, ActionId::Custom(name) if name == crate::core::action::SYNC_ACCESSIBILITY_FOCUS_ACTION)
            {
                return ActionOutcome::Ignored;
            }
            if !cx.is_focused(Some(self.id)) {
                return ActionOutcome::Ignored;
            }
            self.observed
                .borrow_mut()
                .push(ObservedAccessibilityEvent::Action(action.clone()));
            cx.request_redraw();
            ActionOutcome::handled("accessibility action probe")
        }

        fn handle_text_input_event(
            &mut self,
            cx: &mut EventContext,
            event: &TextInputEvent,
        ) -> bool {
            if !cx.is_focused(Some(self.id)) {
                return false;
            }
            self.observed
                .borrow_mut()
                .push(ObservedAccessibilityEvent::Text(event.clone()));
            cx.request_redraw();
            true
        }

        fn accessibility(
            &self,
            cx: &AccessibilityContext,
        ) -> Result<Option<AccessibilityNode>, crate::core::accessibility::AccessibilityError>
        {
            Ok(Some(
                AccessibilityNode::label_required(self.id, AccessibilityRole::TextInput, "Probe")?
                    .with_focused(cx.a11y_has_focus(self.id))
                    .with_actions(self.actions.iter().copied()),
            ))
        }
    }

    fn accessibility_presenter() -> (
        Presenter<AccessibilityActionProbe>,
        ElementId,
        Rc<RefCell<Vec<ObservedAccessibilityEvent>>>,
    ) {
        let id = ElementId::from(42);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let probe = AccessibilityActionProbe {
            id,
            style: Style::default(),
            observed: Rc::clone(&observed),
            actions: vec![
                AccessibilityAction::Activate,
                AccessibilityAction::SetValue,
                AccessibilityAction::ScrollForward,
                AccessibilityAction::ScrollBackward,
            ],
        };
        (
            Presenter::with_root(Size::new(100.0, 80.0), probe),
            id,
            observed,
        )
    }

    fn accessibility_request(
        id: ElementId,
        action: AccessibilityAction,
        value: Option<&str>,
    ) -> MacAccessibilityActionRequest {
        MacAccessibilityActionRequest {
            id,
            request: MacAccessibilityRequest::Action {
                action,
                value: value.map(str::to_string),
            },
            bounds: Bounds::from_xywh(10.0, 20.0, 30.0, 40.0),
        }
    }

    fn accessibility_child_id<E: Element>(presenter: &Presenter<E>, index: usize) -> ElementId {
        let nodes = presenter
            .root()
            .accessibility_nodes(&AccessibilityContext::new(None))
            .expect("accessibility tree should build");
        nodes[0].a11y_children()[index].a11y_id()
    }

    #[test]
    fn only_key_down_events_are_forwarded_to_elements() {
        assert!(should_forward_key_event_to_tree(true));
        assert!(!should_forward_key_event_to_tree(false));
    }

    #[test]
    fn native_ime_owner_is_cancelled_before_events_can_reach_new_focus() {
        let first = ElementId::from(1);
        let second = ElementId::from(2);
        let mut state = NativeImeState::default();

        assert_eq!(
            state.target_for_event(
                &TextInputEvent::BeginComposition("draft".to_string()),
                Some(first),
            ),
            Some(first)
        );
        assert_eq!(state.cancel_owner_after_focus_change(Some(first)), None);
        assert_eq!(
            state.cancel_owner_after_focus_change(Some(second)),
            Some(first)
        );
        assert_eq!(
            state.target_for_event(
                &TextInputEvent::UpdateComposition("stale".to_string()),
                Some(second),
            ),
            None
        );
    }

    #[test]
    fn native_accessibility_activate_focuses_and_dispatches_to_the_element() {
        let (mut presenter, id, observed) = accessibility_presenter();
        let request = accessibility_request(id, AccessibilityAction::Activate, None);

        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, true)
        );
        assert_eq!(presenter.focused_element(), Some(id));
        assert_eq!(
            observed.borrow().as_slice(),
            [ObservedAccessibilityEvent::Action(ActionId::Standard(
                StandardAction::Activate,
            ))]
        );
    }

    #[test]
    fn native_accessibility_focus_requests_update_presenter_focus() {
        let (mut presenter, id, _) = accessibility_presenter();
        let mut request = MacAccessibilityActionRequest {
            id,
            request: MacAccessibilityRequest::Focus(true),
            bounds: Bounds::default(),
        };

        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, false)
        );
        assert_eq!(presenter.focused_element(), Some(id));

        request.request = MacAccessibilityRequest::Focus(false);
        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, false)
        );
        assert_eq!(presenter.focused_element(), None);
    }

    #[test]
    fn native_accessibility_focus_requests_run_input_focus_callbacks() {
        let id = ElementId::new();
        let focused = Rc::new(RefCell::new(0));
        let blurred = Rc::new(RefCell::new(0));
        let focus_count = Rc::clone(&focused);
        let blur_count = Rc::clone(&blurred);
        let input = Input::new()
            .id(id)
            .accessibility_label("Name")
            .on_focus(move || *focus_count.borrow_mut() += 1)
            .on_blur(move || *blur_count.borrow_mut() += 1);
        let mut presenter = Presenter::with_root(Size::new(100.0, 40.0), input);
        let mut request = MacAccessibilityActionRequest {
            id,
            request: MacAccessibilityRequest::Focus(true),
            bounds: Bounds::default(),
        };

        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, true)
        );
        request.request = MacAccessibilityRequest::Focus(false);
        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, true)
        );
        assert_eq!((*focused.borrow(), *blurred.borrow()), (1, 1));
    }

    #[test]
    fn native_accessibility_revalidates_target_and_capability_before_dispatch() {
        let (mut presenter, id, observed) = accessibility_presenter();
        presenter.root_mut().actions = vec![AccessibilityAction::Activate];

        assert_eq!(
            dispatch_accessibility_action(
                &mut presenter,
                &accessibility_request(id, AccessibilityAction::SetValue, Some("stale")),
            ),
            (false, false)
        );
        assert_eq!(
            dispatch_accessibility_action(
                &mut presenter,
                &accessibility_request(ElementId::new(), AccessibilityAction::Activate, None,),
            ),
            (false, false)
        );
        assert!(observed.borrow().is_empty());
        assert_eq!(presenter.focused_element(), None);
    }

    #[test]
    fn native_accessibility_set_value_selects_existing_text_before_inserting() {
        let (mut presenter, id, observed) = accessibility_presenter();
        let request = accessibility_request(id, AccessibilityAction::SetValue, Some("renamed"));

        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, true)
        );
        assert_eq!(
            observed.borrow().as_slice(),
            [
                ObservedAccessibilityEvent::Action(ActionId::Standard(StandardAction::SelectAll,)),
                ObservedAccessibilityEvent::Text(
                    TextInputEvent::InsertText("renamed".to_string(),)
                ),
            ]
        );
    }

    #[test]
    fn native_accessibility_set_value_cancels_composition_before_replacing_text() {
        let id = ElementId::new();
        let mut input = Input::new()
            .id(id)
            .accessibility_label("Name")
            .value("base");
        input
            .apply_text_input_event(TextInputEvent::BeginComposition("draft".to_string()))
            .expect("composition should begin");
        let mut presenter = Presenter::with_root(Size::new(100.0, 40.0), input);

        assert_eq!(
            dispatch_accessibility_action(
                &mut presenter,
                &accessibility_request(id, AccessibilityAction::SetValue, Some("renamed")),
            ),
            (true, true)
        );
        assert_eq!(presenter.root().get_value(), "renamed");
    }

    #[test]
    fn native_accessibility_scroll_dispatches_a_targeted_action() {
        let (mut presenter, id, observed) = accessibility_presenter();
        let request = accessibility_request(id, AccessibilityAction::ScrollForward, None);

        assert_eq!(
            dispatch_accessibility_action(&mut presenter, &request),
            (true, true)
        );
        assert_eq!(
            observed.borrow().as_slice(),
            [ObservedAccessibilityEvent::Action(ActionId::custom(
                crate::core::action::ACCESSIBILITY_SCROLL_FORWARD_ACTION,
            ))]
        );
    }

    #[test]
    fn native_accessibility_activate_targets_composite_children_and_rows() {
        let size = Size::new(200.0, 100.0);

        let mut list = Presenter::with_root(
            size,
            DataList::new([
                DataListItem::new("one", "One"),
                DataListItem::new("two", "Two"),
            ])
            .accessibility_label("Items"),
        );
        let target = accessibility_child_id(&list, 1);
        assert!(
            dispatch_accessibility_action(
                &mut list,
                &accessibility_request(target, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert_eq!(list.root().selected_value(), Some("two"));

        let mut tree = Presenter::with_root(
            size,
            DataTree::new([
                DataTreeItem::new("one", "One"),
                DataTreeItem::new("two", "Two"),
            ])
            .accessibility_label("Items"),
        );
        let target = accessibility_child_id(&tree, 1);
        assert!(
            dispatch_accessibility_action(
                &mut tree,
                &accessibility_request(target, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert_eq!(tree.root().selected_value(), Some("two"));

        let mut menu = Presenter::with_root(size, Menu::new("File", [("new", "New")]));
        let target = accessibility_child_id(&menu, 0);
        assert!(
            dispatch_accessibility_action(
                &mut menu,
                &accessibility_request(target, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert_eq!(menu.root().selected_value(), Some("new"));

        let mut tabs = Presenter::with_root(
            size,
            TabList::new([Tab::new("one", "One"), Tab::new("two", "Two")], "one")
                .accessibility_label("Sections"),
        );
        let target = accessibility_child_id(&tabs, 1);
        assert!(
            dispatch_accessibility_action(
                &mut tabs,
                &accessibility_request(target, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert_eq!(tabs.root().selected_value(), "two");

        let mut segments = Presenter::with_root(
            size,
            SegmentedControl::new([("one", "One"), ("two", "Two")], "one")
                .accessibility_label("View"),
        );
        let target = accessibility_child_id(&segments, 1);
        assert!(
            dispatch_accessibility_action(
                &mut segments,
                &accessibility_request(target, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert_eq!(segments.root().selected_value(), "two");

        let row_id = ElementId::from(99);
        let mut row = Presenter::with_root(
            size,
            DataTableRow::new([DataTableCell::new("Value")]).id(row_id),
        );
        assert!(
            dispatch_accessibility_action(
                &mut row,
                &accessibility_request(row_id, AccessibilityAction::Activate, None),
            )
            .0
        );
        assert!(row.root().interaction_state().selected());
    }
}
