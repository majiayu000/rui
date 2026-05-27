#[allow(dead_code)]
#[path = "../examples/advanced_ui_controls.rs"]
mod advanced_ui_controls;

use advanced_ui_controls::{
    DOGFOOD_CLAIM_GATE_ID, DOGFOOD_PANEL_CONTROL_ID, DOGFOOD_REFRESH_BUTTON_ID,
    DogfoodControlsView, DogfoodPanel, LocalDogfoodData, LocalVerificationCheck,
    controls_panel_from_data, load_local_dogfood_data,
};
use rui::core::accessibility::AccessibilityNode;
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
