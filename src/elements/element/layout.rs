//! Layout: the context elements lay out in, kept layout nodes, measured text
//! and the conversion from [`Style`] to taffy.

use super::*;
use crate::core::SharedStr;
use crate::renderer::text::hash::FastMap;
use smallvec::SmallVec;

/// Source of [`LayoutNodes`] epochs: a new one whenever the kept nodes are
/// started over, so layout recorded against old nodes is never reused.
static NEXT_LAYOUT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn next_layout_epoch() -> u64 {
    NEXT_LAYOUT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// Layout context passed during layout phase
pub struct LayoutContext<'a> {
    pub(crate) taffy: &'a mut TaffyTree<ElementId>,
    pub(crate) available_space: Size,
    pub(crate) focused: Option<ElementId>,
    text_measurer: LayoutTextMeasurer<'a>,
    pub(super) retained: Option<RetainedPass<'a>>,
    /// Text style inherited by the element being laid out. Shared, so layout
    /// records can keep the one they were laid out with cheaply.
    pub(super) text_style: Rc<TextStyle>,
    /// Wrapped text in this pass, measured by taffy in [`Self::compute_layout`].
    pub(super) measured_text: HashMap<NodeId, MeasuredText>,
    /// Wrapped text of the previous retained pass, to dirty nodes whose text
    /// changed while their taffy style did not.
    pub(super) previous_measured_text: HashMap<NodeId, MeasuredText>,
    /// Identity of the element whose children are being laid out.
    pub(super) identity_parent: ElementId,
    /// Position the next child laid out under `identity_parent` takes.
    pub(super) next_child_index: u64,
}

/// Lines of a wrapped text, filled in once layout has given it a width.
pub(crate) type WrappedLines = Rc<RefCell<Option<Arc<WrappedText>>>>;

/// What a wrapped text leaf needs to be measured at any width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextWrapRequest {
    pub(crate) content: SharedStr,
    pub(crate) font_size: f32,
    pub(crate) font_weight: u16,
    pub(crate) font_family: Option<Arc<str>>,
    pub(crate) line_height: f32,
    pub(crate) max_lines: Option<usize>,
}

impl TextWrapRequest {
    fn request(&self) -> TextRequest<'_> {
        TextRequest::new(
            &self.content,
            self.font_size,
            self.font_weight,
            self.font_family.as_deref(),
            self.line_height,
        )
    }

    fn wrap(&self, measurer: &mut TextMeasureCache, width: f32) -> Arc<WrappedText> {
        match measurer.wrap_shared(self.request(), width, self.max_lines) {
            Ok(wrapped) => wrapped,
            Err(err) => panic!("text layout failed: {:?}", err),
        }
    }

    fn wrapped_size(&self, measurer: &mut TextMeasureCache, width: f32) -> Size {
        match measurer.wrapped_size(self.request(), width, self.max_lines) {
            Ok(size) => size,
            Err(err) => panic!("text layout failed: {:?}", err),
        }
    }
}

/// Lines of a rich text, filled in once layout has given it a width.
pub(crate) type RichLines = Rc<RefCell<Option<Arc<RichWrapped>>>>;

/// What a rich text leaf needs to be measured at any width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RichWrapRequest {
    pub(crate) content: Arc<str>,
    pub(crate) spans: Vec<RichSpan>,
    pub(crate) max_lines: Option<usize>,
}

#[derive(Debug)]
pub(super) enum MeasuredText {
    Plain {
        request: TextWrapRequest,
        lines: WrappedLines,
    },
    Rich {
        request: RichWrapRequest,
        lines: RichLines,
    },
}

impl MeasuredText {
    fn size(&self, measurer: &mut TextMeasureCache, width: f32) -> Size {
        match self {
            Self::Plain { request, .. } => request.wrapped_size(measurer, width),
            Self::Rich { request, .. } => match measurer.wrapped_rich_size(
                &request.content,
                &request.spans,
                width,
                request.max_lines,
            ) {
                Ok(size) => size,
                Err(err) => panic!("rich text layout failed: {:?}", err),
            },
        }
    }

