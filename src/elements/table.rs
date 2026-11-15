//! Table element for rendering tabular data

use crate::core::ElementId;
use crate::core::color::{Color, Rgba};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Background, Corners, Style};
use crate::elements::element::{Element, LayoutContext, PaintContext, style_to_taffy};
use crate::elements::text::{FontWeight, TextAlign};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use smallvec::SmallVec;
use taffy::prelude::*;

/// A table cell element
#[derive(Debug, Clone)]
pub struct TableCell {
    content: String,
    colspan: usize,
    rowspan: usize,
    align: TextAlign,
    color: Color,
    background: Background,
    font_weight: FontWeight,
    font_size: f32,
    padding: Edges,
}

impl TableCell {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            colspan: 1,
            rowspan: 1,
            align: TextAlign::Left,
            color: Color::BLACK,
            background: Background::None,
            font_weight: FontWeight::Regular,
            font_size: 14.0,
            padding: Edges::all(8.0),
        }
    }

    /// Set the column span for this cell
    pub fn colspan(mut self, span: usize) -> Self {
        self.colspan = span.max(1);
        self
    }

    /// Set the row span for this cell
    pub fn rowspan(mut self, span: usize) -> Self {
        self.rowspan = span.max(1);
        self
    }

    /// Set text alignment
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Center align text
    pub fn center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// Right align text
    pub fn right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }

    /// Set text color
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// Set background color
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.background = Background::Solid(color.into());
        self
    }

    /// Set font weight
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    /// Make text bold
    pub fn bold(mut self) -> Self {
        self.font_weight = FontWeight::Bold;
        self
    }

    /// Set font size
    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set padding
    pub fn p(mut self, padding: f32) -> Self {
        self.padding = Edges::all(padding);
        self
    }

    /// Set horizontal padding
    pub fn px(mut self, padding: f32) -> Self {
        self.padding.left = padding;
        self.padding.right = padding;
        self
    }

    /// Set vertical padding
    pub fn py(mut self, padding: f32) -> Self {
        self.padding.top = padding;
        self.padding.bottom = padding;
        self
    }

    /// Get the content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get colspan
    pub fn get_colspan(&self) -> usize {
        self.colspan
    }

    /// Get rowspan
    pub fn get_rowspan(&self) -> usize {
        self.rowspan
    }

    /// Estimate cell width based on content
    fn estimate_width(&self) -> f32 {
        let text_width = self.content.len() as f32 * self.font_size * 0.5;
        text_width + self.padding.horizontal_sum()
    }

    /// Estimate cell height
    fn estimate_height(&self) -> f32 {
        self.font_size * 1.4 + self.padding.vertical_sum()
    }
}

/// Create a new table cell
pub fn cell(content: impl Into<String>) -> TableCell {
    TableCell::new(content)
}

/// A table row element
#[derive(Debug, Clone)]
pub struct TableRow {
    cells: SmallVec<[TableCell; 8]>,
    is_header: bool,
    background: Background,
    height: Option<f32>,
}

impl TableRow {
    pub fn new() -> Self {
        Self {
            cells: SmallVec::new(),
            is_header: false,
            background: Background::None,
            height: None,
        }
    }

    /// Create a header row
    pub fn header() -> Self {
        Self {
            cells: SmallVec::new(),
            is_header: true,
            background: Background::None,
            height: None,
        }
    }

    /// Add a cell to the row
    pub fn cell(mut self, cell: impl Into<TableCell>) -> Self {
        let mut cell = cell.into();
        if self.is_header && cell.font_weight == FontWeight::Regular {
            cell.font_weight = FontWeight::Bold;
        }
        self.cells.push(cell);
        self
    }

