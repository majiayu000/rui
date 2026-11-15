//! The overlay layer, driven through mounted views: open lists, popovers and
//! dialogs float over the page without moving it, are hit before it, close
//! on a press outside or Escape, stay inside the window, and modal dialogs
//! keep focus inside them.

use rui::advanced_ui as ui;
use rui::core::accessibility::AccessibilityNode;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::elements::{Div, Element, div};
use rui::renderer::{Primitive, Reach};
use rui::testing::{HeadlessSession, HeadlessViewSession, mount_view};
use std::cell::Cell;
use std::rc::Rc;

const WINDOW: Size = Size::new(320.0, 240.0);
const SELECT: ElementId = ElementId(41_001);
const BELOW: ElementId = ElementId(41_002);
const FAR: ElementId = ElementId(41_003);
const OPENER: ElementId = ElementId(41_004);
const BEHIND: ElementId = ElementId(41_005);
const DIALOG: ElementId = ElementId(41_006);
const FIRST: ElementId = ElementId(41_007);
const SECOND: ElementId = ElementId(41_008);
const ELSEWHERE: ElementId = ElementId(41_009);

fn key(key: KeyCode) -> KeyEvent {
    KeyEvent::new(key, Modifiers::none())
}

fn click<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &mut HeadlessSession<F, E>,
    x: f32,
    y: f32,
) {
    session.pointer_down(Point::new(x, y));
    session.pointer_up(Point::new(x, y));
    session.frame().expect("frame after click");
}

fn node<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
    id: ElementId,
) -> AccessibilityNode {
    let tree = session.accessibility_tree().expect("accessibility tree");
    tree.find(id).expect("node").clone()
}

fn bounds_of<F: FnMut(&mut AppContext) -> E, E: Element>(
    session: &HeadlessSession<F, E>,
    id: ElementId,
) -> Bounds {
    session
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.hit_regions())
        .find(|region| region.id == id)
        .map(|region| region.bounds)
        .unwrap_or_else(|| panic!("no hit region for {id:?}"))
}

fn fruits() -> [(&'static str, &'static str); 3] {
    [
        ("apple", "Apple"),
        ("banana", "Banana"),
        ("cherry", "Cherry"),
    ]
}

struct SelectStack {
    fruit: String,
    clicks: Rc<Cell<u32>>,
}

impl View for SelectStack {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let (below, far) = (Rc::clone(&self.clicks), Rc::clone(&self.clicks));
        div()
            .flex_col()
            .p(8.0)
            .gap(8.0)
            .child(
                ui::select("Fruit", fruits(), self.fruit.clone())
                    .id(SELECT)
                    .on_change(cx.text_listener(|this, value, _| this.fruit = value)),
            )
            .child(
                ui::button("Below")
                    .id(BELOW)
                    .on_click(move || below.set(below.get() + 1)),
            )
            .child(
                ui::button("Far")
                    .id(FAR)
                    .on_click(move || far.set(far.get() + 1)),
            )
    }
}

fn select_stack() -> (HeadlessViewSession<SelectStack>, Rc<Cell<u32>>) {
    let clicks = Rc::new(Cell::new(0));
    let view = SelectStack {
        fruit: "apple".to_string(),
        clicks: Rc::clone(&clicks),
    };
    let mut session = mount_view(WINDOW, view).expect("mount");
    session.frame().expect("first frame");
    (session, clicks)
}

#[test]
fn an_open_select_floats_over_its_siblings_without_moving_them() {
    let (mut session, clicks) = select_stack();
    let below = bounds_of(&session, BELOW);
    let far = bounds_of(&session, FAR);
    let trigger = bounds_of(&session, SELECT);

    click(&mut session, trigger.x() + 10.0, trigger.y() + 10.0);
    let select = node(&session, SELECT);
    let list = select.a11y_children().first().expect("open list").clone();
    let list_bounds = bounds_of(&session, list.a11y_id());

    assert_eq!(
        bounds_of(&session, BELOW),
        below,
        "siblings keep their place"
    );
    assert_eq!(bounds_of(&session, FAR), far);
    assert!(
        list_bounds.intersects(&below) && list_bounds.intersects(&far),
        "the list covers the buttons after the select: {list_bounds:?}"
    );

    // Drawn after the later siblings, and hit before them.
    let primitives = session.primitives();
    let label_at = |label: &str| {
        primitives
            .iter()
            .position(|primitive| matches!(primitive, Primitive::Text { content, .. } if &**content == label))
            .expect("label primitive")
    };
    let list_at = primitives
        .iter()
        .position(|primitive| matches!(primitive, Primitive::Quad { bounds, .. } if *bounds == list_bounds))
        .expect("list panel primitive");
    assert!(
        list_at > label_at("Far"),
        "the list draws over the later siblings"
    );
    let over_far = Point::new(far.x() + 10.0, far.y() + far.height() / 2.0);
    assert_eq!(session.scene().hit_test(over_far), Some(list.a11y_id()));

    // A click there picks the item under it, not the button.
    click(&mut session, over_far.x, over_far.y);
    let picked = node(&session, SELECT);
    assert_ne!(picked.a11y_value(), Some("Apple"), "an item was picked");
    assert!(picked.a11y_children().is_empty(), "picking closes the list");
    assert_eq!(
        clicks.get(),
        0,
        "the buttons under the list are not clicked"
    );
}

