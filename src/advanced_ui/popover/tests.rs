use super::*;
use crate::advanced_ui::text_field::TextField;
use crate::advanced_ui::tokens::ThemeDensity;
use crate::advanced_ui::{button, container, text};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::event::{Modifiers, MouseButton};
use crate::core::geometry::{Point, Size};
use crate::elements::element::PointerEventKind;
use std::cell::Cell;
use std::rc::Rc;
use taffy::TaffyTree;

fn pointer(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        kind,
        position: Point::new(x, y),
        button: Some(MouseButton::Left),
    }
}

fn layout(element: &mut impl Element) -> (TaffyTree<ElementId>, NodeId) {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(320.0, 240.0));
    let node = element.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(320.0),
                height: AvailableSpace::Definite(240.0),
            },
        )
        .expect("overlay layout should compute");
    (taffy, node)
}

#[test]
fn advanced_ui_popover_hides_content_when_closed() {
    let popover = Popover::new("Inspector", button("Open"), text("Details"));
    let nodes = popover
        .accessibility_nodes(&AccessibilityContext::default())
        .expect("closed popover accessibility should build");

    assert!(!popover.is_open());
    assert_eq!(popover.children().len(), 1);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Button);
}

#[test]
fn advanced_ui_popover_exposes_content_when_open() {
    let id = ElementId::from(800);
    let popover = Popover::new("Inspector", button("Open"), text("Details"))
        .id(id)
        .open(true);
    let nodes = popover
        .accessibility_nodes(&AccessibilityContext::new(Some(id)))
        .expect("open popover accessibility should build");

    assert_eq!(popover.children().len(), 2);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Popover);
    assert_eq!(nodes[0].a11y_label(), Some("Inspector"));
    assert!(nodes[0].a11y_focused());
    assert_eq!(nodes[0].a11y_children().len(), 2);
}

#[test]
fn advanced_ui_popover_theme_density_changes_layout_gap() {
    let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
    let popover = Popover::new("Inspector", button("Open"), text("Details")).theme(theme);

    assert_eq!(popover.style().gap, 9.0);
}

#[test]
fn advanced_ui_popover_escape_dismisses_and_announces() {
    let dismissed = Rc::new(Cell::new(false));
    let dismissed_ref = Rc::clone(&dismissed);
    let mut popover = Popover::new("Inspector", button("Open"), text("Details"))
        .open(true)
        .on_dismiss(move || dismissed_ref.set(true));
    let (taffy, _) = layout(&mut popover);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(popover.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none())));
    assert!(!popover.is_open());
    assert!(dismissed.get());
    assert_eq!(
        cx.take_accessibility_announcements()[0].message(),
        "Inspector dismissed"
    );
}

#[test]
fn advanced_ui_dialog_exposes_modal_accessibility_tree() {
    let id = ElementId::from(801);
    let dialog = Dialog::new(
        "Confirm delete",
        container().w(180.0).h(90.0).child(text("Delete item?")),
    )
    .id(id);
    let nodes = dialog
        .accessibility_nodes(&AccessibilityContext::new(Some(id)))
        .expect("dialog accessibility should build");

    assert!(dialog.is_modal());
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Dialog);
    assert_eq!(nodes[0].a11y_label(), Some("Confirm delete"));
    assert!(nodes[0].a11y_focused());
    assert_eq!(nodes[0].a11y_children().len(), 1);
}

#[test]
fn advanced_ui_dialog_escape_dismisses_when_enabled() {
    let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0));
    let (taffy, _) = layout(&mut dialog);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(dialog.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none())));
    assert!(!dialog.is_open());
    assert!(cx.redraw_requested());
}

#[test]
fn advanced_ui_dialog_read_only_does_not_dismiss() {
    let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0)).read_only(true);
    let (taffy, _) = layout(&mut dialog);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(dialog.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none()),));
    assert!(dialog.is_open());
}

