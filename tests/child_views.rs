//! Views placed in a tree render again only when they, or an entity their last
//! render read, are notified; the tree around them is not rebuilt for that.

use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{Div, view};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Primitive, Scene};
use rui::{AppContext, Bounds, Color, ElementId, Size, View, ViewContext, div, text};
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
    models: Vec<Entity<RowModel>>,
    rows: Vec<Entity<RowView>>,
    renders: Vec<Rc<Cell<u32>>>,
    builds: Rc<Cell<u32>>,
    /// Read by the root, so notifying it rebuilds everything.
    title: Entity<String>,
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
}

impl App {
    fn new(count: usize) -> Self {
        let mut context = AppContext::new();
        let mut models = Vec::new();
        let mut rows = Vec::new();
        let mut renders = Vec::new();
        for index in 0..count {
            let model = context.create(RowModel {
                label: format!("row {index}"),
                color: 0x223344 + index as u32,
            });
            let count = Rc::new(Cell::new(0));
            rows.push(context.new_view(RowView {
                model,
                renders: Rc::clone(&count),
                clicks: 0,
            }));
            models.push(model);
            renders.push(count);
        }
        let title = context.create(String::from("rows"));
        Self {
            context,
            models,
            rows,
            renders,
            builds: Rc::new(Cell::new(0)),
            title,
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
        }
    }

    fn build(&self) -> impl FnMut(&mut AppContext) -> Div + use<> {
        let (rows, builds, title) = (self.rows.clone(), Rc::clone(&self.builds), self.title);
        move |cx: &mut AppContext| {
            builds.set(builds.get() + 1);
            let heading = cx.get(title).map(|title| title.clone()).unwrap_or_default();
            div()
                .w(320.0)
                .flex_col()
                .child(text(heading))
                .children(rows.iter().map(|row| view(*row)))
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
