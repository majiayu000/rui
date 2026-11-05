use rui::advanced_ui::{Button, ControlSize, ControlVariant, Theme, row};
use rui::renderer::Primitive;
use rui::renderer::text::{TextMeasureCache, TextRequest};
use rui::testing::mount;
use rui::{ElementId, Point, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn button_labels_keep_their_padding_with_real_font_metrics() {
    let theme = Theme::default();
    let mut cache = TextMeasureCache::new();
    for label in [
        "iiiiiiii",
        "WWWWWWWW",
        "保存所有修改并关闭窗口",
        "Cafe\u{301}",
    ] {
        let session = mount(Size::new(800.0, 120.0), |_| Button::new(label)).unwrap();
        let metrics = cache
            .measure_single_line(TextRequest::new(
                label,
                theme.text_size(ControlSize::Medium),
                theme.typography.control_weight,
                None,
                theme.typography.line_height,
            ))
            .unwrap();
        let expected = metrics.size.width + theme.horizontal_padding(ControlSize::Medium) * 2.0;
        assert!(
            (session.root_bounds().width() - expected).abs() <= 1.0,
            "{label:?}: width {}, expected {expected}",
            session.root_bounds().width()
        );
    }
}

#[test]
fn nested_button_paints_and_clears_runtime_focus_for_every_variant() {
    let id = ElementId::from(701);
    for variant in [
        ControlVariant::Primary,
        ControlVariant::Secondary,
        ControlVariant::Outline,
        ControlVariant::Ghost,
        ControlVariant::Danger,
        ControlVariant::Success,
    ] {
        let mut session = mount(Size::new(300.0, 80.0), |_| {
            row().child(Button::new("Save").id(id).variant(variant))
        })
        .unwrap();
        let before = format!("{:?}", session.primitives());
        session.request_focus(Some(id));
        session.frame().unwrap();
        assert_ne!(
            before,
            format!("{:?}", session.primitives()),
            "{variant:?} needs visible focus"
        );
        assert!(session.primitives().iter().any(|primitive| matches!(primitive,
            Primitive::Quad {border_color, border_widths, ..}
            if *border_color == Theme::default().state.focus_ring.to_rgba() && *border_widths == rui::Edges::all(2.0))));
        session.request_focus(None);
        session.frame().unwrap();
        assert_eq!(before, format!("{:?}", session.primitives()));
    }
}

#[test]
fn reordering_buttons_before_input_targets_the_current_layout() {
    let swapped = Rc::new(Cell::new(false));
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let mut session = mount(Size::new(500.0, 100.0), |_| {
        let ids = if swapped.get() { [2, 1] } else { [1, 2] };
        let a = Rc::clone(&clicked);
        let b = Rc::clone(&clicked);
        row()
            .child(
                Button::new("AAAA")
                    .id(ElementId::from(ids[0]))
                    .on_click(move || a.borrow_mut().push(ids[0])),
            )
            .child(
                Button::new("BBBB")
                    .id(ElementId::from(ids[1]))
                    .on_click(move || b.borrow_mut().push(ids[1])),
            )
    })
    .unwrap();
    swapped.set(true);
    session.app_context_mut().request_rebuild();
    assert!(session.pointer_down(Point::new(10.0, 10.0)));
    assert!(session.pointer_up(Point::new(10.0, 10.0)));
    assert_eq!(*clicked.borrow(), [2]);
    assert_eq!(session.focused_element(), Some(ElementId::from(2)));
}

#[test]
fn related_controls_fit_wide_labels_in_their_painted_text_bounds() {
    use rui::advanced_ui::{Checkbox, DataTableRow, Menu, SegmentedControl, TabList, Tooltip};
    use rui::elements::element::AnyElement;
    let label = "WWWWWWWWWWWWWWWWWWWWWWWW保存修改";
    let controls: Vec<(&str, AnyElement)> = vec![
        ("checkbox", Checkbox::new(label).into()),
        ("menu", Menu::new("Actions", [("save", label)]).into()),
        (
            "segments",
            SegmentedControl::new([("save", label)], "save").into(),
        ),
        ("tabs", TabList::new([("save", label)], "save").into()),
        ("table", DataTableRow::new([label]).into()),
        ("tooltip", Tooltip::new(Button::new("Hover"), label).into()),
    ];
    let mut cache = TextMeasureCache::new();
    for (name, root) in controls {
        let mut root = Some(root);
        let mut session = mount(Size::new(1200.0, 120.0), |_| {
            row().child(root.take().unwrap())
        })
        .unwrap();
        if name == "tooltip" {
            session.pointer_move(Point::new(5.0, 5.0));
        }
        let mut found = false;
        for primitive in session.primitives() {
            if let Primitive::Text {
                bounds,
                content,
                font_size,
                font_weight,
                font_family,
                line_height,
                ..
            } = primitive
            {
                if content != label {
                    continue;
                }
                found = true;
                let metrics = cache
                    .measure_single_line(TextRequest::new(
                        content,
                        *font_size,
                        *font_weight,
                        font_family.as_deref(),
                        *line_height,
                    ))
                    .unwrap();
                assert!(
                    bounds.width() + 1.0 >= metrics.size.width,
                    "{name}: text bounds {} cannot fit measured width {}",
                    bounds.width(),
                    metrics.size.width
                );
            }
        }
        assert!(found, "{name}: label was not painted");
    }
}

#[test]
fn hit_refresh_preserves_the_render_scenes_previous_frame() {
    use rui::core::Presenter;
    use rui::elements::element::{PointerEvent, PointerEventKind};
    let size = Size::new(300.0, 80.0);
    let mut presenter = Presenter::with_root(size, Button::new("Save"));
    presenter.layout(size).unwrap();
    presenter.paint();
    let rendered_frame = presenter.scene().frame();
    presenter.layout(size).unwrap();
    presenter.dispatch_pointer_event(&PointerEvent {
        kind: PointerEventKind::Move,
        position: Point::new(5.0, 5.0),
        button: None,
    });
    assert_eq!(
        presenter.scene().frame(),
        rendered_frame,
        "hit refresh is not a rendered frame"
    );
    presenter.paint();
    assert_eq!(
        presenter.scene().previous_frame(),
        rendered_frame,
        "damage must be compared with the frame the renderer actually saw"
    );
}

#[test]
fn disabled_button_does_not_show_an_actionable_focus_ring() {
    let id = ElementId::from(702);
    let mut session = mount(Size::new(300.0, 80.0), |_| {
        Button::new("Save").id(id).disabled(true)
    })
    .unwrap();
    let before = format!("{:?}", session.primitives());
    session.request_focus(Some(id));
    session.frame().unwrap();
    assert_eq!(before, format!("{:?}", session.primitives()));
}
