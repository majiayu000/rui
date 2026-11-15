use super::*;
use crate::core::geometry::{Bounds, Size};
use crate::core::view::{View, ViewContext};
use crate::elements::{Div, Element, ImageSource, ImageState, div, text};
use crate::renderer::{Primitive, Reach};
use crate::testing::mount_view;
use std::io::Cursor;

/// A `width` by `height` PNG, every pixel different.
fn png(width: u32, height: u32) -> Vec<u8> {
    let pixels = (0..width * height)
        .flat_map(|index| [(index * 37) as u8, (index * 11) as u8, 0x80, 0xff])
        .collect();
    let image = image::RgbaImage::from_raw(width, height, pixels).expect("pixels fit");
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner()
}

/// An image next to a label, so a frame can show what was painted again.
struct Gallery {
    asset: ImageAsset,
}

impl View for Gallery {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        div()
            .flex_row()
            .child(text("caption"))
            .child(cx.image(self.asset.clone()).size(Size::new(40.0, 30.0)))
    }
}

fn images(scene: &crate::renderer::Scene) -> Vec<(Bounds, ImageSource)> {
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Image { bounds, source, .. } => Some((*bounds, source.clone())),
            _ => None,
        })
        .collect()
}

fn texts(scene: &crate::renderer::Scene) -> Vec<String> {
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_image_shows_its_placeholder_until_decoded_then_only_it_is_painted_again() {
    let bytes: Arc<[u8]> = png(6, 4).into();
    let viewport = Size::new(200.0, 60.0);
    let mut session = mount_view(
        viewport,
        Gallery {
            asset: ImageAsset::Bytes(bytes),
        },
    )
    .expect("mount view");
    session.frame().expect("first frame");
    assert!(images(session.scene()).is_empty(), "nothing to draw yet");
    assert_eq!(session.app_context().image_bytes(), 0);

    // Decoding runs on the background executor, inline in a headless app.
    session.run_until_parked();
    session.frame().expect("frame after the decode");
    let drawn = images(session.scene());
    let [(bounds, ImageSource::Decoded(image))] = drawn.as_slice() else {
        panic!("expected one decoded image, got {drawn:?}");
    };
    assert_eq!((image.width(), image.height()), (6, 4));
    assert_eq!(session.app_context().image_bytes(), 6 * 4 * 4);
    // The caption was replayed; only the image and its placeholder changed.
    let Reach::Within(damage) =
        session
            .scene()
            .damage(Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height))
    else {
        panic!("damage is not bounded");
    };
    assert!(
        damage.x() >= bounds.x() - 1.0 && damage.width() <= bounds.width() + 2.0,
        "damage {damage:?} reaches past the image at {bounds:?}"
    );
    assert!(
        session.scene().replayed_len() > 0,
        "the caption was not replayed"
    );

    // Built again, it is the same image and nothing is painted again.
    session.app_context_mut().request_rebuild();
    session.frame().expect("rebuilt frame");
    assert_eq!(
        session
            .scene()
            .damage(Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height)),
        Reach::Nothing
    );
}

#[test]
fn a_failed_image_says_why() {
    for (asset, expected) in [
        (
            ImageAsset::Path("/no/such/image.png".into()),
            "failed to load image /no/such/image.png",
        ),
        (
            ImageAsset::Static(b"not an image"),
            "failed to decode 12 image bytes",
        ),
    ] {
        let mut session = mount_view(Size::new(200.0, 60.0), Gallery { asset }).expect("mount");
        session.frame().expect("first frame");
        session.run_until_parked();
        session.frame().expect("frame after the load");
        let shown = texts(session.scene());
        assert!(
            shown.iter().any(|text| text.starts_with(expected)),
            "{expected:?} not in {shown:?}"
        );
        assert!(images(session.scene()).is_empty());
    }
}

#[test]
fn an_alt_text_replaces_the_error() {
    let mut app = AppContext::new();
    app.image(ImageAsset::Static(b"broken"));
    app.run_until_parked();
    let failed = app.image(ImageAsset::Static(b"broken"));
    assert_eq!(failed.state(), ImageState::Error);
    assert!(failed.error().is_some_and(|error| error.contains("decode")));
}

#[test]
fn one_asset_is_decoded_once() {
    let bytes: Arc<[u8]> = png(3, 3).into();
    let mut app = AppContext::new();
    let first = app.image(ImageAsset::Bytes(Arc::clone(&bytes)));
    let second = app.image(ImageAsset::Bytes(Arc::clone(&bytes)));
    assert_eq!(
        (first.state(), second.state()),
        (ImageState::Loading, ImageState::Loading)
    );
    app.run_until_parked();
    let a = app.image(ImageAsset::Bytes(Arc::clone(&bytes)));
    let b = app.image(ImageAsset::Bytes(bytes));
    assert_eq!(a.state(), ImageState::Loaded);
    assert!(a.same_props(&b), "both show the same decoded image");
    assert_eq!(app.image_bytes(), 3 * 3 * 4);
}

#[test]
fn the_budget_drops_the_least_recently_built_image_nothing_shows() {
    let assets: Vec<ImageAsset> = (1..=3)
        .map(|size| ImageAsset::Bytes(png(size * 4, 4).into()))
        .collect();
    let mut app = AppContext::new();
    for asset in &assets {
        app.image(asset.clone());
    }
    app.run_until_parked();
    let all = (16 + 32 + 48) * 4;
    assert_eq!(
        app.image_bytes(),
        all,
        "nothing evicts what was never built"
    );

    // Built once each, then only the first is still shown.
    let shown = app.image(assets[0].clone());
    for asset in &assets[1..] {
        drop(app.image(asset.clone()));
    }
    app.set_image_budget(16 * 4 + 48 * 4);
    // The second was built before the third, so it goes first.
    assert_eq!(app.image_bytes(), 16 * 4 + 48 * 4);
    app.set_image_budget(0);
    assert_eq!(app.image_bytes(), 16 * 4, "the shown image stays");
    drop(shown);
    app.set_image_budget(0);
    assert_eq!(app.image_bytes(), 0);
    // Dropped, it loads again.
    assert_eq!(app.image(assets[0].clone()).state(), ImageState::Loading);
}