    /// Break the text at its final `width` and hand the lines to its element.
    fn break_lines(&self, measurer: &mut TextMeasureCache, width: f32) {
        match self {
            Self::Plain { request, lines } => {
                *lines.borrow_mut() = Some(request.wrap(measurer, width));
            }
            Self::Rich { request, lines } => {
                let wrapped =
                    measurer.wrap_rich(&request.content, &request.spans, width, request.max_lines);
                match wrapped {
                    Ok(wrapped) => *lines.borrow_mut() = Some(wrapped),
                    Err(err) => panic!("rich text layout failed: {:?}", err),
                }
            }
        }
    }

    fn same_request(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plain { request: a, .. }, Self::Plain { request: b, .. }) => a == b,
            (Self::Rich { request: a, .. }, Self::Rich { request: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// Where in the previous pass's nodes one element's nodes are, in creation order.
type OwnedNodes = SmallVec<[u32; 1]>;

/// Layout nodes kept from one frame to the next.
///
/// A retained layout pass hands each element the nodes the element with the same
/// identity created in the previous pass, in creation order, and only restyles or
/// rewires them when their style or children changed. Taffy then dirties just those
/// nodes and their ancestors, so unchanged subtrees keep their cached layout, also
/// when they moved: a scrolled list's rows keep their nodes. A change of tree shape
/// costs more restyling but stays correct: every node's style and children are
/// still written.
#[derive(Debug)]
pub struct LayoutNodes {
    /// Nodes of the last pass in creation order, each with the element that made it.
    pub(super) nodes: Vec<(ElementId, NodeId)>,
    /// Emptied storage of the pass before, reused so a pass does not allocate it.
    spare_nodes: Vec<(ElementId, NodeId)>,
    spare_owners: FastMap<ElementId, OwnedNodes>,
    spare_claimed: Vec<bool>,
    /// Changes whenever `nodes` are started over.
    pub(super) epoch: u64,
    /// Node count of the tree after the last retained pass. Anything else means the
    /// tree was changed outside a retained pass and the kept ids are stale.
    tree_node_count: usize,
    measured_text: HashMap<NodeId, MeasuredText>,
}

impl Default for LayoutNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            spare_nodes: Vec::new(),
            spare_owners: FastMap::default(),
            spare_claimed: Vec::new(),
            epoch: next_layout_epoch(),
            tree_node_count: 0,
            measured_text: HashMap::new(),
        }
    }
}

impl LayoutNodes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nodes created in the last retained pass.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

pub(super) struct RetainedPass<'a> {
    /// Collects this pass's nodes.
    pub(super) kept: &'a mut LayoutNodes,
    /// Nodes of the previous pass in creation order.
    pub(super) previous: Vec<(ElementId, NodeId)>,
    /// Which of `previous` this pass reused.
    claimed: Vec<bool>,
    /// Where the next element's nodes most likely are in `previous`: right after
    /// the last ones reused, as long as the tree keeps its order.
    next: usize,
    /// `previous` by the element that made each node, built the first time an
    /// element's nodes are not at `next`.
    owners: FastMap<ElementId, OwnedNodes>,
    owners_built: bool,
}

impl<'a> RetainedPass<'a> {
    fn new(kept: &'a mut LayoutNodes) -> Self {
        let previous = std::mem::replace(&mut kept.nodes, std::mem::take(&mut kept.spare_nodes));
        let mut claimed = std::mem::take(&mut kept.spare_claimed);
        claimed.resize(previous.len(), false);
        let owners = std::mem::take(&mut kept.spare_owners);
        Self {
            kept,
            previous,
            claimed,
            next: 0,
            owners,
            owners_built: false,
        }
    }

