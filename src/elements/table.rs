//! Table element for rendering tabular data

use crate::core::color::{Color, Rgba};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Background, Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::elements::text::{FontWeight, TextAlign};
use crate::renderer::Primitive;
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
            width: self.style.width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Length(total_width)),
            height: self.style.height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Length(total_height)),
        };

        let node = cx
            .taffy
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
                    content: cell.content.clone(),
                    color: cell.color.to_rgba(),
                    font_size: cell.font_size,
                    font_weight: cell.font_weight.to_value(),
                    font_family: None,
                    line_height: 1.4,
                    align: cell.align,
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
mod tests {
    use super::*;

    #[test]
    fn test_table_cell_creation() {
        let cell = TableCell::new("Test");
        assert_eq!(cell.content(), "Test");
        assert_eq!(cell.get_colspan(), 1);
        assert_eq!(cell.get_rowspan(), 1);
    }

    #[test]
    fn test_table_cell_builder_pattern() {
        let cell = cell("Value")
            .colspan(2)
            .rowspan(3)
            .center()
            .bold()
            .size(16.0)
            .p(12.0);

        assert_eq!(cell.content(), "Value");
        assert_eq!(cell.get_colspan(), 2);
        assert_eq!(cell.get_rowspan(), 3);
        assert_eq!(cell.align, TextAlign::Center);
        assert_eq!(cell.font_weight, FontWeight::Bold);
        assert_eq!(cell.font_size, 16.0);
        assert_eq!(cell.padding, Edges::all(12.0));
    }

    #[test]
    fn test_table_cell_alignment() {
        let left = cell("Left").align(TextAlign::Left);
        let center = cell("Center").center();
        let right = cell("Right").right();

        assert_eq!(left.align, TextAlign::Left);
        assert_eq!(center.align, TextAlign::Center);
        assert_eq!(right.align, TextAlign::Right);
    }

    #[test]
    fn test_table_cell_from_str() {
        let cell: TableCell = "Test".into();
        assert_eq!(cell.content(), "Test");
    }

    #[test]
    fn test_table_cell_from_string() {
        let cell: TableCell = String::from("Test").into();
        assert_eq!(cell.content(), "Test");
    }

    #[test]
    fn test_table_cell_colspan_minimum() {
        let cell = cell("Test").colspan(0);
        assert_eq!(cell.get_colspan(), 1); // Minimum is 1
    }

    #[test]
    fn test_table_cell_padding() {
        let cell = cell("Test").px(10.0).py(5.0);
        assert_eq!(cell.padding.left, 10.0);
        assert_eq!(cell.padding.right, 10.0);
        assert_eq!(cell.padding.top, 5.0);
        assert_eq!(cell.padding.bottom, 5.0);
    }

    #[test]
    fn test_table_row_creation() {
        let row = TableRow::new();
        assert!(!row.is_header());
        assert_eq!(row.cell_count(), 0);
    }

    #[test]
    fn test_table_row_header() {
        let row = TableRow::header();
        assert!(row.is_header());
    }

    #[test]
    fn test_table_row_with_cells() {
        let row = row()
            .cell(cell("A"))
            .cell(cell("B"))
            .cell(cell("C"));

        assert_eq!(row.cell_count(), 3);
        assert_eq!(row.get_cells()[0].content(), "A");
        assert_eq!(row.get_cells()[1].content(), "B");
        assert_eq!(row.get_cells()[2].content(), "C");
    }

    #[test]
    fn test_table_row_cells_from_strings() {
        let row = row().cells(["A", "B", "C"]);
        assert_eq!(row.cell_count(), 3);
    }

    #[test]
    fn test_table_header_row_makes_cells_bold() {
        let row = header_row().cells(["Header 1", "Header 2"]);
        assert!(row.is_header());
        for cell in row.get_cells() {
            assert_eq!(cell.font_weight, FontWeight::Bold);
        }
    }

