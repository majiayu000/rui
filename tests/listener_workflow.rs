use rui::advanced_ui::Button;
use rui::core::entity::Entity;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::prelude::*;
use rui::testing::mount_view;

const DRAFT: ElementId = ElementId(1);
const SAVED: ElementId = ElementId(2);

#[derive(Default)]
struct Editor {
    draft: String,
    saved: Option<Entity<String>>,
}

impl View for Editor {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        let saved = *self
            .saved
            .get_or_insert_with(|| cx.app_mut().create(String::new()));
        let saved_text = cx.app().get(saved).expect("saved entity").clone();
        div()
            .w(320.0)
            .h(160.0)
            .flex_col()
            .child(
                input()
                    .id(DRAFT)
                    .accessibility_label("Draft")
                    .value(self.draft.clone())
                    .on_change(cx.text_listener(|this, value, _| this.draft = value))
                    .on_submit(cx.text_listener(|this, value, cx| {
                        let saved = this.saved.expect("created during render");
                        *cx.app_mut().get_mut(saved).expect("saved entity") = value;
                        this.draft.clear();
                    })),
            )
            .child(Button::new(format!("Saved: {saved_text}")).id(SAVED))
    }
}

#[test]
fn typing_and_submitting_updates_an_entity_and_displays_it_on_the_next_frame() {
    let mut session = mount_view(Size::new(320.0, 160.0), Editor::default()).expect("mount editor");
    session.request_focus(Some(DRAFT));
    for ch in "Hi你".chars() {
        assert!(session.dispatch_key_event(
            &KeyEvent::new(KeyCode::Unknown(ch as u32), Modifiers::none()).with_char(ch)
        ));
    }
    session.frame().expect("frame after typing");
    let tree = session.accessibility_tree().expect("typed tree");
    assert_eq!(tree.find(DRAFT).expect("draft").a11y_value(), Some("Hi你"));
    assert_eq!(
        tree.find(SAVED).expect("saved").a11y_label(),
        Some("Saved: ")
    );

    assert!(session.dispatch_key_event(&KeyEvent::new(KeyCode::Enter, Modifiers::none())));
    session.frame().expect("frame after submit");
    let tree = session.accessibility_tree().expect("submitted tree");
    assert_eq!(tree.find(DRAFT).expect("draft").a11y_value(), Some(""));
    assert_eq!(
        tree.find(SAVED).expect("saved").a11y_label(),
        Some("Saved: Hi你")
    );

    // The next edit must not change the saved entity or replay the prior submission.
    assert!(session.dispatch_key_event(
        &KeyEvent::new(KeyCode::Unknown('x' as u32), Modifiers::none()).with_char('x')
    ));
    session.frame().expect("frame after another edit");
    let tree = session.accessibility_tree().expect("edited tree");
    assert_eq!(tree.find(DRAFT).expect("draft").a11y_value(), Some("x"));
    assert_eq!(
        tree.find(SAVED).expect("saved").a11y_label(),
        Some("Saved: Hi你")
    );
}