#[test]
fn advanced_ui_dialog_modal_consumes_inside_pointer_events() {
    let mut dialog = Dialog::new("Confirm", container().w(120.0).h(80.0));
    let (taffy, _) = layout(&mut dialog);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(dialog.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 4.0, 4.0)));
}

#[test]
fn advanced_ui_dialog_content_containment_respects_hit_clip() {
    let mut dialog = Dialog::new("Confirm", container().w(300.0).h(300.0));
    let (taffy, _) = layout(&mut dialog);
    let mut focused = None;
    let mut root = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );
    // Visible viewport is only the top 100px; content layout still covers y=150.
    let mut cx = root.with_bounds_and_hit_clip(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        Bounds::from_xywh(0.0, 0.0, 320.0, 100.0),
    );

    assert!(
        !dialog.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 50.0, 150.0)),
        "pointer in unclipped content but outside hit_clip must not be consumed"
    );
    assert!(
        dialog.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 50.0, 40.0)),
        "pointer inside hit_clip should still be consumed by modal dialog"
    );
}

fn child_pointer(taffy: &TaffyTree<ElementId>, root: NodeId, index: usize) -> Point {
    let child = taffy
        .children(root)
        .expect("overlay children")
        .get(index)
        .copied()
        .expect("overlay child");
    let layout = taffy.layout(child).expect("overlay child layout");
    Point::new(layout.location.x + 8.0, layout.location.y + 8.0)
}

fn assert_inactive_overlay_keeps_nested_editor<E: Element>(
    label: &str,
    overlay: &mut E,
    field_id: ElementId,
    content_child: usize,
    is_open: impl Fn(&E) -> bool,
) {
    let (taffy, root) = layout(overlay);
    let point = child_pointer(&taffy, root, content_child);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(
        overlay.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, point.x, point.y)),
        "{label}: pointer down should reach the nested editor"
    );
    assert_eq!(
        cx.focused_id(),
        Some(field_id),
        "{label}: pointer down should focus the nested editor"
    );
    assert!(
        overlay.handle_key_event(
            &mut cx,
            &KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a'),
        ),
        "{label}: character key should reach the nested editor"
    );
    assert_eq!(
        overlay
            .text_input_snapshot(field_id)
            .expect("snapshot")
            .text(),
        "a",
        "{label}: character key should change the nested editor value"
    );
    assert!(
        overlay.handle_text_input_command(&mut cx, &TextInputCommand::InsertText("z".into())),
        "{label}: text input command should reach the nested editor"
    );
    assert!(
        overlay.handle_text_input_event(&mut cx, &TextInputEvent::InsertText("q".into())),
        "{label}: text input event should reach the nested editor"
    );
    assert_eq!(
        overlay
            .text_input_snapshot(field_id)
            .expect("snapshot")
            .text(),
        "azq",
        "{label}: text input should apply to the nested editor"
    );
    assert!(
        overlay.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none())),
        "{label}: escape should be delivered without dismissing"
    );
    assert!(is_open(overlay), "{label}: escape must not dismiss");
    assert!(
        !overlay.handle_scroll_event(
            &mut cx,
            &ScrollEvent {
                position: point,
                delta_x: 0.0,
                delta_y: 12.0,
                modifiers: Modifiers::none(),
            },
        ),
        "{label}: scroll must stay unhandled"
    );
    assert!(
        overlay.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::F1, Modifiers::none())),
        "{label}: unhandled key should stay consumed"
    );

    let select_all = ActionId::from(StandardAction::SelectAll);
    assert!(
        overlay.dispatch_action(&mut cx, &select_all).is_handled(),
        "{label}: select all should reach the focused editor"
    );
    let selection = overlay
        .text_input_snapshot(field_id)
        .expect("snapshot")
        .selection()
        .normalized_range();
    assert_eq!(selection.start(), 0, "{label}: select all start");
    assert_eq!(selection.end(), 3, "{label}: select all end");

    let cancel = ActionId::from(StandardAction::Cancel);
    assert_eq!(
        overlay.dispatch_action(&mut cx, &cancel),
        ActionOutcome::Ignored,
        "{label}: cancel must not dismiss or reach the editor"
    );
    assert!(
        is_open(overlay),
        "{label}: cancel must leave the overlay open"
    );
}

