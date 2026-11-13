//! Slider, select, radio group, switch and splitter driven through a mounted
//! view: pointer and key events in, view state and accessibility out.

#[allow(dead_code)]
#[path = "../examples/controls.rs"]
mod controls;

use rui::advanced_ui::{self as ui, SplitAxis};
use rui::core::accessibility::{AccessibilityAction, AccessibilityNode, AccessibilityRole};
use rui::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton};
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::{Element, div};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::testing::{HeadlessSession, mount_view};

const ID: ElementId = ElementId(31_001);

fn key(key: KeyCode) -> KeyEvent {
    KeyEvent::new(key, Modifiers::none())
}

fn drag_to<F: FnMut(&mut AppContext) -> E, E: Element>(session: &mut HeadlessSession<F, E>, x: f32, y: f32) {
    session.dispatch_pointer_event(PointerEvent {
        kind: PointerEventKind::Move,
        position: Point::new(x, y),
        button: Some(MouseButton::Left),
    });
    session.frame().expect("frame after drag");
}

fn click<F: FnMut(&mut AppContext) -> E, E: Element>(session: &mut HeadlessSession<F, E>, x: f32, y: f32) {
    assert!(session.pointer_down(Point::new(x, y)), "press at {x},{y}");
    session.pointer_up(Point::new(x, y));
    session.frame().expect("frame after click");
}

fn node<F: FnMut(&mut AppContext) -> E, E: Element>(session: &HeadlessSession<F, E>, id: ElementId) -> AccessibilityNode {
    let tree = session.accessibility_tree().expect("accessibility tree");
    tree.find(id).expect("control node").clone()
}

struct SliderView {
    value: f32,
}

impl View for SliderView {
    type Element = ui::Slider;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        ui::slider("Volume", self.value)
            .id(ID)
            .width(216.0)
            .on_change(cx.listener_with(|this, value, _| this.value = value))
    }
}

#[test]
fn slider_drag_survives_rerenders_and_keys_step_the_value() {
    let mut session =
        mount_view(Size::new(300.0, 100.0), SliderView { value: 0.0 }).expect("mount");

    // The track runs from x = 8 to x = 208.
    assert!(session.pointer_down(Point::new(108.0, 18.0)));
    session.frame().expect("frame after press");
    assert_eq!(node(&session, ID).a11y_value(), Some("50"));
    drag_to(&mut session, 158.0, 30.0);
    drag_to(&mut session, 178.0, 60.0);
    assert_eq!(node(&session, ID).a11y_value(), Some("85"));
    session.pointer_up(Point::new(178.0, 60.0));
    drag_to(&mut session, 8.0, 18.0);
    assert_eq!(
        node(&session, ID).a11y_value(),
        Some("85"),
        "release ends the drag"
    );

    assert_eq!(session.focused_element(), Some(ID));
    session.dispatch_key_event(&key(KeyCode::ArrowRight));
    session.dispatch_key_event(&key(KeyCode::PageUp));
    session.frame().expect("frame after keys");
    assert_eq!(node(&session, ID).a11y_value(), Some("96"));
    session.dispatch_key_event(&key(KeyCode::End));
    session.frame().expect("frame after End");
    let slider = node(&session, ID);
    assert_eq!(slider.a11y_role(), AccessibilityRole::Slider);
    assert_eq!(slider.a11y_value(), Some("100"));
    assert_eq!(
        slider.a11y_numeric_value().map(|value| value.max()),
        Some(100.0)
    );
}

struct SwitchView {
    on: bool,
}

impl View for SwitchView {
    type Element = ui::Switch;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        ui::switch("Wi-Fi")
            .id(ID)
            .on(self.on)
            .on_change(cx.listener_with(|this, on, _| this.on = on))
    }
}

