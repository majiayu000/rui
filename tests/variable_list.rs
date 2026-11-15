use rui::core::frame_pipeline::FramePipeline;
use rui::core::presenter::Presenter;
use rui::core::style::Style;
use rui::core::text_editing::TextInputCommand;
use rui::core::{Color, ElementId, Point, Size};
use rui::elements::element::{AnyElement, LayoutContext, LayoutNodes, PaintContext, reconcile};
use rui::elements::{
    Element, TextArea, VariableList, VariableListHandle, div, text, variable_list,
};
use rui::renderer::{Scene, text::TextMeasureCache};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use taffy::{NodeId, TaffyTree};

fn id(i: u64) -> ElementId {
    ElementId::from(10_000 + i)
}
const VIEW: Size = Size::new(240.0, 300.0);
struct Probe {
    child: AnyElement,
    measured: Rc<Cell<usize>>,
}
impl Element for Probe {
    fn style(&self) -> &Style {
        self.child.style()
    }
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.measured.set(self.measured.get() + 1);
        self.child.layout(cx)
    }
    fn paint(&mut self, cx: &mut PaintContext) {
        self.child.paint(cx)
    }
    fn children(&self) -> &[AnyElement] {
        std::slice::from_ref(&self.child)
    }
    fn children_mut(&mut self) -> &mut [AnyElement] {
        std::slice::from_mut(&mut self.child)
    }
}
fn list(
    handle: &VariableListHandle,
    ids: Vec<ElementId>,
    width: f32,
    built: Rc<Cell<usize>>,
    measured: Rc<Cell<usize>>,
) -> VariableList {
    variable_list(handle, ids, move |key| {
        built.set(built.get() + 1);
        Probe {
            child: div()
                .h(if key.0 % 2 == 0 { 20.0 } else { 80.0 })
                .bg(Color::hex(key.0 as u32))
                .into(),
            measured: measured.clone(),
        }
    })
    .w(width)
    .h(300.0)
}
fn replace(p: &mut Presenter<VariableList>, mut next: VariableList) {
    reconcile(&mut next, p.root_mut());
    *p.root_mut() = next;
}
#[test]
fn hundred_thousand_rows_build_and_measure_only_the_neighborhood() {
    let h = VariableListHandle::new();
    let built = Rc::new(Cell::new(0));
    let measured = Rc::new(Cell::new(0));
    let mut p = Presenter::with_root(
        VIEW,
        list(
            &h,
            (0..100_000).map(id).collect(),
            240.0,
            built.clone(),
            measured.clone(),
        ),
    );
    p.layout(VIEW).unwrap();
    assert!(
        built.get() < 30 && measured.get() < 30,
        "first frame builds={} measures={}",
        built.get(),
        measured.get()
    );
    println!(
        "100k first frame: builds={} measures={}",
        built.get(),
        measured.get()
    );
    for (target, width) in [(5000, 240.0), (99_900, 130.0)] {
        built.set(0);
        measured.set(0);
        h.scroll_to_item(id(target));
        replace(
            &mut p,
            list(
                &h,
                (0..100_000).map(id).collect(),
                width,
                built.clone(),
                measured.clone(),
            ),
        );
        p.layout(VIEW).unwrap();
        p.paint();
        assert_eq!(h.anchor(), Some((id(target), 0.0)));
        assert!(
            built.get() < 30 && measured.get() < 30,
            "jump/resize builds={} measures={}",
            built.get(),
            measured.get()
        );
        println!(
            "100k target={target} width={width}: builds={} measures={}",
            built.get(),
            measured.get()
        );
    }
}
#[test]
fn prepend_append_invalidation_deletion_and_clear_preserve_item_anchor() {
    let h = VariableListHandle::new();
    h.scroll_to_item(id(5000));
    let b = Rc::new(Cell::new(0));
    let m = b.clone();
    let mut ids: Vec<_> = (100..10_000).map(id).collect();
    let mut p = Presenter::with_root(VIEW, list(&h, ids.clone(), 240.0, b.clone(), m.clone()));
    p.layout(VIEW).unwrap();
    h.scroll_to(h.offset() + 7.0);
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), Some((id(5000), 7.0)));
    ids.splice(0..0, (0..100).map(id));
    ids.extend((10_000..10_100).map(id));
    h.invalidate(id(4999));
    replace(&mut p, list(&h, ids.clone(), 240.0, b.clone(), m.clone()));
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), Some((id(5000), 7.0)));
    ids.retain(|key| *key != id(5000));
    replace(&mut p, list(&h, ids.clone(), 240.0, b.clone(), m.clone()));
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), Some((id(5001), 0.0)));
    ids.retain(|key| key.0 < id(5001).0);
    replace(&mut p, list(&h, ids, 240.0, b.clone(), m.clone()));
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), Some((id(4999), 0.0)));
    replace(&mut p, list(&h, vec![], 240.0, b, m));
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), None);
    assert!(p.root().built_items().is_empty());
}
#[test]
fn click_uses_displayed_row_and_clipped_rows_do_not_receive_pointer() {
    let h = VariableListHandle::new();
    h.scroll_to_item(id(5000));
    let clicked = Rc::new(Cell::new(None));
    let clicked2 = clicked.clone();
    let h2 = h.clone();
    let mut s = rui::testing::mount(Size::new(300.0, 400.0), move |_| {
        let clicked = clicked2.clone();
        variable_list(&h2, (0..10_000).map(id), move |key| {
            let clicked = clicked.clone();
            div().h(80.0).on_click(move || clicked.set(Some(key)))
        })
        .w(240.0)
        .h(300.0)
    })
    .unwrap();
    h.scroll_to(h.offset() + 10.0);
    s.frame().unwrap();
    s.pointer_down(Point::new(20.0, 5.0));
    s.pointer_up(Point::new(20.0, 5.0));
    assert_eq!(clicked.get(), Some(id(5000)));
    clicked.set(None);
    s.pointer_down(Point::new(20.0, 310.0));
    s.pointer_up(Point::new(20.0, 310.0));
    assert_eq!(clicked.get(), None);
}
fn scene(p: &mut VariableList, tree: &TaffyTree<ElementId>, bounds: rui::Bounds) -> String {
    let mut scene = Scene::new();
    FramePipeline::paint_root(p, tree, &mut scene, bounds);
    format!("{:?}", scene.primitives())
}
#[test]
fn wrapped_text_resize_and_visible_invalidation_match_fresh_layout() {
    let h = VariableListHandle::new();
    h.scroll_to_item(id(50));
    let mut tree = TaffyTree::new();
    let mut nodes = LayoutNodes::new();
    let mut cache = TextMeasureCache::new();
    for (frame, width) in [240.0, 130.0, 310.0].into_iter().enumerate() {
        h.invalidate(id(50));
        let make = |handle: &VariableListHandle| {
            variable_list(handle,(0..100).map(id),move |key| div().flex_col().child(text(format!("frame {frame} item {} wrapped text with enough words to span multiple lines at the narrow viewport",key.0)).wrap())).w(width).h(300.0)
        };
        let mut retained = make(&h);
        let bounds = FramePipeline::layout_root_retained(
            &mut retained,
            &mut tree,
            &mut nodes,
            &mut cache,
            VIEW,
        )
        .unwrap();
        let fh = VariableListHandle::new();
        fh.scroll_to_item(id(50));
        let mut fresh = make(&fh);
        let mut ft = TaffyTree::new();
        let fb = FramePipeline::layout_root(&mut fresh, &mut ft, VIEW).unwrap();
        assert_eq!(
            scene(&mut retained, &tree, bounds),
            scene(&mut fresh, &ft, fb)
        );
        assert_eq!(tree.total_node_count(), nodes.len());
        assert_eq!(h.anchor().unwrap().0, id(50));
        assert_eq!(retained.built_items(), fresh.built_items());
    }
}
#[test]
fn focused_editor_survives_scroll_rebuild_and_cancels_composition_before_deletion() {
    let h = VariableListHandle::new();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let editor = ElementId::from(7u64);
    let make = |ids: Vec<ElementId>| {
        let changes = changes.clone();
        variable_list(&h, ids, move |key| -> AnyElement {
            if key == id(0) {
                let changes = changes.clone();
                TextArea::new()
                    .id(editor)
                    .value("original")
                    .h(60.0)
                    .on_change(move |v| changes.borrow_mut().push(v.to_owned()))
                    .into()
            } else {
                div().h(40.0).into()
            }
        })
        .w(240.0)
        .h(300.0)
    };
    let mut p = Presenter::with_root(VIEW, make((0..100).map(id).collect()));
    p.layout(VIEW).unwrap();
    p.set_focused_element(Some(editor));
    p.with_event_context(|root, cx| {
        root.handle_pointer_event(
            cx,
            &rui::elements::element::PointerEvent {
                kind: rui::elements::element::PointerEventKind::Move,
                position: Point::new(20.0, 20.0),
                button: None,
            },
        )
    });
    p.with_event_context(|root, cx| {
        root.handle_text_input_command(cx, &TextInputCommand::BeginComposition("中文".into()))
    });
    assert!(
        p.root()
            .text_input_snapshot(editor)
            .unwrap()
            .composition()
            .is_some()
    );
    p.paint();
    let visible_caret = p
        .root()
        .text_input_snapshot(editor)
        .unwrap()
        .caret_bounds()
        .expect("visible focused caret");
    assert!(visible_caret.y() >= 0.0 && visible_caret.max_y() < 300.0);
    h.scroll_to_item(id(80));
    replace(&mut p, make((0..100).map(id).collect()));
    p.layout(VIEW).unwrap();
    assert_eq!(p.focused_element(), Some(editor));
    assert!(
        p.root()
            .text_input_snapshot(editor)
            .unwrap()
            .composition()
            .is_some()
    );
    assert!(p.root().built_items().contains(&id(0)));
    p.paint();
    let caret = p.root().text_input_snapshot(editor).unwrap().caret_bounds();
    assert!(
        caret.is_some_and(|bounds| bounds.max_y() <= 0.0),
        "offscreen editor must not expose stale visible caret: {caret:?}"
    );
    replace(&mut p, make((1..100).map(id).collect()));
    p.layout(VIEW).unwrap();
    assert_eq!(p.focused_element(), None);
    assert!(p.root().text_input_snapshot(editor).is_none());
    assert_eq!(
        changes.borrow().last().unwrap(),
        "original",
        "logical editor cancellation restores original text before removal"
    );
    let n = changes.borrow().len();
    p.with_event_context(|root, cx| {
        root.handle_text_input_command(cx, &TextInputCommand::CommitComposition("late".into()))
    });
    assert_eq!(changes.borrow().len(), n);
}

