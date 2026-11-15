//! Text is read by screen readers by default: plain and styled text, and
//! Markdown built from them, each become a static-text node with its own
//! bounds, which survive paint replay.

use rui::advanced_ui::button;
use rui::core::accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use rui::core::geometry::{Bounds, Size};
use rui::elements::{div, markdown, span, styled_text, text};
use rui::testing::mount;
use std::cell::Cell;
use std::rc::Rc;

const VIEWPORT: Size = Size::new(320.0, 240.0);

fn texts(nodes: &[AccessibilityNode], out: &mut Vec<(String, Bounds)>) {
    for node in nodes {
        if node.a11y_role() == AccessibilityRole::Text {
            let label = node.a11y_label().unwrap_or_default().to_string();
            out.push((label, node.a11y_bounds().unwrap_or_default()));
        }
        texts(node.a11y_children(), out);
    }
}

fn read(tree: &AccessibilityTree) -> Vec<(String, Bounds)> {
    let mut out = Vec::new();
    texts(tree.roots(), &mut out);
    out
}

#[test]
fn text_keeps_its_own_bounds_when_its_paint_is_replayed_moved() {
    let spacer = Rc::new(Cell::new(10.0));
    let height = Rc::clone(&spacer);
    let mut session = mount(VIEWPORT, move |_| {
        div()
            .flex_col()
            .child(div().h(height.get()))
            .child(div().p(8.0).child(text("Release notes")))
    })
    .expect("mount");
    session.frame().expect("first frame");
    let tree = session.accessibility_tree().expect("tree");
    let [(label, first)] = read(&tree).try_into().expect("one text node");
    assert_eq!(label, "Release notes");
    // Its own box inside the padding, not the padded parent's.
    assert_eq!((first.x(), first.y()), (8.0, 18.0));

    // The text's subtree is unchanged and moves down 30pt: it is replayed.
    spacer.set(40.0);
    session.app_context_mut().request_rebuild();
    session.frame().expect("second frame");
    assert!(session.scene().replayed_len() > 0, "the text's paint was replayed");
    let tree = session.accessibility_tree().expect("tree");
    let [(_, moved)] = read(&tree).try_into().expect("one text node");
    assert_eq!((moved.x(), moved.y()), (8.0, 48.0));
    assert_eq!(moved.size, first.size);
}

#[test]
fn a_button_does_not_repeat_its_own_text() {
    let mut session = mount(VIEWPORT, |_| div().child(button("Save")).child(text("Draft")))
        .expect("mount");
    session.frame().expect("frame");
    let tree = session.accessibility_tree().expect("tree");
    let labels: Vec<String> = read(&tree).into_iter().map(|(label, _)| label).collect();
    assert_eq!(labels, ["Draft"], "the button names itself; only the loose text is read");
}

#[test]
fn styled_text_and_markdown_are_read_as_their_characters() {
    let mut session = mount(VIEWPORT, |_| {
        div()
            .flex_col()
            .child(styled_text([span("Hello "), span("world").bold()]))
            .child(markdown("# Title\n\nSome *emphasis* here."))
    })
    .expect("mount");
    session.frame().expect("frame");
    let tree = session.accessibility_tree().expect("tree");
    let labels: Vec<String> = read(&tree).into_iter().map(|(label, _)| label).collect();
    assert_eq!(labels, ["Hello world", "Title", "Some emphasis here."]);
}

#[test]
fn hidden_text_is_left_out() {
    let mut session = mount(VIEWPORT, |_| {
        div()
            .child(text("×").accessibility_hidden())
            .child(styled_text([span("decor")]).accessibility_hidden())
    })
    .expect("mount");
    session.frame().expect("frame");
    let tree = session.accessibility_tree().expect("tree");
    assert!(read(&tree).is_empty());
}

#[test]
fn markdown_lists_read_their_numbers_but_not_their_bullets() {
    let mut session = mount(VIEWPORT, |_| {
        div()
            .flex_col()
            .child(markdown("- apples\n- pears\n\n1. first\n2. second"))
    })
    .expect("mount");
    session.frame().expect("frame");
    let tree = session.accessibility_tree().expect("tree");
    let labels: Vec<String> = read(&tree).into_iter().map(|(label, _)| label).collect();
    assert_eq!(labels, ["apples", "pears", "1.", "first", "2.", "second"]);
}