#[test]
fn switch_toggles_by_click_and_space() {
    let mut session = mount_view(Size::new(200.0, 60.0), SwitchView { on: false }).expect("mount");

    click(&mut session, 10.0, 18.0);
    assert_eq!(node(&session, ID).a11y_checked(), Some(true));
    assert!(session.dispatch_key_event(&key(KeyCode::Space)));
    session.frame().expect("frame after Space");
    let switch = node(&session, ID);
    assert_eq!(switch.a11y_role(), AccessibilityRole::Switch);
    assert_eq!(switch.a11y_checked(), Some(false));
    assert_eq!(switch.a11y_value(), Some("off"));
}

struct RadioView {
    size: String,
}

impl View for RadioView {
    type Element = ui::RadioGroup;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        ui::radio_group(
            "Size",
            [("s", "Small"), ("m", "Medium"), ("l", "Large")],
            self.size.clone(),
        )
        .id(ID)
        .on_change(cx.text_listener(|this, value, _| this.size = value))
    }
}

#[test]
fn radio_group_click_picks_and_arrows_wrap() {
    let mut session = mount_view(
        Size::new(200.0, 160.0),
        RadioView {
            size: "s".to_string(),
        },
    )
    .expect("mount");

    // Rows are 36pt tall.
    click(&mut session, 10.0, 90.0);
    assert_eq!(node(&session, ID).a11y_value(), Some("l"));
    session.dispatch_key_event(&key(KeyCode::ArrowDown));
    session.frame().expect("frame after ArrowDown");
    let group = node(&session, ID);
    assert_eq!(group.a11y_role(), AccessibilityRole::RadioGroup);
    assert_eq!(group.a11y_value(), Some("s"));
    let radios: Vec<_> = group
        .a11y_children()
        .iter()
        .map(|radio| (radio.a11y_role(), radio.a11y_selected()))
        .collect();
    assert_eq!(radios[0], (AccessibilityRole::RadioButton, Some(true)));
    assert_eq!(radios[2], (AccessibilityRole::RadioButton, Some(false)));
}

struct SelectView {
    fruit: String,
}

impl View for SelectView {
    type Element = ui::Select;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        ui::select(
            "Fruit",
            [
                ("apple", "Apple"),
                ("banana", "Banana"),
                ("cherry", "Cherry"),
            ],
            self.fruit.clone(),
        )
        .id(ID)
        .on_change(cx.text_listener(|this, value, _| this.fruit = value))
    }
}

#[test]
fn select_opens_on_click_and_picks_an_item_by_pointer_or_keyboard() {
    let mut session = mount_view(
        Size::new(240.0, 240.0),
        SelectView {
            fruit: "apple".to_string(),
        },
    )
    .expect("mount");

    click(&mut session, 20.0, 18.0);
    let open = node(&session, ID);
    assert_eq!(open.a11y_role(), AccessibilityRole::ComboBox);
    let list = open.a11y_children().first().expect("open list");
    assert_eq!(list.a11y_role(), AccessibilityRole::Menu);
    assert_eq!(list.a11y_children().len(), 3);

    // The list sits 4pt below the 36pt trigger, with 6pt padding and 36pt items.
    click(&mut session, 20.0, 64.0 + 36.0 * 2.0);
    let picked = node(&session, ID);
    assert_eq!(picked.a11y_value(), Some("Cherry"));
    assert!(picked.a11y_children().is_empty(), "picking closes the list");
    assert_eq!(session.focused_element(), Some(ID));

    session.dispatch_key_event(&key(KeyCode::ArrowDown));
    session.frame().expect("frame after open");
    session.dispatch_key_event(&KeyEvent::new(KeyCode::B, Modifiers::none()).with_char('b'));
    session.frame().expect("frame after type-ahead");
    session.dispatch_key_event(&key(KeyCode::Enter));
    session.frame().expect("frame after Enter");
    assert_eq!(node(&session, ID).a11y_value(), Some("Banana"));

    session.dispatch_key_event(&key(KeyCode::ArrowDown));
    session.dispatch_key_event(&key(KeyCode::ArrowDown));
    session.dispatch_key_event(&key(KeyCode::Escape));
    session.frame().expect("frame after Escape");
    let closed = node(&session, ID);
    assert_eq!(closed.a11y_value(), Some("Banana"));
    assert!(closed.a11y_children().is_empty());
}

