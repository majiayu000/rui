//! Views placed in a tree render again only when they, or an entity their last
//! render read, are notified; the tree around them is not rebuilt for that.

use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{Div, view};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Primitive, Scene};
use rui::{AppContext, Bounds, Color, ElementId, Size, View, ViewContext, div, scroll_view, text};
use std::cell::Cell;
use std::rc::Rc;
use taffy::prelude::TaffyTree;

const VIEWPORT: Size = Size {
    width: 320.0,
    height: 900.0,
};

struct RowModel {
    label: String,
    color: u32,
}

struct RowView {
    model: Entity<RowModel>,
    renders: Rc<Cell<u32>>,
    /// Bumped by the row's own click listener.
    clicks: u32,
}

impl View for RowView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        self.renders.set(self.renders.get() + 1);
        let model = match cx.app().get(self.model) {
            Some(model) => model,
            None => panic!("the row's model is gone"),
        };
        let row = div()
            .h(20.0)
            .bg(Color::hex(model.color))
            .child(text(format!("{} ({} clicks)", model.label, self.clicks)));
        drop(model);
        row.on_click(cx.listener(|this, _| this.clicks += 1))
    }
}

struct App {
    context: AppContext,
    /// By creation order, as are `rows` and `renders`.
    models: Vec<Entity<RowModel>>,
    rows: Vec<Entity<RowView>>,
    renders: Vec<Rc<Cell<u32>>>,
    /// The rows the root places, in order, by creation index.
    placed: Vec<usize>,
    /// Wraps each row's view in a div keyed by the row.
    keyed_divs: bool,
    builds: Rc<Cell<u32>>,
    /// Read by the root, so notifying it rebuilds everything.
    title: Entity<String>,
    /// Places the rows inside a `ScrollView` instead of directly in the root.
    in_scroll_view: bool,
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
}

impl App {
    fn new(count: usize) -> Self {
        let context = AppContext::new();
        let mut app = Self {
            context,
            models: Vec::new(),
            rows: Vec::new(),
            renders: Vec::new(),
            placed: Vec::new(),
            keyed_divs: false,
            builds: Rc::new(Cell::new(0)),
            title: Entity::new(Default::default()),
            in_scroll_view: false,
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
        };
        app.title = app.context.create(String::from("rows"));
        for _ in 0..count {
            let row = app.new_row();
            app.placed.push(row);
        }
        app
    }

    /// Make another row view, not placed yet; returns its creation index.
    fn new_row(&mut self) -> usize {
        let index = self.rows.len();
        let model = self.context.create(RowModel {
            label: format!("row {index}"),
            color: 0x223344 + index as u32,
        });
        let count = Rc::new(Cell::new(0));
        self.rows.push(self.context.new_view(RowView {
            model,
            renders: Rc::clone(&count),
            clicks: 0,
        }));
        self.models.push(model);
        self.renders.push(count);
        index
    }

    /// Place `placed` from the next frame on, rebuilding the root.
    fn place(&mut self, placed: Vec<usize>) {
        self.placed = placed;
        self.context.request_rebuild();
    }

    /// Whether row `index`'s view was dropped: its render counter is no
    /// longer shared with it.
    fn dropped(&self, index: usize) -> bool {
        Rc::strong_count(&self.renders[index]) == 1
    }

    fn build(&self) -> impl FnMut(&mut AppContext) -> Div + use<> {
        let rows: Vec<_> = self
            .placed
            .iter()
            .map(|index| (*index, self.rows[*index]))
            .collect();
        let (builds, title) = (Rc::clone(&self.builds), self.title);
        let (in_scroll_view, keyed_divs) = (self.in_scroll_view, self.keyed_divs);
        move |cx: &mut AppContext| {
            builds.set(builds.get() + 1);
            let heading = cx.get(title).map(|title| title.clone()).unwrap_or_default();
            let root = div().w(320.0).flex_col().child(text(heading));
            let rows = rows.iter().map(|(index, row)| {
                if keyed_divs {
                    rui::elements::element::AnyElement::new(
                        div().id(ElementId(*index as u64)).child(view(*row)),
                    )
                } else {
                    rui::elements::element::AnyElement::new(view(*row))
                }
            });
            if in_scroll_view {
                root.child(scroll_view().h(400.0).children(rows))
            } else {
                root.children(rows)
            }
        }
    }

