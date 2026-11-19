use rui::elements::{ScrollbarVisibility, VirtualListHandle, virtual_list};
use rui::renderer::Primitive;
use rui::{AppContext, Bounds, Color, ElementId, Size, div, text};
use std::cell::Cell;

use super::compare_terminal::terminal_rows;
use super::workbench_data::*;
use super::{Harness, gpu_time, measure};

enum Panel {
    Sidebar {
        scroll: VirtualListHandle,
        renders: std::rc::Rc<Cell<usize>>,
    },
    Terminal(usize),
}

pub(super) fn run(case: &str, full: bool) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = Size::new(WIDTH, HEIGHT);
    let mut harness = Harness::new()?;
    let mut context = AppContext::new();
    let renders = std::rc::Rc::new(Cell::new(0));
    let sidebar = context.create(Panel::Sidebar {
        scroll: VirtualListHandle::new(),
        renders: renders.clone(),
    });
    let terminal = context.create(Panel::Terminal(0));
    let panels = [sidebar, terminal];
    let mut build = |context: &mut AppContext| {
        div()
            .w(WIDTH)
            .h(HEIGHT)
            .flex_row()
            .children(context.rows(&panels, |panel, _| {
                match panel {
                    Panel::Sidebar { scroll, renders } => {
                        renders.set(renders.get() + 1);
                        div().w(SIDEBAR_WIDTH).h(HEIGHT).child(
                            virtual_list(scroll, SESSIONS, ROW_HEIGHT, |index| {
                                div()
                                    .w(SIDEBAR_WIDTH)
                                    .h(ROW_HEIGHT)
                                    .bg(Color::hex(SIDEBAR_COLORS[index % SIDEBAR_COLORS.len()]))
                                    .child(
                                        text(session_text(index))
                                            .font("SF Mono")
                                            .size(FONT_SIZE)
                                            .line_height(ROW_HEIGHT)
                                            .color(Color::hex(TEXT_COLOR)),
                                    )
                            })
                            .id(ElementId(10))
                            .w(SIDEBAR_WIDTH)
                            .h(HEIGHT)
                            .scrollbar(ScrollbarVisibility::Never),
                        )
                    }
                    Panel::Terminal(turn) => div()
                        .w(PANE_WIDTH)
                        .h(HEIGHT)
                        .child(terminal_rows(*turn, PANE_WIDTH)),
                }
            }))
    };
    let mut root = div();
    harness.app_frame(&mut context, &mut root, &mut build, viewport)?;
    let initial_renders = renders.get();
    assert!(initial_renders > 0, "the sidebar renders at startup");
    measure(case, &mut harness, |harness, turn| {
        if let Panel::Terminal(turn_state) = &mut *context.get_mut(terminal).expect("terminal") {
            *turn_state = turn + 1;
        }
        context.notify(terminal.id());
        if full {
            context.notify(sidebar.id());
            context.request_rebuild();
        }
        harness.app_frame(&mut context, &mut root, &mut build, viewport)
    })?;

    let turns = gpu_time::WARMUP_FRAMES + gpu_time::sample_count();
    assert_eq!(
        renders.get() - initial_renders,
        if full { turns } else { 0 }
    );
    let primitives = harness.scene.primitives();
    let sidebar_colors = SIDEBAR_COLORS.map(|color| Color::hex(color).to_rgba());
    let sidebar_rows: Vec<_> = primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad {
                bounds, background, ..
            } if sidebar_colors.contains(background) => Some(*bounds),
            _ => None,
        })
        .collect();
    assert_eq!(sidebar_rows.len(), VISIBLE_ROWS);
    for (row, bounds) in sidebar_rows.iter().enumerate() {
        assert_eq!(
            *bounds,
            Bounds::from_xywh(0.0, row as f32 * ROW_HEIGHT, SIDEBAR_WIDTH, ROW_HEIGHT)
        );
    }

    assert!(primitives.iter().any(|primitive| matches!(primitive,
        Primitive::Quad { bounds, background, .. }
        if *bounds == Bounds::from_xywh(SIDEBAR_WIDTH, 0.0, PANE_WIDTH, HEIGHT)
            && *background == Color::hex(PANE_BG).to_rgba())));
    let terminal_rows: Vec<_> = primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text {
                bounds, content, ..
            } if bounds.x() == SIDEBAR_WIDTH => Some(content),
            _ => None,
        })
        .collect();
    assert_eq!(terminal_rows.len(), VISIBLE_ROWS);
    for (row, actual) in terminal_rows.into_iter().enumerate() {
        assert_eq!(actual.as_ref(), terminal_text(row, turns));
    }
    println!(
        "rui {case} sidebar_rows={VISIBLE_ROWS} terminal_rows={VISIBLE_ROWS} terminal_turn={turns} sidebar_renders={} viewport_width={WIDTH} viewport_height={HEIGHT}",
        renders.get() - initial_renders,
    );
    Ok(())
}