struct SplitView {
    at: f32,
    axis: SplitAxis,
}

impl View for SplitView {
    type Element = ui::Splitter;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        ui::splitter("Sidebar", div().w_full().h_full(), div().w_full().h_full())
            .id(ID)
            .axis(self.axis)
            .position(self.at)
            .limits(80.0, 260.0)
            .on_change(cx.listener_with(|this, at, _| this.at = at))
    }
}

fn split_value<F: FnMut(&mut AppContext) -> E, E: Element>(session: &HeadlessSession<F, E>) -> String {
    let tree = session.accessibility_tree().expect("accessibility tree");
    let splitter = tree
        .roots()
        .iter()
        .find(|node| node.a11y_role() == AccessibilityRole::Splitter)
        .expect("splitter node");
    splitter.a11y_value().expect("splitter value").to_string()
}

#[test]
fn splitter_drags_within_limits_and_moves_with_arrows() {
    let view = SplitView {
        at: 120.0,
        axis: SplitAxis::Horizontal,
    };
    let mut session = mount_view(Size::new(400.0, 200.0), view).expect("mount");

    // The 6pt divider starts at x = 120; grab it 3pt in.
    assert!(session.pointer_down(Point::new(123.0, 100.0)));
    drag_to(&mut session, 203.0, 100.0);
    assert_eq!(split_value(&session), "200");
    drag_to(&mut session, 390.0, 100.0);
    assert_eq!(split_value(&session), "260", "max limit");
    drag_to(&mut session, 0.0, 100.0);
    assert_eq!(split_value(&session), "80", "min limit");
    session.pointer_up(Point::new(0.0, 100.0));

    session.dispatch_key_event(&key(KeyCode::ArrowRight));
    session.frame().expect("frame after ArrowRight");
    assert_eq!(split_value(&session), "90");
    session.dispatch_key_event(&key(KeyCode::End));
    session.frame().expect("frame after End");
    assert_eq!(split_value(&session), "260");
}

#[test]
fn vertical_splitter_moves_with_up_and_down() {
    let view = SplitView {
        at: 100.0,
        axis: SplitAxis::Vertical,
    };
    let mut session = mount_view(Size::new(300.0, 400.0), view).expect("mount");

    assert!(session.pointer_down(Point::new(150.0, 102.0)));
    drag_to(&mut session, 150.0, 152.0);
    session.pointer_up(Point::new(150.0, 152.0));
    assert_eq!(split_value(&session), "150");
    session.dispatch_key_event(&key(KeyCode::ArrowUp));
    session.frame().expect("frame after ArrowUp");
    assert_eq!(split_value(&session), "140");
}

#[test]
fn controls_example_renders_and_tab_reaches_every_control() {
    let mut session =
        mount_view(Size::new(900.0, 600.0), controls::ControlsView::default()).expect("mount");
    assert!(!session.primitives().is_empty());

    let mut roles = Vec::new();
    for _ in 0..5 {
        let mut tab = KeyEvent::new(KeyCode::Tab, Modifiers::none());
        tab.is_repeat = false;
        session.dispatch_key_event(&tab);
        session.frame().expect("frame after Tab");
        let focused = session.focused_element().expect("Tab focuses a control");
        let tree = session.accessibility_tree().expect("accessibility tree");
        let node = tree.find(focused).expect("focused node");
        roles.push(node.a11y_role());
        if node.a11y_role() == AccessibilityRole::Slider {
            assert!(
                node.a11y_actions()
                    .contains(&AccessibilityAction::Increment)
            );
        }
    }
    assert_eq!(
        roles,
        [
            AccessibilityRole::Switch,
            AccessibilityRole::RadioGroup,
            AccessibilityRole::Splitter,
            AccessibilityRole::Slider,
            AccessibilityRole::ComboBox,
        ]
    );
}
