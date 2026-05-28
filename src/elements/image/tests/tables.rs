use super::support::*;

// ==================== Table-Driven Tests ====================

#[test]
fn test_fit_methods_table() {
    struct TestCase {
        name: &'static str,
        builder: fn(Image) -> Image,
        expected: ImageFit,
    }

    let test_cases = [
        TestCase {
            name: "cover",
            builder: |img| img.cover(),
            expected: ImageFit::Cover,
        },
        TestCase {
            name: "contain",
            builder: |img| img.contain(),
            expected: ImageFit::Contain,
        },
        TestCase {
            name: "fill",
            builder: |img| img.fill(),
            expected: ImageFit::Fill,
        },
        TestCase {
            name: "fit(Cover)",
            builder: |img| img.fit(ImageFit::Cover),
            expected: ImageFit::Cover,
        },
        TestCase {
            name: "fit(Contain)",
            builder: |img| img.fit(ImageFit::Contain),
            expected: ImageFit::Contain,
        },
        TestCase {
            name: "fit(Fill)",
            builder: |img| img.fit(ImageFit::Fill),
            expected: ImageFit::Fill,
        },
        TestCase {
            name: "fit(None)",
            builder: |img| img.fit(ImageFit::None),
            expected: ImageFit::None,
        },
        TestCase {
            name: "fit(ScaleDown)",
            builder: |img| img.fit(ImageFit::ScaleDown),
            expected: ImageFit::ScaleDown,
        },
    ];

    for case in test_cases {
        let image = (case.builder)(Image::from_file("test.png"));
        assert_eq!(image.fit, case.expected, "Failed for case: {}", case.name);
    }
}

#[test]
fn test_size_methods_table() {
    struct TestCase {
        name: &'static str,
        builder: fn(Image) -> Image,
        expected_width: Option<f32>,
        expected_height: Option<f32>,
    }

    let test_cases = [
        TestCase {
            name: "w only",
            builder: |img| img.w(100.0),
            expected_width: Some(100.0),
            expected_height: None,
        },
        TestCase {
            name: "h only",
            builder: |img| img.h(200.0),
            expected_width: None,
            expected_height: Some(200.0),
        },
        TestCase {
            name: "w and h",
            builder: |img| img.w(100.0).h(200.0),
            expected_width: Some(100.0),
            expected_height: Some(200.0),
        },
        TestCase {
            name: "size",
            builder: |img| img.size((300.0, 400.0)),
            expected_width: Some(300.0),
            expected_height: Some(400.0),
        },
    ];

    for case in test_cases {
        let image = (case.builder)(Image::from_file("test.png"));
        assert_eq!(
            image.style.width, case.expected_width,
            "Width mismatch for case: {}",
            case.name
        );
        assert_eq!(
            image.style.height, case.expected_height,
            "Height mismatch for case: {}",
            case.name
        );
    }
}

#[test]
fn test_constructor_variants_table() {
    struct TestCase {
        name: &'static str,
        image: Image,
        check: fn(&ImageSource) -> bool,
    }

    let test_cases = [
        TestCase {
            name: "from_file",
            image: Image::from_file("test.png"),
            check: |s| matches!(s, ImageSource::File(_)),
        },
        TestCase {
            name: "from_data",
            image: Image::from_data(vec![0u8; 4], 1, 1),
            check: |s| matches!(s, ImageSource::Data { .. }),
        },
        TestCase {
            name: "from_texture",
            image: Image::from_texture(1),
            check: |s| matches!(s, ImageSource::Texture(_)),
        },
    ];

    for case in test_cases {
        assert!(
            (case.check)(&case.image.source),
            "Source check failed for case: {}",
            case.name
        );
    }
}

#[test]
fn test_calculate_dest_bounds_table() {
    struct TestCase {
        name: &'static str,
        fit: ImageFit,
        container: Bounds,
        image_size: Size,
        expected_width: f32,
        expected_height: f32,
    }

    let test_cases = [
        TestCase {
            name: "Fill - stretch to container",
            fit: ImageFit::Fill,
            container: Bounds::from_xywh(0.0, 0.0, 200.0, 100.0),
            image_size: Size::new(50.0, 50.0),
            expected_width: 200.0,
            expected_height: 100.0,
        },
        TestCase {
            name: "Contain - landscape in square",
            fit: ImageFit::Contain,
            container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            image_size: Size::new(200.0, 100.0),
            expected_width: 100.0,
            expected_height: 50.0,
        },
        TestCase {
            name: "Cover - landscape in square",
            fit: ImageFit::Cover,
            container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            image_size: Size::new(200.0, 100.0),
            expected_width: 200.0,
            expected_height: 100.0,
        },
        TestCase {
            name: "None - no scaling",
            fit: ImageFit::None,
            container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            image_size: Size::new(50.0, 50.0),
            expected_width: 50.0,
            expected_height: 50.0,
        },
    ];

    for case in test_cases {
        let image = Image::from_file("test.png").fit(case.fit);
        let result = image.calculate_dest_bounds(case.container, case.image_size);

        assert!(
            (result.width() - case.expected_width).abs() < 0.001,
            "Width mismatch for case: {}. Expected {}, got {}",
            case.name,
            case.expected_width,
            result.width()
        );
        assert!(
            (result.height() - case.expected_height).abs() < 0.001,
            "Height mismatch for case: {}. Expected {}, got {}",
            case.name,
            case.expected_height,
            result.height()
        );
    }
}
