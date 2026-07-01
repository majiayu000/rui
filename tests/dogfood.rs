#[allow(dead_code)]
#[path = "../examples/advanced_ui_controls.rs"]
mod advanced_ui_controls;

#[allow(dead_code)]
#[path = "../examples/native_dogfood.rs"]
mod native_dogfood_example;

use advanced_ui_controls::{
    DOGFOOD_CLAIM_GATE_ID, DOGFOOD_PANEL_CONTROL_ID, DOGFOOD_REFRESH_BUTTON_ID,
    DogfoodControlsView, DogfoodPanel, LocalDogfoodData, LocalVerificationCheck,
    controls_panel_from_data, load_local_dogfood_data,
};
use rui::core::accessibility::AccessibilityNode;
use rui::core::action::{
    ActionHandler, ActionId, ActionOutcome, ActionRouter, Keymap, StandardAction,
};
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::text_editing::{TextEditBuffer, TextInputEvent};
use rui::core::{ElementId, Point, Size};
use rui::testing::{mount, mount_view};

#[test]
fn dogfood_loads_repository_owned_data() {
    let data = match load_local_dogfood_data() {
        Ok(data) => data,
        Err(err) => panic!("dogfood data should load from this checkout: {err}"),
    };

    assert_eq!(data.package_name, "rui");
    assert!(!data.package_version.trim().is_empty());
    assert!(data.advanced_ui_files > 0);
    assert!(data.integration_tests > 0);
    assert!(data.examples > 0);
    assert!(!data.git_changes.is_empty());
    assert!(data.verification_checks.iter().any(|check| check.passed));
}

#[test]
fn dogfood_controls_mount_real_data_headlessly() {
    let data = match load_local_dogfood_data() {
        Ok(data) => data,
        Err(err) => panic!("dogfood data should load from this checkout: {err}"),
    };
    let session = match mount(Size::new(760.0, 520.0), |_cx| {
        controls_panel_from_data(&data, DogfoodPanel::Changes, false, 0)
    }) {
        Ok(session) => session,
        Err(err) => panic!("dogfood controls should mount headlessly: {err}"),
    };

    assert!(!session.primitives().is_empty());

    let snapshot = match session.primitive_snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("dogfood primitives should snapshot: {err}"),
    };
    assert!(snapshot.as_str().contains("rui local dogfood"));
    assert!(
        data.git_changes
            .iter()
            .any(|change| snapshot.as_str().contains(change))
    );
}

#[test]
fn dogfood_view_dispatches_controls_and_rebuilds() {
    let data = fixture_data();
    let view = DogfoodControlsView::with_data(data, false);
    let mut session = match mount_view(Size::new(760.0, 520.0), view) {
        Ok(session) => session,
        Err(err) => panic!("dogfood view should mount headlessly: {err}"),
    };

    click_element(&mut session, DOGFOOD_CLAIM_GATE_ID);
    if let Err(err) = session.frame() {
        panic!("dogfood claim gate should rebuild: {err}");
    }

    let tree = match session.accessibility_tree() {
        Ok(tree) => tree,
        Err(err) => panic!("dogfood accessibility tree should build: {err}"),
    };
    let claim_gate = match tree.find(DOGFOOD_CLAIM_GATE_ID) {
        Some(node) => node,
        None => panic!("dogfood claim gate should be accessible"),
    };
    assert_eq!(claim_gate.a11y_checked(), Some(true));

    click_rightmost_element(&mut session, DOGFOOD_PANEL_CONTROL_ID);
    if let Err(err) = session.frame() {
        panic!("dogfood panel change should rebuild: {err}");
    }
    let labels = accessibility_labels(match session.accessibility_tree() {
        Ok(tree) => tree.roots().to_vec(),
        Err(err) => panic!("dogfood accessibility tree should rebuild: {err}"),
    });
    assert!(labels.iter().any(|label| label == "Verification"));

    click_element(&mut session, DOGFOOD_REFRESH_BUTTON_ID);
    if let Err(err) = session.frame() {
        panic!("dogfood refresh should rebuild: {err}");
    }
    let snapshot = match session.primitive_snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("dogfood snapshot should serialize after events: {err}"),
    };
    assert!(snapshot.as_str().contains("Refresh local scan (1)"));
}

