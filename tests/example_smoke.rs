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