#[test]
fn visible_content_rebuilds_and_growing_row_above_does_not_move_anchor() {
    let h = VariableListHandle::new();
    h.scroll_to_item(id(50));
    let tall = Rc::new(Cell::new(false));
    let version = Rc::new(Cell::new(0));
    let make = || {
        let tall = tall.clone();
        let version = version.get();
        variable_list(&h, (0..200).map(id), move |key| {
            div()
                .h(if key == id(49) && tall.get() {
                    200.0
                } else {
                    40.0
                })
                .bg(if key == id(50) && version == 1 {
                    Color::hex(0xff0000)
                } else {
                    Color::hex(0x000000)
                })
        })
        .w(240.0)
        .h(300.0)
    };
    let mut p = Presenter::with_root(VIEW, make());
    p.layout(VIEW).unwrap();
    h.scroll_to(h.offset() + 9.0);
    p.layout(VIEW).unwrap();
    tall.set(true);
    version.set(1);
    h.invalidate(id(49));
    h.invalidate(id(50));
    replace(&mut p, make());
    p.layout(VIEW).unwrap();
    p.paint();
    assert_eq!(h.anchor(), Some((id(50), 9.0)));
    assert!(p.scene().primitives().iter().any(|primitive| matches!(primitive,rui::renderer::Primitive::Quad { background,.. } if *background==Color::hex(0xff0000).to_rgba())));
}

