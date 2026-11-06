//! Public frame-entry checks for the structural tree, separate from visibility.
use rui::advanced_ui::{
    Button, Dialog, Hoverable, Popover, Tab, TabPanel, Tabs, Toolbar, Tooltip, container,
};
use rui::core::accessibility::{AccessibilityContext, AccessibilityRole, AccessibilityTree};
use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{AnyElement, Element, view};
use rui::renderer::{Scene, text::TextMeasureCache};
use rui::{AppContext, Div, ElementId, Size, View, ViewContext, div};
use std::{cell::Cell, rc::Rc};

struct Child {
    label: Entity<String>,
    renders: Rc<Cell<usize>>,
    id: ElementId,
}
impl View for Child {
    type Element = Div;
    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        self.renders.set(self.renders.get() + 1);
        let label = cx.app().get(self.label).unwrap().clone();
        div().child(Button::new(label).id(self.id))
    }
}

struct Harness {
    app: AppContext,
    root: Div,
    tree: taffy::TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
}
impl Harness {
    fn new() -> Self {
        Self {
            app: AppContext::new(),
            root: div(),
            tree: taffy::TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
        }
    }

    fn child(&mut self, id: u64) -> (Entity<Child>, Entity<String>, Rc<Cell<usize>>) {
        let label = self.app.create(format!("child {id}"));
        let renders = Rc::new(Cell::new(0));
        let child = self.app.new_view(Child {
            label,
            renders: renders.clone(),
            id: ElementId::from(id),
        });
        (child, label, renders)
    }
    fn frame(&mut self, build: &mut impl FnMut(&mut AppContext) -> Div) {
        FramePipeline::build_frame_retained(
            &mut self.app,
            &mut self.root,
            build,
            &mut self.tree,
            &mut self.nodes,
            &mut self.scene,
            &mut self.measurer,
            Size::new(400.0, 300.0),
        )
        .unwrap();
    }
    fn a11y(&self) -> AccessibilityTree {
        AccessibilityTree::new(
            self.root
                .accessibility_nodes(&AccessibilityContext::default())
                .unwrap(),
        )
    }
    fn set<T: 'static>(&mut self, entity: Entity<T>, value: T) {
        *self.app.get_mut(entity).unwrap() = value;
        self.app.notify(entity.id());
    }
}

fn wrap(kind: usize, child: AnyElement) -> AnyElement {
    match kind {
        0 => Tooltip::new(child, "help").into(),
        1 => Hoverable::new(child).into(),
        2 => Tooltip::new(Hoverable::new(child), "help").into(),
        3 => Tabs::new([Tab::new("a", "A")], "a")
            .accessibility_label("tabs")
            .panel(TabPanel::new("a", child))
            .into(),
        4 => Dialog::new("dialog", child).into(),
        5 => Popover::new("popover", Button::new("anchor"), child)
            .open(true)
            .into(),
        6 => Toolbar::new("tools").child(child).into(),
        7 => container().child(child).into(),
        _ => TabPanel::new("a", child).into(),
    }
}

#[test]
fn containers_render_and_update_only_the_notified_child() {
    for kind in 0..9 {
        let mut h = Harness::new();
        let (child, label, renders) = h.child(101);
        let (sibling, _, sibling_renders) = h.child(102);
        let builds = Rc::new(Cell::new(0));
        let mut build = |_: &mut AppContext| {
            builds.set(builds.get() + 1);
            div()
                .child(wrap(kind, view(child).into()))
                .child(view(sibling))
        };
        h.frame(&mut build);
        assert_eq!(
            (renders.get(), sibling_renders.get()),
            (1, 1),
            "kind {kind}"
        );
        let tree = h.a11y();
        let node = tree.find(ElementId::from(101)).unwrap();
        assert_eq!(node.a11y_role(), AccessibilityRole::Button);
        assert_eq!(node.a11y_label(), Some("child 101"));
        fn count(nodes: &[rui::core::accessibility::AccessibilityNode], id: ElementId) -> usize {
            nodes
                .iter()
                .map(|node| usize::from(node.a11y_id() == id) + count(node.a11y_children(), id))
                .sum()
        }
        assert_eq!(count(tree.roots(), ElementId::from(101)), 1);
        h.frame(&mut build);
        assert_eq!(renders.get(), 1, "clean frame, kind {kind}");
        h.set(label, "updated child".to_string());
        h.frame(&mut build);
        assert_eq!(
            (renders.get(), sibling_renders.get(), builds.get()),
            (2, 1, 1),
            "kind {kind}"
        );
        assert_eq!(
            h.a11y().find(ElementId::from(101)).unwrap().a11y_label(),
            Some("updated child")
        );
        assert!(format!("{:?}", h.scene.primitives()).contains("updated child"));
        assert_eq!(h.app.child_view_count(), 2);
    }
}

