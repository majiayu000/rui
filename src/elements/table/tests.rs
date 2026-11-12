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
    let row = row().cell(cell("A")).cell(cell("B")).cell(cell("C"));

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
    let t = table().column_widths(vec![100.0, 200.0, 150.0]);

    assert_eq!(t.column_widths, Some(vec![100.0, 200.0, 150.0]));
}

#[test]
fn test_table_border_settings() {
    let t = table().border_color(Color::hex(0x000000)).border_width(2.0);

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
    let t = table().row(row().cells(["Short", "A much longer text value"]));

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
    let t = table().row(row().cells(["A"])).row(row().cells(["B"]));

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
    let t = table().row(row().cells(["A"])); // Single character

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