    /// Where the next node of this pass goes in [`LayoutNodes::nodes`].
    pub(super) fn cursor(&self) -> usize {
        self.kept.nodes.len()
    }

    /// The first node `owner` made in the previous pass that this pass has not
    /// reused yet.
    fn take_previous(&mut self, owner: ElementId) -> Option<NodeId> {
        let index = match self.previous.get(self.next) {
            Some((made_by, _)) if *made_by == owner && !self.claimed[self.next] => self.next,
            _ => self.find(owner)?,
        };
        self.claimed[index] = true;
        self.next = index + 1;
        Some(self.previous[index].1)
    }

    fn find(&mut self, owner: ElementId) -> Option<usize> {
        if !self.owners_built {
            self.owners_built = true;
            for (index, (made_by, _)) in self.previous.iter().enumerate() {
                self.owners.entry(*made_by).or_default().push(index as u32);
            }
        }
        let claimed = &self.claimed;
        self.owners
            .get(&owner)?
            .iter()
            .map(|index| *index as usize)
            .find(|index| !claimed[*index])
    }

    fn push(&mut self, owner: ElementId, node: NodeId) {
        self.kept.nodes.push((owner, node));
    }

    /// Reuse the previous pass's nodes `start..end`, in order, when they are a
    /// subtree that ends with `last`: a subtree's nodes are made together, its
    /// root's last, so they are when `last` still sits at the end. Not when one
    /// was reused already. Returns `last`.
    pub(super) fn keep_range(&mut self, start: usize, end: usize, last: NodeId) -> Option<NodeId> {
        let claimed = self.claimed.get(start..end)?;
        if start == end
            || self.previous[end - 1].1 != last
            || claimed.iter().any(|claimed| *claimed)
        {
            return None;
        }
        self.claimed[start..end].fill(true);
        self.kept.nodes.extend_from_slice(&self.previous[start..end]);
        self.next = end;
        Some(self.previous[end - 1].1)
    }

    /// Remove nodes the previous pass had and this one did not reuse.
    fn finish(self, taffy: &mut TaffyTree<ElementId>) -> &'a mut LayoutNodes {
        let Self {
            kept,
            mut previous,
            mut claimed,
            mut owners,
            ..
        } = self;
        for ((_, node), claimed) in previous.iter().zip(&claimed) {
            if !claimed {
                let _ = taffy.remove(*node);
            }
        }
        previous.clear();
        claimed.clear();
        owners.clear();
        kept.spare_nodes = previous;
        kept.spare_claimed = claimed;
        kept.spare_owners = owners;
        kept.tree_node_count = taffy.total_node_count();
        kept
    }
}

/// Where a layout pass gets its text measurement cache from.
///
/// Frame-driving callers hold one cache across frames and pass it in as
/// [`LayoutTextMeasurer::Borrowed`], so measurements survive between frames.
/// One-off layout passes own a throwaway cache instead.
enum LayoutTextMeasurer<'a> {
    Owned(Box<TextMeasureCache>),
    Borrowed(&'a mut TextMeasureCache),
}

