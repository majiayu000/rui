use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::{ElementId, Point, Size, View, ViewContext};
use rui::testing::mount_view;
use std::sync::{Arc, Mutex};

const BUTTON: ElementId = ElementId(43_001);

/// Clicking starts a task that computes off the UI thread in steps and shows
/// each step as it arrives.
struct Steps {
    reply: Option<Entity<String>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl View for Steps {
    type Element = Button;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let reply = *self
            .reply
            .get_or_insert_with(|| cx.app_mut().create(String::from("idle")));
        let label = cx.app().get(reply).expect("reply").clone();
        let log = Arc::clone(&self.log);
        Button::new(label)
            .id(BUTTON)
            .on_click(cx.listener(move |_, cx| {
                let log = Arc::clone(&log);
                cx.app_mut()
                    .spawn(move |cx| async move {
                        for step in 1..=3_u32 {
                            let log = Arc::clone(&log);
                            let squared = cx
                                .background_spawn(async move {
                                    log.lock().unwrap().push(format!("background {step}"));
                                    step * step
                                })
                                .await;
                            cx.update(|cx| {
                                *cx.get_mut(reply).expect("reply") = format!("step {squared}");
                                cx.notify(reply.id());
                            });
                        }
                    })
                    .detach();
            }))
    }
}

fn label(session: &rui::testing::HeadlessViewSession<Steps>) -> String {
    let tree = session.accessibility_tree().expect("accessibility tree");
    let node = tree.find(BUTTON).expect("button");
    node.a11y_label().expect("label").to_string()
}

#[test]
fn spawned_work_reaches_a_rendered_frame() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut session = mount_view(
        Size::new(200.0, 60.0),
        Steps {
            reply: None,
            log: Arc::clone(&log),
        },
    )
    .expect("mount view");

    session.pointer_down(Point::new(5.0, 5.0));
    session.pointer_up(Point::new(5.0, 5.0));
    session.frame().expect("frame that runs the click listener");
    assert_eq!(label(&session), "idle");

    session.run_until_parked();
    session.frame().expect("frame after the task finished");

    assert_eq!(label(&session), "step 9");
    assert_eq!(
        *log.lock().unwrap(),
        ["background 1", "background 2", "background 3"]
    );
    assert!(!session.app_context().has_pending_work());
}

#[test]
fn each_frame_polls_tasks_woken_before_it() {
    let mut session = mount_view(
        Size::new(200.0, 60.0),
        Steps {
            reply: None,
            log: Arc::default(),
        },
    )
    .expect("mount view");
    let reply = session.app_context_mut().create(String::new());
    session
        .app_context_mut()
        .spawn(move |cx| async move {
            cx.update(|cx| *cx.get_mut(reply).expect("reply") = String::from("first poll"));
        })
        .detach();
    assert!(session.app_context().has_pending_work());

    // A frame polls ready tasks without run_until_parked.
    session.frame().expect("frame");

    assert_eq!(
        *session.app_context().get(reply).expect("reply"),
        "first poll"
    );
}
