#[allow(dead_code)]
#[path = "../examples/advanced_ui_controls.rs"]
mod advanced_ui_controls;
#[allow(dead_code)]
#[path = "../examples/advanced_ui_layout.rs"]
mod advanced_ui_layout;

use rui::core::Size;
use rui::testing::{HeadlessError, mount};

#[test]
fn example_smoke_advanced_ui_controls_renders_one_frame() {
    let session = match mount(Size::new(760.0, 520.0), |_cx| {
        advanced_ui_controls::controls_panel()
    }) {
        Ok(session) => session,
        Err(err) => panic!("advanced controls example should render headlessly: {err}"),
    };

    assert!(!session.primitives().is_empty());
}

#[test]
fn example_smoke_advanced_ui_layout_renders_one_frame() {
    let session = match mount(Size::new(920.0, 620.0), |_cx| {
        advanced_ui_layout::dashboard()
    }) {
        Ok(session) => session,
        Err(err) => panic!("advanced layout example should render headlessly: {err}"),
    };

    assert!(!session.primitives().is_empty());
}

#[test]
fn example_smoke_reports_missing_frame_capture_backend() {
    let session = match mount(Size::new(760.0, 520.0), |_cx| {
        advanced_ui_controls::controls_panel()
    }) {
        Ok(session) => session,
        Err(err) => panic!("advanced controls example should render headlessly: {err}"),
    };

    let error = match session.capture_current_frame() {
        Ok(_) => panic!("example smoke capture should report missing backend"),
        Err(err) => err,
    };

    match error {
        HeadlessError::Renderer(err) => {
            assert!(err.to_string().contains("backend unavailable"));
        }
        other => panic!("expected renderer backend error, got {other}"),
    }
}

#[allow(dead_code)]
#[path = "../examples/assets.rs"]
mod assets;

#[test]
fn example_smoke_assets_shows_every_fit_the_decoded_photo_and_icons() {
    use rui::renderer::Primitive;
    use rui::{ImageSource, testing::mount_view};

    let mut session = match mount_view(Size::new(720.0, 520.0), assets::Gallery::new()) {
        Ok(session) => session,
        Err(err) => panic!("assets example should render headlessly: {err}"),
    };
    let count = |session: &rui::testing::HeadlessViewSession<assets::Gallery>| {
        let (mut data, mut decoded, mut paths) = (0, 0, 0);
        let mut texts = Vec::new();
        for primitive in session.scene().primitives() {
            match primitive {
                Primitive::Image {
                    source: ImageSource::Data { .. },
                    ..
                } => data += 1,
                Primitive::Image {
                    source: ImageSource::Decoded(_),
                    ..
                } => decoded += 1,
                Primitive::Path { .. } => paths += 1,
                Primitive::Text { content, .. } => texts.push(content.clone()),
                _ => {}
            }
        }
        (data, decoded, paths, texts)
    };
    session.frame().expect("first frame");
    let (data, decoded, _, _) = count(&session);
    assert_eq!(
        (data, decoded),
        (5, 0),
        "one image per fit, the photo still loading"
    );

    session.run_until_parked();
    session.frame().expect("frame after the decode");
    let (data, decoded, paths, texts) = count(&session);
    assert_eq!((data, decoded), (5, 1));
    assert!(paths >= 6, "{paths} icon paths");
    assert!(texts.iter().any(|text| text == "no such file"));
    assert!(
        texts
            .iter()
            .any(|text| text.starts_with("svg <circle>: fill: \"url(#glow)\""))
    );
}
