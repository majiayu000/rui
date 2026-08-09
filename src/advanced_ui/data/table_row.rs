use super::common::{
    DATA_PADDING_X, DEFAULT_DATA_WIDTH, DataRowHandler, base_data_style, disabled_aware_color,
    row_background, validate_dimension,
};
use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::elements::text::{FontWeight, TextAlign};
use crate::renderer::Primitive;
use taffy::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub struct DataTableCell {
    id: ElementId,
    content: String,
    width: Option<f32>,
    align: TextAlign,
    color: Option<Color>,
    background: Option<Color>,
    font_weight: FontWeight,
    accessibility_label: Option<String>,
    invalid: bool,
}

impl DataTableCell {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        require_non_empty(&content, "data table cell content must not be empty");

        Self {
            id: ElementId::new(),
            content,
            width: None,
            align: TextAlign::Left,
            color: None,
            background: None,
            font_weight: FontWeight::Regular,
            accessibility_label: None,
            invalid: false,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        validate_dimension(
            width,
            "data table cell width must be finite and non-negative",
        );
        self.width = Some(width);
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn right(self) -> Self {
        self.align(TextAlign::Right)
    }

    pub fn center(self) -> Self {
        self.align(TextAlign::Center)
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    pub fn semibold(mut self) -> Self {
        self.font_weight = FontWeight::Semibold;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(
            &label,
            "data table cell accessibility label must not be empty",
        );
        self.accessibility_label = Some(label);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    fn preferred_width(&self, size: ControlSize, theme: Theme) -> f32 {
        self.width.unwrap_or_else(|| {
            self.content.chars().count() as f32 * theme.text_size(size) * 0.56
                + DATA_PADDING_X * 2.0
        })
    }
}

impl From<&str> for DataTableCell {
    fn from(content: &str) -> Self {
        Self::new(content)
    }
}

impl From<String> for DataTableCell {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

pub struct DataTableRow {
    id: ElementId,
    cells: Vec<DataTableCell>,
    accessibility_label: Option<String>,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    selected: bool,
    header: bool,
    on_select: Option<DataRowHandler>,
}

impl DataTableRow {
    pub fn new<I, O>(cells: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataTableCell>,
    {
        let cells: Vec<DataTableCell> = cells.into_iter().map(Into::into).collect();
        if cells.is_empty() {
            panic!("data table row requires at least one cell");
        }

        Self {
            id: ElementId::new(),
            cells,
            accessibility_label: None,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            style: base_data_style(0.0),
            selected: false,
            header: false,
            on_select: None,
        }
    }

    pub fn header<I, O>(cells: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataTableCell>,
    {
        Self::new(cells).header_row(true)
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(
            &label,
            "data table row accessibility label must not be empty",
        );
        self.accessibility_label = Some(label);
        self
    }

    pub fn cell(mut self, cell: impl Into<DataTableCell>) -> Self {
        self.cells.push(cell.into());
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.state.set_selected(selected);
        self
    }

    pub fn header_row(mut self, header: bool) -> Self {
        self.header = header;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        validate_dimension(
            width,
            "data table row width must be finite and non-negative",
        );
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        validate_dimension(
            height,
            "data table row height must be finite and non-negative",
        );
        self.style.height = Some(height);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.set_invalid(invalid);
        self
    }

    pub fn on_select(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn preferred_width(&self) -> f32 {
        self.style.width.unwrap_or_else(|| {
            self.cells
                .iter()
                .map(|cell| cell.preferred_width(self.size, self.theme))
                .sum::<f32>()
                .max(DEFAULT_DATA_WIDTH)
        })
    }

    fn preferred_height(&self) -> f32 {
        self.style.height.unwrap_or(
            self.theme
                .control_height(self.size)
                .max(self.theme.text_size(self.size) * 1.4 + 12.0),
        )
    }

    fn resolved_cell_widths(&self, bounds: Bounds) -> Vec<f32> {
        if self.cells.is_empty() {
            return Vec::new();
        }
        let explicit_total: f32 = self.cells.iter().filter_map(|cell| cell.width).sum();
        let flexible_count = self
            .cells
            .iter()
            .filter(|cell| cell.width.is_none())
            .count();
        let flexible_width = if flexible_count == 0 {
            0.0
        } else {
            ((bounds.width() - explicit_total).max(0.0)) / flexible_count as f32
        };
        let widths = self
            .cells
            .iter()
            .map(|cell| cell.width.unwrap_or(flexible_width))
            .collect::<Vec<_>>();
        let mut remaining = bounds.width().max(0.0);
        widths
            .into_iter()
            .map(|width| {
                let clamped = width.max(0.0).min(remaining);
                remaining -= clamped;
                clamped
            })
            .collect()
    }
}

impl Element for DataTableRow {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.preferred_width());
        style.size.height = Dimension::Length(self.preferred_height());
        cx.taffy
            .new_leaf(style)
            .expect("failed to create advanced data table row layout node")
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        let background = if self.header {
            self.theme.colors.surface_muted
        } else {
            row_background(
                self.selected,
                self.state.hovered(),
                self.state.pressed(),
                self.state.disabled(),
                self.theme,
            )
        };

        cx.paint(Primitive::Quad {
            bounds,
            background: background.to_rgba(),
            border_color: self
                .theme
                .validation_border_color(self.state.invalid(), self.theme.colors.border)
                .to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::ZERO,
        });

        let mut x = bounds.x();
        for (cell, width) in self.cells.iter().zip(self.resolved_cell_widths(bounds)) {
            let cell_bounds = Bounds::from_xywh(x, bounds.y(), width, bounds.height());
            paint_cell(
                cx,
                cell,
                cell_bounds,
                self.size,
                self.theme,
                self.header,
                self.state.disabled(),
            );
            x += width;
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let label = self.accessibility_label.clone().unwrap_or_else(|| {
            self.cells
                .iter()
                .map(|cell| cell.content.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        });
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::DataTableRow, label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_selected(self.selected)
                .with_focused(cx.a11y_has_focus(self.id));
        if self.state.can_activate() {
            node = node.with_action(AccessibilityAction::Activate);
        }
        for cell in &self.cells {
            let label = cell
                .accessibility_label
                .as_deref()
                .unwrap_or(cell.content.as_str());
            node = node.with_child(
                AccessibilityNode::label_required(
                    cell.id,
                    AccessibilityRole::DataTableCell,
                    label,
                )?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid() || cell.invalid),
            );
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.update_hover(cx.bounds(), event.position, cx);
                false
            }
            PointerEventKind::Down => self.state.press_inside(inside, cx),
            PointerEventKind::Up => {
                let release = self.state.release_inside(inside, cx);
                if release.activated {
                    self.selected = true;
                    self.state.set_selected(true);
                    if let Some(handler) = &self.on_select {
                        handler();
                    }
                    cx.announce_accessibility_action(self.id, "row selected");
                    return true;
                }
                false
            }
        }
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !cx.is_focused(Some(self.id))
            || !self.state.can_activate()
            || !matches!(action, ActionId::Standard(StandardAction::Activate))
        {
            return ActionOutcome::Ignored;
        }
        self.selected = true;
        self.state.set_selected(true);
        if let Some(handler) = &self.on_select {
            handler();
        }
        cx.announce_accessibility_action(self.id, "row selected");
        cx.request_redraw();
        ActionOutcome::handled("advanced_ui.data_table_row")
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.cells.iter().any(|cell| cell.id == id)
    }
}

fn paint_cell(
    cx: &mut PaintContext,
    cell: &DataTableCell,
    bounds: Bounds,
    size: ControlSize,
    theme: Theme,
    header: bool,
    disabled: bool,
) {
    if bounds.is_empty() {
        return;
    }
    cx.register_hit_region(cell.id, bounds);
    if let Some(background) = cell.background {
        cx.paint(Primitive::Quad {
            bounds,
            background: background.to_rgba(),
            border_color: Color::TRANSPARENT.to_rgba(),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        });
    }
    cx.paint(Primitive::Quad {
        bounds,
        background: Color::TRANSPARENT.to_rgba(),
        border_color: theme
            .validation_border_color(cell.invalid, theme.colors.border)
            .to_rgba(),
        border_widths: Edges::new(0.0, 1.0, 0.0, 0.0),
        corner_radii: Corners::ZERO,
    });
    cx.paint(Primitive::Text {
        bounds: Bounds::from_xywh(
            bounds.x() + DATA_PADDING_X,
            bounds.y(),
            (bounds.width() - DATA_PADDING_X * 2.0).max(0.0),
            bounds.height(),
        ),
        content: cell.content.clone(),
        color: disabled_aware_color(cell.color.unwrap_or(theme.colors.text), disabled).to_rgba(),
        font_size: theme.text_size(size),
        font_weight: if header {
            FontWeight::Semibold.to_value()
        } else {
            cell.font_weight.to_value()
        },
        font_family: None,
        line_height: theme.typography.line_height,
        align: cell.align,
    });
}

pub fn data_table_row<I, O>(cells: I) -> DataTableRow
where
    I: IntoIterator<Item = O>,
    O: Into<DataTableCell>,
{
    DataTableRow::new(cells)
}
