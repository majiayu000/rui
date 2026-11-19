//! A family RUI cannot load draws in the default family instead of failing the
//! frame or the layout (#326).

use rui::core::Size;
use rui::elements::{div, span, styled_text, text};
use rui::renderer::Primitive;
use rui::testing::mount;

const UNKNOWN: &str = "No Such Family 326";

fn text_widths(primitives: &[Primitive]) -> Vec<(Option<String>, f32)> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text {
                bounds,
                font_family,
                ..
            } => Some((font_family.as_deref().map(str::to_string), bounds.width())),
            _ => None,
        })
        .collect()
}

#[test]
fn layout_with_an_unknown_family_measures_it_in_the_default_family() {
    let session = match mount(Size::new(400.0, 300.0), |_cx| {
        div()
            .child(text("Hello"))
            .child(text("Hello").font(UNKNOWN))
            .child(text("Hello wrapped").font(UNKNOWN).wrap())
            .child(div().font_family(UNKNOWN).child(text("Hello")))
            .child(styled_text([span("Hel"), span("lo").font(UNKNOWN)]))
    }) {
        Ok(session) => session,
        Err(err) => panic!("mount with an unknown family should succeed: {err}"),
    };

    let widths = text_widths(session.primitives());
    let default = widths
        .iter()
        .find(|(family, _)| family.is_none())
        .map(|(_, width)| *width)
        .expect("default text painted");
    let unknown = widths
        .iter()
        .find(|(family, _)| family.as_deref() == Some(UNKNOWN))
        .map(|(_, width)| *width)
        .expect("unknown-family text painted");
    assert_eq!(unknown, default, "{widths:?}");
}

#[cfg(target_os = "macos")]
mod metal {
    use super::UNKNOWN;
    use rui::core::color::Rgba;
    use rui::core::{Bounds, Corners, Edges, Size};
    use rui::elements::text::TextAlign;
    use rui::renderer::metal::MetalRenderer;
    use rui::renderer::text::BaseDirection;
    use rui::renderer::{Primitive, RendererError, Scene};

    const VIEWPORT: Size = Size {
        width: 160.0,
        height: 60.0,
    };

    fn label(x: f32, family: Option<&str>) -> Primitive {
        Primitive::Text {
            bounds: Bounds::from_xywh(x, 10.0, 60.0, 40.0),
            content: "Hi".into(),
            color: Rgba::new(0.0, 1.0, 0.0, 1.0),
            font_size: 24.0,
            font_weight: 400,
            font_family: family.map(Into::into),
            font_features: Default::default(),
            line_height: 1.2,
            align: TextAlign::Left,
            direction: BaseDirection::Auto,
        }
    }

    #[test]
    fn frame_with_an_unknown_family_renders_and_reports_it() {
        let mut renderer = match MetalRenderer::new() {
            Ok(renderer) => renderer,
            Err(RendererError::BackendUnavailable(_)) => return,
            Err(err) => panic!("metal renderer should initialize: {err}"),
        };
        let mut scene = Scene::new();
        scene.insert(Primitive::Quad {
            bounds: Bounds::from_xywh(140.0, 0.0, 20.0, 20.0),
            background: Rgba::new(1.0, 0.0, 0.0, 1.0),
            border_color: Rgba::TRANSPARENT,
            border_widths: Edges::default(),
            corner_radii: Corners::default(),
        });
        scene.insert(label(0.0, None));
        scene.insert(label(70.0, Some(UNKNOWN)));
        scene.finish();

        for _ in 0..2 {
            let pixels = renderer
                .capture_frame_pixels_at_scale(&scene, VIEWPORT, 1.0)
                .expect("a frame with an unknown family renders");
            let width = VIEWPORT.width as usize;
            let pixels = pixels.as_slice();
            let green = |x0: usize| {
                (10..50)
                    .flat_map(|y| (x0..x0 + 60).map(move |x| pixels[(y * width + x) * 4 + 1]))
                    .collect::<Vec<_>>()
            };
            assert!(green(0).iter().any(|&g| g > 128), "default text drew");
            assert_eq!(green(70), green(0), "unknown family drew as the default");
            let quad = (5 * width + 150) * 4;
            assert!(pixels[quad].max(pixels[quad + 2]) > 128, "the quad drew");
        }

        let diagnostics = renderer.diagnostics();
        assert_eq!(diagnostics.unknown_font_families, vec![UNKNOWN.to_string()]);
    }
}