    #[test]
    fn test_table_row_background() {
        let row = row().bg(Color::hex(0xFF0000));
        match row.background {
            Background::Solid(color) => {
                let rgba = color.to_rgba();
                assert!((rgba.r - 1.0).abs() < 0.01);
            }
            _ => panic!("Expected solid background"),
        }
    }

    #[test]
    fn test_table_row_height() {
        let row = row().h(50.0);
        assert_eq!(row.height, Some(50.0));
    }

    #[test]
    fn test_table_creation() {
        let t = Table::new();
        assert_eq!(t.row_count(), 0);
        assert_eq!(t.column_count(), 0);
    }

    #[test]
    fn test_table_with_rows() {
        let t = table()
            .row(header_row().cells(["Name", "Age", "City"]))
            .row(row().cells(["Alice", "30", "NYC"]))
            .row(row().cells(["Bob", "25", "LA"]));

        assert_eq!(t.row_count(), 3);
        assert_eq!(t.column_count(), 3);
    }

    #[test]
    fn test_table_column_widths() {
        let t = table()
            .column_widths(vec![100.0, 200.0, 150.0]);

        assert_eq!(t.column_widths, Some(vec![100.0, 200.0, 150.0]));
    }

    #[test]
    fn test_table_border_settings() {
        let t = table()
            .border_color(Color::hex(0x000000))
            .border_width(2.0);

        assert_eq!(t.border_width, 2.0);
    }

    #[test]
    fn test_table_header_background() {
        let t = table().header_bg(Color::hex(0xCCCCCC));
        match t.header_background {
            Background::Solid(_) => {}
            _ => panic!("Expected solid background"),
        }
    }

    #[test]
    fn test_table_striped() {
        let t = table().striped(Color::hex(0xF0F0F0));
        assert!(t.stripe_background.is_some());
    }

    #[test]
    fn test_table_cell_padding_setting() {
        let t = table().cell_padding(16.0);
        assert_eq!(t.cell_padding, Edges::all(16.0));
    }

    #[test]
    fn test_table_size() {
        let t = table().w(500.0).h(300.0);
        assert_eq!(t.style.width, Some(500.0));
        assert_eq!(t.style.height, Some(300.0));
    }

    #[test]
    fn test_table_size_combined() {
        let t = table().size(400.0, 200.0);
        assert_eq!(t.style.width, Some(400.0));
        assert_eq!(t.style.height, Some(200.0));
    }

    #[test]
    fn test_table_default() {
        let t = Table::default();
        assert_eq!(t.row_count(), 0);
    }

    #[test]
    fn test_table_row_default() {
        let r = TableRow::default();
        assert!(!r.is_header());
        assert_eq!(r.cell_count(), 0);
    }

    #[test]
    fn test_table_calculate_column_widths_with_explicit_widths() {
        let t = table()
            .column_widths(vec![100.0, 200.0])
            .row(row().cells(["A", "B"]));

        let widths = t.calculate_column_widths();
        assert_eq!(widths, vec![100.0, 200.0]);
    }

    #[test]
    fn test_table_calculate_column_widths_auto() {
        let t = table()
            .row(row().cells(["Short", "A much longer text value"]));

        let widths = t.calculate_column_widths();
        assert_eq!(widths.len(), 2);
        assert!(widths[1] > widths[0]); // Longer text should have wider column
    }

    #[test]
    fn test_table_calculate_row_heights() {
        let t = table()
            .row(row().h(50.0).cells(["A"]))
            .row(row().cells(["B"]));

        let heights = t.calculate_row_heights();
        assert_eq!(heights.len(), 2);
        assert_eq!(heights[0], 50.0); // Explicit height
        assert!(heights[1] >= 30.0); // Minimum height
    }

    #[test]
    fn test_table_empty_column_count() {
        let t = table();
        assert_eq!(t.column_count(), 0);
    }

    #[test]
    fn test_table_column_count_from_first_row() {
        let t = table()
            .row(row().cells(["A", "B", "C", "D"]))
            .row(row().cells(["1", "2"])); // Fewer cells in second row

        assert_eq!(t.column_count(), 4); // Based on first row
    }

