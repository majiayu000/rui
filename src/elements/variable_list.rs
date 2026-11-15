//! Variable-height rows with stable item anchors and an explicit viewport.
use crate::core::event::{KeyEvent, ScrollEvent};
use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::core::{
    ElementId,
    color::Color,
    geometry::Bounds,
    style::{Background, Style},
};
use crate::elements::{
    div,
    element::{
        AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
        PointerEventKind, StableId, dispatch_action_to_children, style_to_taffy,
    },
};
use crate::renderer::{Primitive, text::TextMeasureCache};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
use taffy::prelude::*;

const ESTIMATE: f32 = 40.0;
const OVERSCAN: f32 = 80.0;

#[derive(Default)]
struct State {
    ids: Vec<ElementId>,
    heights: HashMap<ElementId, f32>,
    invalid: HashSet<ElementId>,
    anchor: Option<(ElementId, f32)>,
    width: f32,
    offset: f32,
    total: f32,
    pending_item: Option<ElementId>,
    pending_offset: Option<f32>,
}

/// Persistent reading position. Keep one handle per list across view renders.
#[derive(Clone, Default)]
pub struct VariableListHandle(Rc<RefCell<State>>);
impl VariableListHandle {
    pub fn new() -> Self {
        Self::default()
    }
    /// First visible item and pixel offset inside it, after layout.
    pub fn anchor(&self) -> Option<(ElementId, f32)> {
        self.0.borrow().anchor
    }
    /// Estimated distance from the start; unknown rows use an internal estimate.
    pub fn offset(&self) -> f32 {
        self.0.borrow().offset
    }
    /// Estimated total height, exact only once all rows have been measured.
    pub fn estimated_total_height(&self) -> f32 {
        self.0.borrow().total
    }
    /// Rebuild and remeasure this row at its next visible layout.
    /// If focused, cancel its active composition and clear focus before replacing it.
    pub fn invalidate(&self, id: ElementId) {
        let mut s = self.0.borrow_mut();
        s.heights.remove(&id);
        s.invalid.insert(id);
    }
    /// Place this item at the top without building intervening rows.
    pub fn scroll_to_item(&self, id: ElementId) {
        let mut s = self.0.borrow_mut();
        s.pending_item = Some(id);
        s.pending_offset = None;
    }
    /// Scroll to an estimated pixel position. Prefer item IDs for distant jumps.
    pub fn scroll_to(&self, offset: f32) {
        assert!(offset.is_finite(), "VariableList offset must be finite");
        let mut s = self.0.borrow_mut();
        s.pending_offset = Some(offset.max(0.0));
        s.pending_item = None;
    }
}

/// A variable-height vertical list. Requires finite positive `.w()` and `.h()`.
/// IDs must be unique and stable, rows must have a positive measured height.
/// Metadata reconciliation is O(N); building and measuring visit nearby rows.
pub struct VariableList {
    id: StableId,
    style: Style,
    handle: VariableListHandle,
    ids: Vec<ElementId>,
    row: Box<dyn Fn(ElementId) -> AnyElement>,
    rows: Vec<AnyElement>,
    row_ids: Vec<ElementId>,
    geometry: Vec<(f32, f32)>,
    width: f32,
    height: f32,
    reset_rows: bool,
}

pub fn variable_list<E: Into<AnyElement>>(
    handle: &VariableListHandle,
    ids: impl IntoIterator<Item = ElementId>,
    row: impl Fn(ElementId) -> E + 'static,
) -> VariableList {
    let ids: Vec<_> = ids.into_iter().collect();
    let unique: HashSet<_> = ids.iter().copied().collect();
    assert!(
        unique.len() == ids.len() && ids.iter().all(|id| !id.is_ephemeral()),
        "VariableList requires unique stable item IDs"
    );
    VariableList {
        id: StableId::new(),
        style: Style::default(),
        handle: handle.clone(),
        ids,
        row: Box::new(move |id| row(id).into()),
        rows: Vec::new(),
        row_ids: Vec::new(),
        geometry: Vec::new(),
        width: 0.0,
        height: 0.0,
        reset_rows: false,
    }
}
impl VariableList {
    // Called only by reconciliation: transfer live instances, never old props or handle.
    pub(crate) fn take_rows_from(&mut self, previous: &mut Self) {
        self.reset_rows = !Rc::ptr_eq(&self.handle.0, &previous.handle.0)
            || ((self.id.is_explicit() || previous.id.is_explicit())
                && self.id.get() != previous.id.get());
        self.rows = std::mem::take(&mut previous.rows);
        self.row_ids = std::mem::take(&mut previous.row_ids);
        self.geometry = std::mem::take(&mut previous.geometry);
    }

