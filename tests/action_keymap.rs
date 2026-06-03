use rui::advanced_ui::{TabList, button, checkbox, hoverable};
use rui::core::action::{
    ActionError, ActionHandler, ActionId, ActionOutcome, ActionRouter, KeyChord, Keymap,
    StandardAction,
};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::{AppContext, ElementId, Size};
use rui::elements::element::EventContext;
use rui::elements::{Element, input, list, scroll_view};
use rui::testing::{HeadlessSession, mount};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::TaffyTree;

fn action_result<T>(result: Result<T, ActionError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("action operation failed: {err}"),
    }
}

fn mount_or_panic<F, E>(build_root: F) -> HeadlessSession<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    match mount(Size::new(180.0, 80.0), build_root) {
        Ok(session) => session,
        Err(err) => panic!("headless action session should mount: {err}"),
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

#[test]
fn action_keymap_runtime_routes_focused_component_before_app_fallback() {
    let button_id = ElementId::from(9201);
    let clicked = Rc::new(Cell::new(0));
    let clicked_ref = Rc::clone(&clicked);
    let mut session = mount_or_panic(move |_cx| {
        let clicked_ref = Rc::clone(&clicked_ref);
        button("Run").id(button_id).on_click(move || {
            clicked_ref.set(clicked_ref.get() + 1);
        })
    });
    let app_calls = Rc::new(RefCell::new(Vec::new()));
    session
        .app_context_mut()
        .add_action_handler(RecordingHandler::new(
            "app",
            vec![ActionId::from(StandardAction::Activate)],
            Rc::clone(&app_calls),
        ));

    session.request_focus(Some(button_id));
    assert_eq!(session.focused_element(), Some(button_id));
    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::Enter, Modifiers::none())));

    assert_eq!(clicked.get(), 1);
    assert!(app_calls.borrow().is_empty());
}

#[test]
fn action_keymap_runtime_ignored_activation_falls_through_to_app() {
    let action = ActionId::from(StandardAction::Activate);
    let app_calls = Rc::new(RefCell::new(Vec::new()));
    let disabled_button_id = ElementId::from(9202);
    let clicked = Rc::new(Cell::new(false));
    let clicked_ref = Rc::clone(&clicked);
    let mut disabled = mount_or_panic(move |_cx| {
        let clicked_ref = Rc::clone(&clicked_ref);
        button("Disabled")
            .id(disabled_button_id)
            .disabled(true)
            .on_click(move || {
                clicked_ref.set(true);
            })
    });
    disabled
        .app_context_mut()
        .add_action_handler(RecordingHandler::new(
            "app",
            vec![action.clone()],
            Rc::clone(&app_calls),
        ));

    disabled.request_focus(Some(disabled_button_id));
    assert!(disabled.dispatch_key_event(&KeyEvent::new(KeyCode::Enter, Modifiers::none())));
    assert!(!clicked.get());

    let read_only_checkbox_id = ElementId::from(9203);
    let changed = Rc::new(Cell::new(false));
    let changed_ref = Rc::clone(&changed);
    let mut read_only = mount_or_panic(move |_cx| {
        let changed_ref = Rc::clone(&changed_ref);
        checkbox("Read only")
            .id(read_only_checkbox_id)
            .read_only(true)
            .on_change(move |_| changed_ref.set(true))
    });
    read_only
        .app_context_mut()
        .add_action_handler(RecordingHandler::new(
            "app",
            vec![action],
            Rc::clone(&app_calls),
        ));

    read_only.request_focus(Some(read_only_checkbox_id));
    assert!(read_only.dispatch_key_event(&KeyEvent::new(KeyCode::Enter, Modifiers::none())));
    assert!(!changed.get());
    assert_eq!(app_calls.borrow().as_slice(), ["app", "app"]);
}

#[test]
fn action_keymap_runtime_preserves_raw_text_editing_details() {
    let input_id = ElementId::from(9204);
    let latest = Rc::new(RefCell::new(String::new()));
    let latest_ref = Rc::clone(&latest);
    let mut session = mount_or_panic(move |_cx| {
        let latest_ref = Rc::clone(&latest_ref);
        input()
            .id(input_id)
            .on_change(move |value| *latest_ref.borrow_mut() = value.to_string())
    });

    session.request_focus(Some(input_id));
    let event = KeyEvent::new(KeyCode::Unknown(0), Modifiers::none()).with_char('r');
    assert!(session.dispatch_key_event(&event));

    assert_eq!(latest.borrow().as_str(), "r");
}

#[test]
fn action_keymap_runtime_forwards_unbound_modified_arrows_to_inputs() {
    let input_id = ElementId::new();
    let mut session = mount_or_panic(move |_cx| {
        input()
            .id(input_id)
            .accessibility_label("Search")
            .value("hello world")
    });
    session.request_focus(Some(input_id));

    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::ctrl())));

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("input accessibility tree should build: {err}"),
    };
    let node = match tree.find(input_id) {
        Some(node) => node,
        None => panic!("input node should be present"),
    };
    assert!(
        node.a11y_text_caret().unwrap_or(usize::MAX) < "hello world".len(),
        "ctrl-left should move the caret through the raw input fallback"
    );
}

#[test]
fn action_keymap_select_all_reaches_inputs_inside_wrappers() {
    fn assert_select_all_reaches<E, B>(build_root: B)
    where
        E: Element,
        B: Fn(ElementId, Rc<RefCell<String>>) -> E,
    {
        let input_id = ElementId::new();
        let latest = Rc::new(RefCell::new(String::from("abc")));
        let latest_ref = Rc::clone(&latest);
        let mut session = mount_or_panic(move |_cx| build_root(input_id, Rc::clone(&latest_ref)));
        session.request_focus(Some(input_id));

        assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::A, Modifiers::meta())));
        assert!(session.dispatch_key_event(
            &KeyEvent::new(KeyCode::Unknown(0), Modifiers::none()).with_char('x')
        ));
        assert_eq!(latest.borrow().as_str(), "x");
    }

    assert_select_all_reaches(|input_id, latest| {
        scroll_view().child(
            input()
                .id(input_id)
                .accessibility_label("Search")
                .value("abc")
                .on_change(move |value| *latest.borrow_mut() = value.to_string()),
        )
    });
    assert_select_all_reaches(|input_id, latest| {
        list().item(
            input()
                .id(input_id)
                .accessibility_label("Search")
                .value("abc")
                .on_change(move |value| *latest.borrow_mut() = value.to_string()),
        )
    });
    assert_select_all_reaches(|input_id, latest| {
        hoverable(
            input()
                .id(input_id)
                .accessibility_label("Search")
                .value("abc")
                .on_change(move |value| *latest.borrow_mut() = value.to_string()),
        )
    });
}
