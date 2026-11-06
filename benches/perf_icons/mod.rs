//! `icons.grid_rebuilt`: a grid of SVG icons built again around one changed
//! caption. Each icon's markup is parsed and shaped once; the grid keeps them
//! and paints none again.

use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::renderer::Scene;
use rui::renderer::text::TextMeasureCache;
use rui::{AppContext, Color, Div, ElementId, Size, div, svg, text};
use taffy::prelude::TaffyTree;

/// 400 icons.
const ROWS: usize = 20;
const PER_ROW: usize = 20;

const ICONS: [&str; 4] = [
    r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14M12 5v14"/></svg>"#,
    r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></svg>"#,
    r#"<svg viewBox="0 0 24 24"><path d="M12 2l3 7h7l-5.5 4.5 2 7.5-6.5-4.5-6.5 4.5 2-7.5L2 9h7z"/></svg>"#,
    r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>"#,
];

pub struct IconGrid {
    app: AppContext,
    caption: Entity<usize>,
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    viewport: Size,
}

impl IconGrid {
    pub fn new(viewport: Size, measurer: &mut TextMeasureCache) -> Self {
        let mut app = AppContext::new();
        let caption = app.create(0);
        let mut grid = Self {
            app,
            caption,
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            viewport,
        };
        grid.draw(measurer);
        grid
    }

    /// Change the caption to `round` and draw the frame that follows.
    pub fn frame(&mut self, round: usize, measurer: &mut TextMeasureCache) {
        *self.app.get_mut(self.caption).expect("caption") = round;
        self.app.notify(self.caption.id());
        self.draw(measurer);
    }

    fn draw(&mut self, measurer: &mut TextMeasureCache) {
        let caption = self.caption;
        let mut build = |cx: &mut AppContext| {
            let label = format!("frame {}", *cx.get(caption).expect("caption"));
            let rows = (0..ROWS).map(|row| {
                div().flex_row().children((0..PER_ROW).map(move |column| {
                    svg(ICONS[(row + column) % ICONS.len()])
                        .size(Size::new(24.0, 24.0))
                        .color(Color::hex(0x334455))
                }))
            });
            div().w_full().flex_col().child(text(label)).children(rows)
        };
        FramePipeline::build_frame_retained(
            &mut self.app,
            &mut self.root,
            &mut build,
            &mut self.taffy,
            &mut self.nodes,
            &mut self.scene,
            measurer,
            self.viewport,
        )
        .expect("icon grid frame");
    }
}