fn assert_enabled_overlay_keeps_current_routing<E: Element>(
    label: &str,
    overlay: &mut E,
    field_id: ElementId,
    content_child: usize,
    is_open: impl Fn(&E) -> bool,
) {
    let (taffy, root) = layout(overlay);
    let point = child_pointer(&taffy, root, content_child);
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 320.0, 240.0),
        &taffy,
        &mut focused,
    );

    assert!(
        overlay.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, point.x, point.y)),
        "{label}: pointer down should focus nested content"
    );
    assert_eq!(cx.focused_id(), Some(field_id), "{label}: focus");

    let select_all = ActionId::from(StandardAction::SelectAll);
    assert_eq!(
        overlay.dispatch_action(&mut cx, &select_all),
        ActionOutcome::Ignored,
        "{label}: dispatch_action stays ignored while the overlay can activate"
    );
    let cancel = ActionId::from(StandardAction::Cancel);
    assert_eq!(
        overlay.dispatch_action(&mut cx, &cancel),
        ActionOutcome::Ignored,
        "{label}: cancel stays ignored while the overlay can activate"
    );
    assert!(
        overlay.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Escape, Modifiers::none())),
        "{label}: escape should still dismiss"
    );
    assert!(
        !is_open(overlay),
        "{label}: escape should close an enabled overlay"
    );
}

#[test]
fn advanced_ui_dialog_inactive_forwards_nested_editor() {
    for (label, disabled, read_only) in [
        ("read_only dialog", false, true),
        ("disabled dialog", true, false),
    ] {
        let field_id = ElementId::new();
        let mut dialog = Dialog::new(
            "Edit",
            TextField::new("Name").id(field_id).value("").w(160.0),
        )
        .disabled(disabled)
        .read_only(read_only);
        assert_inactive_overlay_keeps_nested_editor(
            label,
            &mut dialog,
            field_id,
            0,
            Dialog::is_open,
        );
    }

    let field_id = ElementId::new();
    let mut dialog = Dialog::new(
        "Edit",
        TextField::new("Name").id(field_id).value("").w(160.0),
    );
    assert_enabled_overlay_keeps_current_routing(
        "enabled dialog",
        &mut dialog,
        field_id,
        0,
        Dialog::is_open,
    );
}

#[test]
fn advanced_ui_popover_inactive_forwards_nested_editor() {
    let field_id = ElementId::new();
    let mut popover = Popover::new(
        "Edit",
        button("Open"),
        TextField::new("Name").id(field_id).value("").w(160.0),
    )
    .open(true)
    .read_only(true);
    assert_inactive_overlay_keeps_nested_editor(
        "read_only popover",
        &mut popover,
        field_id,
        1,
        Popover::is_open,
    );

    let field_id = ElementId::new();
    let mut popover = Popover::new(
        "Edit",
        button("Open"),
        TextField::new("Name").id(field_id).value("").w(160.0),
    )
    .open(true);
    assert_enabled_overlay_keeps_current_routing(
        "enabled popover",
        &mut popover,
        field_id,
        1,
        Popover::is_open,
    );
}

#[test]
#[should_panic(expected = "popover accessibility label must not be empty")]
fn advanced_ui_popover_rejects_empty_label() {
    drop(Popover::new(" ", button("Open"), text("Details")));
}

#[test]
#[should_panic(expected = "dialog accessibility label must not be empty")]
fn advanced_ui_dialog_rejects_empty_label() {
    drop(Dialog::new(" ", text("Details")));
}
