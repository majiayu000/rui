//! Root layout and focus handoff into the shared layout context.
use super::*;

impl FramePipeline {
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

    /// [`Self::layout_root_with_text_measurer`] keeping the layout tree across
    /// frames: nodes are reused from the previous pass and only changed ones are
    /// restyled, so taffy recomputes just the dirty parts. `taffy` and `nodes` must
    /// be kept together from frame to frame.
    pub fn layout_root_retained<E>(
        root: &mut E,
        taffy: &mut TaffyTree<ElementId>,
        nodes: &mut LayoutNodes,
        text_measurer: &mut TextMeasureCache,
        viewport_size: Size,
    ) -> Result<Bounds, FramePipelineError>
    where
        E: Element,
    {
        Self::layout_root_retained_with_focus(
            root,
            taffy,
            nodes,
            text_measurer,
            viewport_size,
            &mut None,
        )
    }

    pub(crate) fn layout_root_retained_with_focus<E: Element>(
        root: &mut E,
        taffy: &mut TaffyTree<ElementId>,
        nodes: &mut LayoutNodes,
        text_measurer: &mut TextMeasureCache,
        viewport_size: Size,
        focused: &mut Option<ElementId>,
    ) -> Result<Bounds, FramePipelineError> {
        text_measurer.begin_frame();
        let mut layout_cx = LayoutContext::retained(taffy, viewport_size, text_measurer, nodes);
        layout_cx.focused = *focused;
        let root_node = root.layout(&mut layout_cx);
        *focused = layout_cx.focused;
        Self::compute_root_layout(layout_cx, root_node, viewport_size)?;
        Self::root_bounds(taffy, root_node)
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
        text_measurer.begin_frame();
        let mut layout_cx = LayoutContext::with_text_measurer(taffy, viewport_size, text_measurer);
        let root_node = root.layout(&mut layout_cx);
        Self::compute_root_layout(layout_cx, root_node, viewport_size)?;
        Self::root_bounds(taffy, root_node)
    }

    fn compute_root_layout(
        layout_cx: LayoutContext<'_>,
        root_node: NodeId,
        viewport_size: Size,
    ) -> Result<(), FramePipelineError> {
        layout_cx
            .compute_layout(
                root_node,
                taffy::Size {
                    width: AvailableSpace::Definite(viewport_size.width),
                    height: AvailableSpace::Definite(viewport_size.height),
                },
            )
            .map_err(|err| FramePipelineError::layout(err.to_string()))
    }

    fn root_bounds(
        taffy: &TaffyTree<ElementId>,
        root_node: NodeId,
    ) -> Result<Bounds, FramePipelineError> {
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
}