#[test]
fn dogfood_workflow_routes_actions_and_edits_repository_filter() {
    let data = match load_local_dogfood_data() {
        Ok(data) => data,
        Err(err) => panic!("dogfood data should load from this checkout: {err}"),
    };
    let mut workflow = DogfoodWorkflow::new(data.clone());

    workflow.apply_text_input(TextInputEvent::InsertText(data.package_name.clone()));
    assert_eq!(workflow.filter_query(), data.package_name);
    assert!(
        workflow
            .visible_lines()
            .iter()
            .any(|line| line.contains(&data.package_name))
    );

    let mut keymap = match Keymap::with_standard_bindings() {
        Ok(keymap) => keymap,
        Err(err) => panic!("dogfood keymap should build: {err}"),
    };
    if let Err(err) = keymap.bind(
        KeyCode::Key1,
        Modifiers::meta(),
        ActionId::custom(DOGFOOD_PANEL_CHANGES_ACTION),
    ) {
        panic!("dogfood panel action should bind: {err}");
    }
    if let Err(err) = keymap.bind(
        KeyCode::R,
        Modifiers::meta(),
        ActionId::custom(DOGFOOD_REFRESH_ACTION),
    ) {
        panic!("dogfood refresh action should bind: {err}");
    }

    let changes_action = mapped_action(&keymap, KeyCode::Key1, Modifiers::meta());
    assert_eq!(
        route_dogfood_action(&mut workflow, &changes_action),
        ActionOutcome::handled(DOGFOOD_WORKFLOW_HANDLER)
    );
    assert_eq!(workflow.selected_panel(), DogfoodPanel::Changes);

    let refresh_action = mapped_action(&keymap, KeyCode::R, Modifiers::meta());
    assert_eq!(
        route_dogfood_action(&mut workflow, &refresh_action),
        ActionOutcome::handled(DOGFOOD_WORKFLOW_HANDLER)
    );
    assert_eq!(workflow.refresh_count(), 1);

    let submit_action = mapped_action(&keymap, KeyCode::Enter, Modifiers::meta());
    assert_eq!(
        route_dogfood_action(&mut workflow, &submit_action),
        ActionOutcome::handled(DOGFOOD_WORKFLOW_HANDLER)
    );
    assert_eq!(workflow.selected_panel(), DogfoodPanel::Verification);

    let cancel_action = mapped_action(&keymap, KeyCode::Escape, Modifiers::none());
    assert_eq!(
        route_dogfood_action(&mut workflow, &cancel_action),
        ActionOutcome::handled(DOGFOOD_WORKFLOW_HANDLER)
    );
    assert_eq!(workflow.filter_query(), "");
    assert_eq!(workflow.selected_panel(), DogfoodPanel::Overview);
}

#[test]
fn native_dogfood_script_contract_launches_example_and_profile() {
    let script = include_str!("../scripts/native_dogfood_macos.sh");
    let example = include_str!("../examples/native_dogfood.rs");
    let mac_runner = include_str!("../src/platform/mac/app.rs");
    let mac_window = include_str!("../src/platform/mac/window.rs");

    for required in [
        "cargo build --example native_dogfood",
        "cargo run --example native_dogfood",
        "RUI_NATIVE_DOGFOOD_PROFILE",
        "RUI_NATIVE_DOGFOOD_INTERACTIVE=1",
        "RUI_NATIVE_DOGFOOD_AUTOMATION=1",
        "\"status\":\"passed\"",
        "\"script_requires_minimize_reopen\":true",
    ] {
        assert!(
            script.contains(required),
            "native dogfood script should contain `{required}`"
        );
    }

    for required in [
        "RUI_NATIVE_DOGFOOD_PROFILE",
        "RUI_NATIVE_DOGFOOD_TEXT",
        "finite_default_exit",
        "scripts/native_dogfood_macos.sh",
        "script_requires_minimize_reopen",
        "cx.app_mut().quit()",
    ] {
        assert!(
            example.contains(required),
            "native dogfood example should contain `{required}`"
        );
    }
    assert!(
        !script.contains("RUI_PROFILE"),
        "native dogfood script should not reuse renderer telemetry RUI_PROFILE"
    );
    assert!(
        !example.contains("RUI_PROFILE"),
        "native dogfood example should not reuse renderer telemetry RUI_PROFILE"
    );

    for required in [
        "NativeDogfoodAutomationPhase::Interact",
        "NativeDogfoodAutomationPhase::Minimize",
        "NativeDogfoodAutomationPhase::Reopen",
        "append_text_keys",
        "append_pointer_click",
    ] {
        assert!(
            mac_runner.contains(required),
            "macOS runner should contain `{required}`"
        );
    }

    for required in ["set_minimized(true)", "set_minimized(false)"] {
        assert!(
            mac_runner.contains(required),
            "macOS runner should call `{required}`"
        );
    }
    assert!(
        mac_window.contains("fn set_minimized") && mac_window.contains("deminiaturize"),
        "macOS window should support native minimize and reopen"
    );
}

