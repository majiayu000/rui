//! Async work and entity changes reach the views that show them without the
//! application moving state out of its view or notifying by hand.

use rui::advanced_ui::Button;
use rui::core::action::ActionId;
use rui::core::entity::Entity;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::{ElementId, Point, Size, View, ViewContext};
use rui::testing::{HeadlessViewSession, mount_view};
use std::cell::Cell;
use std::rc::Rc;

const BUTTON: ElementId = ElementId(44_001);

/// Clicking fetches off the UI thread and stores the reply in the view itself.
#[derive(Default)]
struct Fetch {
    reply: String,
}

impl View for Fetch {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let label = if self.reply.is_empty() {
            "idle".to_string()
        } else {
            self.reply.clone()
        };
        Button::new(label).id(BUTTON).on_click(cx.listener(|_, cx| {
            cx.spawn(|this, cx| async move {
                let reply = cx.background_spawn(async { 6 * 7 }).await;
                this.update(&cx, move |this, _| this.reply = format!("got {reply}"))
                    .expect("the view is still shown");
            })
            .detach();
        }))
    }
}

fn label<V: View + 'static>(session: &HeadlessViewSession<V>) -> String {
    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(BUTTON).expect("button");
    node.a11y_label().expect("label").to_string()
}

fn click<V: View + 'static>(session: &mut HeadlessViewSession<V>) {
    assert!(session.pointer_down(Point::new(4.0, 4.0)));
    assert!(session.pointer_up(Point::new(4.0, 4.0)));
}

#[test]
fn a_spawned_task_updates_its_own_view() {
    let mut session = mount_view(Size::new(200.0, 60.0), Fetch::default()).expect("mount");
    click::<Fetch>(&mut session);
    session.frame().expect("frame that runs the click listener");
    assert_eq!(label::<Fetch>(&session), "idle");

    session.run_until_parked();
    session.frame().expect("frame after the task finished");
    assert_eq!(label::<Fetch>(&session), "got 42");
}

/// Shows a counter kept in an entity, changed only through `Entity::update`.
struct Shared {
    count: Rc<Cell<Option<Entity<u32>>>>,
}

impl View for Shared {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let count = match self.count.get() {
            Some(count) => count,
            None => {
                let count = cx.app_mut().create(0_u32);
                self.count.set(Some(count));
                count
            }
        };
        let shown = *cx.app().get(count).expect("count");
        Button::new(format!("count {shown}")).id(BUTTON)
    }
}

#[test]
fn entity_update_renders_the_views_that_read_it() {
    let count = Rc::new(Cell::new(None));
    let shared = Shared {
        count: Rc::clone(&count),
    };
    let mut session = mount_view(Size::new(200.0, 60.0), shared).expect("mount");
    assert_eq!(label::<Shared>(&session), "count 0");

    let count = count.get().expect("created on first render");
    let previous = count
        .update(session.app_context_mut(), |count, _| {
            *count += 5;
            *count - 5
        })
        .expect("count");
    assert_eq!(previous, 0);
    session.frame().expect("frame after the update");
    assert_eq!(label::<Shared>(&session), "count 5");
}

/// A todo list whose Cmd+N adds an item, handled inside the view.
#[derive(Default)]
struct Todos {
    items: Vec<String>,
}

impl View for Todos {
    type Element = rui::Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        rui::div()
            .on_action(
                ActionId::custom("todo.add"),
                cx.listener(|this, _| this.items.push(format!("todo {}", this.items.len() + 1))),
            )
            .child(Button::new(format!("{} todos", self.items.len())).id(BUTTON))
    }
}

#[test]
fn a_key_binding_runs_a_view_action() {
    let mut session = mount_view(Size::new(200.0, 60.0), Todos::default()).expect("mount");
    let command = Modifiers::meta();
    session
        .app_context_mut()
        .keymap_mut()
        .bind(KeyCode::N, command, ActionId::custom("todo.add"))
        .expect("bind");

    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::N, command)));
    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::N, command)));
    session.frame().expect("frame after the actions");
    assert_eq!(label::<Todos>(&session), "2 todos");
    // A key bound to no action reaches nothing here.
    assert!(!session.dispatch_key_event(&KeyEvent::new(KeyCode::M, command)));
}

const PARENT_BUTTON: ElementId = ElementId(44_002);
const CHILD_BUTTON: ElementId = ElementId(44_003);

