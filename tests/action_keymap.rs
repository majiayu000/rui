use rui::ElementId;
use rui::advanced_ui::TabList;
use rui::core::action::{
    ActionError, ActionHandler, ActionId, ActionOutcome, ActionRouter, KeyChord, Keymap,
    StandardAction,
};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::elements::Element;
use rui::elements::element::EventContext;
use std::cell::RefCell;
use std::rc::Rc;
use taffy::TaffyTree;

fn action_result<T>(result: Result<T, ActionError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("action operation failed: {err}"),
    }
}

#[derive(Debug)]
struct RecordingHandler {
    name: String,
    enabled: bool,
    handled: Vec<ActionId>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl RecordingHandler {
    fn new(name: &str, handled: Vec<ActionId>, calls: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            name: name.to_string(),
            enabled: true,
            handled,
            calls,
        }
    }

    fn action_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl ActionHandler for RecordingHandler {
    fn action_handler_name(&self) -> &str {
        &self.name
    }

    fn action_handler_enabled(&self) -> bool {
        self.enabled
    }

    fn run_action(&mut self, action: &ActionId) -> ActionOutcome {
        self.calls.borrow_mut().push(self.name.clone());
        if self.handled.iter().any(|candidate| candidate == action) {
            ActionOutcome::handled(self.name.clone())
        } else {
            ActionOutcome::Ignored
        }
    }
}

#[test]
fn keymap_detects_conflicting_action_bindings() {
    let mut keymap = Keymap::new();
    action_result(keymap.bind(KeyCode::A, Modifiers::meta(), StandardAction::SelectAll));

    let error = match keymap.bind(
        KeyCode::A,
        Modifiers::meta(),
        ActionId::custom("app.select_everything"),
    ) {
        Ok(_) => panic!("conflicting key chord should fail"),
        Err(err) => err,
    };

    assert!(matches!(
        error,
        ActionError::KeyChordConflict {
            key: KeyCode::A,
            ..
        }
    ));
}

#[test]
fn keymap_rejects_empty_custom_action_names() {
    let mut keymap = Keymap::new();
    let error = match keymap.bind(KeyCode::P, Modifiers::meta(), ActionId::custom("  ")) {
        Ok(_) => panic!("empty custom action name should fail"),
        Err(err) => err,
    };

    assert_eq!(error, ActionError::EmptyActionName);
}

#[test]
fn keymap_maps_standard_actions_from_key_events() {
    let keymap = action_result(Keymap::with_standard_bindings());
    let cases = [
        (
            KeyCode::ArrowLeft,
            Modifiers::none(),
            StandardAction::MoveLeft,
        ),
        (
            KeyCode::ArrowRight,
            Modifiers::none(),
            StandardAction::MoveRight,
        ),
        (KeyCode::ArrowUp, Modifiers::none(), StandardAction::MoveUp),
        (
            KeyCode::ArrowDown,
            Modifiers::none(),
            StandardAction::MoveDown,
        ),
        (
            KeyCode::ArrowLeft,
            Modifiers::shift(),
            StandardAction::SelectLeft,
        ),
        (
            KeyCode::ArrowRight,
            Modifiers::shift(),
            StandardAction::SelectRight,
        ),
        (
            KeyCode::ArrowLeft,
            Modifiers::alt(),
            StandardAction::MoveWordLeft,
        ),
        (
            KeyCode::ArrowRight,
            Modifiers::alt(),
            StandardAction::MoveWordRight,
        ),
        (KeyCode::Tab, Modifiers::none(), StandardAction::FocusNext),
        (
            KeyCode::Tab,
            Modifiers::shift(),
            StandardAction::FocusPrevious,
        ),
        (
            KeyCode::Backspace,
            Modifiers::none(),
            StandardAction::DeleteBackward,
        ),
        (
            KeyCode::Delete,
            Modifiers::none(),
            StandardAction::DeleteForward,
        ),
        (KeyCode::Enter, Modifiers::none(), StandardAction::Activate),
        (
            KeyCode::Enter,
            Modifiers::shift(),
            StandardAction::InsertNewline,
        ),
        (KeyCode::Enter, Modifiers::meta(), StandardAction::Submit),
        (KeyCode::Escape, Modifiers::none(), StandardAction::Cancel),
        (KeyCode::A, Modifiers::meta(), StandardAction::SelectAll),
        (
            KeyCode::P,
            Modifiers::meta(),
            StandardAction::CommandPalette,
        ),
    ];

    assert_eq!(keymap.len(), cases.len());
    for (key, modifiers, action) in cases {
        let event = KeyEvent::new(key, modifiers);
        assert_eq!(
            keymap.action_for_event(&event),
            Some(&ActionId::from(action))
        );
    }

    let unbound = KeyChord::new(KeyCode::F12, Modifiers::none());
    assert!(keymap.action_for_chord(unbound).is_none());
}

#[test]
fn action_keymap_tab_list_preserves_global_tab_focus_binding() {
    let keymap = action_result(Keymap::with_standard_bindings());
    let tab_event = KeyEvent::new(KeyCode::Tab, Modifiers::none());
    assert_eq!(
        keymap.action_for_event(&tab_event),
        Some(&ActionId::from(StandardAction::FocusNext))
    );

    let id = ElementId::new();
    let mut list = TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview")
        .id(id)
        .accessibility_label("Project sections");
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    assert!(!list.handle_key_event(&mut cx, &tab_event));
    assert_eq!(list.selected_value(), "overview");
}

#[test]
fn action_router_sends_focused_handler_before_component_and_app() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let action = ActionId::from(StandardAction::Activate);
    let mut focused = RecordingHandler::new("focused", vec![action.clone()], calls.clone());
    let mut component = RecordingHandler::new("component", vec![action.clone()], calls.clone());
    let mut app = RecordingHandler::new("app", vec![action.clone()], calls.clone());

