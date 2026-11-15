//! Editors keep their focused state across view rebuilds and lose it when
//! focus moves, so the caret (and the caret geometry the macOS input method
//! places its candidate window at) follows the focused editor.

use rui::core::accessibility::AccessibilityRole;
use rui::prelude::*;
use rui::renderer::Primitive;
use rui::testing::{HeadlessViewSession, mount_view};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Input and TextArea paint their caret as a quad this wide.
const CARET_WIDTH: f32 = 1.5;

/// Two controlled editors in a view that renders again on every change.
struct Editors {
    first: Rc<RefCell<String>>,
    notes: Rc<RefCell<String>>,
    first_blurs: Rc<Cell<u32>>,
}

impl Editors {
    fn new() -> Self {
        Self {
            first: Rc::default(),
            notes: Rc::default(),
            first_blurs: Rc::default(),
        }
    }
}

impl View for Editors {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        let (first, notes) = (Rc::clone(&self.first), Rc::clone(&self.notes));
        let (first_changed, notes_changed) = (cx.notifier(), cx.notifier());
        let blurs = Rc::clone(&self.first_blurs);
        div()
            .p(10.0)
            .flex_col()
            .gap(10.0)
            .child(
                input()
                    .w(200.0)
                    .accessibility_label("Name")
                    .value(self.first.borrow().clone())
                    .on_change(move |text| {
                        *first.borrow_mut() = text.to_string();
                        first_changed.notify();
                    })
                    .on_blur(move || blurs.set(blurs.get() + 1)),
            )
            // No explicit id: a text area is still focusable and exposed.
            .child(
                text_area()
                    .w(200.0)
                    .h(60.0)
                    .accessibility_label("Notes")
                    .value(self.notes.borrow().clone())
                    .on_change(move |text| {
                        *notes.borrow_mut() = text.to_string();
                        notes_changed.notify();
                    }),
            )
    }
}

fn carets(session: &HeadlessViewSession<Editors>) -> usize {
    session
        .primitives()
        .iter()
        .filter(|primitive| {
            matches!(primitive, Primitive::Quad { bounds, .. }
                if (bounds.width() - CARET_WIDTH).abs() < 0.01)
        })
        .count()
}

fn click(session: &mut HeadlessViewSession<Editors>, point: Point) {
    session.pointer_down(point);
    session.pointer_up(point);
    session.frame().expect("frame after click");
}

fn type_char(session: &mut HeadlessViewSession<Editors>, key: KeyCode, ch: char) {
    session.dispatch_key_event(&KeyEvent::new(key, Modifiers::none()).with_char(ch));
    session.frame().expect("frame after typing");
}

#[test]
fn controlled_input_keeps_its_caret_after_the_view_renders_again() {
    let mut session = mount_view(Size::new(300.0, 200.0), Editors::new()).expect("mount");
    click(&mut session, Point::new(50.0, 25.0));
    assert_eq!(carets(&session), 1);

    type_char(&mut session, KeyCode::A, 'a');

    assert_eq!(
        carets(&session),
        1,
        "typing rebuilt the input without its caret"
    );
}

#[test]
fn caret_and_blur_follow_focus_to_a_text_area_without_an_id() {
    let editors = Editors::new();
    let (notes, first_blurs) = (Rc::clone(&editors.notes), Rc::clone(&editors.first_blurs));
    let mut session = mount_view(Size::new(300.0, 200.0), editors).expect("mount");
    click(&mut session, Point::new(50.0, 25.0));
    type_char(&mut session, KeyCode::A, 'a');

    click(&mut session, Point::new(50.0, 90.0));
    type_char(&mut session, KeyCode::B, 'b');

    assert_eq!(notes.borrow().as_str(), "b");
    assert_eq!(
        carets(&session),
        1,
        "only the focused editor paints a caret"
    );
    assert_eq!(first_blurs.get(), 1);
    let tree = session.accessibility_tree().expect("accessibility tree");
    let mut stack: Vec<_> = tree.roots().iter().collect();
    let mut area = None;
    while let Some(node) = stack.pop() {
        if node.a11y_label() == Some("Notes") {
            area = Some(node);
        }
        stack.extend(node.a11y_children());
    }
    let area = area.expect("a text area without an id is exposed");
    assert_eq!(area.a11y_role(), AccessibilityRole::TextInput);
    assert!(area.a11y_multiline());
    assert!(area.a11y_focused());
}
