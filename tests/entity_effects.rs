use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::core::{AppContext, ElementId, Point, Size, View, ViewContext};
use rui::testing::mount_view;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn observer_derives_state_from_a_notified_entity() {
    let mut cx = AppContext::new();
    let items = cx.create(vec![1, 2, 3]);
    let total = cx.create(0);
    cx.observe(items, move |items, cx| {
        let sum = cx.get(items).expect("items").iter().sum();
        *cx.get_mut(total).expect("total") = sum;
    })
    .detach();

    cx.get_mut(items).expect("items").push(4);
    cx.notify(items.id());
    FramePipeline::prepare_frame(&mut cx);

    assert_eq!(*cx.get(total).expect("total"), 10);
}

#[derive(Debug, PartialEq)]
struct Saved(u32);
struct Closed;

#[test]
fn subscriber_receives_only_its_event_type_until_dropped() {
    let mut cx = AppContext::new();
    let document = cx.create(());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let subscription = cx.subscribe(document, move |_, event: &Saved, _| {
        log.borrow_mut().push(event.0);
    });

    cx.emit(document, Saved(1));
    cx.emit(document, Closed);
    cx.emit(document, Saved(2));
    FramePipeline::prepare_frame(&mut cx);
    assert_eq!(*seen.borrow(), [1, 2]);

    drop(subscription);
    cx.emit(document, Saved(3));
    FramePipeline::prepare_frame(&mut cx);
    assert_eq!(*seen.borrow(), [1, 2]);
}

#[test]
fn releasing_an_entity_drops_its_observers() {
    let mut cx = AppContext::new();
    let model = cx.create(0);
    let calls = Rc::new(Cell::new(0));
    let counted = Rc::clone(&calls);
    cx.observe(model, move |_, _| counted.set(counted.get() + 1))
        .detach();

    cx.notify(model.id());
    FramePipeline::prepare_frame(&mut cx);
    assert_eq!(calls.get(), 1);

    assert!(cx.release(model));
    cx.notify(model.id());
    FramePipeline::prepare_frame(&mut cx);
    assert_eq!(calls.get(), 1);
    // The callback, and the Rc it captured, are gone.
    assert_eq!(Rc::strong_count(&calls), 1);
}

#[test]
fn observer_that_notifies_itself_does_not_hang_the_frame() {
    let mut cx = AppContext::new();
    let model = cx.create(0u32);
    cx.observe(model, |model, cx| {
        *cx.get_mut(model).expect("model") += 1;
        cx.notify(model.id());
    })
    .detach();

    cx.notify(model.id());
    FramePipeline::prepare_frame(&mut cx);

    assert_eq!(*cx.get(model).expect("model"), 64);
    assert!(cx.has_frame_work());
}

fn button_id() -> ElementId {
    ElementId::from(5)
}

struct Todos {
    items: Option<Entity<Vec<&'static str>>>,
    summary: Option<Entity<String>>,
}

impl View for Todos {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if self.items.is_none() {
            let app = cx.app_mut();
            let items = app.create(Vec::new());
            let summary = app.create("0 todos".to_string());
            app.observe(items, move |items, cx| {
                let count = cx.get(items).expect("items").len();
                *cx.get_mut(summary).expect("summary") = format!("{count} todos");
            })
            .detach();
            self.items = Some(items);
            self.summary = Some(summary);
        }
        let label = cx
            .app()
            .get(self.summary.expect("summary"))
            .expect("summary")
            .clone();
        Button::new(label)
            .id(button_id())
            .on_click(cx.listener(|this, cx| {
                let items = this.items.expect("items");
                cx.app_mut()
                    .get_mut(items)
                    .expect("items")
                    .push("write tests");
                cx.app_mut().notify(items.id());
            }))
    }
}

#[test]
fn a_view_shows_state_derived_by_an_observer_in_the_same_frame() {
    let view = Todos {
        items: None,
        summary: None,
    };
    let mut session = mount_view(Size::new(160.0, 80.0), view).expect("mount view");
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
    session.frame().expect("frame after click");

    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(button_id()).expect("button node");
    assert_eq!(node.a11y_label(), Some("1 todos"));
}