    let outcome = {
        let mut router = ActionRouter::new()
            .focused(&mut focused)
            .component(&mut component)
            .app(&mut app);
        router.route_action(&action)
    };

    assert_eq!(outcome, ActionOutcome::handled("focused"));
    assert_eq!(calls.borrow().as_slice(), ["focused"]);
}

#[test]
fn action_router_falls_through_ignored_handlers_in_order() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let action = ActionId::from(StandardAction::Cancel);
    let mut focused = RecordingHandler::new("focused", Vec::new(), calls.clone());
    let mut component = RecordingHandler::new("component", vec![action.clone()], calls.clone());
    let mut app = RecordingHandler::new("app", vec![action.clone()], calls.clone());

    let outcome = {
        let mut router = ActionRouter::new()
            .focused(&mut focused)
            .component(&mut component)
            .app(&mut app);
        router.route_action(&action)
    };

    assert_eq!(outcome, ActionOutcome::handled("component"));
    assert_eq!(calls.borrow().as_slice(), ["focused", "component"]);
}

#[test]
fn action_router_skips_disabled_activation_handlers() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let action = ActionId::from(StandardAction::Activate);
    let mut focused =
        RecordingHandler::new("focused", vec![action.clone()], calls.clone()).action_disabled();
    let mut app = RecordingHandler::new("app", vec![action.clone()], calls.clone());

    let outcome = {
        let mut router = ActionRouter::new().focused(&mut focused).app(&mut app);
        router.route_action(&action)
    };

    assert_eq!(outcome, ActionOutcome::handled("app"));
    assert_eq!(calls.borrow().as_slice(), ["app"]);
}

#[test]
fn action_router_reports_ignored_when_no_handler_accepts_action() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let action = ActionId::custom("app.refresh");
    let mut focused = RecordingHandler::new("focused", Vec::new(), calls.clone());
    let mut app = RecordingHandler::new("app", Vec::new(), calls.clone());

    let outcome = {
        let mut router = ActionRouter::new().focused(&mut focused).app(&mut app);
        router.route_action(&action)
    };

    assert_eq!(outcome, ActionOutcome::Ignored);
    assert_eq!(calls.borrow().as_slice(), ["focused", "app"]);
}