    fn frame(&mut self) {
        let mut build = self.build();
        if let Err(err) = FramePipeline::build_frame_retained(
            &mut self.context,
            &mut self.root,
            &mut build,
            &mut self.taffy,
            &mut self.nodes,
            &mut self.scene,
            &mut self.measurer,
            VIEWPORT,
        ) {
            panic!("frame failed: {err}");
        }
    }

    fn renders(&self) -> Vec<u32> {
        self.renders.iter().map(|count| count.get()).collect()
    }

    fn set_label(&mut self, row: usize, label: &str) {
        let model = self.models[row];
        match self.context.get_mut(model) {
            Some(mut model) => model.label = label.to_owned(),
            None => panic!("row {row} has no model"),
        }
        self.context.notify(model.id());
    }

    /// The same frame built from scratch, as primitive descriptions.
    fn full_frame(&mut self) -> Vec<String> {
        let mut root = div();
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut nodes = LayoutNodes::new();
        let mut scene = Scene::new();
        self.context.request_rebuild();
        let mut build = self.build();
        // A fresh tree renders every view it places.
        if let Err(err) = FramePipeline::build_frame_retained(
            &mut self.context,
            &mut root,
            &mut build,
            &mut taffy,
            &mut nodes,
            &mut scene,
            &mut self.measurer,
            VIEWPORT,
        ) {
            panic!("full frame failed: {err}");
        }
        describe(scene.primitives())
    }
}

fn describe(primitives: &[Primitive]) -> Vec<String> {
    let viewport = Bounds::from_xywh(0.0, 0.0, VIEWPORT.width, VIEWPORT.height);
    primitives
        .iter()
        .filter(|primitive| primitive.may_touch(&viewport))
        .map(|primitive| format!("{primitive:?}"))
        .collect()
}

#[test]
fn a_notified_model_renders_only_the_view_that_read_it() {
    let mut app = App::new(30);
    app.frame();
    assert_eq!(app.builds.get(), 1);
    assert_eq!(app.renders(), vec![1; 30]);

    app.set_label(7, "seven");
    app.frame();
    assert_eq!(app.builds.get(), 1, "the root is not built again");
    let mut expected = vec![1; 30];
    expected[7] = 2;
    assert_eq!(app.renders(), expected);
    assert!(
        format!("{:?}", app.scene.primitives()).contains("seven (0 clicks)"),
        "the row shows its new label"
    );

    // Nothing notified: nothing renders.
    app.context.request_redraw();
    app.frame();
    assert_eq!(app.builds.get(), 1);
    assert_eq!(app.renders(), expected);
}

#[test]
fn a_view_inside_a_scroll_view_renders_again_when_notified() {
    let mut app = App::new(30);
    app.in_scroll_view = true;
    app.frame();
    assert_eq!(app.renders(), vec![1; 30]);

    app.set_label(3, "three");
    app.frame();
    assert_eq!(app.builds.get(), 1, "the root is not built again");
    let mut expected = vec![1; 30];
    expected[3] = 2;
    assert_eq!(app.renders(), expected);
    assert!(
        format!("{:?}", app.scene.primitives()).contains("three (0 clicks)"),
        "the row inside the scroll view shows its new label"
    );
}

#[test]
fn an_entity_the_root_read_rebuilds_the_root_and_keeps_the_views() {
    let mut app = App::new(10);
    app.frame();
    match app.context.get_mut(app.title) {
        Some(mut title) => *title = String::from("renamed"),
        None => panic!("no title"),
    }
    app.context.notify(app.title.id());
    app.frame();
    assert_eq!(app.builds.get(), 2, "the root read the title");
    assert_eq!(app.renders(), vec![1; 10], "views are kept, not rendered");
    assert!(format!("{:?}", app.scene.primitives()).contains("renamed"));
}

#[test]
fn an_entity_nothing_read_rebuilds_the_root_as_before() {
    let mut app = App::new(5);
    app.frame();
    let unread = app.context.create(0u32);
    app.context.notify(unread.id());
    app.frame();
    assert_eq!(app.builds.get(), 2);
    assert_eq!(app.renders(), vec![1; 5]);
}

