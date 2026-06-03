use rui::advanced_ui::{
    DataList, DataListItem, DataTableCell, DataTableRow, DataTree, DataTreeItem, Theme,
    ThemeDensity, data_list, data_table_row, data_tree,
};
use rui::core::event::MouseButton;
use rui::elements::Element;
use rui::elements::element::{
    EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, ElementId, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::prelude::{AvailableSpace, TaffyTree};

fn data_pointer(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
    PointerEvent {
        kind,
        position: rui::Point::new(x, y),
        button: Some(MouseButton::Left),
    }
}

fn painted_data_primitives(mut root: impl Element, viewport: Size) -> Vec<Primitive> {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let root_node = root.layout(&mut layout_cx);
    taffy
        .compute_layout(
            root_node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .expect("advanced data layout should compute");

    let layout = taffy.layout(root_node).expect("root layout should exist");
    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(
        &mut scene,
        Bounds::from_xywh(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        ),
        &taffy,
    );
    root.paint(&mut paint_cx);
    scene.primitives().to_vec()
}

fn data_layout_size(mut root: impl Element, viewport: Size) -> Size {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let root_node = root.layout(&mut layout_cx);
    taffy
        .compute_layout(
            root_node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .expect("advanced data layout should compute");

    let layout = taffy.layout(root_node).expect("root layout should exist");
    Size::new(layout.size.width, layout.size.height)
}

fn text_contents(primitives: &[Primitive]) -> Vec<&str> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn advanced_ui_data_theme_density_changes_layout_tokens() {
    let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });

    let list_size = data_layout_size(
        DataList::new([("open", "Open"), ("done", "Done")]).theme(theme),
        Size::new(260.0, 140.0),
    );
    assert_eq!(list_size.height, 108.0);

    let tree_size = data_layout_size(
        DataTree::new([
            DataTreeItem::new("src", "src").child(DataTreeItem::new("advanced", "advanced_ui"))
        ])
        .theme(theme),
        Size::new(260.0, 140.0),
    );
    assert_eq!(tree_size.height, 108.0);

    let row_size = data_layout_size(
        DataTableRow::new(["Name", "Status"]).theme(theme),
        Size::new(260.0, 100.0),
    );
    assert_eq!(row_size.height, 54.0);
}

#[test]
fn advanced_ui_data_list_paints_rows_and_static_details() {
    let primitives = painted_data_primitives(
        data_list([
            ("changed", "Changed files", "from git status"),
            ("tests", "Validation tests", "from local test inventory"),
        ])
        .accessibility_label("Repository summary")
        .selected("tests")
        .w(260.0)
        .row_height(32.0),
        Size::new(260.0, 64.0),
    );

    let text = text_contents(&primitives);
    assert!(text.contains(&"Changed files"));
    assert!(text.contains(&"Validation tests"));
    assert!(text.contains(&"from local test inventory"));
}

#[test]
fn advanced_ui_data_list_selects_enabled_item_only() {
    let selected = Rc::new(RefCell::new(Vec::<String>::new()));
    let selected_ref = Rc::clone(&selected);
    let mut list = DataList::new([
        DataListItem::new("open", "Open"),
        DataListItem::new("done", "Done"),
    ])
    .on_select(move |value| selected_ref.borrow_mut().push(value.to_string()));
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 200.0, 72.0),
        &taffy,
        &mut focused,
    );

    assert!(list.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Down, 10.0, 44.0)));
    assert!(list.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Up, 10.0, 44.0)));
    assert_eq!(list.selected_value(), Some("done"));
    assert_eq!(&*selected.borrow(), &["done".to_string()]);

    let mut disabled = DataList::new([DataListItem::new("blocked", "Blocked").disabled(true)]);
    assert!(
        !disabled.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Down, 10.0, 10.0))
    );
    assert_eq!(disabled.selected_value(), None);
}

#[test]
fn advanced_ui_data_tree_uses_expanded_visible_rows() {
    let tree = data_tree([
        DataTreeItem::new("src", "src")
            .child(DataTreeItem::new("advanced", "advanced_ui"))
            .expanded(true),
        DataTreeItem::new("tests", "tests")
            .child(DataTreeItem::new("hidden", "hidden child"))
            .expanded(false),
    ])
    .accessibility_label("Project tree")
    .w(240.0)
    .row_height(30.0);

    assert_eq!(tree.visible_item_count(), 3);
    let primitives = painted_data_primitives(tree, Size::new(240.0, 90.0));
    let text = text_contents(&primitives);
    assert!(text.contains(&"src"));
    assert!(text.contains(&"advanced_ui"));
    assert!(text.contains(&"tests"));
    assert!(!text.contains(&"hidden child"));
}

#[test]
fn advanced_ui_data_tree_selects_visible_child() {
    let selected = Rc::new(RefCell::new(Vec::<String>::new()));
    let selected_ref = Rc::clone(&selected);
    let mut tree = DataTree::new([
        DataTreeItem::new("src", "src").child(DataTreeItem::new("advanced", "advanced_ui"))
    ])
    .on_select(move |value| selected_ref.borrow_mut().push(value.to_string()));
    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 240.0, 72.0),
        &taffy,
        &mut focused,
    );

    assert!(tree.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Down, 20.0, 44.0)));
    assert!(tree.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Up, 20.0, 44.0)));
    assert_eq!(tree.selected_value(), Some("advanced"));
    assert_eq!(&*selected.borrow(), &["advanced".to_string()]);
}

#[test]
#[should_panic(expected = "data tree selected value must match a visible item")]
fn advanced_ui_data_tree_rejects_collapsed_hidden_selection() {
    drop(
        DataTree::new([DataTreeItem::new("src", "src")
            .child(DataTreeItem::new("hidden", "Hidden"))
            .expanded(false)])
        .selected("hidden"),
    );
}

#[test]
fn advanced_ui_data_table_row_lays_out_cells_and_selects() {
    let selected = Rc::new(Cell::new(false));
    let selected_ref = Rc::clone(&selected);
    let mut row = DataTableRow::new([
        DataTableCell::new("Name").w(120.0),
        DataTableCell::new("Status").center(),
        DataTableCell::new("Owner").right(),
    ])
    .w(300.0)
    .h(40.0)
    .on_select(move || selected_ref.set(true));

    let primitives = painted_data_primitives(
        data_table_row([
            DataTableCell::new("Name").w(120.0),
            DataTableCell::new("Status").center(),
            DataTableCell::new("Owner").right(),
        ])
        .w(300.0)
        .h(40.0),
        Size::new(300.0, 40.0),
    );
    assert_eq!(text_contents(&primitives), vec!["Name", "Status", "Owner"]);

    let taffy = TaffyTree::<ElementId>::new();
    let mut focused = None;
    let mut cx = EventContext::new(
        Bounds::from_xywh(0.0, 0.0, 300.0, 40.0),
        &taffy,
        &mut focused,
    );
    assert!(row.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Down, 4.0, 4.0)));
    assert!(row.handle_pointer_event(&mut cx, &data_pointer(PointerEventKind::Up, 4.0, 4.0)));
    assert!(row.interaction_state().selected());
    assert!(selected.get());
}

#[test]
#[should_panic(expected = "data table row requires at least one cell")]
fn advanced_ui_data_table_row_rejects_empty_cells() {
    drop(DataTableRow::new(Vec::<DataTableCell>::new()));
}
