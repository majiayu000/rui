//! Inputs styled with an advanced UI [`Theme`] (formerly `advanced_ui::TextField`).

use super::super::*;
use crate::advanced_ui::{ControlSize, Theme, ThemeDensity};
use crate::advanced_ui::{
    container, dialog, hoverable, popover, tab, tab_panel, tabs, toolbar, tooltip,
};
use crate::core::accessibility::AccessibilityContext;
use crate::core::accessibility::AccessibilityTextRange;
use crate::core::color::Color;
use crate::core::event::Cursor;
use crate::core::event::{KeyCode, Modifiers, MouseButton};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::presenter::Presenter;
use crate::core::text_editing::{TextInputCommand, TextRange, Utf16TextRange};
use crate::elements::element::AnyElement;
use crate::elements::scroll_view;
use crate::renderer::Scene;
use std::cell::RefCell;
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, TaffyTree};

/// An input with a label and the default theme, as `text_field(label)` built.
fn field(label: &str) -> Input {
    Input::new()
        .accessibility_label(label)
        .theme(Theme::default())
}

fn pointer(kind: crate::elements::element::PointerEventKind) -> PointerEvent {
    PointerEvent {
        kind,
        position: Point::new(4.0, 4.0),
        button: Some(MouseButton::Left),
    }
}

fn assert_rich_ime_wrapper(root: impl Into<AnyElement>, focused: ElementId) {
    let viewport = Size::new(320.0, 120.0);
    let mut root = root.into();
    let mut taffy = TaffyTree::new();
    let node = root.layout(&mut LayoutContext::new(&mut taffy, viewport));
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .unwrap_or_else(|err| panic!("rich IME wrapper layout failed: {err}"));
    let layout = taffy
        .layout(node)
        .unwrap_or_else(|err| panic!("rich IME wrapper bounds lookup failed: {err}"));
    let bounds = Bounds::from_xywh(
        layout.location.x,
        layout.location.y,
        layout.size.width,
        layout.size.height,
    );
    let mut text_measurer = TextMeasureCache::new();
    root.refresh_text_geometry(&mut text_measurer);
    let mut scene = Scene::new();
    root.paint(&mut PaintContext::new(&mut scene, bounds, &taffy));
    let mut focus = Some(focused);
    let mut event_cx = EventContext::new(bounds, &taffy, &mut focus);

    let replacement_range = Utf16TextRange::new(1, 2)
        .unwrap_or_else(|err| panic!("valid replacement range rejected: {err}"));
    let handled = root.handle_text_input_command(
        &mut event_cx,
        &TextInputCommand::BeginCompositionReplacing {
            text: "你".to_string(),
            replacement_range,
        },
    );
    assert!(handled, "wrapper dropped rich composition command");

    let marked_selection = Utf16TextRange::new(0, 1)
        .unwrap_or_else(|err| panic!("valid marked selection rejected: {err}"));
    let handled = root.handle_text_input_command(
        &mut event_cx,
        &TextInputCommand::SetCompositionSelection(marked_selection),
    );
    assert!(handled, "wrapper dropped marked selection command");

    root.refresh_text_geometry(&mut text_measurer);
    root.paint(&mut PaintContext::new(&mut scene, bounds, &taffy));
    let snapshot = root
        .text_input_snapshot(focused)
        .unwrap_or_else(|| panic!("wrapper dropped focused text snapshot"));
    assert_eq!(snapshot.text(), "a你bc");
    assert_eq!(snapshot.composition(), Some(TextRange::ordered(1, 4)));
    assert_eq!(
        snapshot.selection().normalized_range(),
        TextRange::ordered(1, 4)
    );
    assert!(
        snapshot.geometry().is_some(),
        "wrapper dropped text geometry"
    );
}

fn rich_ime_field(focused: ElementId) -> Input {
    let mut field = field("Editor").id(focused).value("a😀bc");
    let taffy = TaffyTree::<ElementId>::new();
    let mut focus = None;
    let mut cx = EventContext::new(Bounds::from_xywh(0.0, 0.0, 200.0, 40.0), &taffy, &mut focus);
    assert!(field.handle_pointer_event(
        &mut cx,
        &pointer(crate::elements::element::PointerEventKind::Down),
    ));
    field
}

