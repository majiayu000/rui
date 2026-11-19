//! `views.churn`: row views whose oldest rows are replaced by new ones each
//! frame, so released views must not keep their memory.

use super::{CHURN_PER_FRAME, CHURN_ROWS, ListRow, Surfaces, VIEWPORT};
use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::view;
use rui::renderer::text::TextMeasureCache;
use rui::{AppContext, Div, Size, div};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// Row views with their label entities, oldest first.
type ChurnRows = Rc<RefCell<VecDeque<(Entity<ListRow>, Entity<String>)>>>;

/// A list of row views whose oldest rows are replaced by new ones each frame.
pub(super) struct Churn {
    rows: ChurnRows,
    made: usize,
    root: Div,
    surfaces: Surfaces,
}

impl Churn {
    pub(super) fn new(context: &mut AppContext) -> Self {
        let mut churn = Self {
            rows: Rc::default(),
            made: 0,
            root: div(),
            surfaces: Surfaces::new(),
        };
        for _ in 0..CHURN_ROWS {
            churn.push_row(context);
        }
        churn
    }

    fn push_row(&mut self, context: &mut AppContext) {
        // Labels repeat, so the text caches stay warm and the same size.
        let label = context.create(format!("row {}", self.made % CHURN_ROWS));
        let row = context.new_view(ListRow {
            index: self.made,
            label,
        });
        self.rows.borrow_mut().push_back((row, label));
        self.made += 1;
    }

    pub(super) fn frame(&mut self, context: &mut AppContext, measurer: &mut TextMeasureCache) {
        for _ in 0..CHURN_PER_FRAME {
            let (_, label) = self.rows.borrow_mut().pop_front().expect("a row");
            // The view goes once no tree places it; its label is ours to release.
            context.release(label);
            self.push_row(context);
        }
        context.request_rebuild();
        let rows = Rc::clone(&self.rows);
        let mut build = move |_: &mut AppContext| {
            div()
                .w_full()
                .flex_col()
                .children(rows.borrow().iter().map(|(row, _)| view(*row)))
        };
        FramePipeline::build_frame_retained(
            context,
            &mut self.root,
            &mut build,
            &mut self.surfaces.taffy,
            &mut self.surfaces.nodes,
            &mut self.surfaces.scene,
            measurer,
            Size::new(VIEWPORT.0, VIEWPORT.1),
        )
        .expect("churn frame");
    }
}