#[test]
fn native_dogfood_profile_body_is_valid_json_contract() {
    let profile = native_dogfood_example::native_dogfood_profile_body(
        "passed",
        "typed\nvalue",
        "typed\nvalue",
        true,
        true,
    );
    let parsed: serde_json::Value = match serde_json::from_str(&profile) {
        Ok(parsed) => parsed,
        Err(err) => panic!("native dogfood profile should be valid JSON: {err}\n{profile}"),
    };

    assert_eq!(parsed["schema"], "rui.native_dogfood.v1");
    assert_eq!(parsed["status"], "passed");
    assert_eq!(parsed["typed_text"], "typed\nvalue");
    assert_eq!(parsed["expected_text"], "typed\nvalue");
    assert_eq!(parsed["text_matched"], true);
    assert_eq!(parsed["submitted"], true);
    assert_eq!(parsed["interactive"], true);
    assert_eq!(parsed["script_requires_minimize_reopen"], true);
    assert_eq!(parsed["driver"], "scripts/native_dogfood_macos.sh");
}

#[test]
fn runtime_roadmap_separates_product_pressure_readiness_gates() {
    let roadmap = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/advanced-ui-runtime-roadmap.md"),
    )
    .expect("runtime roadmap should be readable");

    for required in [
        "## Product Pressure Readiness Gates",
        "Finite example smoke",
        "Repository dogfood",
        "Native macOS dogfood",
        "Benchmark policy",
        "Benchmark report",
        "Docs/API drift",
        "`cargo test example_smoke`",
        "`cargo test dogfood`",
        "`cargo test benchmark_config`",
        "`cargo test --test docs_api_drift`",
        "`scripts/native_dogfood_macos.sh`",
        "`cargo bench --bench runtime_baselines`",
        "performance enforcement remains advisory while `enforcement_enabled` is false",
    ] {
        assert!(
            roadmap.contains(required),
            "runtime roadmap should document product-pressure gate `{required}`"
        );
    }
}

fn fixture_data() -> LocalDogfoodData {
    LocalDogfoodData {
        package_name: String::from("rui"),
        package_version: String::from("0.2.0"),
        advanced_ui_files: 7,
        integration_tests: 12,
        examples: 8,
        git_changes: vec![String::from("M examples/advanced_ui_controls.rs")],
        verification_checks: vec![
            LocalVerificationCheck {
                label: String::from("advanced UI module is wired"),
                passed: true,
            },
            LocalVerificationCheck {
                label: String::from("headless testing module is wired"),
                passed: true,
            },
        ],
    }
}

const DOGFOOD_WORKFLOW_HANDLER: &str = "dogfood-workflow";
const DOGFOOD_PANEL_CHANGES_ACTION: &str = "dogfood.panel.changes";
const DOGFOOD_REFRESH_ACTION: &str = "dogfood.refresh";

struct DogfoodWorkflow {
    data: LocalDogfoodData,
    selected_panel: DogfoodPanel,
    filter: TextEditBuffer,
    refresh_count: u32,
}

impl DogfoodWorkflow {
    fn new(data: LocalDogfoodData) -> Self {
        Self {
            data,
            selected_panel: DogfoodPanel::Overview,
            filter: TextEditBuffer::new(),
            refresh_count: 0,
        }
    }

    fn apply_text_input(&mut self, event: TextInputEvent) {
        match self.filter.apply_text_input_event(event) {
            Ok(outcome) => assert!(outcome.changed),
            Err(err) => panic!("dogfood filter text edit should apply: {err}"),
        }
    }

    fn filter_query(&self) -> &str {
        self.filter.text()
    }

    fn selected_panel(&self) -> DogfoodPanel {
        self.selected_panel
    }

    fn refresh_count(&self) -> u32 {
        self.refresh_count
    }