#[test]
fn popover_forwards_rich_ime_contract_to_open_content() {
    let focused = ElementId::new();
    assert_rich_ime_wrapper(
        popover("Editor", container(), rich_ime_field(focused)).open(true),
        focused,
    );
}

#[test]
fn dialog_forwards_rich_ime_contract_to_open_content() {
    let focused = ElementId::new();
    assert_rich_ime_wrapper(dialog("Editor", rich_ime_field(focused)), focused);
}

#[test]
fn tab_panel_forwards_rich_ime_contract_to_its_content() {
    let focused = ElementId::new();
    assert_rich_ime_wrapper(tab_panel("editor", rich_ime_field(focused)), focused);
}

#[test]
fn tabs_forwards_rich_ime_contract_to_the_selected_panel() {
    let focused = ElementId::new();
    let root = tabs([tab("editor", "Editor")], "editor")
        .panel(tab_panel("editor", rich_ime_field(focused)));
    assert_rich_ime_wrapper(root, focused);
}

#[test]
fn themed_input_applies_input_events_and_reports_change() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let changes_ref = Rc::clone(&changes);
    let mut field =
        field("Name").on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));

    let outcome = field
        .apply_text_input_event(TextInputEvent::InsertText("RUI".to_string()))
        .expect("text input should apply");

    assert!(outcome.changed);
    assert_eq!(field.get_value(), "RUI");
    assert_eq!(&*changes.borrow(), &["RUI".to_string()]);
}

#[test]
fn wrappers_preserve_rich_ime_commands_snapshot_and_geometry() {
    let focused = ElementId::new();
    let mut field = field("Editor").id(focused).value("a😀bc");
    let focus_taffy = TaffyTree::<ElementId>::new();
    let mut focus = None;
    let mut focus_cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 200.0, 40.0),
        &focus_taffy,
        &mut focus,
    );
    assert!(field.handle_pointer_event(
        &mut focus_cx,
        &pointer(crate::elements::element::PointerEventKind::Down),
    ));
    assert_eq!(focus_cx.focused_id(), Some(focused));
    let root = tooltip(
        hoverable(toolbar("Editing").child(scroll_view().child(container().child(field)))),
        "Editing help",
    );
    let viewport = Size::new(320.0, 120.0);
    let mut presenter = Presenter::with_root(viewport, root);
    presenter.set_focused_element(Some(focused));
    if let Err(err) = presenter.layout(viewport) {
        panic!("wrapped input layout failed: {err}");
    }
    presenter.paint();

    let replacement = match Utf16TextRange::new(1, 2) {
        Ok(range) => range,
        Err(err) => panic!("valid replacement range rejected: {err}"),
    };
    let (handled, _) = presenter.with_event_context(|root, event_cx| {
        root.handle_text_input_command(
            event_cx,
            &TextInputCommand::InsertTextReplacing {
                text: "X".to_string(),
                replacement_range: replacement,
            },
        )
    });
    assert!(handled);

    let composition_range = match Utf16TextRange::new(1, 1) {
        Ok(range) => range,
        Err(err) => panic!("valid composition range rejected: {err}"),
    };
    let (handled, _) = presenter.with_event_context(|root, event_cx| {
        root.handle_text_input_command(
            event_cx,
            &TextInputCommand::BeginCompositionReplacing {
                text: "你".to_string(),
                replacement_range: composition_range,
            },
        )
    });
    assert!(handled);

    let marked_selection = match Utf16TextRange::new(0, 1) {
        Ok(range) => range,
        Err(err) => panic!("valid marked selection rejected: {err}"),
    };
    let (handled, _) = presenter.with_event_context(|root, event_cx| {
        root.handle_text_input_command(
            event_cx,
            &TextInputCommand::SetCompositionSelection(marked_selection),
        )
    });
    assert!(handled);

    presenter.paint();
    let snapshot = match presenter.root().text_input_snapshot(focused) {
        Some(snapshot) => snapshot,
        None => panic!("wrapped input did not expose its native snapshot"),
    };
    assert_eq!(snapshot.text(), "a你bc");
    assert_eq!(snapshot.composition(), Some(TextRange::ordered(1, 4)));
    assert_eq!(
        snapshot.selection().normalized_range(),
        TextRange::ordered(1, 4)
    );
    assert!(
        snapshot.geometry().is_some(),
        "wrapped snapshot lost geometry: {snapshot:?}"
    );
}

