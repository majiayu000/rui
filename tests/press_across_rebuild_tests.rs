//! Every control that keeps its own hover and press keeps a press across a
//! rebuild between mouse-down and mouse-up, so the release still activates it.

use rui::advanced_ui as ui;
use rui::core::geometry::Bounds;
use rui::core::{ElementId, Point, Size};
use rui::elements::{AnyElement, div};
use rui::renderer::{Primitive, Scene};
use rui::testing::mount;
use std::cell::Cell;
use std::rc::Rc;

const WINDOW: Size = Size::new(480.0, 360.0);
const CONTROL: ElementId = ElementId(42_001);

/// Where to press: the middle, or the last item of a row or a column.
#[derive(Clone, Copy)]
enum At {
    Middle,
    LastInRow,
    LastInColumn,
}

impl At {
    fn point(self, bounds: Bounds) -> Point {
        match self {
            At::Middle => Point::new(bounds.x() + 6.0, bounds.y() + bounds.height() / 2.0),
            At::LastInRow => Point::new(bounds.max_x() - 6.0, bounds.y() + bounds.height() / 2.0),
            At::LastInColumn => Point::new(
                bounds.x() + bounds.width() / 2.0,
                bounds.max_y() - bounds.height() / 4.0,
            ),
        }
    }
}

type Build = fn(Rc<Cell<u32>>) -> AnyElement;

fn count(hits: &Rc<Cell<u32>>) -> impl Fn() + 'static {
    let hits = Rc::clone(hits);
    move || hits.set(hits.get() + 1)
}

fn controls() -> Vec<(&'static str, At, Build)> {
    vec![
        ("button", At::Middle, |hits| {
            ui::button("Press")
                .id(CONTROL)
                .on_click(count(&hits))
                .into()
        }),
        ("checkbox", At::Middle, |hits| {
            let hit = count(&hits);
            ui::checkbox("Check")
                .id(CONTROL)
                .on_change(move |_| hit())
                .into()
        }),
        ("switch", At::Middle, |hits| {
            let hit = count(&hits);
            ui::switch("Toggle")
                .id(CONTROL)
                .on_change(move |_| hit())
                .into()
        }),
        ("radio group", At::LastInColumn, |hits| {
            let hit = count(&hits);
            ui::radio_group("Size", [("s", "Small"), ("l", "Large")], "s")
                .id(CONTROL)
                .on_change(move |_| hit())
                .into()
        }),
        ("segmented control", At::LastInRow, |hits| {
            let hit = count(&hits);
            ui::segmented_control([("a", "Alpha"), ("b", "Beta")], "a")
                .id(CONTROL)
                .on_change(move |_| hit())
                .into()
        }),
        ("tab list", At::LastInRow, |hits| {
            let hit = count(&hits);
            ui::tab_list([("a", "Alpha"), ("b", "Beta")], "a")
                .id(CONTROL)
                .on_change(move |_| hit())
                .into()
        }),
        ("menu", At::LastInColumn, |hits| {
            let hit = count(&hits);
            ui::menu("Actions", [("copy", "Copy"), ("paste", "Paste")])
                .id(CONTROL)
                .on_select(move |_| hit())
                .into()
        }),
        ("data list", At::LastInColumn, |hits| {
            let hit = count(&hits);
            ui::data_list([("one", "One"), ("two", "Two")])
                .id(CONTROL)
                .on_select(move |_| hit())
                .into()
        }),
        ("data tree", At::LastInColumn, |hits| {
            let hit = count(&hits);
            ui::data_tree([
                ui::DataTreeItem::new("one", "One"),
                ui::DataTreeItem::new("two", "Two"),
            ])
            .id(CONTROL)
            .on_select(move |_| hit())
            .into()
        }),
        ("data table row", At::Middle, |hits| {
            ui::data_table_row(["Name", "Value"])
                .id(CONTROL)
                .on_select(count(&hits))
                .into()
        }),
    ]
}

fn bounds_of(scene: &Scene, id: ElementId) -> Option<Bounds> {
    scene
        .layers()
        .iter()
        .flat_map(|layer| layer.hit_regions())
        .find(|region| region.id == id)
        .map(|region| region.bounds)
}

#[test]
fn every_pressable_control_keeps_its_press_across_a_rebuild() {
    let mut lost = Vec::new();
    for (name, at, build) in controls() {
        let hits = Rc::new(Cell::new(0));
        let for_build = Rc::clone(&hits);
        let mut builds = 0;
        let mut session = mount(WINDOW, move |_| {
            builds += 1;
            div()
                .flex_col()
                .p(8.0)
                .child(ui::text(format!("build {builds}")))
                .child(build(Rc::clone(&for_build)))
        })
        .expect("mount");
        session.frame().expect("first frame");
        let bounds =
            bounds_of(session.scene(), CONTROL).unwrap_or_else(|| panic!("{name}: no hit region"));
        let point = at.point(bounds);

        session.pointer_down(point);
        // The tree is built again between the press and the release.
        session.app_context_mut().request_rebuild();
        session.frame().expect("frame that rebuilds the tree");
        assert!(
            session
                .primitives()
                .iter()
                .any(|p| matches!(p, Primitive::Text { content, .. } if &**content == "build 2")),
            "{name}: the tree was rebuilt between press and release"
        );
        session.pointer_up(point);
        session.frame().expect("frame after release");
        if hits.get() != 1 {
            lost.push(name);
        }
    }
    assert!(lost.is_empty(), "the release did not activate: {lost:?}");
}

#[test]
fn a_select_keeps_its_press_across_a_rebuild() {
    let mut builds = 0;
    let mut session = mount(WINDOW, move |_| {
        builds += 1;
        div()
            .p(8.0)
            .child(ui::text(format!("build {builds}")))
            .child(ui::select("Fruit", [("a", "Apple"), ("b", "Banana")], "a").id(CONTROL))
    })
    .expect("mount");
    session.frame().expect("first frame");
    let point = At::Middle.point(bounds_of(session.scene(), CONTROL).expect("select"));
    session.pointer_down(point);
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame that rebuilds the tree");
    session.pointer_up(point);
    session.frame().expect("frame after release");
    assert_eq!(
        session.scene().overlays().len(),
        1,
        "the release still opens the list"
    );
}