/// A button showing `shown`; clicking it adds one to `bumps`.
fn counter_button<V: 'static>(
    cx: &mut ViewContext<V>,
    id: ElementId,
    shown: Entity<u32>,
    bumps: Entity<u32>,
) -> rui::Div {
    let value = *cx.app().get(shown).expect("shown");
    let button = Button::new(format!("shown {value}"))
        .id(id)
        .on_click(cx.listener(move |_, cx| {
            bumps
                .update(cx.app_mut(), |count, _| *count += 1)
                .expect("bumps");
        }));
    rui::div().h(30.0).child(button)
}

/// Shows the parent's count; its click bumps the parent's.
struct Child {
    shown: Entity<u32>,
    bumps: Entity<u32>,
    renders: Rc<Cell<u32>>,
}

impl View for Child {
    type Element = rui::Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        self.renders.set(self.renders.get() + 1);
        counter_button(cx, CHILD_BUTTON, self.shown, self.bumps)
    }
}

/// Shows its own count above a child view; each one's click bumps the
/// count the other shows.
#[derive(Default)]
struct Parent {
    parts: Option<(Entity<u32>, Entity<u32>, Entity<Child>)>,
    child_renders: Rc<Cell<u32>>,
    rendered: bool,
}

impl View for Parent {
    type Element = rui::Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let renders = Rc::clone(&self.child_renders);
        let (parent_count, child_count, child) = *self.parts.get_or_insert_with(|| {
            let app = cx.app_mut();
            let (parent_count, child_count) = (app.create(0_u32), app.create(0_u32));
            let child = app.new_view(Child {
                shown: child_count,
                bumps: parent_count,
                renders,
            });
            (parent_count, child_count, child)
        });
        let root = rui::div().w(200.0).flex_col().child(counter_button(
            cx,
            PARENT_BUTTON,
            parent_count,
            child_count,
        ));
        // The child is placed from the second render on: the first one is
        // mount's, which lays out views it places before they render (#342).
        if std::mem::replace(&mut self.rendered, true) {
            root.child(rui::div().h(30.0).child(rui::elements::view(child)))
        } else {
            root
        }
    }
}

fn shown(session: &HeadlessViewSession<Parent>, id: ElementId) -> String {
    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(id).expect("button");
    node.a11y_label().expect("label").to_string()
}

fn click_at(session: &mut HeadlessViewSession<Parent>, at: Point) {
    assert!(session.pointer_down(at));
    assert!(session.pointer_up(at));
}

#[test]
fn a_parent_listener_update_shows_in_the_child_on_the_next_frame() {
    let child_renders = Rc::new(Cell::new(0));
    let parent = Parent {
        child_renders: Rc::clone(&child_renders),
        ..Parent::default()
    };
    let mut session = mount_view(Size::new(200.0, 60.0), parent).expect("mount");
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame that places the child");
    assert_eq!(shown(&session, CHILD_BUTTON), "shown 0");
    click_at(&mut session, Point::new(4.0, 4.0));
    session.frame().expect("frame that runs the listener");
    assert_eq!(shown(&session, CHILD_BUTTON), "shown 1");
    assert_eq!(shown(&session, PARENT_BUTTON), "shown 0");
    assert!(
        !session.app_context().has_frame_work(),
        "nothing left for another frame"
    );
    assert_eq!(
        child_renders.get(),
        2,
        "the child rendered once more, for the click"
    );
}

#[test]
fn a_child_listener_update_shows_in_the_parent_on_the_next_frame() {
    let child_renders = Rc::new(Cell::new(0));
    let parent = Parent {
        child_renders: Rc::clone(&child_renders),
        ..Parent::default()
    };
    let mut session = mount_view(Size::new(200.0, 60.0), parent).expect("mount");
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame that places the child");
    assert_eq!(shown(&session, PARENT_BUTTON), "shown 0");
    click_at(&mut session, Point::new(4.0, 34.0));
    session.frame().expect("frame that runs the listener");
    assert_eq!(shown(&session, PARENT_BUTTON), "shown 1");
    assert_eq!(shown(&session, CHILD_BUTTON), "shown 0");
    assert!(
        !session.app_context().has_frame_work(),
        "nothing left for another frame"
    );
    assert_eq!(
        child_renders.get(),
        2,
        "the child rendered once more, for the click"
    );
}
