use super::support::*;

// ==================== ImageFit Enum Tests ====================

#[test]
fn test_image_fit_default() {
    let fit: ImageFit = ImageFit::default();
    assert_eq!(fit, ImageFit::Contain);
}

#[test]
fn test_image_fit_variants() {
    let test_cases = [
        (ImageFit::Cover, "Cover"),
        (ImageFit::Contain, "Contain"),
        (ImageFit::Fill, "Fill"),
        (ImageFit::None, "None"),
        (ImageFit::ScaleDown, "ScaleDown"),
    ];

    for (fit, expected_name) in test_cases {
        let debug_str = format!("{:?}", fit);
        assert!(
            debug_str.contains(expected_name),
            "Expected {} in {:?}",
            expected_name,
            debug_str
        );
    }
}

#[test]
fn test_image_fit_equality() {
    assert_eq!(ImageFit::Cover, ImageFit::Cover);
    assert_eq!(ImageFit::Contain, ImageFit::Contain);
    assert_ne!(ImageFit::Cover, ImageFit::Contain);
    assert_ne!(ImageFit::Fill, ImageFit::None);
}

#[test]
fn test_image_fit_clone() {
    let fit = ImageFit::Cover;
    let cloned = fit.clone();
    assert_eq!(fit, cloned);
}

#[test]
fn test_image_fit_copy() {
    let fit = ImageFit::ScaleDown;
    let copied = fit;
    assert_eq!(fit, copied);
}

// ==================== ImageState Enum Tests ====================

#[test]
fn test_image_state_variants() {
    let test_cases = [
        (ImageState::Loading, "Loading"),
        (ImageState::Loaded, "Loaded"),
        (ImageState::Error, "Error"),
    ];

    for (state, expected_name) in test_cases {
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains(expected_name));
    }
}

#[test]
fn test_image_state_equality() {
    assert_eq!(ImageState::Loading, ImageState::Loading);
    assert_eq!(ImageState::Loaded, ImageState::Loaded);
    assert_eq!(ImageState::Error, ImageState::Error);
    assert_ne!(ImageState::Loading, ImageState::Loaded);
    assert_ne!(ImageState::Loaded, ImageState::Error);
}

// ==================== ImageSource Enum Tests ====================

#[test]
fn test_image_source_file() {
    let source = ImageSource::File("test.png".to_string());
    if let ImageSource::File(path) = source {
        assert_eq!(path, "test.png");
    } else {
        panic!("Expected File variant");
    }
}

#[test]
fn test_image_source_data() {
    let data = vec![255u8, 0, 0, 255]; // One RGBA pixel
    let source = ImageSource::Data {
        data: data.clone(),
        width: 1,
        height: 1,
    };
    if let ImageSource::Data {
        data: d,
        width,
        height,
    } = source
    {
        assert_eq!(d, data);
        assert_eq!(width, 1);
        assert_eq!(height, 1);
    } else {
        panic!("Expected Data variant");
    }
}

#[test]
fn test_image_source_texture() {
    let source = ImageSource::Texture(42);
    if let ImageSource::Texture(id) = source {
        assert_eq!(id, 42);
    } else {
        panic!("Expected Texture variant");
    }
}

#[test]
fn test_image_source_clone() {
    let source = ImageSource::File("test.png".to_string());
    let cloned = source.clone();
    if let (ImageSource::File(p1), ImageSource::File(p2)) = (source, cloned) {
        assert_eq!(p1, p2);
    } else {
        panic!("Clone failed");
    }
}
