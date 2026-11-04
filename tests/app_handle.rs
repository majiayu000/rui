use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::{AppClosed, AppContext, ElementId, Size, View, ViewContext};
use rui::testing::mount_view;
use std::cell::Cell;
use std::rc::Rc;

fn button_id() -> ElementId {
    ElementId::from(9)
}

struct Reply {
    text: Rc<Cell<Option<Entity<String>>>>,
}

impl View for Reply {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let text = match self.text.get() {
            Some(text) => text,
            None => {
                let text = cx.app_mut().create("waiting".to_string());
                self.text.set(Some(text));
                text
            }
        };
        let label = cx.app().get(text).expect("reply entity").clone();
        Button::new(label).id(button_id())
    }
}

#[test]
fn background_thread_updates_reach_the_next_frame() {
    let text = Rc::new(Cell::new(None));
    let mut session = mount_view(
        Size::new(200.0, 80.0),
        Reply {
            text: Rc::clone(&text),
        },
    )
    .expect("mount view");
    let entity = text.get().expect("created on first render");
    let handle = session.app_context().app_handle();

    std::thread::spawn(move || {
        for word in ["streamed", "reply"] {
            handle
                .update(move |cx| {
                    let mut text = cx.get_mut(entity).expect("reply entity");
                    if text.as_str() == "waiting" {
                        text.clear();
                    } else {
                        text.push(' ');
                    }
                    text.push_str(word);
                    drop(text);
                    cx.request_rebuild();
                })
                .expect("app is open");
        }
    })
    .join()
    .expect("background thread");

    session.frame().expect("frame after background update");
    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(button_id()).expect("button node");
    assert_eq!(node.a11y_label(), Some("streamed reply"));
}

#[test]
fn handle_reports_a_closed_app() {
    let handle = AppContext::new().app_handle();
    assert_eq!(handle.update(|_| {}), Err(AppClosed));
}
