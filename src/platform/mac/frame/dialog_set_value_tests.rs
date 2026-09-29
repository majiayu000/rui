use super::dispatch_accessibility_action;
use super::tests::accessibility_request;
use crate::advanced_ui::{Dialog, TextField};
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityAction;
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;

#[test]
fn native_accessibility_set_value_replaces_dialog_text() {
    let field_id = ElementId::new();
    let dialog = Dialog::new("Edit", TextField::new("Name").id(field_id).value("keep"));
    let mut presenter = Presenter::with_root(Size::new(320.0, 240.0), dialog);
    let tree = presenter
        .accessibility_tree()
        .expect("dialog accessibility tree should build");
    let field = tree.find(field_id).expect("text field node");
    assert!(
        field
            .a11y_actions()
            .contains(&AccessibilityAction::SetValue),
        "text field inside an enabled dialog should advertise SetValue"
    );

    assert!(
        dispatch_accessibility_action(
            &mut presenter,
            &accessibility_request(field_id, AccessibilityAction::SetValue, Some("next")),
            None,
        )
        .0,
        "SetValue should replace the dialog field text"
    );
    let tree = presenter
        .accessibility_tree()
        .expect("dialog accessibility tree should rebuild");
    assert_eq!(
        tree.find(field_id).expect("text field node").a11y_value(),
        Some("next")
    );
}