#[test]
fn changing_handle_or_explicit_container_identity_rebuilds_rows() {
    let h = VariableListHandle::new();
    let other = VariableListHandle::new();
    let b = Rc::new(Cell::new(0));
    let m = Rc::new(Cell::new(0));
    let mut p = Presenter::with_root(
        VIEW,
        list(&h, (0..100).map(id).collect(), 240.0, b.clone(), m.clone()).id(ElementId::from(1u64)),
    );
    p.layout(VIEW).unwrap();
    b.set(0);
    replace(
        &mut p,
        list(
            &other,
            (0..100).map(id).collect(),
            240.0,
            b.clone(),
            m.clone(),
        )
        .id(ElementId::from(1u64)),
    );
    p.layout(VIEW).unwrap();
    assert!(b.get() > 0);
    b.set(0);
    replace(
        &mut p,
        list(&other, (0..100).map(id).collect(), 240.0, b.clone(), m).id(ElementId::from(2u64)),
    );
    p.layout(VIEW).unwrap();
    assert!(b.get() > 0);
}

struct ObserveDrop {
    child: AnyElement,
    owner: ElementId,
    observed: Rc<RefCell<Option<rui::core::text_editing::TextInputSnapshot>>>,
}
impl Drop for ObserveDrop {
    fn drop(&mut self) {
        *self.observed.borrow_mut() = self.child.text_input_snapshot(self.owner);
    }
}
impl Element for ObserveDrop {
    fn style(&self) -> &Style {
        self.child.style()
    }
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.child.layout(cx)
    }
    fn paint(&mut self, cx: &mut PaintContext) {
        self.child.paint(cx)
    }
    fn children(&self) -> &[AnyElement] {
        std::slice::from_ref(&self.child)
    }
    fn children_mut(&mut self) -> &mut [AnyElement] {
        std::slice::from_mut(&mut self.child)
    }
    fn handle_text_input_command(
        &mut self,
        cx: &mut rui::elements::element::EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.child.handle_text_input_command(cx, command)
    }
}
#[test]
fn disabled_and_read_only_editors_cancel_before_their_instance_is_dropped() {
    for multiline in [false, true] {
        for disabled in [false, true] {
            let h = VariableListHandle::new();
            let observed = Rc::new(RefCell::new(None));
            let owner = ElementId::from(9u64);
            let observe = observed.clone();
            let list = variable_list(&h, [id(0)], move |_| {
                let child: AnyElement = if multiline {
                    let mut input = TextArea::new().id(owner).value("original");
                    input
                        .apply_text_input_command(TextInputCommand::BeginComposition("临时".into()))
                        .unwrap();
                    input.disabled(disabled).read_only(!disabled).into()
                } else {
                    let mut input = rui::elements::Input::new().id(owner).value("original");
                    input
                        .apply_text_input_command(TextInputCommand::BeginComposition("临时".into()))
                        .unwrap();
                    input.disabled(disabled).read_only(!disabled).into()
                };
                ObserveDrop {
                    child,
                    owner,
                    observed: observe.clone(),
                }
            })
            .w(240.0)
            .h(300.0);
            let mut p = Presenter::with_root(VIEW, list);
            p.layout(VIEW).unwrap();
            p.set_focused_element(Some(owner));
            let before = p.root().text_input_snapshot(owner).unwrap();
            assert!(before.composition().is_some());
            replace(
                &mut p,
                variable_list(&h, [], |_| div().h(40.0)).w(240.0).h(300.0),
            );
            p.layout(VIEW).unwrap();
            let observed = observed.borrow();
            let after = observed.as_ref().unwrap();
            assert_eq!(after.text(), "original");
            assert_eq!(after.composition(), None);
            assert_eq!(
                after.selection(),
                rui::core::text_editing::TextSelection::collapsed(8)
            );
            assert_eq!(p.focused_element(), None);
        }
    }
}