impl<'a> LayoutContext<'a> {
    pub fn new(taffy: &'a mut TaffyTree<ElementId>, available_space: Size) -> Self {
        Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Owned(Box::default()),
            None,
        )
    }

    fn with_measurer(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: LayoutTextMeasurer<'a>,
        retained: Option<RetainedPass<'a>>,
    ) -> Self {
        Self {
            taffy,
            available_space,
            focused: None,
            text_measurer,
            retained,
            text_style: Rc::default(),
            measured_text: HashMap::new(),
            previous_measured_text: HashMap::new(),
            identity_parent: ElementId::ROOT,
            next_child_index: 0,
        }
    }

    /// A pass that reuses the nodes `kept` from the previous pass over `taffy`.
    /// Call [`Self::finish`] once the root is laid out.
    pub fn retained(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: &'a mut TextMeasureCache,
        kept: &'a mut LayoutNodes,
    ) -> Self {
        if kept.tree_node_count != taffy.total_node_count() {
            taffy.clear();
            kept.nodes.clear();
            kept.measured_text.clear();
            kept.epoch = next_layout_epoch();
        }
        let previous_measured_text = std::mem::take(&mut kept.measured_text);
        let mut cx = Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Borrowed(text_measurer),
            Some(RetainedPass::new(kept)),
        );
        cx.previous_measured_text = previous_measured_text;
        cx
    }

    /// End a retained pass, dropping nodes this pass did not reuse.
    ///
    /// Wrapped text is only measured by [`Self::compute_layout`], which also
    /// finishes the pass.
    pub fn finish(mut self) {
        if let Some(pass) = self.retained.take() {
            pass.finish(self.taffy).measured_text = std::mem::take(&mut self.measured_text);
        }
    }

    /// Finish the pass and lay out the tree under `root`, measuring wrapped text
    /// against the width its parent offers it.
    pub fn compute_layout(
        mut self,
        root: NodeId,
        available_space: taffy::Size<AvailableSpace>,
    ) -> taffy::TaffyResult<()> {
        let kept = self.retained.take().map(|pass| pass.finish(self.taffy));
        self.compute_subtree(root, available_space)?;
        let measurer = match &mut self.text_measurer {
            LayoutTextMeasurer::Owned(cache) => cache,
            LayoutTextMeasurer::Borrowed(cache) => &mut **cache,
        };
        for (node, text) in &self.measured_text {
            text.break_lines(measurer, self.taffy.unrounded_layout(*node).size.width);
        }
        if let Some(kept) = kept {
            kept.measured_text = std::mem::take(&mut self.measured_text);
        }
        Ok(())
    }

    /// Measure a subtree without ending the retained pass. Final root layout
    /// remains responsible for reclamation and final wrapped-text geometry.
    pub(crate) fn compute_subtree(
        &mut self,
        root: NodeId,
        available_space: taffy::Size<AvailableSpace>,
    ) -> taffy::TaffyResult<()> {
        let measured = &self.measured_text;
        if measured.is_empty() {
            self.taffy.compute_layout(root, available_space)?;
        } else {
            let measurer = match &mut self.text_measurer {
                LayoutTextMeasurer::Owned(cache) => cache,
                LayoutTextMeasurer::Borrowed(cache) => &mut **cache,
            };
            self.taffy.compute_layout_with_measure(
                root,
                available_space,
                |known, available, node, _, _| {
                    let Some(text) = measured.get(&node) else {
                        return taffy::Size::ZERO;
                    };
                    let width = known.width.unwrap_or(match available.width {
                        AvailableSpace::Definite(width) => width,
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => f32::INFINITY,
                    });
                    let wrapped = text.size(measurer, width);
                    taffy::Size {
                        width: known.width.unwrap_or(wrapped.width),
                        height: known.height.unwrap_or(wrapped.height),
                    }
                },
            )?;
        }
        Ok(())
    }

    /// Text style the element being laid out inherits.
    pub fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    /// Run `f` with `refinement` applied over the inherited text style.
    pub fn with_text_style<R>(
        &mut self,
        refinement: &TextStyleRefinement,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if refinement.is_empty() {
            return f(self);
        }
        let inherited = Rc::new(self.text_style.refined(refinement));
        let parent = std::mem::replace(&mut self.text_style, inherited);
        let result = f(self);
        self.text_style = parent;
        result
    }

    /// Lay out `child` as the child at `index` of the element being laid out,
    /// instead of at the next position. For elements that build only some of
    /// their children, such as the rows in view, so each keeps its identity.
    pub fn layout_child_at(&mut self, index: u64, child: &mut AnyElement) -> NodeId {
        child.layout_at(self, index)
    }

    /// A layout node without children.
    pub fn new_leaf(&mut self, style: taffy::Style) -> taffy::TaffyResult<NodeId> {
        self.new_with_children(style, &[])
    }

    /// A leaf measured by taffy as text wrapped to the width it is offered.
    /// `lines` receives the final lines in [`Self::compute_layout`].
    pub(crate) fn new_wrapped_text_leaf(
        &mut self,
        style: taffy::Style,
        request: TextWrapRequest,
        lines: WrappedLines,
    ) -> taffy::TaffyResult<NodeId> {
        *lines.borrow_mut() = None;
        self.new_measured_text_leaf(style, MeasuredText::Plain { request, lines })
    }

    /// A leaf measured by taffy as rich text wrapped to the width it is
    /// offered. `lines` receives the final lines in [`Self::compute_layout`].
    pub(crate) fn new_rich_text_leaf(
        &mut self,
        style: taffy::Style,
        request: RichWrapRequest,
        lines: RichLines,
    ) -> taffy::TaffyResult<NodeId> {
        *lines.borrow_mut() = None;
        self.new_measured_text_leaf(style, MeasuredText::Rich { request, lines })
    }

    fn new_measured_text_leaf(
        &mut self,
        style: taffy::Style,
        text: MeasuredText,
    ) -> taffy::TaffyResult<NodeId> {
        let node = self.reuse_or_create(style, &[])?;
        let previous = self.previous_measured_text.remove(&node);
        if previous.is_none_or(|previous| !previous.same_request(&text)) {
            self.taffy.mark_dirty(node)?;
        }
        self.measured_text.insert(node, text);
        Ok(node)
    }

    /// A layout node with `children`, reused from the previous pass when retained.
    pub fn new_with_children(
        &mut self,
        style: taffy::Style,
        children: &[NodeId],
    ) -> taffy::TaffyResult<NodeId> {
        let node = self.reuse_or_create(style, children)?;
        // A node that was wrapped text last pass keeps its measured size cached.
        if !self.previous_measured_text.is_empty()
            && self.previous_measured_text.remove(&node).is_some()
        {
            self.taffy.mark_dirty(node)?;
        }
        Ok(node)
    }

    fn reuse_or_create(
        &mut self,
        style: taffy::Style,
        children: &[NodeId],
    ) -> taffy::TaffyResult<NodeId> {
        let Some(pass) = self.retained.as_mut() else {
            return self.taffy.new_with_children(style, children);
        };
        // Nodes belong to the element being laid out.
        let owner = self.identity_parent;
        let Some(node) = pass.take_previous(owner) else {
            // `set_children`, unlike `new_with_children`, detaches each child from
            // the parent it had last pass, which that parent may still list.
            let node = self.taffy.new_leaf(style)?;
            if !children.is_empty() {
                self.taffy.set_children(node, children)?;
            }
            pass.push(owner, node);
            return Ok(node);
        };
        pass.push(owner, node);
        if *self.taffy.style(node)? != style {
            self.taffy.set_style(node, style)?;
        }
        let same_children = self.taffy.child_count(node) == children.len()
            && children
                .iter()
                .enumerate()
                .all(|(index, child)| self.taffy.child_at_index(node, index).ok() == Some(*child));
        if !same_children {
            self.taffy.set_children(node, children)?;
        }
        Ok(node)
    }

    /// Layout against a text measurement cache owned by the caller, so cached
    /// metrics outlive this single layout pass.
    pub fn with_text_measurer(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: &'a mut TextMeasureCache,
    ) -> Self {
        Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Borrowed(text_measurer),
            None,
        )
    }

    pub fn text_measurer(&mut self) -> &mut TextMeasureCache {
        match &mut self.text_measurer {
            LayoutTextMeasurer::Owned(cache) => cache,
            LayoutTextMeasurer::Borrowed(cache) => cache,
        }
    }
}