    fn cancel_removed_focus(&mut self, cx: &mut LayoutContext) {
        let Some(owner) = cx.focused else {
            return;
        };
        let Some(index) = self.rows.iter().position(|row| row.contains_id(owner)) else {
            return;
        };
        if !self.reset_rows
            && self.ids.contains(&self.row_ids[index])
            && !self
                .handle
                .0
                .borrow()
                .invalid
                .contains(&self.row_ids[index])
        {
            return;
        }
        let row = &mut self.rows[index];
        if row
            .text_input_snapshot(owner)
            .is_some_and(|s| s.composition().is_some())
        {
            let mut focused = Some(owner);
            let mut event = EventContext::new(
                Bounds::from_xywh(0.0, 0.0, self.width, self.height),
                cx.taffy,
                &mut focused,
            );
            row.handle_text_input_command(&mut event, &TextInputCommand::CancelComposition);
            assert!(
                row.text_input_snapshot(owner)
                    .is_some_and(|s| s.composition().is_none()),
                "VariableList cannot remove an editor that refused composition cancellation"
            );
        }
        cx.focused = None;
    }
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }
    pub fn w(mut self, width: f32) -> Self {
        self.width = width;
        self.style.set_width_dimension(width);
        self
    }
    pub fn h(mut self, height: f32) -> Self {
        self.height = height;
        self.style.set_height_dimension(height);
        self
    }
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.style.background = Background::Solid(color.into());
        self
    }
    pub fn built_items(&self) -> &[ElementId] {
        &self.row_ids
    }
    fn row_bounds(&self, bounds: Bounds, index: usize) -> Bounds {
        let (y, height) = self.geometry[index];
        Bounds::from_xywh(bounds.x(), bounds.y() + y, self.width, height)
    }
    fn build_measure(
        &mut self,
        cx: &mut LayoutContext,
        index: usize,
        kept: &mut HashMap<ElementId, AnyElement>,
    ) -> (AnyElement, NodeId, f32) {
        let id = self.ids[index];
        let invalid = self.handle.0.borrow_mut().invalid.remove(&id);
        let mut row = if !invalid { kept.remove(&id) } else { None }.unwrap_or_else(|| {
            div()
                .id(id)
                .w(self.width)
                .flex_col()
                .child((self.row)(id))
                .into()
        });
        let node = cx.layout_child_at(id.0, &mut row);
        // Clear last pass's absolute placement before intrinsic measurement.
        let mut style = cx.taffy.style(node).unwrap().clone();
        style.position = taffy::Position::Relative;
        style.size.width = Dimension::Length(self.width);
        style.size.height = Dimension::Auto;
        style.inset = taffy::Rect::auto();
        cx.taffy.set_style(node, style).unwrap();
        cx.compute_subtree(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(self.width),
                height: AvailableSpace::MaxContent,
            },
        )
        .expect("measure list row");
        let height = cx.taffy.layout(node).unwrap().size.height;
        assert!(
            height.is_finite() && height > 0.0,
            "VariableList rows must measure to finite positive heights"
        );
        self.handle.0.borrow_mut().heights.insert(id, height);
        (row, node, height)
    }
}