#[test]
fn a_press_outside_closes_the_list() {
    let (mut session, clicks) = select_stack();
    let trigger = bounds_of(&session, SELECT);
    click(&mut session, trigger.x() + 10.0, trigger.y() + 10.0);
    assert!(!node(&session, SELECT).a11y_children().is_empty());
    let list = node(&session, SELECT).a11y_children()[0].a11y_id();
    let list = bounds_of(&session, list);
    let window = Bounds::from_xywh(0.0, 0.0, WINDOW.width, WINDOW.height);

    // The frame the press closes it in redraws what it covered.
    session.pointer_down(Point::new(300.0, 220.0));
    assert!(node(&session, SELECT).a11y_children().is_empty());
    match session.scene().damage(window) {
        Reach::Within(damage) => assert!(
            damage.intersection(&list) == Some(list),
            "damage {damage:?} covers the closed list {list:?}"
        ),
        Reach::Unbounded => {}
        Reach::Nothing => panic!("closing the list damaged nothing"),
    }
    session.pointer_up(Point::new(300.0, 220.0));
    session.frame().expect("frame after release");
    assert_eq!(node(&session, SELECT).a11y_value(), Some("Apple"));
    assert_eq!(clicks.get(), 0);
    assert!(
        session.scene().overlays().is_empty(),
        "closing removes the overlay"
    );
}

struct CornerSelect;

impl View for CornerSelect {
    type Element = Div;

    fn render(&mut self, _cx: &mut ViewContext<Self>) -> Self::Element {
        div()
            .w_full()
            .h_full()
            .flex()
            .justify_end()
            .items_end()
            .child(ui::select("Fruit", fruits(), "apple").id(SELECT))
    }
}

#[test]
fn a_list_near_the_bottom_right_corner_flips_and_shifts_into_the_window() {
    let mut session = mount_view(WINDOW, CornerSelect).expect("mount");
    session.frame().expect("first frame");
    let trigger = bounds_of(&session, SELECT);
    assert_eq!(
        trigger.max_y(),
        WINDOW.height,
        "the select sits on the bottom edge"
    );

    click(&mut session, trigger.x() + 10.0, trigger.y() + 10.0);
    let list = node(&session, SELECT)
        .a11y_children()
        .first()
        .expect("open list")
        .a11y_id();
    let list = bounds_of(&session, list);
    let window = Bounds::from_xywh(0.0, 0.0, WINDOW.width, WINDOW.height);
    assert!(
        list.x() >= 0.0 && list.max_x() <= window.max_x(),
        "shifted inside horizontally: {list:?}"
    );
    assert!(
        list.max_y() <= trigger.y(),
        "flipped above the trigger: {list:?}"
    );
    assert!(list.y() >= 0.0);
}

struct DialogPage {
    open: bool,
    behind_clicks: Rc<Cell<u32>>,
}

impl View for DialogPage {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let behind = Rc::clone(&self.behind_clicks);
        div()
            .flex_col()
            .p(40.0)
            .gap(8.0)
            .child(
                ui::button("Open")
                    .id(OPENER)
                    .on_click(cx.listener(|this, _| this.open = true)),
            )
            .child(
                ui::button("Behind")
                    .id(BEHIND)
                    .on_click(move || behind.set(behind.get() + 1)),
            )
            .child(
                ui::dialog(
                    "Confirm",
                    div()
                        .flex()
                        .p(12.0)
                        .gap(8.0)
                        .child(ui::button("First").id(FIRST))
                        .child(ui::button("Second").id(SECOND)),
                )
                .id(DIALOG)
                .open(self.open)
                .on_dismiss(cx.listener(|this, _| this.open = false)),
            )
    }
}

fn dialog_page(open: bool) -> (HeadlessViewSession<DialogPage>, Rc<Cell<u32>>) {
    let clicks = Rc::new(Cell::new(0));
    let view = DialogPage {
        open,
        behind_clicks: Rc::clone(&clicks),
    };
    let mut session = mount_view(WINDOW, view).expect("mount");
    session.frame().expect("first frame");
    (session, clicks)
}

#[test]
fn a_modal_dialog_backdrop_covers_the_whole_window() {
    let (session, _) = dialog_page(true);
    let window = Bounds::from_xywh(0.0, 0.0, WINDOW.width, WINDOW.height);

    let overlays = session.scene().overlays();
    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].owner, DIALOG);
    assert_eq!(
        overlays[0].bounds, window,
        "not offset by the parent's padding"
    );
    assert!(session.primitives().iter().any(|primitive| matches!(
        primitive,
        Primitive::Quad { bounds, background, .. } if *bounds == window && background.a > 0.0 && background.a < 1.0
    )));
    assert_eq!(session.scene().hit_test(Point::new(2.0, 2.0)), Some(DIALOG));
    let behind = bounds_of(&session, BEHIND);
    assert_eq!(
        session
            .scene()
            .hit_test(Point::new(behind.x() + 4.0, behind.y() + 4.0)),
        Some(DIALOG),
        "the backdrop takes the pointer from the page"
    );
}

