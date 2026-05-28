use super::support::*;

// ==================== Element Trait Tests ====================

#[test]
fn test_element_id_trait() {
    let image = Image::from_file("test.png");
    assert!(Element::id(&image).is_none());

    let image_with_id = Image::from_file("test.png").id(ElementId(42));
    assert_eq!(Element::id(&image_with_id), Some(ElementId(42)));
}

#[test]
fn test_element_style_trait() {
    let image = Image::from_file("test.png").w(200.0).h(100.0);

    let style = image.style();
    assert_eq!(style.width, Some(200.0));
    assert_eq!(style.height, Some(100.0));
}

// ==================== Default Values Tests ====================

#[test]
fn test_image_default_values() {
    let image = Image::from_file("test.png");

    // Check all default values
    assert!(image.id.is_none());
    assert_eq!(image.fit, ImageFit::Contain);
    assert_eq!(image.state, ImageState::Loading);
    assert!(image.alt_text.is_none());
    assert!(image.texture_id.is_none());
    assert!(image.intrinsic_size.is_none());
    assert!(image.on_load.is_none());
    assert!(image.on_error.is_none());
    assert!(image.layout_node.is_none());

    // Style defaults
    assert!(image.style.width.is_none());
    assert!(image.style.height.is_none());
    assert!(image.style.border.radius.is_zero());
}

#[test]
fn test_image_placeholder_default_color() {
    let image = Image::from_file("test.png");
    // Default placeholder color is Color::hex(0xf3f4f6)
    let expected = Color::hex(0xf3f4f6);
    assert_eq!(image.placeholder_color, expected);
}

// ==================== Helper Function Tests ====================

#[test]
fn test_image_helper_function() {
    let img = image("path/to/file.png");

    if let ImageSource::File(path) = &img.source {
        assert_eq!(path, "path/to/file.png");
    } else {
        panic!("Expected File source from image() helper");
    }
}

// ==================== Edge Cases Tests ====================

#[test]
fn test_image_empty_path() {
    let image = Image::from_file("");

    if let ImageSource::File(path) = &image.source {
        assert_eq!(path, "");
    } else {
        panic!("Expected File source");
    }
}

#[test]
fn test_image_empty_data() {
    let image = Image::from_data(vec![], 0, 0);

    if let ImageSource::Data {
        data,
        width,
        height,
    } = &image.source
    {
        assert!(data.is_empty());
        assert_eq!(*width, 0);
        assert_eq!(*height, 0);
    } else {
        panic!("Expected Data source");
    }
}

#[test]
fn test_image_zero_dimensions() {
    let image = Image::from_file("test.png").w(0.0).h(0.0);

    assert_eq!(image.style.width, Some(0.0));
    assert_eq!(image.style.height, Some(0.0));
}

#[test]
fn test_image_negative_dimensions() {
    let image = Image::from_file("test.png").w(-100.0).h(-50.0);

    // Negative values should be accepted (validation happens elsewhere)
    assert_eq!(image.style.width, Some(-100.0));
    assert_eq!(image.style.height, Some(-50.0));
}

#[test]
fn test_image_large_dimensions() {
    let image = Image::from_file("test.png").w(f32::MAX).h(f32::MAX);

    assert_eq!(image.style.width, Some(f32::MAX));
    assert_eq!(image.style.height, Some(f32::MAX));
}

#[test]
fn test_image_zero_radius() {
    let image = Image::from_file("test.png").rounded(0.0);

    let radius = image.style.border.radius;
    assert!(radius.is_zero());
}

#[test]
fn test_image_empty_alt_text() {
    let image = Image::from_file("test.png").alt("");
    assert_eq!(image.alt_text, Some("".to_string()));
}

#[test]
fn test_image_unicode_alt_text() {
    let image = Image::from_file("test.png").alt("A photo of a cat");
    assert_eq!(image.alt_text, Some("A photo of a cat".to_string()));
}

#[test]
fn test_image_unicode_path() {
    let image = Image::from_file("image.png");

    if let ImageSource::File(path) = &image.source {
        assert_eq!(path, "image.png");
    } else {
        panic!("Expected File source");
    }
}
