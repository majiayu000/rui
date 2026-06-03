use rui::advanced_ui::{Theme, button};
use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Edges, Size};
use rui::core::style::Corners;
use rui::elements::{div, image, text};
use rui::renderer::Primitive;
use rui::testing::{
    PrimitiveSnapshot, PrimitiveSnapshotError, assert_primitive_snapshot_text, mount,
    primitive_snapshot,
};
use rui::{ImageFit, ImageSource};

#[test]
fn primitive_snapshot_is_deterministic_across_runs() {
    let session = match mount(Size::new(180.0, 80.0), |_cx| {
        div()
            .w(180.0)
            .h(80.0)
            .bg(rui::Color::hex(0x111827))
            .child(text("Snapshot").size(14.0).color(rui::Color::WHITE))
    }) {
        Ok(session) => session,
        Err(err) => panic!("headless mount should render snapshot fixture: {err}"),
    };

    let first = match session.primitive_snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("primitive snapshot should serialize: {err}"),
    };
    let second = match primitive_snapshot(session.primitives()) {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("primitive snapshot should serialize twice: {err}"),
    };

    assert_eq!(first, second);
    assert!(first.as_str().contains("primitive[0].quad"));
    assert!(first.as_str().contains("content=\"Snapshot\""));
}

#[test]
fn primitive_snapshot_reports_inline_diff() {
    let actual = PrimitiveSnapshot::new(String::from("primitive[0].pop_clip"));
    let error = match assert_primitive_snapshot_text(&actual, "primitive[0].push_clip") {
        Ok(_) => panic!("mismatched snapshot text should fail"),
        Err(err) => err,
    };

    match error {
        PrimitiveSnapshotError::Mismatch { diff, .. } => {
            assert!(diff.contains("- primitive[0].push_clip"));
            assert!(diff.contains("+ primitive[0].pop_clip"));
        }
        other => panic!("expected mismatch error, got {other}"),
    }
}

#[test]
fn primitive_snapshot_rejects_unstable_image_sources() {
    let primitives = [Primitive::Image {
        bounds: Bounds::from_xywh(0.0, 0.0, 20.0, 20.0),
        source: ImageSource::File(String::from("assets/image.png")),
        fit: ImageFit::Contain,
        corner_radii: Corners::ZERO,
        opacity: 1.0,
    }];

    let error = match primitive_snapshot(&primitives) {
        Ok(_) => panic!("file image snapshots should be rejected"),
        Err(err) => err,
    };

    assert_eq!(
        error,
        PrimitiveSnapshotError::UnsupportedImageSource {
            index: 0,
            source: "file"
        }
    );
}

#[test]
fn primitive_snapshot_serializes_data_images_stably() {
    let primitives = [Primitive::Image {
        bounds: Bounds::from_xywh(0.0, 0.0, 2.0, 2.0),
        source: ImageSource::Data {
            data: vec![255, 0, 0, 255],
            width: 1,
            height: 1,
        },
        fit: ImageFit::Fill,
        corner_radii: Corners::all(2.0),
        opacity: 0.5,
    }];
    let snapshot = match primitive_snapshot(&primitives) {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("data image snapshot should serialize: {err}"),
    };

    assert!(
        snapshot
            .as_str()
            .contains("source=data(width=1,height=1,len=4")
    );
    assert!(snapshot.as_str().contains("fnv64=0x"));
    assert!(snapshot.as_str().contains("opacity=0.500"));
}

#[test]
fn primitive_snapshot_keeps_manual_primitives_stable() {
    let primitives = [Primitive::Quad {
        bounds: Bounds::from_xywh(0.0, 0.0, 10.0, 12.0),
        background: Rgba::new(1.0, 0.0, 0.0, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    }];
    let snapshot = match primitive_snapshot(&primitives) {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("quad snapshot should serialize: {err}"),
    };

    assert_eq!(
        snapshot.as_str(),
        "primitive[0].quad bounds=(0.000, 0.000, 10.000, 12.000) background=rgba(1.000, 0.000, 0.000, 1.000) border=rgba(0.000, 0.000, 0.000, 0.000) border_widths=(0.000, 0.000, 0.000, 0.000) radii=(0.000, 0.000, 0.000, 0.000)"
    );
}

#[test]
fn primitive_snapshot_rejects_file_image_element_output() {
    let session = match mount(Size::new(40.0, 40.0), |_cx| {
        image("assets/image.png").w(40.0).h(40.0)
    }) {
        Ok(session) => session,
        Err(err) => panic!("headless image fixture should mount: {err}"),
    };

    let error = match session.primitive_snapshot() {
        Ok(_) => panic!("file image element snapshot should fail explicitly"),
        Err(err) => err,
    };

    match error {
        PrimitiveSnapshotError::UnsupportedImageSource { source, .. } => {
            assert_eq!(source, "file");
        }
        other => panic!("expected unsupported image source, got {other}"),
    }
}

#[test]
fn primitive_snapshot_captures_advanced_ui_theme_tokens() {
    let mut theme = Theme::high_contrast();
    theme.colors.primary.rest.background = rui::Color::hex(0x00ff00);
    theme.radius.control = 2.0;
    theme.typography.text_scale = 1.25;

    let session = match mount(Size::new(160.0, 60.0), move |_cx| button("Go").theme(theme)) {
        Ok(session) => session,
        Err(err) => panic!("headless mount should render themed advanced button: {err}"),
    };
    let snapshot = match session.primitive_snapshot() {
        Ok(snapshot) => snapshot,
        Err(err) => panic!("themed advanced button snapshot should serialize: {err}"),
    };

    assert!(
        snapshot
            .as_str()
            .contains("background=rgba(0.000, 1.000, 0.000, 1.000)")
    );
    assert!(
        snapshot
            .as_str()
            .contains("radii=(2.000, 2.000, 2.000, 2.000)")
    );
    assert!(snapshot.as_str().contains("font_size=17.500"));
}
