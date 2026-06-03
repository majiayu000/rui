use rui::ElementId;
use rui::advanced_ui::{ControlSize, TextField, Theme};
use rui::core::accessibility::AccessibilityContext;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::{Bounds, Size};
use rui::elements::Element;
use rui::elements::element::{LayoutContext, PaintContext};
use rui::renderer::{Primitive, Scene};
use taffy::prelude::{AvailableSpace, TaffyTree};

fn layout_and_paint(mut field: TextField, viewport: Size) -> Vec<Primitive> {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
    let node = field.layout(&mut layout_cx);
    taffy
        .compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        )
        .expect("text field layout should compute");
    let layout = taffy.layout(node).expect("text field layout should exist");
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
    field.paint(&mut paint_cx);
    scene.primitives().to_vec()
}

#[test]
fn advanced_text_field_read_only_allows_navigation_without_mutation() {
    let id = ElementId::new();
    let mut field = TextField::new("Package")
        .id(id)
        .value("rui-core")
        .read_only(true);

    let outcome = field
        .apply_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::none()))
        .expect("read-only navigation should be valid");
    assert!(!outcome.changed);
    assert_eq!(field.get_value(), "rui-core");

    let node = field
        .accessibility(&AccessibilityContext::new(Some(id)))
        .expect("text field accessibility should build")
        .expect("text field should expose a node");
    assert_eq!(node.a11y_text_caret(), Some("rui-core".len() - 1));
}

#[test]
fn advanced_text_field_theme_applies_to_normal_paint() {
    let theme = Theme::dark();
    let primitives = layout_and_paint(
        TextField::new("Search")
            .value("query")
            .theme(theme)
            .size(ControlSize::Large)
            .w(180.0),
        Size::new(220.0, 80.0),
    );

    let field_quad = primitives.iter().find_map(|primitive| match primitive {
        Primitive::Quad {
            background,
            border_color,
            ..
        } => Some((*background, *border_color)),
        _ => None,
    });
    assert_eq!(
        field_quad,
        Some((
            theme.colors.surface.to_rgba(),
            theme.colors.border.to_rgba()
        ))
    );

    let text = primitives.iter().find_map(|primitive| match primitive {
        Primitive::Text {
            content,
            color,
            font_size,
            ..
        } if content == "query" => Some((*color, *font_size)),
        _ => None,
    });
    assert_eq!(
        text,
        Some((
            theme.colors.text.to_rgba(),
            theme.text_size(ControlSize::Large)
        ))
    );
}
