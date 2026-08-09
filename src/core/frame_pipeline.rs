use crate::core::ElementId;
use crate::core::app::AppContext;
use crate::core::geometry::{Bounds, Size};
use crate::elements::Element;
use crate::elements::element::{LayoutContext, PaintContext};
use crate::renderer::Scene;
use crate::renderer::text::TextMeasureCache;
use std::error::Error;
use std::fmt;
use taffy::prelude::{AvailableSpace, TaffyTree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramePipelineError {
    message: String,
}

impl FramePipelineError {
    fn layout(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FramePipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "frame pipeline failed: {}", self.message)
    }
}

impl Error for FramePipelineError {}

pub struct FramePipeline;

impl FramePipeline {
    pub fn prepare_frame(context: &mut AppContext) {
        if context.consume_runtime_view_notification() {
            context.request_redraw();
        }
    }

    pub fn rebuild_if_needed<E, F>(context: &mut AppContext, root: &mut E, build_root: &mut F)
    where
        E: Element,
        F: FnMut(&mut AppContext) -> E,
    {
        if context.needs_rebuild || !context.pending_updates.is_empty() {
            context.pending_updates.clear();
            context.needs_rebuild = false;
            *root = build_root(context);
            if !context.pending_updates.is_empty() {
                context.needs_rebuild = true;
            }
        }
    }

    pub fn layout_root<E>(
        root: &mut E,
        taffy: &mut TaffyTree<ElementId>,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        E: Element,
    {
        let mut text_measurer = TextMeasureCache::new();
        Self::layout_root_with_text_measurer(root, taffy, &mut text_measurer, viewport_size)
    }

    pub fn layout_root_with_text_measurer<E>(
        root: &mut E,
        taffy: &mut TaffyTree<ElementId>,
        text_measurer: &mut TextMeasureCache,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        E: Element,
    {
        taffy.clear();
        let mut layout_cx = LayoutContext::with_text_measurer(taffy, viewport_size, text_measurer);
        let root_node = root.layout(&mut layout_cx);

        taffy
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(viewport_size.width),
                    height: AvailableSpace::Definite(viewport_size.height),
                },
            )
            .map_err(|err| FramePipelineError::layout(err.to_string()))?;

        let layout = taffy
            .layout(root_node)
            .map_err(|err| FramePipelineError::layout(err.to_string()))?;
        Ok(Bounds::from_xywh(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        ))
    }

    pub fn paint_root<E>(
        root: &mut E,
        taffy: &TaffyTree<ElementId>,
        scene: &mut Scene,
        bounds: Bounds,
    ) where
        E: Element,
    {
        scene.clear();
        let mut paint_cx = PaintContext::new(scene, bounds, taffy);
        root.paint(&mut paint_cx);
        scene.finish();
    }

    pub fn finish_frame(context: &mut AppContext) {
        context.complete_redraw_frame();
    }

    pub fn build_frame<E, F>(
        context: &mut AppContext,
        root: &mut E,
        build_root: &mut F,
        taffy: &mut TaffyTree<ElementId>,
        scene: &mut Scene,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        E: Element,
        F: FnMut(&mut AppContext) -> E,
    {
        let mut text_measurer = TextMeasureCache::new();
        Self::build_frame_with_text_measurer(
            context,
            root,
            build_root,
            taffy,
            scene,
            &mut text_measurer,
            viewport_size,
        )
    }

    pub fn build_frame_with_text_measurer<E, F>(
        context: &mut AppContext,
        root: &mut E,
        build_root: &mut F,
        taffy: &mut TaffyTree<ElementId>,
        scene: &mut Scene,
        text_measurer: &mut TextMeasureCache,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        E: Element,
        F: FnMut(&mut AppContext) -> E,
    {
        Self::prepare_frame(context);
        Self::rebuild_if_needed(context, root, build_root);
        let bounds =
            Self::layout_root_with_text_measurer(root, taffy, text_measurer, viewport_size)?;
        Self::paint_root(root, taffy, scene, bounds);
        Self::finish_frame(context);
        Ok(bounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Presenter;
    use crate::elements::div;
    use crate::elements::text::text;

    #[test]
    fn text_metrics_survive_across_frames() {
        let viewport_size = Size::new(320.0, 240.0);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut text_measurer = TextMeasureCache::new();
        if !text_measurer.has_fonts() {
            let mut root = div().child(text("fontless layout must fail explicitly"));
            assert!(
                FramePipeline::layout_root_with_text_measurer(
                    &mut root,
                    &mut taffy,
                    &mut text_measurer,
                    viewport_size,
                )
                .is_err(),
                "a fontless environment must not report a successful text layout"
            );
            return;
        }
        let mut root = div().child(text("metrics survive the frame boundary"));

        match FramePipeline::layout_root_with_text_measurer(
            &mut root,
            &mut taffy,
            &mut text_measurer,
            viewport_size,
        ) {
            Ok(_) => {}
            Err(err) => panic!("first frame layout failed: {err}"),
        }
        let after_first_frame = text_measurer.cached_metrics_len();
        assert!(
            after_first_frame > 0,
            "the first frame should populate the metrics cache"
        );

        let hits_before_second_frame = text_measurer.metric_hits();
        match FramePipeline::layout_root_with_text_measurer(
            &mut root,
            &mut taffy,
            &mut text_measurer,
            viewport_size,
        ) {
            Ok(_) => {}
            Err(err) => panic!("second frame layout failed: {err}"),
        }
        assert_eq!(
            text_measurer.cached_metrics_len(),
            after_first_frame,
            "the second frame should hit the entries written by the first"
        );
        assert!(
            text_measurer.metric_hits() > hits_before_second_frame,
            "the second frame must hit metrics written by the first"
        );
    }

    #[test]
    fn a_fresh_measurer_starts_without_cached_metrics() {
        let text_measurer = TextMeasureCache::new();
        assert_eq!(text_measurer.cached_metrics_len(), 0);
    }

    #[test]
    fn pre_cache_pipeline_and_presenter_signatures_remain_source_compatible() {
        let viewport_size = Size::new(64.0, 32.0);
        let mut root = div();
        let mut taffy = TaffyTree::<ElementId>::new();
        assert!(FramePipeline::layout_root(&mut root, &mut taffy, viewport_size).is_ok());

        let mut context = AppContext::new();
        let mut scene = Scene::new();
        let mut build_root = |_context: &mut AppContext| div();
        assert!(
            FramePipeline::build_frame(
                &mut context,
                &mut root,
                &mut build_root,
                &mut taffy,
                &mut scene,
                viewport_size,
            )
            .is_ok()
        );

        let mut presenter = Presenter::new(viewport_size);
        let (_taffy, _scene) = presenter.frame_surfaces_mut();
    }
}