#[test]
fn hidden_overlays_keep_views_but_hide_content_from_accessibility_and_paint() {
    for dialog in [false, true] {
        let mut h = Harness::new();
        let (child, label, renders) = h.child(201);
        let open = h.app.create(false);
        let mut build = |app: &mut AppContext| {
            let is_open = *app.get(open).unwrap();
            let overlay: AnyElement = if dialog {
                Dialog::new("dialog", view(child)).open(is_open).into()
            } else {
                Popover::new(
                    "popover",
                    Button::new("anchor").id(ElementId::from(202)),
                    view(child),
                )
                .open(is_open)
                .into()
            };
            div().child(overlay)
        };
        h.frame(&mut build);
        assert_eq!(renders.get(), 1);
        assert!(h.a11y().find(ElementId::from(201)).is_none());
        assert_eq!(h.a11y().find(ElementId::from(202)).is_some(), !dialog);
        h.set(label, "updated hidden child".into());
        h.frame(&mut build);
        assert_eq!(renders.get(), 2);
        assert!(!format!("{:?}", h.scene.primitives()).contains("updated hidden child"));
        for is_open in [true, false, true] {
            h.set(open, is_open);
            h.frame(&mut build);
            assert_eq!(h.a11y().find(ElementId::from(201)).is_some(), is_open);
            assert_eq!(
                format!("{:?}", h.scene.primitives()).contains("updated hidden child"),
                is_open
            );
            assert_eq!(h.app.child_view_count(), 1);
            assert_eq!(renders.get(), 2, "visibility does not recreate the view");
        }
    }
}

#[test]
fn switching_tabs_keeps_hidden_views_and_only_exposes_the_selected_panel() {
    let mut h = Harness::new();
    let (a, _, a_renders) = h.child(301);
    let (b, b_label, b_renders) = h.child(302);
    let selected = h.app.create(false);
    let mut build = |app: &mut AppContext| {
        let selected = if *app.get(selected).unwrap() {
            "b"
        } else {
            "a"
        };
        div().child(
            Tabs::new([Tab::new("a", "A"), Tab::new("b", "B")], selected)
                .accessibility_label("tabs")
                .panel(TabPanel::new("a", view(a)))
                .panel(TabPanel::new("b", view(b))),
        )
    };
    h.frame(&mut build);
    assert_eq!((a_renders.get(), b_renders.get()), (1, 1));
    assert!(h.a11y().find(ElementId::from(301)).is_some());
    assert!(h.a11y().find(ElementId::from(302)).is_none());
    h.set(b_label, "changed hidden tab".into());
    h.frame(&mut build);
    assert_eq!((a_renders.get(), b_renders.get()), (1, 2));
    assert!(!format!("{:?}", h.scene.primitives()).contains("changed hidden tab"));
    for select_b in [true, false, true] {
        h.set(selected, select_b);
        h.frame(&mut build);
        assert_eq!(h.a11y().find(ElementId::from(301)).is_some(), !select_b);
        assert_eq!(h.a11y().find(ElementId::from(302)).is_some(), select_b);
        assert_eq!(
            format!("{:?}", h.scene.primitives()).contains("changed hidden tab"),
            select_b
        );
        assert_eq!((a_renders.get(), b_renders.get()), (1, 2));
        assert_eq!(h.app.child_view_count(), 2);
    }
}

#[test]
fn wrapped_views_survive_keyed_insertion_reorder_and_are_released_on_removal() {
    let mut h = Harness::new();
    let items = [h.child(401), h.child(402), h.child(403)];
    let order = h.app.create(vec![0usize, 1]);
    let mut build =
        |app: &mut AppContext| {
            let order = app.get(order).unwrap().clone();
            div().children(order.into_iter().map(|i| {
                Tooltip::new(view(items[i].0), "help").id(ElementId::from(500 + i as u64))
            }))
        };
    h.frame(&mut build);
    h.set(order, vec![1, 2, 0]);
    h.frame(&mut build);
    assert_eq!(
        items.iter().map(|item| item.2.get()).collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    h.set(order, vec![2, 0]);
    h.frame(&mut build);
    assert_eq!(h.app.child_view_count(), 2);
    assert!(h.a11y().find(ElementId::from(402)).is_none());
    assert!(h.a11y().find(ElementId::from(401)).is_some());
    assert!(h.a11y().find(ElementId::from(403)).is_some());
}

#[test]
fn rebuilding_an_interactive_div_keeps_its_child_view() {
    let mut h = Harness::new();
    let (child, _, renders) = h.child(601);
    let root_state = h.app.create(false);
    let mut build = |app: &mut AppContext| {
        let _ = *app.get(root_state).unwrap();
        div().child(div().on_click(|| {}).child(view(child)))
    };
    h.frame(&mut build);
    h.set(root_state, true);
    h.frame(&mut build);
    assert_eq!(renders.get(), 1);
    assert!(h.a11y().find(ElementId::from(601)).is_some());
}