    /// Add multiple cells from strings
    pub fn cells<I, S>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for content in cells {
            let mut cell = TableCell::new(content);
            if self.is_header {
                cell.font_weight = FontWeight::Bold;
            }
            self.cells.push(cell);
        }
        self
    }

    /// Set background color
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.background = Background::Solid(color.into());
        self
    }

    /// Set row height
    pub fn h(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Check if this is a header row
    pub fn is_header(&self) -> bool {
        self.is_header
    }

    /// Get cells in this row
    pub fn get_cells(&self) -> &[TableCell] {
        &self.cells
    }

    /// Get mutable cells in this row
    pub fn get_cells_mut(&mut self) -> &mut [TableCell] {
        &mut self.cells
    }

    /// Get number of cells
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }
}

impl Default for TableRow {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new table row
pub fn row() -> TableRow {
    TableRow::new()
}

/// Create a new header row
pub fn header_row() -> TableRow {
    TableRow::header()
}

/// Allow string to be converted into a TableCell
impl From<&str> for TableCell {
    fn from(s: &str) -> Self {
        TableCell::new(s)
    }
}

impl From<String> for TableCell {
    fn from(s: String) -> Self {
        TableCell::new(s)
    }
}

/// A table element for rendering tabular data
pub struct Table {
    id: Option<ElementId>,
    style: Style,
    rows: SmallVec<[TableRow; 16]>,
    column_widths: Option<Vec<f32>>,
    border_color: Color,
    border_width: f32,
    header_background: Background,
    stripe_background: Option<Color>,
    cell_padding: Edges,
    layout_node: Option<NodeId>,
}

impl Table {
    pub fn new() -> Self {
        Self {
            id: None,
            style: Style::new(),
            rows: SmallVec::new(),
            column_widths: None,
            border_color: Color::hex(0xE0E0E0),
            border_width: 1.0,
            header_background: Background::Solid(Color::hex(0xF5F5F5)),
            stripe_background: None,
            cell_padding: Edges::all(8.0),
            layout_node: None,
        }
    }

    /// Set element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Add a row to the table
    pub fn row(mut self, row: TableRow) -> Self {
        self.rows.push(row);
        self
    }

    /// Add multiple rows
    pub fn rows<I>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = TableRow>,
    {
        self.rows.extend(rows);
        self
    }

    /// Set explicit column widths
    pub fn column_widths(mut self, widths: Vec<f32>) -> Self {
        self.column_widths = Some(widths);
        self
    }

    /// Set border color
    pub fn border_color(mut self, color: impl Into<Color>) -> Self {
        self.border_color = color.into();
        self
    }

    /// Set border width
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    /// Set header background color
    pub fn header_bg(mut self, color: impl Into<Color>) -> Self {
        self.header_background = Background::Solid(color.into());
        self
    }

    /// Enable striped rows
    pub fn striped(mut self, color: impl Into<Color>) -> Self {
        self.stripe_background = Some(color.into());
        self
    }

    /// Set cell padding
    pub fn cell_padding(mut self, padding: f32) -> Self {
        self.cell_padding = Edges::all(padding);
        self
    }

