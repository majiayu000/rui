use rui::advanced_ui::{
    Button, CrossAxisAlignment, MainAxisAlignment, Theme, ThemeDensity, ThemeMode, column,
    container, row, text,
};
use rui::core::style::{Dimension, Shadow, Style};
use rui::elements::element::{Element, LayoutContext, PaintContext, style_to_taffy};
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Size};
use taffy::prelude::{AvailableSpace, Dimension as TaffyDimension, TaffyTree};

fn painted_primitives(mut root: impl Element, viewport: Size) -> Vec<Primitive> {
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
        .expect("advanced layout should compute");

    let layout = taffy.layout(root_node).expect("root layout should exist");
    let root_bounds = Bounds::from_xywh(
        layout.location.x,
        layout.location.y,
        layout.size.width,
        layout.size.height,
    );

    let mut scene = Scene::new();
    let mut paint_cx = PaintContext::new(&mut scene, root_bounds, &taffy);
    root.paint(&mut paint_cx);
    scene.primitives().to_vec()
}

fn quad_bounds(primitives: &[Primitive]) -> Vec<Bounds> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .collect()
}

#[test]
fn advanced_layout_row_applies_padding_and_spacing_to_child_bounds() {
    let root = row()
        .w(160.0)
        .h(60.0)
        .padding(10.0)
        .spacing(8.0)
        .child(container().w(40.0).h(20.0).background(Color::hex(0xff0000)))
        .child(container().w(30.0).h(20.0).background(Color::hex(0x0000ff)));

    let bounds = quad_bounds(&painted_primitives(root, Size::new(160.0, 60.0)));

    assert_eq!(
        bounds,
        vec![
            Bounds::from_xywh(10.0, 10.0, 40.0, 20.0),
            Bounds::from_xywh(58.0, 10.0, 30.0, 20.0),
        ]
    );
}

#[test]
fn advanced_layout_column_aligns_children_on_both_axes() {
    let root = column()
        .w(100.0)
        .h(100.0)
        .padding(10.0)
        .spacing(4.0)
        .main_axis_alignment(MainAxisAlignment::Center)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .child(container().w(20.0).h(10.0).background(Color::hex(0xff0000)))
        .child(container().w(40.0).h(10.0).background(Color::hex(0x0000ff)));

    let bounds = quad_bounds(&painted_primitives(root, Size::new(100.0, 100.0)));

    assert_eq!(
        bounds,
        vec![
            Bounds::from_xywh(40.0, 38.0, 20.0, 10.0),
            Bounds::from_xywh(30.0, 52.0, 40.0, 10.0),
        ]
    );
}

#[test]
fn advanced_container_exposes_semantic_style_controls() {
    let panel = container()
        .size(Size::new(120.0, 80.0))
        .min_w(80.0)
        .min_h(40.0)
        .max_w(160.0)
        .max_h(120.0)
        .padding(12.0)
        .margin(4.0)
        .background(Color::WHITE)
        .border(1.0, Color::BLACK)
        .radius(6.0)
        .shadow(Shadow::new(0.0, 2.0, 8.0, Color::rgba(0.0, 0.0, 0.0, 0.2)));

    let style = panel.style();

    assert_eq!(style.width, Some(120.0));
    assert_eq!(style.height, Some(80.0));
    assert_eq!(style.min_width, Some(80.0));
    assert_eq!(style.min_height, Some(40.0));
    assert_eq!(style.max_width, Some(160.0));
    assert_eq!(style.max_height, Some(120.0));
    assert_eq!(style.padding, rui::Edges::all(12.0));
    assert_eq!(style.margin, rui::Edges::all(4.0));
    assert_eq!(style.border.width, rui::Edges::all(1.0));
    assert_eq!(style.border.color, Color::BLACK);
    assert_eq!(style.border.radius, rui::Corners::all(6.0));
    assert!(style.shadow.is_some());
}

#[test]
fn structured_dimensions_convert_to_taffy_without_float_sentinels() {
    let mut style = Style::new();
    style.set_width_dimension(Dimension::percent(50.0));
    style.set_height_dimension(Dimension::fill());
    style.set_min_width_dimension(Dimension::px(120.0));
    style.set_max_height_dimension(Dimension::auto());

    let taffy_style = style_to_taffy(&style);

    assert_eq!(taffy_style.size.width, TaffyDimension::Percent(0.5));
    assert_eq!(taffy_style.size.height, TaffyDimension::Percent(1.0));
    assert_eq!(taffy_style.min_size.width, TaffyDimension::Length(120.0));
    assert_eq!(taffy_style.max_size.height, TaffyDimension::Auto);
}

#[test]
fn responsive_dimensions_resize_without_horizontal_clipping() {
    for viewport in [
        Size::new(320.0, 240.0),
        Size::new(760.0, 520.0),
        Size::new(1200.0, 800.0),
    ] {
        let root = column()
            .w_full()
            .h_full()
            .padding(16.0)
            .spacing(8.0)
            .child(
                container()
                    .w_percent(50.0)
                    .h(28.0)
                    .background(Color::hex(0xff0000)),
            )
            .child(
                container()
                    .w_full()
                    .h(28.0)
                    .background(Color::hex(0x0000ff)),
            )
            .child(
                row()
                    .w_full()
                    .h(32.0)
                    .spacing(8.0)
                    .child(
                        container()
                            .w_percent(35.0)
                            .h(24.0)
                            .background(Color::hex(0x00ff00)),
                    )
                    .child(
                        container()
                            .w_percent(35.0)
                            .h(24.0)
                            .background(Color::hex(0xff00ff)),
                    ),
            );

        let bounds = quad_bounds(&painted_primitives(root, viewport));
        assert!(!bounds.is_empty());
        for bounds in bounds {
            assert!(bounds.x() >= 0.0, "bounds clipped left: {bounds:?}");
            assert!(
                bounds.x() + bounds.width() <= viewport.width + 0.1,
                "bounds clipped right in {viewport:?}: {bounds:?}"
            );
        }
    }
}

#[test]
fn advanced_text_wraps_existing_text_painting() {
    let root = container().child(text("Status").size(16.0).semibold().color(Color::BLACK));
    let primitives = painted_primitives(root, Size::new(120.0, 40.0));

    let text_primitive = primitives.iter().find_map(|primitive| match primitive {
        Primitive::Text {
            content,
            font_size,
            font_weight,
            ..
        } => Some((content.as_str(), *font_size, *font_weight)),
        _ => None,
    });

    assert_eq!(text_primitive, Some(("Status", 16.0, 600)));
}

#[test]
fn advanced_theme_presets_cover_light_dark_and_high_contrast() {
    let light = Theme::light();
    let dark = Theme::dark();
    let high_contrast = Theme::high_contrast();

    assert_eq!(light.mode, ThemeMode::Light);
    assert_eq!(dark.mode, ThemeMode::Dark);
    assert_eq!(high_contrast.mode, ThemeMode::HighContrast);
    assert_ne!(light.colors.surface, dark.colors.surface);
    assert_ne!(
        light.colors.primary.rest.background,
        high_contrast.colors.primary.rest.background
    );
}

#[test]
fn advanced_control_theme_density_changes_layout_tokens() {
    let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
    let primitives = painted_primitives(Button::new("Dense").theme(theme), Size::new(240.0, 80.0));

    let button_bounds = primitives.iter().find_map(|primitive| match primitive {
        Primitive::Quad { bounds, .. } => Some(*bounds),
        _ => None,
    });

    assert_eq!(button_bounds.map(|bounds| bounds.height()), Some(54.0));
}
