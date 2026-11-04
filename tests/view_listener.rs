use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::{ElementId, Point, Size, View, ViewContext};
use rui::testing::mount_view;
use std::cell::Cell;
use std::rc::Rc;

fn button_id() -> ElementId {
    ElementId::from(7)
}

struct Counter {
    count: i32,
    total: Rc<Cell<Option<Entity<i32>>>>,
}

impl View for Counter {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if self.total.get().is_none() {
            self.total.set(Some(cx.app_mut().create(0)));
        }
        Button::new(format!("Count {}", self.count))
            .id(button_id())
            .on_click(cx.listener(|this, cx| {
                this.count += 1;
                let total = this.total.get().expect("created on first render");
                *cx.app_mut().get_mut(total).expect("total entity") += 10;
            }))
    }
}

fn label(session: &rui::testing::HeadlessViewSession<Counter>) -> String {
    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(button_id()).expect("button node");
    node.a11y_label().expect("button label").to_string()
}

fn click(session: &mut rui::testing::HeadlessViewSession<Counter>) {
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
}

#[test]
fn listener_updates_the_view_and_other_entities_before_the_next_frame() {
    let total = Rc::new(Cell::new(None));
    let view = Counter {
        count: 0,
        total: Rc::clone(&total),
    };
    let mut session = mount_view(Size::new(160.0, 80.0), view).expect("mount view");
    assert_eq!(label(&session), "Count 0");

    // Two clicks before a frame both apply, in order, without any notifier in the view.
    click(&mut session);
    click(&mut session);
    session.frame().expect("frame after listener");

    assert_eq!(label(&session), "Count 2");
    let total = total.get().expect("total entity");
    assert_eq!(*session.app_context().get(total).expect("total value"), 20);
}

struct Sync {
    checked: bool,
}

impl View for Sync {
    type Element = rui::advanced_ui::Checkbox;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        rui::advanced_ui::Checkbox::new("Sync")
            .id(button_id())
            .checked(self.checked)
            .on_change(cx.listener_with(|this, value, _| this.checked = value))
    }
}

#[test]
fn listener_with_passes_the_event_value() {
    let mut session =
        mount_view(Size::new(160.0, 80.0), Sync { checked: false }).expect("mount view");
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
    session.frame().expect("frame after listener");

    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(button_id()).expect("checkbox node");
    assert_eq!(node.a11y_checked(), Some(true));
}