    fn visible_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("package {}", self.data.package_name),
            format!("version {}", self.data.package_version),
            format!("advanced UI files {}", self.data.advanced_ui_files),
        ];
        lines.extend(
            self.data
                .git_changes
                .iter()
                .map(|change| format!("change {change}")),
        );
        lines.extend(
            self.data
                .verification_checks
                .iter()
                .map(|check| format!("check {} {}", check.label, check.passed)),
        );

        let query = self.filter.text().trim();
        if query.is_empty() {
            return lines;
        }
        lines
            .into_iter()
            .filter(|line| line.contains(query))
            .collect()
    }
}

impl ActionHandler for DogfoodWorkflow {
    fn action_handler_name(&self) -> &str {
        DOGFOOD_WORKFLOW_HANDLER
    }

    fn run_action(&mut self, action: &ActionId) -> ActionOutcome {
        match action {
            ActionId::Custom(name) if name == DOGFOOD_PANEL_CHANGES_ACTION => {
                self.selected_panel = DogfoodPanel::Changes;
                ActionOutcome::handled(self.action_handler_name())
            }
            ActionId::Custom(name) if name == DOGFOOD_REFRESH_ACTION => {
                self.refresh_count = self.refresh_count.saturating_add(1);
                ActionOutcome::handled(self.action_handler_name())
            }
            ActionId::Standard(StandardAction::Submit) => {
                self.selected_panel = DogfoodPanel::Verification;
                ActionOutcome::handled(self.action_handler_name())
            }
            ActionId::Standard(StandardAction::Cancel) => {
                self.filter = TextEditBuffer::new();
                self.selected_panel = DogfoodPanel::Overview;
                ActionOutcome::handled(self.action_handler_name())
            }
            _ => ActionOutcome::Ignored,
        }
    }
}

fn mapped_action(keymap: &Keymap, key: KeyCode, modifiers: Modifiers) -> ActionId {
    let event = KeyEvent::new(key, modifiers);
    match keymap.action_for_event(&event) {
        Some(action) => action.clone(),
        None => panic!("dogfood keymap should bind {key:?}"),
    }
}

fn route_dogfood_action(workflow: &mut DogfoodWorkflow, action: &ActionId) -> ActionOutcome {
    ActionRouter::new().focused(workflow).route_action(action)
}

fn click_element<F, E>(session: &mut rui::testing::HeadlessSession<F, E>, id: ElementId)
where
    F: FnMut(&mut rui::core::AppContext) -> E,
    E: rui::elements::Element,
{
    let point = find_hit_point(session, id);
    assert!(session.pointer_down(point));
    assert!(session.pointer_up(point));
}

fn click_rightmost_element<F, E>(session: &mut rui::testing::HeadlessSession<F, E>, id: ElementId)
where
    F: FnMut(&mut rui::core::AppContext) -> E,
    E: rui::elements::Element,
{
    let point = find_rightmost_hit_point(session, id);
    assert!(session.pointer_down(point));
    assert!(session.pointer_up(point));
}

fn find_hit_point<F, E>(session: &rui::testing::HeadlessSession<F, E>, id: ElementId) -> Point
where
    F: FnMut(&mut rui::core::AppContext) -> E,
    E: rui::elements::Element,
{
    let mut y = 4.0;
    while y < 520.0 {
        let mut x = 4.0;
        while x < 760.0 {
            let point = Point::new(x, y);
            if session.scene().hit_test(point) == Some(id) {
                return point;
            }
            x += 8.0;
        }
        y += 8.0;
    }
    panic!("element {id:?} should have a hit-testable point");
}

fn find_rightmost_hit_point<F, E>(
    session: &rui::testing::HeadlessSession<F, E>,
    id: ElementId,
) -> Point
where
    F: FnMut(&mut rui::core::AppContext) -> E,
    E: rui::elements::Element,
{
    let mut best = None;
    let mut y = 4.0;
    while y < 520.0 {
        let mut x = 4.0;
        while x < 760.0 {
            let point = Point::new(x, y);
            if session.scene().hit_test(point) == Some(id) {
                best = Some(point);
            }
            x += 8.0;
        }
        y += 8.0;
    }
    match best {
        Some(point) => point,
        None => panic!("element {id:?} should have a rightmost hit-testable point"),
    }
}

fn accessibility_labels(nodes: Vec<AccessibilityNode>) -> Vec<String> {
    let mut labels = Vec::new();
    let mut pending = nodes;
    while let Some(node) = pending.pop() {
        if let Some(label) = node.a11y_label() {
            labels.push(String::from(label));
        }
        pending.extend_from_slice(node.a11y_children());
    }
    labels
}