#[test]
fn a_listener_renders_only_its_own_view() {
    let mut app = App::new(10);
    app.frame();
    // Row 4 sits under the 18 px heading line.
    let target = 4;
    let position = rui::Point::new(10.0, 20.0 * target as f32 + 30.0);
    let mut presenter_events = |kind| {
        let event = rui::elements::element::PointerEvent {
            kind,
            position,
            button: Some(rui::core::event::MouseButton::Left),
        };
        let bounds = Bounds::from_xywh(0.0, 0.0, VIEWPORT.width, VIEWPORT.height);
        let mut focused = None;
        let mut cx = rui::elements::element::EventContext::new(bounds, &app.taffy, &mut focused);
        rui::Element::handle_pointer_event(&mut app.root, &mut cx, &event);
    };
    presenter_events(rui::elements::element::PointerEventKind::Down);
    presenter_events(rui::elements::element::PointerEventKind::Up);
    app.frame();
    let renders = app.renders();
    let clicked: Vec<usize> = (0..10).filter(|row| renders[*row] == 2).collect();
    assert_eq!(clicked.len(), 1, "one row rendered again: {renders:?}");
    assert_eq!(app.builds.get(), 1);
    assert!(format!("{:?}", app.scene.primitives()).contains("(1 clicks)"));
}

/// xorshift64*: deterministic without a dependency.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) % n
    }
}

#[test]
fn view_frames_match_full_frames_over_random_model_edits() {
    for seed in 1..=8u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut app = App::new(60);
        app.frame();
        for frame in 0..20 {
            for _ in 0..1 + rng.below(3) {
                let row = rng.below(60) as usize;
                let model = app.models[row];
                if let Some(mut model) = app.context.get_mut(model) {
                    match rng.below(3) {
                        0 => model.color = rng.below(0xff_ffff) as u32,
                        1 => model.label = format!("row {row} edit {frame}"),
                        _ => model.label.push_str(" and a longer tail"),
                    }
                }
                app.context.notify(model.id());
            }
            if rng.below(6) == 0 {
                if let Some(mut title) = app.context.get_mut(app.title) {
                    title.push('!');
                }
                app.context.notify(app.title.id());
            }
            app.frame();
            let kept = describe(app.scene.primitives());
            let full = app.full_frame();
            assert_eq!(kept, full, "seed {seed}, frame {frame}");
        }
    }
}

/// Every row's render count is still `count`, for the rows in `rows`.
fn assert_renders(app: &App, rows: impl IntoIterator<Item = usize>, count: u32, what: &str) {
    for row in rows {
        assert_eq!(app.renders[row].get(), count, "row {row}: {what}");
    }
}

#[test]
fn reordering_views_renders_none_of_them() {
    for keyed_divs in [false, true] {
        let mut app = App::new(30);
        app.keyed_divs = keyed_divs;
        app.frame();
        let mut order: Vec<usize> = (0..30).rev().collect();
        order.rotate_left(7);
        order.swap(3, 20);
        app.place(order);
        app.frame();
        assert_eq!(app.builds.get(), 2);
        assert_renders(&app, 0..30, 1, "moved, not rendered");
        let kept = describe(app.scene.primitives());
        assert_eq!(kept, app.full_frame(), "keyed divs: {keyed_divs}");
    }
}

#[test]
fn a_view_no_tree_places_is_released() {
    let mut app = App::new(10);
    app.frame();
    assert_eq!(app.context.child_view_count(), 10);
    app.place(vec![0, 1, 2, 4, 5, 6, 7, 8, 9]);
    app.frame();
    assert!(app.dropped(3), "the view and its state are dropped");
    assert!(!app.dropped(4));
    assert_eq!(app.context.child_view_count(), 9);
    // Its model no longer renders anything.
    app.set_label(3, "gone");
    app.frame();
    assert_renders(
        &app,
        (0..10).filter(|row| *row != 3),
        1,
        "nothing reads the model",
    );
    assert!(!format!("{:?}", app.scene.primitives()).contains("gone"));
}

