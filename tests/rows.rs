//! `AppContext::rows`: a view per entity, made on first use, rendered again only
//! when its entity is notified, and released once no tree places it.

use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::Div;
use rui::elements::element::LayoutNodes;
use rui::renderer::Scene;
use rui::renderer::text::TextMeasureCache;
use rui::{AppContext, ElementId, Size, div, text};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use taffy::prelude::TaffyTree;

const VIEWPORT: Size = Size {
    width: 320.0,
    height: 900.0,
};

struct Task {
    title: String,
}

struct Harness {
    context: AppContext,
    tasks: Vec<Entity<Task>>,
    /// The tasks the root places, in order.
    placed: Rc<RefCell<Vec<Entity<Task>>>>,
    /// Put before every title by the row closure of the next root build.
    prefix: Rc<RefCell<String>>,
    /// Row renders, by task title as first created.
    renders: Rc<RefCell<HashMap<String, u32>>>,
    builds: Rc<Cell<u32>>,
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
}

impl Harness {
    fn new(count: usize) -> Self {
        let mut context = AppContext::new();
        let tasks: Vec<_> = (0..count)
            .map(|index| {
                context.create(Task {
                    title: format!("task {index}"),
                })
            })
            .collect();
        Self {
            context,
            placed: Rc::new(RefCell::new(tasks.clone())),
            tasks,
            prefix: Rc::new(RefCell::new(String::new())),
            renders: Rc::default(),
            builds: Rc::new(Cell::new(0)),
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
        }
    }

    fn frame(&mut self) {
        let (placed, builds) = (Rc::clone(&self.placed), Rc::clone(&self.builds));
        let (prefix, renders) = (Rc::clone(&self.prefix), Rc::clone(&self.renders));
        let mut build = move |cx: &mut AppContext| {
            builds.set(builds.get() + 1);
            let prefix = prefix.borrow().clone();
            let renders = Rc::clone(&renders);
            let rows = cx.rows(&placed.borrow(), move |task: &Task, _| {
                *renders.borrow_mut().entry(task.title.clone()).or_default() += 1;
                div().h(20.0).child(text(format!("{prefix}{}", task.title)))
            });
            div().w(320.0).flex_col().children(rows)
        };
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

    fn rename(&mut self, index: usize, title: &str) {
        let task = self.tasks[index];
        match self.context.get_mut(task) {
            Some(mut task) => task.title = title.to_owned(),
            None => panic!("task {index} is gone"),
        }
        self.context.notify(task.id());
    }

    fn renders_of(&self, title: &str) -> u32 {
        self.renders.borrow().get(title).copied().unwrap_or(0)
    }

    fn shows(&self, text: &str) -> bool {
        format!("{:?}", self.scene.primitives()).contains(text)
    }

    fn place(&mut self, indices: &[usize]) {
        *self.placed.borrow_mut() = indices.iter().map(|index| self.tasks[*index]).collect();
        self.context.request_rebuild();
    }
}

#[test]
fn notifying_an_entity_renders_only_its_row() {
    let mut harness = Harness::new(20);
    harness.frame();
    assert_eq!(harness.context.child_view_count(), 20);
    assert!(harness.shows("task 7"));

    harness.rename(7, "renamed");
    harness.frame();
    assert_eq!(harness.builds.get(), 1, "the root is not rebuilt");
    assert_eq!(harness.renders_of("renamed"), 1);
    for index in (0..20).filter(|index| *index != 7) {
        assert_eq!(
            harness.renders_of(&format!("task {index}")),
            1,
            "row {index}"
        );
    }
    assert!(harness.shows("renamed"));
    assert!(!harness.shows("task 7"));
}

#[test]
fn a_rebuilt_root_keeps_its_rows() {
    let mut harness = Harness::new(10);
    harness.frame();
    harness.place(&[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    harness.frame();
    assert_eq!(harness.builds.get(), 2);
    assert_eq!(harness.context.child_view_count(), 10, "no row made twice");
    for index in 0..10 {
        assert_eq!(
            harness.renders_of(&format!("task {index}")),
            1,
            "row {index}"
        );
    }
}

#[test]
fn a_row_no_tree_places_is_released_and_made_again_when_shown() {
    let mut harness = Harness::new(5);
    harness.frame();
    harness.place(&[0, 1, 3, 4]);
    harness.frame();
    assert_eq!(harness.context.child_view_count(), 4);
    assert!(!harness.shows("task 2"));

    harness.place(&[0, 1, 2, 3, 4]);
    harness.frame();
    assert_eq!(harness.context.child_view_count(), 5);
    assert_eq!(harness.renders_of("task 2"), 2, "a new row renders it");
    assert!(harness.shows("task 2"));
}

#[test]
fn a_row_renders_with_the_newest_closure() {
    let mut harness = Harness::new(3);
    harness.frame();
    *harness.prefix.borrow_mut() = String::from("> ");
    harness.context.request_rebuild();
    harness.frame();
    assert!(
        harness.shows("task 1"),
        "a new closure renders nothing again"
    );
    assert!(!harness.shows("> task 1"));

    harness.rename(1, "edited");
    harness.frame();
    assert!(harness.shows("> edited"));
}

#[test]
fn each_call_site_has_its_own_rows() {
    let mut context = AppContext::new();
    let task = context.create(Task {
        title: String::from("shared"),
    });
    let first = context.rows(&[task], |task: &Task, _| text(task.title.clone()));
    let second = context.rows(&[task], |task: &Task, _| text(format!("{}!", task.title)));
    assert_eq!(context.child_view_count(), 2);
    drop((first, second));
}

#[test]
#[should_panic(expected = "which was released")]
fn a_row_of_a_released_entity_fails_to_render() {
    let mut harness = Harness::new(2);
    harness.frame();
    let task = harness.tasks[1];
    harness.context.release(task);
    harness.context.notify(task.id());
    harness.frame();
}
