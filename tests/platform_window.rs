use rui::core::{Size, WindowOptions};
use rui::platform::{
    PlatformImeEvent, PlatformInputEvent, PlatformWindow, PlatformWindowError, PlatformWindowEvent,
    PlatformWindowFeature, PlatformWindowFeatures, UnsupportedPlatformWindow,
    validate_window_options,
};
use rui::renderer::RendererError;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn platform_required_features_are_complete() {
    assert_eq!(
        PlatformWindowFeatures::REQUIRED,
        [
            PlatformWindowFeature::Lifecycle,
            PlatformWindowFeature::InputEvents,
            PlatformWindowFeature::Dpi,
            PlatformWindowFeature::Resizing,
            PlatformWindowFeature::Focus,
            PlatformWindowFeature::Clipboard,
            PlatformWindowFeature::RendererAttachment,
        ]
    );

    assert!(
        PlatformWindowFeatures::supported()
            .missing_required()
            .is_empty()
    );
    assert!(PlatformWindowFeatures::supported().supports(PlatformWindowFeature::MultiWindow));
}

#[test]
fn unsupported_platform_window_returns_explicit_errors() {
    let mut window =
        UnsupportedPlatformWindow::new("linux", WindowOptions::default().size(320.0, 240.0));

    assert_eq!(
        window.features().missing_required(),
        PlatformWindowFeatures::REQUIRED.to_vec()
    );

    let state = match window.state() {
        Ok(state) => state,
        Err(err) => panic!("state should be available for diagnostics: {err}"),
    };
    assert_eq!(state.size, Size::new(320.0, 240.0));
    assert_eq!(state.scale_factor, 1.0);
    assert!(!state.visible);
    assert!(!state.renderer_attached);

    assert_eq!(
        window.show(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Lifecycle,
        ))
    );
    assert_eq!(
        window.set_title("new title"),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Lifecycle,
        ))
    );
    assert_eq!(
        window.read_clipboard_text(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Clipboard,
        ))
    );
    assert_eq!(
        window.write_clipboard_text("copied"),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Clipboard,
        ))
    );
    assert_eq!(
        window.renderer_attachment(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::RendererAttachment,
        ))
    );
    assert_eq!(
        window.request_redraw(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Lifecycle,
        ))
    );
    assert_eq!(
        window.close(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::Lifecycle,
        ))
    );
}

#[test]
fn platform_window_option_validation_rejects_invalid_sizes() {
    let zero_size = WindowOptions::default().size(0.0, 240.0);
    assert!(matches!(
        validate_window_options(&zero_size),
        Err(PlatformWindowError::InvalidOptions { .. })
    ));

    let invalid_min_size = WindowOptions::default().min_size(0.0, 480.0);
    assert!(matches!(
        validate_window_options(&invalid_min_size),
        Err(PlatformWindowError::InvalidOptions { .. })
    ));

    let invalid_max_size = WindowOptions::default().max_size(640.0, -1.0);
    assert!(matches!(
        validate_window_options(&invalid_max_size),
        Err(PlatformWindowError::InvalidOptions { .. })
    ));

    let inverted_bounds = WindowOptions::default()
        .min_size(640.0, 480.0)
        .max_size(320.0, 240.0);
    assert!(matches!(
        validate_window_options(&inverted_bounds),
        Err(PlatformWindowError::InvalidOptions { .. })
    ));
}

#[test]
fn platform_window_events_identify_redraw_work() {
    assert!(PlatformWindowEvent::Created.requests_redraw());
    assert!(PlatformWindowEvent::Resized(Size::new(640.0, 480.0)).requests_redraw());
    assert!(PlatformWindowEvent::ScaleFactorChanged(2.0).requests_redraw());
    assert!(PlatformWindowEvent::FocusChanged(true).requests_redraw());
    assert!(PlatformWindowEvent::ApplicationActivated(true).requests_redraw());
    assert!(PlatformWindowEvent::Minimized(false).requests_redraw());
    assert!(PlatformWindowEvent::ReopenRequested.requests_redraw());
    assert!(PlatformWindowEvent::RedrawRequested.requests_redraw());
    assert!(
        PlatformWindowEvent::Input(PlatformInputEvent::Ime(PlatformImeEvent::Commit(
            String::from("text")
        )))
        .requests_redraw()
    );
    assert!(!PlatformWindowEvent::CloseRequested.requests_redraw());
    assert!(!PlatformWindowEvent::Minimized(true).requests_redraw());
    assert!(!PlatformWindowEvent::QuitRequested.requests_redraw());
}

#[test]
fn renderer_errors_flow_through_platform_window_error() {
    let renderer_error = RendererError::backend_unavailable("no device");
    let platform_error = PlatformWindowError::from(renderer_error.clone());

    assert_eq!(
        platform_error,
        PlatformWindowError::Renderer(renderer_error)
    );
}

#[test]
fn platform_boundary_docs_define_backend_readiness_gates() {
    let docs = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/platform-window-boundary.md"),
    )
    .expect("platform boundary docs should be readable");

    for required in [
        "## Backend Readiness Matrix",
        "macOS/AppKit + Metal",
        "Headless",
        "Windows",
        "Linux",
        "Web/WASM",
        "## Backend Adoption Gates",
        "UnsupportedPlatformWindow",
    ] {
        assert!(
            docs.contains(required),
            "platform boundary docs should include {required}"
        );
    }
}

#[test]
fn renderer_sources_do_not_depend_on_native_window_apis() {
    let mut files = Vec::new();
    collect_rs_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/renderer"),
        &mut files,
    );

    let forbidden_fragments = [
        "NSWindow",
        "objc2_app_kit",
        "CAMetalLayer",
        "NSEvent",
        "NSApplication",
        "contentView",
        "windowNumber",
    ];

    for file in files {
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(err) => panic!("failed to read {}: {err}", file.display()),
        };

        for fragment in forbidden_fragments {
            assert!(
                !source.contains(fragment),
                "renderer source {} should not reference native window API fragment {fragment}",
                file.display()
            );
        }
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[test]
fn platform_macos_backend_reports_implemented_window_features() {
    let backend = rui::platform::mac::MacWindowBackend::new();
    let features = backend.features();

    assert!(features.lifecycle);
    assert!(features.input_events);
    assert!(features.dpi);
    assert!(features.resizing);
    assert!(features.focus);
    assert!(features.clipboard);
    assert!(features.renderer_attachment);
    assert!(!features.multi_window);
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => panic!("failed to read {}: {err}", dir.display()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => panic!("failed to read directory entry in {}: {err}", dir.display()),
        };
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}
