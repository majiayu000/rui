//! `images.grid_rebuilt`: a grid of decoded images built again around one
//! changed caption. The images are found in the app's cache and kept, not
//! painted again.

use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::renderer::Scene;
use rui::renderer::text::TextMeasureCache;
use rui::testing::{HeadlessApp, headless_app};
use rui::{AppContext, Div, ElementId, ImageAsset, Size, div, text};
use std::io::Cursor;
use taffy::prelude::TaffyTree;

const ASSETS: usize = 50;
/// 400 images.
const ROWS: usize = 20;
const PER_ROW: usize = 20;

pub struct ImageGrid {
    app: HeadlessApp,
    caption: Entity<usize>,
    assets: Vec<ImageAsset>,
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    viewport: Size,
}

impl ImageGrid {
    /// Built and drawn with every image decoded.
    pub fn new(viewport: Size, measurer: &mut TextMeasureCache) -> Self {
        let mut app = headless_app();
        let caption = app.app_context_mut().create(0);
        let assets = (0..ASSETS)
            .map(|index| ImageAsset::Bytes(png(8 + index as u32 % 8, 8).into()))
            .collect();
        let mut grid = Self {
            app,
            caption,
            assets,
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            viewport,
        };
        grid.draw(measurer);
        grid.app.run_until_parked();
        grid.draw(measurer);
        grid
    }

    /// Change the caption to `round` and draw the frame that follows.
    pub fn frame(&mut self, round: usize, measurer: &mut TextMeasureCache) {
        let context = self.app.app_context_mut();
        *context.get_mut(self.caption).expect("caption") = round;
        context.notify(self.caption.id());
        self.draw(measurer);
    }

    fn draw(&mut self, measurer: &mut TextMeasureCache) {
        let (caption, assets) = (self.caption, &self.assets);
        let mut build = |cx: &mut AppContext| {
            let label = format!("frame {}", *cx.get(caption).expect("caption"));
            let rows: Vec<Div> = (0..ROWS)
                .map(|row| {
                    let images: Vec<_> = (0..PER_ROW)
                        .map(|column| {
                            let index = row * PER_ROW + column;
                            cx.image(assets[index % assets.len()].clone())
                                .cover()
                                .size(Size::new(32.0, 32.0))
                        })
                        .collect();
                    div().flex_row().children(images)
                })
                .collect();
            div().w_full().flex_col().child(text(label)).children(rows)
        };
        FramePipeline::build_frame_retained(
            self.app.app_context_mut(),
            &mut self.root,
            &mut build,
            &mut self.taffy,
            &mut self.nodes,
            &mut self.scene,
            measurer,
            self.viewport,
        )
        .expect("image grid frame");
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let pixels = (0..width * height)
        .flat_map(|index| [(index * 29) as u8, (index * 7) as u8, 0x90, 0xff])
        .collect();
    let image = image::RgbaImage::from_raw(width, height, pixels).expect("pixels fit");
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}