impl Element for VariableList {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }
    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }
    fn style(&self) -> &Style {
        &self.style
    }
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        assert!(
            self.width.is_finite()
                && self.width > 0.0
                && self.height.is_finite()
                && self.height > 0.0,
            "VariableList requires explicit finite positive .w() and .h()"
        );
        self.cancel_removed_focus(cx);
        if self.reset_rows {
            self.rows.clear();
            self.row_ids.clear();
            self.geometry.clear();
            self.reset_rows = false;
        }
        let focused_item = cx
            .focused
            .and_then(|owner| self.rows.iter().position(|row| row.contains_id(owner)))
            .map(|i| self.row_ids[i]);
        let pixel_scroll = self.handle.0.borrow().pending_offset.is_some();
        let (mut anchor_index, mut within) = {
            let mut s = self.handle.0.borrow_mut();
            let positions: HashMap<_, _> = self
                .ids
                .iter()
                .enumerate()
                .map(|(i, id)| (*id, i))
                .collect();
            if let Some((id, _)) = s.anchor
                && !positions.contains_key(&id)
            {
                let old = s.ids.iter().position(|v| *v == id).unwrap_or(0);
                let replacement = s.ids[old..]
                    .iter()
                    .chain(s.ids[..old].iter().rev())
                    .find(|id| positions.contains_key(id))
                    .copied();
                s.anchor = replacement.map(|id| (id, 0.0));
            }
            if s.width != self.width {
                s.heights.clear();
                s.width = self.width;
            }
            s.heights.retain(|id, _| positions.contains_key(id));
            s.invalid.retain(|id| positions.contains_key(id));
            s.ids = self.ids.clone();
            if let Some(id) = s.pending_item.take()
                && positions.contains_key(&id)
            {
                s.anchor = Some((id, 0.0));
            }
            if let Some(offset) = s.pending_offset.take() {
                let total: f32 = self
                    .ids
                    .iter()
                    .map(|id| s.heights.get(id).copied().unwrap_or(ESTIMATE))
                    .sum();
                let mut remaining = offset.min((total - self.height).max(0.0));
                for id in &self.ids {
                    let h = s.heights.get(id).copied().unwrap_or(ESTIMATE);
                    s.anchor = Some((*id, remaining));
                    if remaining < h {
                        break;
                    }
                    remaining -= h;
                }
            }
            if self.ids.is_empty() {
                s.anchor = None;
            }
            if s.anchor.is_none() {
                s.anchor = self.ids.first().map(|id| (*id, 0.0));
            }
            s.anchor
                .map(|(id, offset)| (positions[&id], offset))
                .unwrap_or((0, 0.0))
        };
        let mut kept: HashMap<_, _> = std::mem::take(&mut self.row_ids)
            .into_iter()
            .zip(std::mem::take(&mut self.rows))
            .collect();
        self.geometry.clear();
        let mut built = Vec::new();
        if !self.ids.is_empty() {
            let (row, node, h) = self.build_measure(cx, anchor_index, &mut kept);
            within = within.min((h - 0.001).max(0.0));
            let mut bottom = h - within;
            built.push((anchor_index, row, node, -within, h));
            for i in anchor_index + 1..self.ids.len() {
                if bottom >= self.height + OVERSCAN {
                    break;
                }
                let (row, node, h) = self.build_measure(cx, i, &mut kept);
                built.push((i, row, node, bottom, h));
                bottom += h;
            }
            let mut top = -within;
            let end_gap = if pixel_scroll {
                (self.height - bottom).max(0.0)
            } else {
                0.0
            };
            for i in (0..anchor_index).rev() {
                if top <= -OVERSCAN - end_gap {
                    break;
                }
                let (row, node, h) = self.build_measure(cx, i, &mut kept);
                top -= h;
                built.push((i, row, node, top, h));
            }
            if end_gap > 0.0 {
                let shift = end_gap.min(-top);
                for row in &mut built {
                    row.3 += shift;
                }
                if let Some(row) = built.iter().find(|row| row.3 <= 0.0 && row.3 + row.4 > 0.0) {
                    anchor_index = row.0;
                    within = -row.3;
                }
            }
        }
        if let Some(id) = focused_item
            && let Some(index) = self.ids.iter().position(|item| *item == id)
            && !built.iter().any(|r| r.0 == index)
        {
            let (row, node, h) = self.build_measure(cx, index, &mut kept);
            // Keep one offscreen editor in the tree for keyboard/IME ownership.
            built.push((index, row, node, -h - OVERSCAN, h));
        }
        built.sort_by_key(|r| r.0);
        let mut nodes = Vec::new();
        for (index, row, node, y, height) in built {
            let mut style = cx.taffy.style(node).unwrap().clone();
            style.position = taffy::Position::Absolute;
            style.inset.top = LengthPercentageAuto::Length(y);
            style.inset.left = LengthPercentageAuto::Length(0.0);
            style.size.height = Dimension::Length(height);
            cx.taffy.set_style(node, style).unwrap();
            self.rows.push(row);
            self.row_ids.push(self.ids[index]);
            self.geometry.push((y, height));
            nodes.push(node);
        }
        {
            let mut s = self.handle.0.borrow_mut();
            s.anchor = self.ids.get(anchor_index).map(|id| (*id, within));
            s.offset = self.ids[..anchor_index]
                .iter()
                .map(|id| s.heights.get(id).copied().unwrap_or(ESTIMATE))
                .sum::<f32>()
                + within;
            s.total = self
                .ids
                .iter()
                .map(|id| s.heights.get(id).copied().unwrap_or(ESTIMATE))
                .sum();
        }
        let mut style = style_to_taffy(&self.style);
        style.flex_shrink = 0.0;
        style.min_size = style.size;
        style.max_size = style.size;
        style.overflow = taffy::Point {
            x: taffy::Overflow::Hidden,
            y: taffy::Overflow::Hidden,
        };
        cx.new_with_children(style, &nodes).expect("list layout")
    }
    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        if let Background::Solid(color) = &self.style.background {
            cx.paint(Primitive::Quad {
                bounds,
                background: color.to_rgba(),
                border_color: self.style.border.color.to_rgba(),
                border_widths: self.style.border.width,
                corner_radii: self.style.border.radius,
            });
        }
        cx.scene.push_layer(bounds);
        for index in 0..self.rows.len() {
            let row_bounds = self.row_bounds(bounds, index);
            // Even the retained offscreen editor updates its caret geometry.
            // The scene layer clips both pixels and registered hit regions.
            self.rows[index].paint(&mut cx.with_bounds(row_bounds));
        }
        cx.scene.pop_layer();
    }
    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for row in &mut self.rows {
            row.refresh_text_geometry(text_measurer);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let bounds = cx.layout_bounds();
        let is_move = matches!(event.kind, PointerEventKind::Move);
        let mut handled = false;
        for built_index in (0..self.rows.len()).rev() {
            let row_bounds = self.row_bounds(bounds, built_index);
            let mut row_cx = cx.with_bounds_and_hit_clip(row_bounds, bounds);
            if self.rows[built_index].handle_pointer_event(&mut row_cx, event) && !is_move {
                handled = true;
                break;
            }
        }
        handled
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        let bounds = cx.layout_bounds();
        for built_index in (0..self.rows.len()).rev() {
            let row_bounds = self.row_bounds(bounds, built_index);
            let mut row_cx = cx.with_bounds_and_hit_clip(row_bounds, bounds);
            if self.rows[built_index].handle_scroll_event(&mut row_cx, event) {
                return true;
            }
        }
        if !cx.contains_pointer(event.position) || event.delta_y == 0.0 {
            return false;
        }
        let max = (self.handle.estimated_total_height() - bounds.height()).max(0.0);
        let offset = {
            let state = self.handle.0.borrow();
            state.pending_offset.unwrap_or(state.offset)
        };
        let next = (offset + event.delta_y).clamp(0.0, max);
        if next == offset {
            return false;
        }
        self.handle.scroll_to(next);
        cx.request_redraw();
        true
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if let Some(focused) = cx.focused_id()
            && let Some(row) = self
                .rows
                .iter_mut()
                .rev()
                .find(|row| row.contains_id(focused))
        {
            return row.handle_key_event(cx, event);
        }
        self.rows
            .iter_mut()
            .rev()
            .any(|row| row.handle_key_event(cx, event))
    }

    fn dispatch_action(
        &mut self,
        cx: &mut EventContext,
        action: &crate::core::action::ActionId,
    ) -> crate::core::action::ActionOutcome {
        dispatch_action_to_children(&mut self.rows, cx, action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if let Some(focused) = cx.focused_id()
            && let Some(row) = self
                .rows
                .iter_mut()
                .rev()
                .find(|row| row.contains_id(focused))
        {
            return row.handle_text_input_event(cx, event);
        }
        false
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        if let Some(focused) = cx.focused_id()
            && let Some(row) = self
                .rows
                .iter_mut()
                .rev()
                .find(|row| row.contains_id(focused))
        {
            return row.handle_text_input_command(cx, command);
        }
        false
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.rows
            .iter_mut()
            .rev()
            .any(|row| row.handle_window_event(event))
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut self.rows
    }

    fn children(&self) -> &[AnyElement] {
        &self.rows
    }
}
