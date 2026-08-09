use crate::core::ElementId;
use crate::core::app::AppContext;
use crate::core::geometry::{Bounds, Size};
use crate::core::presenter::Presenter;
use crate::elements::Element;
use crate::elements::element::{LayoutContext, PaintContext};
use crate::renderer::RendererDiagnostics;
use crate::renderer::Scene;
use crate::renderer::text::TextMeasureCache;
use std::error::Error;
use std::fmt;
use std::time::Instant;
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

    pub fn stage(stage: FrameStage, message: impl Into<String>) -> Self {
        Self {
            message: format!("{stage:?} stage failed: {}", message.into()),
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
}

/// One named step of a frame, in the order [`FramePipeline::run_frame`] runs it.
///
/// The native and headless runners are both driven by [`FrameStage::ORDER`], so
/// reordering that array reorders both paths. Neither runner writes its own
/// stage sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStage {
    /// Fold pending runtime-view notifications into the redraw request.
    Prepare,
    /// Rebuild the root element tree when it is stale.
    Rebuild,
    /// Compute layout for the rebuilt tree.
    Layout,
    /// Dispatch backend-neutral events into the laid-out tree.
    ///
    /// This runs *after* [`FrameStage::Layout`], so handlers see the bounds of
    /// the frame being drawn, and *before* [`FrameStage::Paint`], so this
    /// frame's paint reflects layout from before dispatch. Both runners share
    /// that position; headless simply supplies no events.
    DispatchEvents,
    /// Paint the tree into the presenter's scene.
    Paint,
    /// Hand the painted scene to a backend. Headless supplies no backend.
    Present,
    /// Close out redraw bookkeeping and record the completed frame.
    Finish,
}

impl FrameStage {
    /// Canonical frame order, shared by every runner.
    pub const ORDER: [FrameStage; 7] = [
        FrameStage::Prepare,
        FrameStage::Rebuild,
        FrameStage::Layout,
        FrameStage::DispatchEvents,
        FrameStage::Paint,
        FrameStage::Present,
        FrameStage::Finish,
    ];
}

/// Wall-clock cost of the stages a runner cares about timing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStageDurations {
    pub layout_ns: u128,
    pub dispatch_ns: u128,
    pub paint_ns: u128,
    pub present_ns: u128,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameOutcome {
    pub root_bounds: Bounds,
    pub durations: FrameStageDurations,
    pub presented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FramePresentation {
    Presented(Option<RendererDiagnostics>),
    Deferred,
}

/// What a runner wants to happen when [`FrameStage::Prepare`] leaves nothing to
/// draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlePolicy {
    /// Run every remaining stage regardless. Headless drives frames explicitly,
    /// so a requested frame is always produced.
    AlwaysDraw,
    /// Stop after `Prepare` and report no frame. The native runner waits for the
    /// next platform event instead of redrawing.
    SkipWhenIdle,
}

impl FramePipeline {
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