#[test]
fn tab_focus_is_kept_when_programmatic_scroll_immediately_removes_its_row_from_view() {
    use rui::core::event::{KeyCode, KeyEvent, Modifiers};
    let h = VariableListHandle::new();
    let cloned = h.clone();
    let editor = ElementId::from(8u64);
    let mut s = rui::testing::mount(VIEW, move |_| {
        variable_list(&cloned, (0..100).map(id), move |key| -> AnyElement {
            if key == id(0) {
                rui::elements::Input::new()
                    .id(editor)
                    .accessibility_label("Editor")
                    .value("type here")
                    .into()
            } else {
                div().h(40.0).into()
            }
        })
        .w(240.0)
        .h(300.0)
    })
    .unwrap();
    assert!(s.dispatch_key_event(&KeyEvent::new(KeyCode::Tab, Modifiers::none())));
    assert_eq!(s.focused_element(), Some(editor));
    h.scroll_to_item(id(80));
    s.app_context_mut().request_rebuild();
    s.frame().unwrap();
    assert_eq!(s.focused_element(), Some(editor));
    assert!(s.accessibility_tree().unwrap().find(editor).is_some());
}

#[test]
fn replacing_list_identity_cancels_old_editor_and_does_not_focus_same_id_new_editor() {
    for replace_handle in [false, true] {
        let first = VariableListHandle::new();
        let second = if replace_handle {
            VariableListHandle::new()
        } else {
            first.clone()
        };
        let owner = ElementId::from(7u64);
        let make = |handle: &VariableListHandle, container| {
            variable_list(handle, [id(0)], move |_| {
                TextArea::new().id(owner).value("original")
            })
            .w(240.0)
            .h(300.0)
            .id(ElementId::from(container))
        };
        let mut p = Presenter::with_root(VIEW, make(&first, 1u64));
        p.layout(VIEW).unwrap();
        p.set_focused_element(Some(owner));
        p.with_event_context(|root, cx| {
            root.handle_text_input_command(cx, &TextInputCommand::BeginComposition("中文".into()))
        });
        replace(&mut p, make(&second, if replace_handle { 1 } else { 2 }));
        p.layout(VIEW).unwrap();
        assert_eq!(p.focused_element(), None);
        let snapshot = p.root().text_input_snapshot(owner).unwrap();
        assert_eq!(snapshot.text(), "original");
        assert_eq!(snapshot.composition(), None);
    }
}

