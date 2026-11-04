//! Layout gives every element an identity that survives rebuilding the tree:
//! explicit ids win, everything else is derived from parent, position and type.

use rui::core::event::MouseButton;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{AnyElement, PointerEvent, PointerEventKind};
use rui::elements::{Div, Element, VirtualListHandle, div, input, virtual_list};
use rui::{AppContext, ElementId, Point, Presenter, Size};
use taffy::prelude::TaffyTree;

const VIEWPORT: Size = Size {
    width: 320.0,
    height: 240.0,
};

fn laid_out(mut root: Div) -> Div {
    let mut taffy = TaffyTree::<ElementId>::new();
    if let Err(err) = FramePipeline::layout_root(&mut root, &mut taffy, VIEWPORT) {
        panic!("layout failed: {err}");
    }
    root
}

/// Stable ids of every `AnyElement` under `children`, in tree order.
fn identities(children: &[AnyElement], out: &mut Vec<ElementId>) {
    for child in children {
        match child.stable_id() {
            Some(id) => out.push(id),
            None => panic!("a laid-out element has no stable id"),
        }
        identities(child.children(), out);
    }
}

fn identities_of(root: &Div) -> Vec<ElementId> {
    let mut out = Vec::new();
    identities(root.children(), &mut out);
    out
}

fn rows(count: usize) -> Div {
    div().children((0..count).map(|_| div().h(10.0).child(div().w(4.0).h(4.0))))
}

#[test]
fn identities_survive_a_rebuild_and_differ_between_elements() {
    let first = identities_of(&laid_out(rows(6)));
    let second = identities_of(&laid_out(rows(6)));
    assert_eq!(first.len(), 12);
    assert_eq!(
        first, second,
        "rebuilding the same tree must keep identities"
    );

    let mut unique = first.clone();
    unique.sort_by_key(|id| id.0);
    unique.dedup();
    assert_eq!(unique.len(), first.len(), "identities must be distinct");
    assert!(first.iter().all(|id| !id.is_ephemeral()));
}

#[test]
fn appending_a_child_keeps_earlier_identities() {
    let short = identities_of(&laid_out(rows(3)));
    let long = identities_of(&laid_out(rows(4)));
    assert_eq!(short[..], long[..short.len()]);
}

#[test]
fn a_different_type_at_the_same_position_gets_a_new_identity() {
    let with_div = identities_of(&laid_out(div().child(div())));
    let with_input = identities_of(&laid_out(div().child(input())));
    assert_ne!(with_div[0], with_input[0]);
}

#[test]
fn explicit_ids_win_and_key_their_subtree() {
    let keyed = |id: u64| div().id(ElementId(id)).child(div().w(2.0));
    let before = identities_of(&laid_out(div().child(keyed(7)).child(keyed(9))));
    // A new first sibling shifts positions; keyed rows and their children keep ids.
    let after = identities_of(&laid_out(
        div().child(div()).child(keyed(7)).child(keyed(9)),
    ));
    assert_eq!(before[0], ElementId(7));
    assert_eq!(before[2], ElementId(9));
    assert_eq!(before[..], after[1..]);
}

#[test]
fn virtual_list_rows_keep_their_identity_while_scrolling() {
    let handle = VirtualListHandle::new();
    let list = |handle: &VirtualListHandle| {
        div()
            .h(100.0)
            .child(virtual_list(handle, 1_000, 10.0, |_| div().h(10.0)).h(100.0))
    };
    let row_ids = |root: &Div| {
        let list = &root.children()[0];
        list.children()
            .iter()
            .map(|row| row.stable_id())
            .collect::<Vec<_>>()
    };

    let at_top = laid_out(list(&handle));
    let top_ids = row_ids(&at_top);
    handle.scroll_to(50.0);
    let scrolled = laid_out(list(&handle));
    let scrolled_ids = row_ids(&scrolled);

    // Scrolled by five rows, the first built row was built further down before
    // and must have kept its id.
    let offset = top_ids
        .iter()
        .position(|id| *id == scrolled_ids[0])
        .unwrap_or_else(|| panic!("the first scrolled row was not in view at the top"));
    assert!(offset > 0, "scrolling should start the built rows later");
    for (scrolled, top) in scrolled_ids.iter().zip(&top_ids[offset..]) {
        assert_eq!(scrolled, top);
    }
}

#[test]
fn focus_on_an_input_without_an_id_survives_a_rebuild() {
    let mut context = AppContext::new();
    let mut build = |_: &mut AppContext| div().child(input().w(200.0).h(30.0));
    let root = build(&mut context);
    let mut presenter = Presenter::with_root(VIEWPORT, root);
    if let Err(err) = presenter.layout(VIEWPORT) {
        panic!("layout failed: {err}");
    }
    presenter.paint();

    let press = |kind| PointerEvent {
        kind,
        position: Point::new(10.0, 10.0),
        button: Some(MouseButton::Left),
    };
    presenter.dispatch_pointer_event(&press(PointerEventKind::Down));
    presenter.dispatch_pointer_event(&press(PointerEventKind::Up));
    let focused = presenter.focused_element();
    let input_id = presenter.root().children()[0].id();
    assert!(focused.is_some(), "clicking the input should focus it");
    assert_eq!(focused, input_id);

    context.request_rebuild();
    presenter.rebuild_if_needed(&mut context, &mut build);
    if let Err(err) = presenter.layout(VIEWPORT) {
        panic!("layout failed: {err}");
    }
    assert_eq!(
        presenter.root().children()[0].id(),
        focused,
        "the rebuilt input must keep the id focus points at"
    );
}