    /// Set table width
    pub fn w(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    /// Set table height
    pub fn h(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    /// Set table size
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.style.width = Some(width);
        self.style.height = Some(height);
        self
    }

    /// Get the number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get the number of columns (based on the first row)
    pub fn column_count(&self) -> usize {
        self.rows.first().map(|r| r.cell_count()).unwrap_or(0)
    }

    /// Get rows
    pub fn get_rows(&self) -> &[TableRow] {
        &self.rows
    }

    /// Calculate column widths based on content
    fn calculate_column_widths(&self) -> Vec<f32> {
        if let Some(ref widths) = self.column_widths {
            return widths.clone();
        }

        let num_cols = self.column_count();
        if num_cols == 0 {
            return Vec::new();
        }

        let mut widths = vec![0.0f32; num_cols];

        for row in &self.rows {
            for (i, cell) in row.cells.iter().enumerate() {
                if i < num_cols && cell.colspan == 1 {
                    widths[i] = widths[i].max(cell.estimate_width());
                }
            }
        }

        // Ensure minimum width
        for width in &mut widths {
            if *width < 40.0 {
                *width = 40.0;
            }
        }

        widths
    }

    /// Calculate row heights
    fn calculate_row_heights(&self) -> Vec<f32> {
        self.rows
            .iter()
            .map(|row| {
                if let Some(h) = row.height {
                    h
                } else {
                    row.cells
                        .iter()
                        .map(|c| c.estimate_height())
                        .fold(0.0f32, |a, b| a.max(b))
                        .max(30.0) // Minimum row height
                }
            })
            .collect()
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for Table {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let col_widths = self.calculate_column_widths();
        let row_heights = self.calculate_row_heights();

        let total_width: f32 = col_widths.iter().sum::<f32>() + self.border_width;
        let total_height: f32 = row_heights.iter().sum::<f32>() + self.border_width;

        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: self
                .style
                .width
                .map(|w| Dimension::Length(w))
                .unwrap_or(Dimension::Length(total_width)),
            height: self
                .style
                .height
                .map(|h| Dimension::Length(h))
                .unwrap_or(Dimension::Length(total_height)),
        };

        let node = cx
            .new_leaf(style)
            .expect("Failed to create table layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let col_widths = self.calculate_column_widths();
        let row_heights = self.calculate_row_heights();

        // Paint table background/border
        cx.paint(Primitive::Quad {
            bounds,
            background: Rgba::WHITE,
            border_color: self.border_color.to_rgba(),
            border_widths: Edges::all(self.border_width),
            corner_radii: Corners::ZERO,
        });

        let mut y = bounds.y() + self.border_width;

        for (row_idx, row) in self.rows.iter().enumerate() {
            let row_height = row_heights.get(row_idx).copied().unwrap_or(30.0);
            let mut x = bounds.x() + self.border_width;

            // Determine row background
            let row_bg = if row.is_header {
                self.header_background
            } else if let Some(stripe_color) = self.stripe_background {
                if row_idx % 2 == 1 {
                    Background::Solid(stripe_color)
                } else {
                    row.background
                }
            } else {
                row.background
            };

            // Paint row background
            if let Background::Solid(color) = row_bg {
                let row_width: f32 = col_widths.iter().sum();
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(x, y, row_width, row_height),
                    background: color.to_rgba(),
                    border_color: Rgba::TRANSPARENT,
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::ZERO,
                });
            }

            for (col_idx, cell) in row.cells.iter().enumerate() {
                let cell_width = if cell.colspan > 1 {
                    col_widths[col_idx..col_idx + cell.colspan.min(col_widths.len() - col_idx)]
                        .iter()
                        .sum()
                } else {
                    col_widths.get(col_idx).copied().unwrap_or(100.0)
                };

                // Paint cell background if set
                if let Background::Solid(color) = cell.background {
                    cx.paint(Primitive::Quad {
                        bounds: Bounds::from_xywh(x, y, cell_width, row_height),
                        background: color.to_rgba(),
                        border_color: Rgba::TRANSPARENT,
                        border_widths: Edges::ZERO,
                        corner_radii: Corners::ZERO,
                    });
                }

                // Paint cell border
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(x, y, cell_width, row_height),
                    background: Rgba::TRANSPARENT,
                    border_color: self.border_color.to_rgba(),
                    border_widths: Edges::new(0.0, self.border_width, self.border_width, 0.0),
                    corner_radii: Corners::ZERO,
                });

                // Paint cell text
                let text_x = x + cell.padding.left;
                let text_y = y + cell.padding.top;
                let text_width = cell_width - cell.padding.horizontal_sum();
                let text_height = row_height - cell.padding.vertical_sum();

                cx.paint(Primitive::Text {
                    bounds: Bounds::from_xywh(text_x, text_y, text_width, text_height),
                    content: cell.content.clone().into(),
                    color: cell.color.to_rgba(),
                    font_size: cell.font_size,
                    font_weight: cell.font_weight.to_value(),
                    font_family: None,
                    line_height: 1.4,
                    align: cell.align,
                    direction: BaseDirection::Auto,
                });

                x += cell_width;
            }

            y += row_height;
        }
    }
}

/// Create a new table
pub fn table() -> Table {
    Table::new()
}

#[cfg(test)]
mod tests;
