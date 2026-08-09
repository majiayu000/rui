use super::config::{
    EVENT_BENCHMARK_ID, LAYOUT_BENCHMARK_ID, RENDERER_BENCHMARK_ID, SCENE_BENCHMARK_ID,
    TEXT_BENCHMARK_ID, TEXT_MULTI_FRAME_BENCHMARK_ID,
};
use super::measure::{BenchCase, BenchError};
use rui::core::event::MouseButton;
use rui::elements::element::{
    Element, EventContext, LayoutContext, PointerEvent, PointerEventKind,
};
use rui::renderer::text::{TextMeasureCache, TextRasterCache, TextRequest};
use rui::renderer::{Primitive, RecordingRenderer, Renderer, Scene};
use rui::{Bounds, Color, ElementId, Point, Size, div, text};
use std::cell::Cell;
use std::hint::black_box;
use std::rc::Rc;
use std::time::Instant;
use taffy::prelude::{AvailableSpace, TaffyTree};

const FRAME_ITERATIONS: usize = 64;
const DISPATCH_ITERATIONS: usize = 2_000;
const LAYOUT_ROWS: usize = 12;
const LAYOUT_COLUMNS: usize = 16;
const SCENE_PRIMITIVES: usize = 384;
const TEXT_ITEMS: [&str; 8] = [
    "RUI runtime baseline",
    "layout measurement",
    "text raster cache",
    "scene graph build",
    "pointer dispatch",
    "renderer throughput",
    "resource diagnostics",
    "performance threshold",
];

pub fn runtime_cases() -> Vec<BenchCase> {
    vec![
        BenchCase {
            id: LAYOUT_BENCHMARK_ID,
            category: "layout",
            unit: "ns_per_frame",
            run: bench_layout_flex_tree,
        },
        BenchCase {
            id: TEXT_BENCHMARK_ID,
            category: "text",
            unit: "ns_per_frame",
            run: bench_text_measure_raster,
        },
        BenchCase {
            id: TEXT_MULTI_FRAME_BENCHMARK_ID,
            category: "text",
            unit: "ns_per_frame",
            run: bench_text_multi_frame_layout,
        },
        BenchCase {
            id: SCENE_BENCHMARK_ID,
            category: "scene",
            unit: "ns_per_frame",
            run: bench_scene_build,
        },
        BenchCase {
            id: EVENT_BENCHMARK_ID,
            category: "event",
            unit: "ns_per_dispatch",
            run: bench_event_pointer_dispatch,
        },
        BenchCase {
            id: RENDERER_BENCHMARK_ID,
            category: "renderer",
            unit: "ns_per_frame",
            run: bench_recording_renderer,
        },
    ]
}

fn bench_layout_flex_tree() -> Result<f64, BenchError> {
    let viewport = Size::new(960.0, 640.0);
    let start = Instant::now();
    let mut checksum = 0.0f32;

    for frame in 0..FRAME_ITERATIONS {
        let mut root = layout_tree(frame);
        let mut taffy = TaffyTree::<ElementId>::new();
        let root_node = {
            let mut cx = LayoutContext::new(&mut taffy, viewport);
            root.layout(&mut cx)
        };
        taffy
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
            )
            .map_err(|err| BenchError::new(format!("layout benchmark failed: {err}")))?;
        let layout = taffy
            .layout(root_node)
            .map_err(|err| BenchError::new(format!("layout lookup failed: {err}")))?;
        checksum += layout.size.width + layout.size.height;
    }

    black_box(checksum);
    Ok(start.elapsed().as_nanos() as f64 / FRAME_ITERATIONS as f64)
}

fn layout_tree(frame: usize) -> rui::Div {
    let mut root = div().w(960.0).h(640.0).flex_col().gap(4.0).p(8.0);
    for row in 0..LAYOUT_ROWS {
        let mut row_node = div().h(28.0).flex_row().gap(3.0);
        for column in 0..LAYOUT_COLUMNS {
            let width = 20.0 + ((row + column + frame) % 7) as f32;
            let color = Color::rgb(0.2 + row as f32 * 0.01, 0.3 + column as f32 * 0.01, 0.5);
            row_node = row_node.child(
                div()
                    .w(width)
                    .h(24.0)
                    .flex_shrink(0.0)
                    .rounded(3.0)
                    .bg(color),
            );
        }
        root = root.child(row_node);
    }
    root
}

fn bench_text_measure_raster() -> Result<f64, BenchError> {
    let mut measurer = TextMeasureCache::new();
    let mut raster = TextRasterCache::with_limits(512, 16 * 1024 * 1024);
    let start = Instant::now();
    let mut checksum = 0usize;

    for frame in 0..FRAME_ITERATIONS {
        raster.begin_frame();
        for (index, content) in TEXT_ITEMS.iter().enumerate() {
            let size = 13.0 + ((frame + index) % 5) as f32;
            let line_height = 1.1 + (index % 3) as f32 * 0.1;
            let request = TextRequest::new(content, size, 400, None, line_height);
            let metrics = measurer.measure_single_line(request)?;
            if let Some(entry) = raster.resolve(request)? {
                checksum = checksum.wrapping_add(entry.pixels.len());
            }
            checksum = checksum.wrapping_add(metrics.advance_width as usize);
        }
    }

    black_box(checksum);
    Ok(start.elapsed().as_nanos() as f64 / FRAME_ITERATIONS as f64)
}