#[test]
fn estimated_pixel_scroll_backfills_the_measured_end_without_a_blank_tail() {
    let h = VariableListHandle::new();
    h.scroll_to(3700.0);
    let mut p = Presenter::with_root(
        VIEW,
        variable_list(&h, (0..100).map(id), |_| div().h(20.0).bg(Color::WHITE))
            .w(240.0)
            .h(300.0),
    );
    p.layout(VIEW).unwrap();
    p.paint();
    assert_eq!(h.anchor(), Some((id(85), 0.0)));
    assert!(p.scene().primitives().iter().any(|v| matches!(v,rui::renderer::Primitive::Quad { bounds,.. } if bounds.y()==280.0 && bounds.height()==20.0)));
    let old = h.anchor();
    h.scroll_to(h.offset() + 100.0);
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), old);
}

#[test]
fn retained_pure_wrapped_rows_match_fresh_after_resize_prepend_and_backward_scroll() {
    let h = VariableListHandle::new();
    h.scroll_to_item(id(50));
    let make = |handle: &VariableListHandle, width, start| {
        variable_list(handle,(start..100).map(id),|key| div().flex_col().child(text(format!("item {} long wrapped text across several lines for measurement and cached geometry",key.0)).wrap())).w(width).h(300.0)
    };
    let mut p = Presenter::with_root(VIEW, make(&h, 240.0, 10));
    let mut counts = Vec::new();
    for (width, start, target) in [
        (240.0, 10, 50),
        (130.0, 0, 50),
        (310.0, 0, 47),
        (240.0, 0, 49),
    ] {
        h.scroll_to_item(id(target));
        replace(&mut p, make(&h, width, start));
        p.layout(VIEW).unwrap();
        p.paint();
        counts.push(p.root().built_items().len());
        let fh = VariableListHandle::new();
        fh.scroll_to_item(id(target));
        let mut fresh = Presenter::with_root(VIEW, make(&fh, width, start));
        fresh.layout(VIEW).unwrap();
        fresh.paint();
        assert_eq!(
            format!("{:?}", p.scene().primitives()),
            format!("{:?}", fresh.scene().primitives()),
            "width={width} target={target}"
        );
        assert_eq!(
            p.taffy().total_node_count(),
            fresh.taffy().total_node_count()
        );
    }
    assert!(
        counts[1] < counts[0] && counts[2] > counts[1],
        "narrow text must become taller: {counts:?}"
    );
}

#[test]
fn invalidating_focused_composing_row_cancels_then_rebuilds_fresh_content() {
    let h = VariableListHandle::new();
    let owner = ElementId::from(7u64);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let make = |value: &'static str| {
        let changes = changes.clone();
        variable_list(&h, [id(0)], move |_| {
            let changes = changes.clone();
            TextArea::new()
                .id(owner)
                .value(value)
                .on_change(move |v| changes.borrow_mut().push(v.to_owned()))
        })
        .w(240.0)
        .h(300.0)
    };
    let mut p = Presenter::with_root(VIEW, make("old"));
    p.layout(VIEW).unwrap();
    p.set_focused_element(Some(owner));
    p.with_event_context(|root, cx| {
        root.handle_text_input_command(cx, &TextInputCommand::BeginComposition("中文".into()))
    });
    h.invalidate(id(0));
    replace(&mut p, make("fresh"));
    p.layout(VIEW).unwrap();
    assert_eq!(changes.borrow().last().unwrap(), "old");
    assert_eq!(p.focused_element(), None);
    let snapshot = p.root().text_input_snapshot(owner).unwrap();
    assert_eq!(snapshot.text(), "fresh");
    assert_eq!(snapshot.composition(), None);
}

#[test]
fn wheel_events_accumulate_before_the_next_layout() {
    use rui::core::event::{Modifiers, ScrollEvent};
    let h = VariableListHandle::new();
    let mut p = Presenter::with_root(
        VIEW,
        variable_list(&h, (0..100).map(id), |_| div().h(40.0))
            .w(240.0)
            .h(300.0),
    );
    p.layout(VIEW).unwrap();
    for _ in 0..2 {
        assert!(
            p.with_event_context(|root, cx| root.handle_scroll_event(
                cx,
                &ScrollEvent {
                    position: Point::new(20.0, 20.0),
                    delta_x: 0.0,
                    delta_y: 30.0,
                    modifiers: Modifiers::none()
                }
            ))
            .0
        );
    }
    p.layout(VIEW).unwrap();
    assert_eq!(h.anchor(), Some((id(1), 20.0)));
}
