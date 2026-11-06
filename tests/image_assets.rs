//! Images decoded off the UI thread draw like the same pixels given as data,
//! and the renderer lets go of their textures once nothing shows them.

use rui::core::view::{View, ViewContext};
use rui::renderer::metal::MetalRenderer;
use rui::renderer::{Primitive, RendererError, RendererResourceKind, Scene};
use rui::testing::mount_view;
use rui::{Bounds, Corners, DecodedImage, Div, ImageAsset, ImageFit, ImageSource, Size, div};
use std::io::Cursor;
use std::sync::Arc;

const SCALE: f32 = 2.0;

fn renderer() -> Option<MetalRenderer> {
    match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    }
}

fn gradient(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| {
            let (x, y) = (index % width, index / width);
            [
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                0x60,
                0xff,
            ]
        })
        .collect()
}

fn scene(sources: impl IntoIterator<Item = (ImageSource, ImageFit)>) -> Scene {
    let mut scene = Scene::new();
    for (index, (source, fit)) in sources.into_iter().enumerate() {
        scene.insert(Primitive::Image {
            bounds: Bounds::from_xywh(4.0 + index as f32 * 50.0, 6.5, 44.0, 30.0),
            source,
            fit,
            corner_radii: Corners::all(index as f32 * 2.0),
            opacity: 1.0,
        });
    }
    scene
}

fn live(renderer: &MetalRenderer, kind: RendererResourceKind) -> usize {
    renderer
        .diagnostics()
        .resources
        .iter()
        .filter(|stats| stats.kind == kind)
        .map(|stats| stats.live_entries)
        .sum()
}

const FITS: [ImageFit; 5] = [
    ImageFit::Fill,
    ImageFit::Contain,
    ImageFit::Cover,
    ImageFit::None,
    ImageFit::ScaleDown,
];

#[test]
fn decoded_images_draw_like_the_same_pixels_as_data() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let viewport = Size::new(260.0, 44.0);
    let (width, height) = (9, 5);
    let decoded = Arc::new(DecodedImage::new(width, height, gradient(width, height)).unwrap());
    let as_data = scene(FITS.map(|fit| {
        let data = gradient(width, height);
        (
            ImageSource::Data {
                data,
                width,
                height,
            },
            fit,
        )
    }));
    let as_decoded = scene(FITS.map(|fit| (ImageSource::Decoded(Arc::clone(&decoded)), fit)));
    let expected = renderer
        .capture_frame_pixels_at_scale(&as_data, viewport, SCALE)
        .expect("data capture");
    let actual = renderer
        .capture_frame_pixels_at_scale(&as_decoded, viewport, SCALE)
        .expect("decoded capture");
    assert!(expected.iter().any(|&byte| byte != 0), "nothing was drawn");
    assert!(
        expected == actual,
        "decoded pixels draw differently from data"
    );
}

#[test]
fn textures_of_images_nothing_holds_are_released() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let viewport = Size::new(120.0, 44.0);
    let decoded = Arc::new(DecodedImage::new(4, 4, gradient(4, 4)).unwrap());
    let shown = scene([(ImageSource::Decoded(Arc::clone(&decoded)), ImageFit::Fill)]);
    renderer
        .capture_frame_pixels_at_scale(&shown, viewport, SCALE)
        .expect("capture");
    let (images, textures) = (
        live(&renderer, RendererResourceKind::Image),
        live(&renderer, RendererResourceKind::Texture),
    );
    assert!(images >= 1 && textures >= 1);

    // Held by the scene, so kept while it is drawn again.
    drop(decoded);
    renderer
        .capture_frame_pixels_at_scale(&shown, viewport, SCALE)
        .expect("capture");
    assert_eq!(live(&renderer, RendererResourceKind::Image), images);

    drop(shown);
    renderer
        .capture_frame_pixels_at_scale(&Scene::new(), viewport, SCALE)
        .expect("capture");
    assert_eq!(live(&renderer, RendererResourceKind::Image), images - 1);
    assert_eq!(live(&renderer, RendererResourceKind::Texture), textures - 1);
}

struct Photo(ImageAsset);

impl View for Photo {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        div().child(cx.image(self.0.clone()).fill().size(Size::new(40.0, 20.0)))
    }
}

#[test]
fn an_asynchronously_loaded_image_reaches_the_screen() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let (width, height) = (8, 4);
    let mut png = Cursor::new(Vec::new());
    image::RgbaImage::from_raw(width, height, gradient(width, height))
        .unwrap()
        .write_to(&mut png, image::ImageFormat::Png)
        .expect("encode png");
    let bytes: Arc<[u8]> = png.into_inner().into();
    let viewport = Size::new(40.0, 20.0);
    let mut session = mount_view(viewport, Photo(ImageAsset::Bytes(bytes))).expect("mount");
    session.frame().expect("first frame");
    let placeholder = renderer
        .capture_frame_pixels_at_scale(session.scene(), viewport, SCALE)
        .expect("placeholder capture");
    session.run_until_parked();
    session.frame().expect("frame after the decode");
    let loaded = renderer
        .capture_frame_pixels_at_scale(session.scene(), viewport, SCALE)
        .expect("loaded capture");

    // The same frame with the pixels given as data.
    let mut reference = Scene::new();
    for primitive in session.scene().primitives() {
        let mut primitive = primitive.clone();
        if let Primitive::Image { source, .. } = &mut primitive {
            assert!(matches!(source, ImageSource::Decoded(_)), "{source:?}");
            *source = ImageSource::Data {
                data: gradient(width, height),
                width,
                height,
            };
        }
        reference.insert(primitive);
    }
    let expected = renderer
        .capture_frame_pixels_at_scale(&reference, viewport, SCALE)
        .expect("reference capture");
    assert!(placeholder != loaded, "the decoded image never showed");
    assert!(loaded == expected, "the decoded image draws differently");
}
