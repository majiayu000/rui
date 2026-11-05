//! A list that builds only the rows in view.

use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::event::{KeyEvent, ScrollEvent};
use crate::core::geometry::Bounds;
use crate::core::style::{Background, Dimension as StyleDimension, FlexDirection, Overflow, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    StableId, dispatch_action_to_children, style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use taffy::prelude::*;

/// Rows built beyond each edge of the view, so a short scroll shows built rows.
const OVERSCAN_ROWS: usize = 2;

#[derive(Debug, Default)]
struct ScrollState {
    offset: f32,
    /// Height the list had when it was last painted; 0 before the first paint.
    viewport_height: f32,
}

/// The scroll position of a [`VirtualList`].
///
/// Keep one in the view and pass it to the list on every render: the position then
/// survives rebuilds and changes to the data. Clones share one position.
#[derive(Debug, Clone, Default)]
pub struct VirtualListHandle(Rc<RefCell<ScrollState>>);

impl VirtualListHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Distance scrolled from the top, in pixels.
    pub fn offset(&self) -> f32 {
        self.0.borrow().offset
    }

    /// Scroll to `offset`; the list clamps it to its content when laid out.
    pub fn scroll_to(&self, offset: f32) {
        self.0.borrow_mut().offset = offset.max(0.0);
    }
}

type RowBuilder = Box<dyn Fn(usize) -> AnyElement>;

/// A vertical list of `count` rows of equal height that builds, lays out and paints
/// only the rows in view. Rows are built during layout by `row(index)`.
pub struct VirtualList {
    id: StableId,
    style: Style,
    handle: VirtualListHandle,
    count: usize,
    row_height: f32,
    row: RowBuilder,
    first_row: usize,
    rows: Vec<AnyElement>,
    row_nodes: Vec<NodeId>,
}

/// A [`VirtualList`] of `count` rows, each `row_height` tall, built by `row(index)`.
pub fn virtual_list<E: Into<AnyElement>>(
    handle: &VirtualListHandle,
    count: usize,
    row_height: f32,
    row: impl Fn(usize) -> E + 'static,
) -> VirtualList {
    // Hidden overflow keeps the rows from growing the list to their total height.
    let style = Style {
        flex_direction: FlexDirection::Column,
        overflow_x: Overflow::Hidden,
        overflow_y: Overflow::Hidden,
        ..Style::default()
    };
    VirtualList {
        id: StableId::new(),
        style,
        handle: handle.clone(),
        count,
        row_height: row_height.max(1.0),
        row: Box::new(move |index| row(index).into()),
        first_row: 0,
        rows: Vec::new(),
        row_nodes: Vec::new(),
    }
}

impl VirtualList {
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.set_width_dimension(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.set_height_dimension(height);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.style.set_width_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn h_full(mut self) -> Self {
        self.style.set_height_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.style.background = Background::Solid(color.into());
        self
    }

    /// Rows built in the last layout.
    pub fn built_rows(&self) -> Range<usize> {
        self.first_row..self.first_row + self.rows.len()
    }

    /// Rows that intersect a view `height` tall at `offset`, plus overscan.
    fn rows_in_view(&self, offset: f32, height: f32) -> Range<usize> {
        let first = (offset / self.row_height).floor() as usize;
        let last = ((offset + height) / self.row_height).ceil() as usize;
        first.saturating_sub(OVERSCAN_ROWS)..(last + OVERSCAN_ROWS).min(self.count)
    }

    fn max_offset(&self, viewport_height: f32) -> f32 {
        (self.count as f32 * self.row_height - viewport_height).max(0.0)
    }

    /// Where row `index` of the built rows sits for a list at `bounds`.
    fn row_bounds(&self, bounds: Bounds, built_index: usize) -> Bounds {
        let index = self.first_row + built_index;
        Bounds::from_xywh(
            bounds.x(),
            bounds.y() + index as f32 * self.row_height - self.handle.offset(),
            bounds.width(),
            self.row_height,
        )
    }
}

impl Element for VirtualList {
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
        let viewport_height = {
            let state = self.handle.0.borrow();
            if state.viewport_height > 0.0 {
                state.viewport_height
            } else {
                // Not painted yet: the window bounds how tall the list can be.
                cx.available_space.height
            }
        };
        let offset = self.handle.offset().min(self.max_offset(viewport_height));
        self.handle.0.borrow_mut().offset = offset;

        let range = self.rows_in_view(offset, viewport_height);
        // Rows still in view keep their instance, and with it element state such as
        // a pressed button; only rows entering the view are built.
        let previous = self.first_row..self.first_row + self.rows.len();
        let mut kept: Vec<Option<AnyElement>> = std::mem::take(&mut self.rows)
            .into_iter()
            .map(Some)
            .collect();
        self.rows = range
            .clone()
            .map(|index| {
                if previous.contains(&index)
                    && let Some(row) = kept[index - previous.start].take()
                {
                    return row;
                }
                (self.row)(index)
            })
            .collect();
        self.first_row = range.start;
        // Rows take their row index as position, so scrolling keeps identities.
        self.row_nodes = (range.start as u64..)
            .zip(self.rows.iter_mut())
            .map(|(index, row)| cx.layout_child_at(index, row))
            .collect();
        cx.new_with_children(style_to_taffy(&self.style), &self.row_nodes)
            .expect("Failed to create virtual list layout node")
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        if (self.handle.0.borrow().viewport_height - bounds.height()).abs() > f32::EPSILON {
            self.handle.0.borrow_mut().viewport_height = bounds.height();
        }
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
        for built_index in 0..self.rows.len() {
            let row_bounds = self.row_bounds(bounds, built_index);
            if !row_bounds.intersects(&bounds) {
                continue;
            }
            let mut row_cx = cx.with_bounds(row_bounds);
            self.rows[built_index].paint(&mut row_cx);
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
        let max = self.max_offset(bounds.height());
        let offset = self.handle.offset();
        let next = (offset + event.delta_y).clamp(0.0, max);
        if next == offset {
            return false;
        }
        self.handle.0.borrow_mut().offset = next;
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

    fn children(&self) -> &[AnyElement] {
        &self.rows
    }
}