/// Lays out a text-heavy tree over many frames while reusing one measurement
/// cache, the way a window's frame loop does. Guards the cross-frame caching in
/// `TextMeasureCache`: before it, every frame re-parsed the system fonts and
/// discarded the previous frame's metrics.
fn bench_text_multi_frame_layout() -> Result<f64, BenchError> {
    let viewport = Size::new(720.0, 480.0);
    let mut measurer = TextMeasureCache::new();
    let mut root = text_tree();
    let start = Instant::now();
    let mut checksum = 0.0f32;

    for _ in 0..FRAME_ITERATIONS {
        let mut taffy = TaffyTree::<ElementId>::new();
        let root_node = {
            let mut cx = LayoutContext::with_text_measurer(&mut taffy, viewport, &mut measurer);
            root.layout(&mut cx)
        };
        taffy
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
            )
            .map_err(|err| BenchError::new(format!("text layout benchmark failed: {err}")))?;
        let layout = taffy
            .layout(root_node)
            .map_err(|err| BenchError::new(format!("text layout lookup failed: {err}")))?;
        checksum += layout.size.width + layout.size.height;
    }

    black_box(checksum);
    Ok(start.elapsed().as_nanos() as f64 / FRAME_ITERATIONS as f64)
}

fn text_tree() -> rui::Div {
    let mut root = div().w(720.0).h(480.0).flex_col().gap(4.0).p(8.0);
    for (index, content) in TEXT_ITEMS.iter().enumerate() {
        let font_size = 13.0 + (index % 5) as f32;
        root = root.child(
            div()
                .flex_row()
                .gap(6.0)
                .child(text(*content).size(font_size))
                .child(text(*content).size(font_size)),
        );
    }
    root
}

fn bench_scene_build() -> Result<f64, BenchError> {
    let start = Instant::now();
    let mut checksum = 0usize;

    for frame in 0..FRAME_ITERATIONS {
        let mut scene = Scene::new();
        for index in 0..SCENE_PRIMITIVES {
            scene.insert(sample_quad(index, frame));
            if index % 8 == 0 {
                scene.register_hit_region(
                    ElementId::from(index as u64 + 1),
                    Bounds::from_xywh(index as f32, 0.0, 18.0, 18.0),
                );
            }
        }
        scene.finish();
        checksum = checksum.wrapping_add(scene.len());
        checksum = checksum.wrapping_add(scene.hit_test(Point::new(12.0, 4.0)).is_some() as usize);
    }

    black_box(checksum);
    Ok(start.elapsed().as_nanos() as f64 / FRAME_ITERATIONS as f64)
}

fn bench_event_pointer_dispatch() -> Result<f64, BenchError> {
    let click_count = Rc::new(Cell::new(0usize));
    let target = ElementId::from(192);
    let mut root = event_tree(Rc::clone(&click_count));
    let viewport = Size::new(384.0, 4.0);
    let mut taffy = TaffyTree::<ElementId>::new();
    let root_node = {
        let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
        root.layout(&mut layout_cx)
    };
    taffy
        .compute_layout(
            root_node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .map_err(|err| BenchError::new(format!("event layout failed: {err}")))?;

    let root_bounds = Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height);
    let event = PointerEvent {
        kind: PointerEventKind::Up,
        position: Point::new(191.5, 0.5),
        button: Some(MouseButton::Left),
    };
    let start = Instant::now();
    let mut focused = None;

    for _ in 0..DISPATCH_ITERATIONS {
        let mut cx = EventContext::new(root_bounds, &taffy, &mut focused);
        cx.set_hit_target(Some(target));
        root.dispatch_pointer_event(&mut cx, &event);
    }

    black_box(click_count.get());
    Ok(start.elapsed().as_nanos() as f64 / DISPATCH_ITERATIONS as f64)
}

fn event_tree(click_count: Rc<Cell<usize>>) -> rui::Div {
    let mut root = div().w(384.0).h(4.0).flex_row();
    for index in 0..384 {
        let counter = Rc::clone(&click_count);
        root = root.child(
            div()
                .id(ElementId::from(index as u64 + 1))
                .w(1.0)
                .h(4.0)
                .flex_shrink(0.0)
                .on_click(move || counter.set(counter.get().wrapping_add(1))),
        );
    }
    root
}

fn bench_recording_renderer() -> Result<f64, BenchError> {
    let scene = populated_scene();
    let mut renderer = RecordingRenderer::new();
    let viewport = Size::new(960.0, 640.0);
    let start = Instant::now();

    for _ in 0..FRAME_ITERATIONS {
        renderer.render(&scene, &(), viewport)?;
    }

    black_box(renderer.frames().len());
    Ok(start.elapsed().as_nanos() as f64 / FRAME_ITERATIONS as f64)
}

fn populated_scene() -> Scene {
    let mut scene = Scene::new();
    for index in 0..SCENE_PRIMITIVES {
        scene.insert(sample_quad(index, 0));
    }
    scene
}

fn sample_quad(index: usize, frame: usize) -> Primitive {
    let x = (index % 32) as f32 * 18.0;
    let y = (index / 32) as f32 * 18.0;
    Primitive::Quad {
        bounds: Bounds::from_xywh(x, y, 16.0, 16.0),
        background: Color::rgb(
            0.15 + (frame % 7) as f32 * 0.01,
            0.25 + (index % 11) as f32 * 0.01,
            0.45,
        )
        .to_rgba(),
        border_color: Color::TRANSPARENT.to_rgba(),
        border_widths: rui::Edges::ZERO,
        corner_radii: rui::Corners::all(2.0),
    }
}