#[test]
fn themed_input_blocks_disabled_and_read_only_editing() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let disabled_changes = Rc::clone(&changes);
    let mut disabled = field("Disabled")
        .value("fixed")
        .disabled(true)
        .on_change(move |value| disabled_changes.borrow_mut().push(value.to_string()));

    let outcome = disabled
        .apply_text_input_event(TextInputEvent::InsertText("x".to_string()))
        .expect("disabled edit should be ignored");
    assert!(!outcome.changed);
    assert_eq!(disabled.get_value(), "fixed");
    assert_eq!(disabled.cursor(), Cursor::NotAllowed);

    let mut read_only = field("Read only").value("fixed").read_only(true);
    let outcome = read_only
        .apply_key_event(&KeyEvent::new(KeyCode::Backspace, Modifiers::none()))
        .expect("read-only edit should be ignored");
    assert!(!outcome.changed);
    assert_eq!(read_only.get_value(), "fixed");
    assert_eq!(read_only.cursor(), Cursor::Default);
    assert!(changes.borrow().is_empty());
}

#[test]
fn themed_input_disabled_pointer_does_not_focus() {
    let mut field = field("Name").disabled(true);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
        &taffy,
        &mut focused,
    );

    assert!(!field.handle_pointer_event(
        &mut cx,
        &pointer(crate::elements::element::PointerEventKind::Down)
    ));
    assert_eq!(cx.focused_id(), None);
}

#[test]
fn themed_input_layout_uses_shared_control_size() {
    let mut field = field("Search").size(ControlSize::Large).w(260.0);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(300.0, 80.0));
    let node = field.layout(&mut layout_cx);

    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(80.0),
            },
        )
        .expect("input layout should compute");

    let layout = taffy.layout(node).expect("input layout should exist");
    assert_eq!(layout.size.width, 260.0);
    assert_eq!(layout.size.height, 44.0);
}

#[test]
fn themed_input_theme_density_changes_layout_tokens() {
    let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
    let mut field = field("Search").theme(theme).w(260.0);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(300.0, 80.0));
    let node = field.layout(&mut layout_cx);

    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(300.0),
                height: AvailableSpace::Definite(80.0),
            },
        )
        .expect("input layout should compute");

    let layout = taffy.layout(node).expect("input layout should exist");
    assert_eq!(layout.size.width, 260.0);
    assert_eq!(layout.size.height, 54.0);
}

#[test]
fn themed_input_accessibility_reflects_shared_contract() {
    let id = ElementId::new();
    let mut field = field("Search").id(id).value("alpha");
    field
        .apply_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::shift()))
        .expect("shift-left should create a selection");
    field = field.invalid(true).read_only(true);

    let nodes = field
        .accessibility_nodes(&AccessibilityContext::new(Some(id)))
        .expect("input accessibility should build");
    let node = nodes.first().expect("input should expose one node");

    assert_eq!(node.a11y_id(), id);
    assert_eq!(node.a11y_role(), AccessibilityRole::TextInput);
    assert_eq!(node.a11y_label(), Some("Search"));
    assert_eq!(node.a11y_value(), Some("alpha"));
    assert_eq!(node.a11y_text_caret(), Some(4));
    assert_eq!(
        node.a11y_text_selection(),
        Some(AccessibilityTextRange::new(4, 5))
    );
    assert!(node.a11y_focused());
    assert!(node.a11y_read_only());
    assert!(node.a11y_invalid());
    assert!(node.a11y_actions().is_empty());
}

#[test]
fn themed_input_paints_invalid_border() {
    let mut field = field("Email").invalid(true);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(200.0, 40.0));
    let node = field.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(200.0),
                height: AvailableSpace::Definite(40.0),
            },
        )
        .expect("input layout should compute");

    let mut scene = Scene::new();
    let mut paint_cx =
        PaintContext::new(&mut scene, Bounds::from_xywh(0.0, 0.0, 200.0, 36.0), &taffy);
    field.paint(&mut paint_cx);

    assert!(scene.primitives().iter().any(|primitive| {
        matches!(
            primitive,
            Primitive::Quad { border_color, .. }
                if *border_color == Color::hex(0xdc2626).to_rgba()
        )
    }));
}
