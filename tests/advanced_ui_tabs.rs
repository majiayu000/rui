use rui::ElementId;
use rui::advanced_ui::{
    Tab, TabList, TabPanel, Tabs, Theme, ThemeDensity, container, scrollable, text,
};
use rui::core::accessibility::AccessibilityRole;
use rui::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use rui::core::geometry::{Bounds, Point, Size};
use rui::elements::Element;
use rui::elements::element::{EventContext, PointerEvent, PointerEventKind};
use rui::renderer::Primitive;
use rui::testing::mount;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::TaffyTree;

fn pointer(kind: PointerEventKind, x: f32) -> PointerEvent {
    PointerEvent {
        kind,
        position: Point::new(x, 4.0),
        button: Some(MouseButton::Left),
    }
}

fn event_context<'a>(
    taffy: &'a TaffyTree<ElementId>,
    focused: &'a mut Option<ElementId>,
) -> EventContext<'a> {
    EventContext::new(Bounds::from_xywh(0.0, 0.0, 220.0, 36.0), taffy, focused)
}

fn tab_texts(primitives: &[Primitive]) -> Vec<&str> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn advanced_ui_tabs_render_selected_panel_and_switch_by_pointer() {
    let mut session = mount(Size::new(240.0, 140.0), |_cx| {
        Tabs::new([("overview", "Overview"), ("logs", "Logs")], "overview")
            .accessibility_label("Project sections")
            .panel(TabPanel::new("overview", text("Summary")))
            .panel(TabPanel::new("logs", text("Build logs")))
    })
    .expect("tabs should mount");

    let texts = tab_texts(session.primitives());
    assert!(texts.contains(&"Overview"));
    assert!(texts.contains(&"Summary"));
    assert!(!texts.contains(&"Build logs"));

    assert!(session.pointer_down(Point::new(110.0, 10.0)));
    assert!(session.pointer_up(Point::new(110.0, 10.0)));
    session
        .frame()
        .expect("tabs should repaint after selection");

    let tree = session
        .accessibility_tree()
        .expect("tabs accessibility tree should build");
    let tab_list = tree
        .roots()
        .iter()
        .find(|node| node.a11y_role() == AccessibilityRole::TabList)
        .expect("tab list should be exposed");
    assert_eq!(tab_list.a11y_value(), Some("logs"));
}

#[test]
fn advanced_ui_tab_list_reports_pointer_and_keyboard_changes() {
    let id = ElementId::new();
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let changes_ref = Rc::clone(&changes);
    let mut list = TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview")
        .id(id)
        .accessibility_label("Project sections")
        .on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut cx = event_context(&taffy, &mut focused);

    assert!(list.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 160.0)));
    assert!(list.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 160.0)));
    assert_eq!(cx.focused_id(), Some(id));
    assert_eq!(list.selected_value(), "logs");

    assert!(list.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowLeft, Modifiers::none())
    ));
    assert_eq!(list.selected_value(), "overview");
    assert_eq!(
        &*changes.borrow(),
        &["logs".to_string(), "overview".to_string()]
    );
}

#[test]
fn advanced_ui_tab_list_keyboard_skips_disabled_tabs() {
    let id = ElementId::new();
    let disabled = Tab::new("disabled", "Disabled").disabled(true);
    let mut list = TabList::new(
        [
            Tab::new("overview", "Overview"),
            disabled,
            Tab::new("logs", "Logs"),
        ],
        "overview",
    )
    .id(id);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = event_context(&taffy, &mut focused);

    assert!(list.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowRight, Modifiers::none())
    ));
    assert_eq!(list.selected_value(), "logs");

    assert!(list.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowRight, Modifiers::none())
    ));
    assert_eq!(list.selected_value(), "overview");
}

#[test]
#[should_panic(expected = "tab values must be unique")]
fn advanced_ui_tab_list_rejects_duplicate_values() {
    drop(TabList::new(
        [Tab::new("same", "First"), Tab::new("same", "Second")],
        "same",
    ));
}

#[test]
fn advanced_ui_tab_list_ignores_modified_navigation_keys() {
    let id = ElementId::new();
    let mut list = TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview").id(id);
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(id);
    let mut cx = event_context(&taffy, &mut focused);

    assert!(!list.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowRight, Modifiers::shift())
    ));
    assert_eq!(list.selected_value(), "overview");
}

#[test]
fn advanced_ui_tab_list_moves_focus_when_keyboard_selects_tab() {
    let first_id = ElementId::new();
    let second_id = ElementId::new();
    let mut list = TabList::new(
        [
            Tab::new("overview", "Overview").id(first_id),
            Tab::new("logs", "Logs").id(second_id),
        ],
        "overview",
    );
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = Some(first_id);
    let mut cx = event_context(&taffy, &mut focused);

    assert!(list.handle_key_event(
        &mut cx,
        &KeyEvent::new(KeyCode::ArrowRight, Modifiers::none())
    ));
    assert_eq!(list.selected_value(), "logs");
    assert_eq!(cx.focused_id(), Some(second_id));
}

#[test]
fn advanced_ui_tabs_forward_scroll_to_selected_panel() {
    let did_scroll = Rc::new(Cell::new(false));
    let did_scroll_ref = Rc::clone(&did_scroll);
    let mut session = mount(Size::new(220.0, 140.0), |_cx| {
        Tabs::new([("overview", "Overview"), ("logs", "Logs")], "overview")
            .accessibility_label("Project sections")
            .panel(TabPanel::new(
                "overview",
                scrollable(container().w(120.0).h(240.0))
                    .h(48.0)
                    .on_scroll({
                        let did_scroll_ref = Rc::clone(&did_scroll_ref);
                        move |_, _| did_scroll_ref.set(true)
                    }),
            ))
            .panel(TabPanel::new("logs", text("Logs")))
    })
    .expect("tabs should mount");

    assert!(session.dispatch_scroll_event(&ScrollEvent {
        position: Point::new(20.0, 64.0),
        delta_x: 0.0,
        delta_y: 24.0,
        modifiers: Modifiers::none(),
    }));
    assert!(did_scroll.get());
}

#[test]
fn advanced_ui_tabs_theme_density_changes_layout_tokens() {
    let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
    let mut list =
        TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview").theme(theme);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx =
        rui::elements::element::LayoutContext::new(&mut taffy, Size::new(260.0, 100.0));
    let node = list.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(260.0),
                height: taffy::prelude::AvailableSpace::Definite(100.0),
            },
        )
        .expect("tab list layout should compute");
    let layout = taffy.layout(node).expect("tab list layout should exist");

    assert_eq!(layout.size.height, 54.0);
    assert_eq!(
        Tabs::new([("overview", "Overview")], "overview")
            .theme(theme)
            .style()
            .gap,
        12.0
    );
}

#[test]
fn advanced_ui_tab_list_disabled_and_read_only_do_not_change_selection() {
    for mut list in [
        TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview").disabled(true),
        TabList::new([("overview", "Overview"), ("logs", "Logs")], "overview").read_only(true),
    ] {
        let id = ElementId::new();
        list = list.id(id);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = event_context(&taffy, &mut focused);

        assert!(!list.handle_key_event(
            &mut cx,
            &KeyEvent::new(KeyCode::ArrowRight, Modifiers::none())
        ));
        assert!(!list.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 160.0)));
        assert_eq!(list.selected_value(), "overview");
    }
}