    /// Runs one frame through [`FrameStage::ORDER`].
    ///
    /// `dispatch_events` and `present_frame` are the two stages a runner has to
    /// supply, because only it knows which events arrived and which backend (if
    /// any) receives the scene. Every other stage is shared.
    ///
    /// Returns `Ok(None)` only under [`IdlePolicy::SkipWhenIdle`] when there was
    /// nothing to draw.
    pub fn run_frame<E, F, D, P>(
        context: &mut AppContext,
        presenter: &mut Presenter<E>,
        build_root: &mut F,
        viewport_size: Size,
        idle_policy: IdlePolicy,
        dispatch_events: D,
        present_frame: P,
    ) -> Result<Option<FrameOutcome>, FramePipelineError>
    where
        E: Element,
        F: FnMut(&mut AppContext) -> E,
        D: FnOnce(&mut Presenter<E>, &mut AppContext) -> Result<(), FramePipelineError>,
        P: FnOnce(
            &mut Presenter<E>,
            &mut AppContext,
        ) -> Result<FramePresentation, FramePipelineError>,
    {
        let mut durations = FrameStageDurations::default();
        let mut dispatch_events = Some(dispatch_events);
        let mut present_frame = Some(present_frame);
        let mut presentation = None;
        let mut presented = false;

        for stage in FrameStage::ORDER {
            match stage {
                FrameStage::Prepare => {
                    Self::prepare_frame(context);
                    if matches!(idle_policy, IdlePolicy::SkipWhenIdle) && !context.has_frame_work()
                    {
                        return Ok(None);
                    }
                }
                FrameStage::Rebuild => presenter.rebuild_if_needed(context, build_root),
                FrameStage::Layout => {
                    let started_at = Instant::now();
                    presenter.layout(viewport_size)?;
                    durations.layout_ns = started_at.elapsed().as_nanos();
                }
                FrameStage::DispatchEvents => {
                    let started_at = Instant::now();
                    if let Some(dispatch) = dispatch_events.take() {
                        dispatch(presenter, context)?;
                    }
                    durations.dispatch_ns = started_at.elapsed().as_nanos();
                }
                FrameStage::Paint => {
                    let started_at = Instant::now();
                    presenter.paint();
                    durations.paint_ns = started_at.elapsed().as_nanos();
                }
                FrameStage::Present => {
                    let started_at = Instant::now();
                    if let Some(present) = present_frame.take() {
                        presentation = Some(present(presenter, context)?);
                    }
                    durations.present_ns = started_at.elapsed().as_nanos();
                }
                FrameStage::Finish => {
                    Self::finish_frame(context);
                    match presentation.take() {
                        Some(FramePresentation::Presented(diagnostics)) => {
                            presenter.complete_presented_frame(diagnostics);
                            presented = true;
                        }
                        Some(FramePresentation::Deferred) => context.preserve_frame_work(),
                        None => {
                            return Err(FramePipelineError::layout(
                                "present stage did not report an outcome",
                            ));
                        }
                    }
                }
            }
        }

        Ok(Some(FrameOutcome {
            root_bounds: presenter.root_bounds(),
            durations,
            presented,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Presenter;
    use crate::elements::div;
    use crate::elements::text::text;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn run_recording_frame(
        idle_policy: IdlePolicy,
        prepare: impl FnOnce(&mut AppContext),
        presentation: FramePresentation,
    ) -> (Option<FrameOutcome>, Vec<&'static str>, AppContext, bool) {
        let viewport_size = Size::new(120.0, 80.0);
        let mut context = AppContext::new();
        context.set_viewport_size(viewport_size);
        prepare(&mut context);

        let mut build_root = |_: &mut AppContext| {
            div()
                .w(120.0)
                .h(80.0)
                .bg(crate::core::Color::rgb(0.1, 0.2, 0.3))
        };
        let root = build_root(&mut context);
        let mut presenter = Presenter::with_root(viewport_size, root);
        let log = Rc::new(RefCell::new(Vec::new()));

        let dispatch_log = Rc::clone(&log);
        let present_log = Rc::clone(&log);
        let outcome = match FramePipeline::run_frame(
            &mut context,
            &mut presenter,
            &mut build_root,
            viewport_size,
            idle_policy,
            move |presenter, _| {
                // Layout ran already, so dispatch sees this frame's bounds.
                assert_eq!(presenter.root_bounds().width(), 120.0);
                dispatch_log.borrow_mut().push("dispatch");
                Ok(())
            },
            move |presenter, _| {
                // Paint ran already, so present sees a populated scene.
                assert!(!presenter.scene().primitives().is_empty());
                present_log.borrow_mut().push("present");
                Ok(presentation)
            },
        ) {
            Ok(outcome) => outcome,
            Err(err) => panic!("frame failed: {err}"),
        };

        let stages = log.borrow().clone();
        let recorded = presenter.last_frame().is_some();
        (outcome, stages, context, recorded)
    }

    #[test]
    fn frame_stage_order_places_dispatch_between_layout_and_paint() {
        let order = FrameStage::ORDER;
        let index = |stage: FrameStage| match order.iter().position(|candidate| *candidate == stage)
        {
            Some(index) => index,
            None => panic!("{stage:?} is missing from the canonical frame order"),
        };

        assert!(index(FrameStage::Prepare) < index(FrameStage::Rebuild));
        assert!(index(FrameStage::Rebuild) < index(FrameStage::Layout));
        assert!(index(FrameStage::Layout) < index(FrameStage::DispatchEvents));
        assert!(index(FrameStage::DispatchEvents) < index(FrameStage::Paint));
        assert!(index(FrameStage::Paint) < index(FrameStage::Present));
        assert!(index(FrameStage::Present) < index(FrameStage::Finish));
    }

    #[test]
    fn run_frame_executes_the_runner_supplied_stages_in_order() {
        let (outcome, stages, _, _) = run_recording_frame(
            IdlePolicy::AlwaysDraw,
            |_| {},
            FramePresentation::Presented(None),
        );

        assert!(outcome.is_some(), "AlwaysDraw must produce a frame");
        assert_eq!(stages, ["dispatch", "present"]);
    }

    #[test]
    fn always_draw_runs_a_frame_even_when_nothing_is_dirty() {
        let (outcome, stages, context, _) = run_recording_frame(
            IdlePolicy::AlwaysDraw,
            |context| {
                context.dirty = false;
                context.needs_rebuild = false;
            },
            FramePresentation::Presented(None),
        );

        assert!(outcome.is_some());
        assert_eq!(stages, ["dispatch", "present"]);
        assert!(!context.has_frame_work());
    }

    #[test]
    fn skip_when_idle_stops_after_prepare_without_dispatching_or_presenting() {
        let (outcome, stages, _, _) = run_recording_frame(
            IdlePolicy::SkipWhenIdle,
            |context| {
                context.dirty = false;
                context.needs_rebuild = false;
            },
            FramePresentation::Presented(None),
        );

        assert!(outcome.is_none(), "an idle frame must be skipped");
        assert!(
            stages.is_empty(),
            "a skipped frame must not dispatch or present"
        );
    }

    #[test]
    fn skip_when_idle_still_runs_a_frame_that_has_work() {
        let (outcome, stages, _, _) = run_recording_frame(
            IdlePolicy::SkipWhenIdle,
            |context| context.request_redraw(),
            FramePresentation::Presented(None),
        );

        assert!(outcome.is_some());
        assert_eq!(stages, ["dispatch", "present"]);
    }

    #[test]
    fn a_completed_frame_clears_the_redraw_request() {
        let (_, _, context, _) = run_recording_frame(
            IdlePolicy::AlwaysDraw,
            |context| context.request_redraw(),
            FramePresentation::Presented(None),
        );

        assert!(
            !context.has_frame_work(),
            "the Finish stage should close out the redraw request"
        );
    }

    #[test]
    fn deferred_presentation_does_not_record_a_presented_frame() {
        let (outcome, _, _, recorded) =
            run_recording_frame(IdlePolicy::AlwaysDraw, |_| {}, FramePresentation::Deferred);

        match outcome {
            Some(outcome) => assert!(!outcome.presented),
            None => panic!("AlwaysDraw must produce a frame outcome"),
        }
        assert!(!recorded);
    }

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