fn dimension_to_taffy(dimension: StyleDimension) -> Dimension {
    match dimension {
        StyleDimension::Px(value) => Dimension::Length(value),
        StyleDimension::Percent(value) => Dimension::Percent(value / 100.0),
        StyleDimension::Auto => Dimension::Auto,
        StyleDimension::Fill => Dimension::Percent(1.0),
    }
}

fn dimension_or_px(
    dimension: Option<StyleDimension>,
    pixels: Option<f32>,
    fallback: Dimension,
) -> Dimension {
    dimension
        .map(dimension_to_taffy)
        .or_else(|| pixels.map(Dimension::Length))
        .unwrap_or(fallback)
}

pub(crate) fn style_dimension_or(
    dimension: Option<StyleDimension>,
    pixels: Option<f32>,
    fallback: Dimension,
) -> Dimension {
    dimension_or_px(dimension, pixels, fallback)
}

/// Convert Taffy style to our style
pub fn style_to_taffy(style: &Style) -> taffy::Style {
    taffy::Style {
        display: match style.display {
            crate::core::style::Display::Flex => taffy::Display::Flex,
            crate::core::style::Display::Block => taffy::Display::Block,
            crate::core::style::Display::None => taffy::Display::None,
        },
        position: match style.position {
            crate::core::style::Position::Relative => taffy::Position::Relative,
            crate::core::style::Position::Absolute => taffy::Position::Absolute,
        },
        flex_direction: match style.flex_direction {
            crate::core::style::FlexDirection::Row => taffy::FlexDirection::Row,
            crate::core::style::FlexDirection::Column => taffy::FlexDirection::Column,
            crate::core::style::FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            crate::core::style::FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        justify_content: Some(match style.justify_content {
            crate::core::style::JustifyContent::FlexStart => taffy::JustifyContent::FlexStart,
            crate::core::style::JustifyContent::FlexEnd => taffy::JustifyContent::FlexEnd,
            crate::core::style::JustifyContent::Center => taffy::JustifyContent::Center,
            crate::core::style::JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
            crate::core::style::JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
            crate::core::style::JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
        }),
        align_items: Some(match style.align_items {
            crate::core::style::AlignItems::FlexStart => taffy::AlignItems::FlexStart,
            crate::core::style::AlignItems::FlexEnd => taffy::AlignItems::FlexEnd,
            crate::core::style::AlignItems::Center => taffy::AlignItems::Center,
            crate::core::style::AlignItems::Stretch => taffy::AlignItems::Stretch,
            crate::core::style::AlignItems::Baseline => taffy::AlignItems::Baseline,
        }),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        gap: taffy::Size {
            width: LengthPercentage::Length(style.gap),
            height: LengthPercentage::Length(style.gap),
        },
        size: taffy::Size {
            width: dimension_or_px(style.dimensions.width, style.width, Dimension::Auto),
            height: dimension_or_px(style.dimensions.height, style.height, Dimension::Auto),
        },
        min_size: taffy::Size {
            width: dimension_or_px(style.dimensions.min_width, style.min_width, Dimension::Auto),
            height: dimension_or_px(
                style.dimensions.min_height,
                style.min_height,
                Dimension::Auto,
            ),
        },
        max_size: taffy::Size {
            width: dimension_or_px(style.dimensions.max_width, style.max_width, Dimension::Auto),
            height: dimension_or_px(
                style.dimensions.max_height,
                style.max_height,
                Dimension::Auto,
            ),
        },
        margin: taffy::Rect {
            top: LengthPercentageAuto::Length(style.margin.top),
            right: LengthPercentageAuto::Length(style.margin.right),
            bottom: LengthPercentageAuto::Length(style.margin.bottom),
            left: LengthPercentageAuto::Length(style.margin.left),
        },
        padding: taffy::Rect {
            top: LengthPercentage::Length(style.padding.top),
            right: LengthPercentage::Length(style.padding.right),
            bottom: LengthPercentage::Length(style.padding.bottom),
            left: LengthPercentage::Length(style.padding.left),
        },
        ..Default::default()
    }
}
