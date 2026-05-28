use super::support::*;

// ==================== Image Constructor Tests ====================

#[test]
fn test_image_new() {
    let source = ImageSource::File("test.png".to_string());
    let image = Image::new(source);

    assert!(image.id.is_none());
    assert_eq!(image.fit, ImageFit::Contain);
    assert_eq!(image.state, ImageState::Loading);
    assert!(image.alt_text.is_none());
    assert!(image.texture_id.is_none());
    assert!(image.intrinsic_size.is_none());
    assert!(image.layout_node.is_none());
}

#[test]
fn test_image_from_file() {
    let image = Image::from_file("assets/logo.png");

    if let ImageSource::File(path) = &image.source {
        assert_eq!(path, "assets/logo.png");
    } else {
        panic!("Expected File source");
    }
}

#[test]
fn test_image_from_file_with_string() {
    let path = String::from("assets/logo.png");
    let image = Image::from_file(path);

    if let ImageSource::File(p) = &image.source {
        assert_eq!(p, "assets/logo.png");
    } else {
        panic!("Expected File source");
    }
}

#[test]
fn test_image_from_data() {
    let pixel_data = vec![255u8, 128, 64, 255, 0, 0, 0, 255];
    let image = Image::from_data(pixel_data.clone(), 2, 1);

    if let ImageSource::Data {
        data,
        width,
        height,
    } = &image.source
    {
        assert_eq!(*data, pixel_data);
        assert_eq!(*width, 2);
        assert_eq!(*height, 1);
    } else {
        panic!("Expected Data source");
    }
}

#[test]
fn test_image_from_texture() {
    let image = Image::from_texture(123);

    if let ImageSource::Texture(id) = &image.source {
        assert_eq!(*id, 123);
    } else {
        panic!("Expected Texture source");
    }
}

// ==================== Builder Method Tests ====================

#[test]
fn test_image_id() {
    let id = ElementId(42);
    let image = Image::from_file("test.png").id(id);

    assert_eq!(image.id, Some(id));
}

#[test]
fn test_image_fit_method() {
    let test_cases = [
        (ImageFit::Cover, ImageFit::Cover),
        (ImageFit::Contain, ImageFit::Contain),
        (ImageFit::Fill, ImageFit::Fill),
        (ImageFit::None, ImageFit::None),
        (ImageFit::ScaleDown, ImageFit::ScaleDown),
    ];

    for (input, expected) in test_cases {
        let image = Image::from_file("test.png").fit(input);
        assert_eq!(image.fit, expected);
    }
}

#[test]
fn test_image_cover() {
    let image = Image::from_file("test.png").cover();
    assert_eq!(image.fit, ImageFit::Cover);
}

#[test]
fn test_image_contain() {
    let image = Image::from_file("test.png").contain();
    assert_eq!(image.fit, ImageFit::Contain);
}

#[test]
fn test_image_fill() {
    let image = Image::from_file("test.png").fill();
    assert_eq!(image.fit, ImageFit::Fill);
}

#[test]
fn test_image_w() {
    let image = Image::from_file("test.png").w(200.0);
    assert_eq!(image.style.width, Some(200.0));
}

#[test]
fn test_image_h() {
    let image = Image::from_file("test.png").h(150.0);
    assert_eq!(image.style.height, Some(150.0));
}

#[test]
fn test_image_size_tuple() {
    let image = Image::from_file("test.png").size((300.0, 200.0));
    assert_eq!(image.style.width, Some(300.0));
    assert_eq!(image.style.height, Some(200.0));
}

#[test]
fn test_image_size_struct() {
    let size = Size::new(400.0, 300.0);
    let image = Image::from_file("test.png").size(size);
    assert_eq!(image.style.width, Some(400.0));
    assert_eq!(image.style.height, Some(300.0));
}

#[test]
fn test_image_rounded() {
    let image = Image::from_file("test.png").rounded(8.0);
    let radius = image.style.border.radius;
    assert_eq!(radius.top_left, 8.0);
    assert_eq!(radius.top_right, 8.0);
    assert_eq!(radius.bottom_right, 8.0);
    assert_eq!(radius.bottom_left, 8.0);
}

#[test]
fn test_image_rounded_full() {
    let image = Image::from_file("test.png").rounded_full();
    let radius = image.style.border.radius;
    assert_eq!(radius.top_left, 9999.0);
    assert_eq!(radius.top_right, 9999.0);
    assert_eq!(radius.bottom_right, 9999.0);
    assert_eq!(radius.bottom_left, 9999.0);
}

#[test]
fn test_image_alt() {
    let image = Image::from_file("test.png").alt("A test image");
    assert_eq!(image.alt_text, Some("A test image".to_string()));
}

#[test]
fn test_image_alt_with_string() {
    let alt = String::from("Description");
    let image = Image::from_file("test.png").alt(alt);
    assert_eq!(image.alt_text, Some("Description".to_string()));
}

#[test]
fn test_image_placeholder() {
    let image = Image::from_file("test.png").placeholder(Color::RED);
    assert_eq!(image.placeholder_color, Color::RED);
}

#[test]
fn test_image_placeholder_hex() {
    let image = Image::from_file("test.png").placeholder(Color::hex(0xFF00FF));
    let expected = Color::hex(0xFF00FF);
    assert_eq!(image.placeholder_color, expected);
}

#[test]
fn test_image_on_load() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let image = Image::from_file("test.png").on_load(move || {
        called_clone.store(true, Ordering::SeqCst);
    });

    assert!(image.on_load.is_some());

    // Call the handler
    if let Some(handler) = &image.on_load {
        handler();
    }
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn test_image_on_error() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    let image = Image::from_file("test.png").on_error(move || {
        called_clone.store(true, Ordering::SeqCst);
    });

    assert!(image.on_error.is_some());

    // Call the handler
    if let Some(handler) = &image.on_error {
        handler();
    }
    assert!(called.load(Ordering::SeqCst));
}
