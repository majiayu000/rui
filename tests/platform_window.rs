use rui::core::{Size, WindowOptions};
use rui::platform::{
    PlatformWindow, PlatformWindowError, PlatformWindowFeature, PlatformWindowFeatures,
    UnsupportedPlatformWindow, validate_window_options,
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
        window.renderer_attachment(),
        Err(PlatformWindowError::unsupported(
            "linux",
            PlatformWindowFeature::RendererAttachment,
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

    let inverted_bounds = WindowOptions::default()
        .min_size(640.0, 480.0)
        .max_size(320.0, 240.0);
    assert!(matches!(
        validate_window_options(&inverted_bounds),
        Err(PlatformWindowError::InvalidOptions { .. })
    ));
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

#[cfg(target_os = "macos")]
#[test]
fn platform_macos_backend_reports_implemented_window_features() {
    let backend = rui::platform::mac::MacWindowBackend::new();
    let features = backend.features();

    assert!(features.lifecycle);
    assert!(features.input_events);
    assert!(features.dpi);
    assert!(features.resizing);
    assert!(features.focus);
    assert!(features.renderer_attachment);
    assert!(!features.clipboard);

    assert_eq!(
        backend.unsupported_clipboard_error(),
        PlatformWindowError::unsupported("macos", PlatformWindowFeature::Clipboard)
    );
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