#[test]
fn a_view_not_placed_yet_stays() {
    let mut app = App::new(3);
    let later = app.new_row();
    app.frame();
    app.frame();
    assert!(!app.dropped(later), "never placed, so never released");
    app.place(vec![0, 1, 2, later]);
    app.frame();
    assert_eq!(app.renders[later].get(), 1);
}

#[test]
fn a_released_view_drops_its_queued_listener_updates() {
    let mut app = App::new(10);
    app.frame();
    // Clicks on rows 4 and 5 queue an update for each and notify them.
    for row in [4.0, 5.0] {
        let position = rui::Point::new(10.0, 20.0 * row + 30.0);
        for kind in [
            rui::elements::element::PointerEventKind::Down,
            rui::elements::element::PointerEventKind::Up,
        ] {
            let event = rui::elements::element::PointerEvent {
                kind,
                position,
                button: Some(rui::core::event::MouseButton::Left),
            };
            let bounds = Bounds::from_xywh(0.0, 0.0, VIEWPORT.width, VIEWPORT.height);
            let mut focused = None;
            let mut cx =
                rui::elements::element::EventContext::new(bounds, &app.taffy, &mut focused);
            rui::Element::handle_pointer_event(&mut app.root, &mut cx, &event);
        }
    }
    assert!(app.context.release(app.rows[4]));
    app.place((0..10).filter(|row| *row != 4).collect());
    app.frame();
    assert!(app.dropped(4));
    assert_eq!(app.renders[4].get(), 1, "the queued update never ran");
    assert_eq!(app.renders[5].get(), 2, "the row kept ran its update");
    assert_eq!(app.context.child_view_count(), 9);
}

#[test]
fn row_views_churned_every_frame_keep_the_footprint_flat() {
    let mut app = App::new(40);
    app.frame();
    for frame in 0..200 {
        // The oldest five rows go, five new ones come.
        let mut placed = app.placed.split_off(5);
        for _ in 0..5 {
            placed.push(app.new_row());
        }
        app.place(placed);
        app.frame();
        assert_eq!(app.context.child_view_count(), 40, "frame {frame}");
        let gone = app.rows.len() - 40;
        assert!((0..gone).all(|row| app.dropped(row)), "frame {frame}");
        assert_renders(&app, app.placed.clone(), 1, "kept rows do not render again");
    }
    assert_eq!(describe(app.scene.primitives()), app.full_frame());
}

#[test]
fn view_frames_match_full_frames_over_random_reorders_and_churn() {
    for seed in 1..=8u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut app = App::new(40);
        app.keyed_divs = seed % 2 == 0;
        app.frame();
        let mut ever_placed = vec![true; 40];
        for frame in 0..20 {
            let mut placed = app.placed.clone();
            for _ in 0..1 + rng.below(4) {
                let len = placed.len() as u64;
                match rng.below(5) {
                    0 => placed.swap(rng.below(len) as usize, rng.below(len) as usize),
                    1 if len > 1 => {
                        placed.remove(rng.below(len) as usize);
                    }
                    2 => {
                        let row = app.new_row();
                        placed.insert(rng.below(len + 1) as usize, row);
                    }
                    3 => {
                        let start = rng.below(len) as usize;
                        let end = start + rng.below(len - start as u64) as usize;
                        placed[start..=end].reverse();
                    }
                    _ => {
                        let row = placed[rng.below(len) as usize];
                        let model = app.models[row];
                        if let Some(mut model) = app.context.get_mut(model) {
                            model.label = format!("row {row} edit {frame}");
                        }
                        app.context.notify(model.id());
                    }
                }
            }
            let moved = placed != app.placed;
            if moved {
                app.place(placed);
            }
            app.frame();
            let kept = describe(app.scene.primitives());
            let full = app.full_frame();
            assert_eq!(kept, full, "seed {seed}, frame {frame}");
            ever_placed.resize(app.rows.len(), false);
            for row in app.placed.iter() {
                ever_placed[*row] = true;
            }
            for (row, ever) in ever_placed.iter().enumerate() {
                // Rows made and removed before a frame placed them are kept.
                let released = *ever && !app.placed.contains(&row);
                assert_eq!(
                    app.dropped(row),
                    released,
                    "seed {seed}, frame {frame}, row {row}"
                );
            }
        }
    }
}