#[test]
fn a_modal_dialog_keeps_focus_inside_until_escape_returns_it() {
    let (mut session, behind_clicks) = dialog_page(false);
    let opener = bounds_of(&session, OPENER);
    click(&mut session, opener.x() + 6.0, opener.y() + 6.0);
    assert_eq!(
        session.focused_element(),
        Some(FIRST),
        "opening moves focus in"
    );

    session.dispatch_key_event(&key(KeyCode::Tab));
    assert_eq!(session.focused_element(), Some(SECOND));
    session.dispatch_key_event(&key(KeyCode::Tab));
    assert_eq!(
        session.focused_element(),
        Some(FIRST),
        "Tab wraps inside the dialog"
    );
    session.dispatch_key_event(&KeyEvent::new(KeyCode::Tab, Modifiers::shift()));
    assert_eq!(session.focused_element(), Some(SECOND), "so does Shift+Tab");
    for _ in 0..5 {
        session.dispatch_key_event(&key(KeyCode::Tab));
        session.dispatch_key_event(&key(KeyCode::Enter));
        let focused = session.focused_element();
        assert!(focused == Some(FIRST) || focused == Some(SECOND));
    }
    assert_eq!(behind_clicks.get(), 0, "Return never reaches the page");

    let behind = bounds_of(&session, BEHIND);
    click(&mut session, behind.x() + 4.0, behind.y() + 4.0);
    assert_eq!(behind_clicks.get(), 0, "nor does a click on the backdrop");

    session.dispatch_key_event(&key(KeyCode::Escape));
    session.frame().expect("frame after Escape");
    assert!(session.scene().overlays().is_empty(), "Escape closes it");
    assert_eq!(
        session.focused_element(),
        Some(OPENER),
        "focus returns to the opener"
    );
}

#[test]
fn escape_closes_only_the_topmost_overlay() {
    struct Nested;
    impl View for Nested {
        type Element = ui::Dialog;
        fn render(&mut self, _cx: &mut ViewContext<Self>) -> Self::Element {
            ui::dialog(
                "Settings",
                div()
                    .p(12.0)
                    .child(ui::select("Fruit", fruits(), "apple").id(SELECT)),
            )
            .id(DIALOG)
        }
    }
    let mut session = mount_view(WINDOW, Nested).expect("mount");
    session.frame().expect("first frame");
    assert_eq!(
        session.focused_element(),
        Some(SELECT),
        "the dialog focused its select"
    );
    session.dispatch_key_event(&key(KeyCode::ArrowDown));
    session.frame().expect("frame after open");
    assert_eq!(session.scene().overlays().len(), 2);

    session.dispatch_key_event(&key(KeyCode::Escape));
    session.frame().expect("frame after Escape");
    let owners: Vec<ElementId> = session.scene().overlays().iter().map(|o| o.owner).collect();
    assert_eq!(owners, [DIALOG], "the list closed, the dialog stays");
}

struct PopoverPage {
    dismissed: Rc<Cell<u32>>,
    elsewhere_clicks: Rc<Cell<u32>>,
}

impl View for PopoverPage {
    type Element = Div;

    fn render(&mut self, _cx: &mut ViewContext<Self>) -> Self::Element {
        let elsewhere = Rc::clone(&self.elsewhere_clicks);
        let dismissed = Rc::clone(&self.dismissed);
        div()
            .flex()
            .p(8.0)
            .gap(120.0)
            .child(
                ui::popover("Details", ui::button("Anchor"), ui::text("Floating"))
                    .open(true)
                    .on_dismiss(move || dismissed.set(dismissed.get() + 1)),
            )
            .child(
                ui::button("Elsewhere")
                    .id(ELSEWHERE)
                    .on_click(move || elsewhere.set(elsewhere.get() + 1)),
            )
    }
}

#[test]
fn a_press_on_another_control_closes_an_unfocused_popover() {
    let (clicks, dismissed) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let view = PopoverPage {
        dismissed: Rc::clone(&dismissed),
        elsewhere_clicks: Rc::clone(&clicks),
    };
    let mut session = mount_view(WINDOW, view).expect("mount");
    session.frame().expect("first frame");
    assert_eq!(session.scene().overlays().len(), 1);
    assert_eq!(session.focused_element(), None);

    let elsewhere = bounds_of(&session, ELSEWHERE);
    click(&mut session, elsewhere.x() + 4.0, elsewhere.y() + 4.0);
    assert!(session.scene().overlays().is_empty(), "the popover closed");
    assert_eq!(dismissed.get(), 1);
    assert_eq!(
        clicks.get(),
        1,
        "the press still reaches the control it hit"
    );
}