    #[test]
    fn test_table_get_rows() {
        let t = table()
            .row(row().cells(["A"]))
            .row(row().cells(["B"]));

        let rows = t.get_rows();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_table_multiple_rows_builder() {
        let t = table().rows(vec![
            row().cells(["A", "B"]),
            row().cells(["C", "D"]),
            row().cells(["E", "F"]),
        ]);

        assert_eq!(t.row_count(), 3);
    }

    #[test]
    fn test_table_id() {
        let t = table().id(ElementId(42));
        assert_eq!(t.id, Some(ElementId(42)));
    }

    #[test]
    fn test_cell_estimate_width() {
        let cell = cell("Test").size(14.0).p(8.0);
        let width = cell.estimate_width();
        // 4 chars * 14.0 * 0.5 + 16 (padding) = 28 + 16 = 44
        assert!((width - 44.0).abs() < 0.1);
    }

    #[test]
    fn test_cell_estimate_height() {
        let cell = cell("Test").size(14.0).p(8.0);
        let height = cell.estimate_height();
        // 14.0 * 1.4 + 16 (padding) = 19.6 + 16 = 35.6
        assert!((height - 35.6).abs() < 0.1);
    }

    #[test]
    fn test_table_row_mutable_cells() {
        let mut row = row().cells(["A", "B"]);
        row.get_cells_mut()[0] = cell("Modified");
        assert_eq!(row.get_cells()[0].content(), "Modified");
    }

    #[test]
    fn test_table_cell_weight_variants() {
        let regular = cell("Test").weight(FontWeight::Regular);
        let medium = cell("Test").weight(FontWeight::Medium);
        let semibold = cell("Test").weight(FontWeight::Semibold);

        assert_eq!(regular.font_weight, FontWeight::Regular);
        assert_eq!(medium.font_weight, FontWeight::Medium);
        assert_eq!(semibold.font_weight, FontWeight::Semibold);
    }

    #[test]
    fn test_helper_functions() {
        let t = table();
        let r = row();
        let hr = header_row();
        let c = cell("Test");

        assert_eq!(t.row_count(), 0);
        assert!(!r.is_header());
        assert!(hr.is_header());
        assert_eq!(c.content(), "Test");
    }

    #[test]
    fn test_table_element_trait_style() {
        let t = table().w(100.0);
        let style = t.style();
        assert_eq!(style.width, Some(100.0));
    }

    #[test]
    fn test_table_element_trait_id() {
        let t1 = table();
        let t2 = table().id(ElementId(1));

        assert_eq!(Element::id(&t1), None);
        assert_eq!(Element::id(&t2), Some(ElementId(1)));
    }

    #[test]
    fn test_table_calculate_widths_minimum() {
        // Test that columns have a minimum width even for very short content
        let t = table()
            .row(row().cells(["A"])); // Single character

        let widths = t.calculate_column_widths();
        assert!(widths[0] >= 40.0); // Minimum width enforced
    }

    #[test]
    fn test_table_calculate_widths_empty() {
        let t = table();
        let widths = t.calculate_column_widths();
        assert!(widths.is_empty());
    }

    #[test]
    fn test_complete_table_workflow() {
        // Test a complete realistic table
        let t = table()
            .id(ElementId(1))
            .border_color(Color::hex(0xDDDDDD))
            .border_width(1.0)
            .header_bg(Color::hex(0xF0F0F0))
            .striped(Color::hex(0xFAFAFA))
            .cell_padding(10.0)
            .row(header_row().cells(["ID", "Name", "Email", "Status"]))
            .row(row().cells(["1", "Alice", "alice@example.com", "Active"]))
            .row(row().cells(["2", "Bob", "bob@example.com", "Inactive"]))
            .row(row().cells(["3", "Charlie", "charlie@example.com", "Active"]));

        assert_eq!(t.row_count(), 4);
        assert_eq!(t.column_count(), 4);
        assert!(t.get_rows()[0].is_header());
        assert!(!t.get_rows()[1].is_header());
    }
}
