//! Action feedback in the winit runner: what an element says after a screen
//! reader's action reaches the platform as a polite live node, added afresh
//! for each message, which is what makes AT-SPI and UI Automation announce
//! it.

use super::accessibility::WindowAccessibility;
use super::accessibility_announce::ANNOUNCERS;
use super::accessibility_tree::{TextGeometry, WINDOW_NODE};
use crate::advanced_ui::Button;
use crate::core::ElementId;
use crate::core::geometry::Size;
use crate::core::presenter::Presenter;
use crate::elements::{Div, div};
use crate::platform::accessibility_request::dispatch_accessibility_request;
use accesskit::{Action, ActionRequest, Live, NodeId, TreeId, TreeUpdate};
use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler};
use accesskit_winit::WindowEvent as AccessKitEvent;

const SAVE: ElementId = ElementId(93001);

fn window() -> Presenter<Div> {
    let root = div().w(300.0).h(200.0).child(Button::new("Save").id(SAVE));
    let mut presenter = Presenter::with_root(Size::new(300.0, 200.0), root);
    presenter
        .layout(Size::new(300.0, 200.0))
        .expect("window should lay out");
    presenter.paint();
    // The runner does this while a reader reads.
    presenter.set_accessibility_announcements_enabled(true);
    presenter
}

fn reading() -> WindowAccessibility {
    let mut accessibility = WindowAccessibility::new(None);
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    accessibility
}

/// Hear what the presenter's elements said and build the next update.
fn next(
    accessibility: &mut WindowAccessibility,
    presenter: &mut Presenter<Div>,
) -> Option<TreeUpdate> {
    accessibility.hear(presenter);
    let tree = presenter
        .accessibility_tree()
        .expect("accessibility tree should build");
    accessibility.next_update(tree, TextGeometry::new(), "Feedback", 1.0)
}

fn press(accessibility: &mut WindowAccessibility, presenter: &mut Presenter<Div>) {
    let request = ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: NodeId(SAVE.0),
        data: None,
    };
    assert!(accessibility.handle_event(AccessKitEvent::ActionRequested(request)));
    for (id, request) in accessibility.take_requests() {
        let (handled, _) = dispatch_accessibility_request(
            presenter,
            id,
            &request,
            accessibility.published_tree(),
            "winit",
        );
        assert!(handled);
    }
}

/// The live nodes an update added, as AT-SPI and UI Automation see them.
#[derive(Default)]
struct Added(Vec<(NodeId, String, Live)>);

impl TreeChangeHandler for Added {
    fn node_added(&mut self, node: &NodeRef) {
        if node.live() != Live::Off {
            let name = node.value().unwrap_or_default();
            self.0.push((node.locate().0, name, node.live()));
        }
    }
    fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
    fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
    fn node_removed(&mut self, _: &NodeRef) {}
}

fn announcers(tree: &Tree) -> Vec<NodeId> {
    tree.state()
        .root()
        .children()
        .map(|node| node.locate().0)
        .filter(|id| ANNOUNCERS.contains(id))
        .collect()
}

#[test]
fn a_press_is_announced_by_a_new_live_node_each_time() {
    let mut presenter = window();
    let mut accessibility = reading();
    let mut tree = Tree::new(
        next(&mut accessibility, &mut presenter).expect("the initial tree"),
        true,
    );
    assert!(announcers(&tree).is_empty(), "nothing said yet");

    press(&mut accessibility, &mut presenter);
    let mut added = Added::default();
    tree.update_and_process_changes(
        next(&mut accessibility, &mut presenter).expect("the feedback is new"),
        &mut added,
    );
    assert_eq!(added.0.len(), 1);
    let (first, message, live) = added.0[0].clone();
    assert_eq!(message, "Save activated");
    assert_eq!(live, Live::Polite);
    assert_eq!(announcers(&tree), [first]);
    assert_eq!(tree.state().root().locate().0, WINDOW_NODE);

    assert!(
        next(&mut accessibility, &mut presenter).is_none(),
        "nothing new to say or show"
    );

    // The same words again are a new node, so they are announced again, and
    // the old one is gone.
    press(&mut accessibility, &mut presenter);
    let mut added = Added::default();
    tree.update_and_process_changes(
        next(&mut accessibility, &mut presenter).expect("the feedback is new"),
        &mut added,
    );
    assert_eq!(added.0.len(), 1);
    assert_eq!(added.0[0].1, "Save activated");
    assert_ne!(added.0[0].0, first);
    assert_eq!(announcers(&tree), [added.0[0].0]);
}

#[test]
fn focus_changes_and_old_feedback_are_not_announced() {
    let mut presenter = window();
    let mut accessibility = reading();
    next(&mut accessibility, &mut presenter).expect("the initial tree");
    presenter.set_focused_element(Some(SAVE));
    let update = next(&mut accessibility, &mut presenter).expect("focus moved");
    assert!(
        update.nodes.iter().all(|(id, _)| !ANNOUNCERS.contains(id)),
        "the tree's focus says it"
    );

    press(&mut accessibility, &mut presenter);
    next(&mut accessibility, &mut presenter).expect("feedback");
    // A reader that starts again does not hear the old feedback.
    assert!(accessibility.handle_event(AccessKitEvent::InitialTreeRequested));
    let update = next(&mut accessibility, &mut presenter).expect("a new initial tree");
    assert!(update.nodes.iter().all(|(id, _)| !ANNOUNCERS.contains(id)));
}
