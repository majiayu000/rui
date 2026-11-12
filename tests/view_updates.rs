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
