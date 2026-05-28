use super::support::*;

// ==================== Method Chaining Tests ====================

#[test]
fn test_image_chaining_all_methods() {
    let image = Image::from_file("test.png")
        .id(ElementId(1))
        .cover()
        .w(200.0)
        .h(150.0)
        .rounded(16.0)
        .alt("A beautiful sunset")
        .placeholder(Color::BLACK);

    assert_eq!(image.id, Some(ElementId(1)));
    assert_eq!(image.fit, ImageFit::Cover);
    assert_eq!(image.style.width, Some(200.0));
    assert_eq!(image.style.height, Some(150.0));
    assert_eq!(image.style.border.radius.top_left, 16.0);
    assert_eq!(image.alt_text, Some("A beautiful sunset".to_string()));
    assert_eq!(image.placeholder_color, Color::BLACK);
}

#[test]
fn test_image_chaining_size_then_individual() {
    let image = Image::from_file("test.png").size((100.0, 100.0)).w(200.0);

    // w() should override the width from size()
    assert_eq!(image.style.width, Some(200.0));
    assert_eq!(image.style.height, Some(100.0));
}

#[test]
fn test_image_chaining_fit_override() {
    let image = Image::from_file("test.png").cover().contain().fill();

    // Last fit should win
    assert_eq!(image.fit, ImageFit::Fill);
}

// ==================== calculate_dest_bounds Tests ====================

fn create_test_image() -> Image {
    Image::from_file("test.png")
}

#[test]
fn test_calculate_dest_bounds_fill() {
    let mut image = create_test_image().fill();
    image.intrinsic_size = Some(Size::new(100.0, 100.0));

    let container = Bounds::from_xywh(0.0, 0.0, 200.0, 300.0);
    let image_size = Size::new(100.0, 100.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Fill should return the container bounds exactly
    assert_eq!(result.x(), 0.0);
    assert_eq!(result.y(), 0.0);
    assert_eq!(result.width(), 200.0);
    assert_eq!(result.height(), 300.0);
}

#[test]
fn test_calculate_dest_bounds_contain_landscape_image() {
    let mut image = create_test_image().contain();
    image.intrinsic_size = Some(Size::new(200.0, 100.0));

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(200.0, 100.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Should scale by 2x (400/200), resulting in 400x200, centered vertically
    assert_eq!(result.width(), 400.0);
    assert_eq!(result.height(), 200.0);
    assert_eq!(result.x(), 0.0);
    assert_eq!(result.y(), 100.0); // (400-200)/2
}

#[test]
fn test_calculate_dest_bounds_contain_portrait_image() {
    let mut image = create_test_image().contain();
    image.intrinsic_size = Some(Size::new(100.0, 200.0));

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(100.0, 200.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Should scale by 2x (400/200), resulting in 200x400, centered horizontally
    assert_eq!(result.width(), 200.0);
    assert_eq!(result.height(), 400.0);
    assert_eq!(result.x(), 100.0); // (400-200)/2
    assert_eq!(result.y(), 0.0);
}

#[test]
fn test_calculate_dest_bounds_cover_landscape_image() {
    let mut image = create_test_image().cover();
    image.intrinsic_size = Some(Size::new(200.0, 100.0));

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(200.0, 100.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Should scale by 4x (400/100), resulting in 800x400, centered horizontally
    assert_eq!(result.width(), 800.0);
    assert_eq!(result.height(), 400.0);
    assert_eq!(result.x(), -200.0); // (400-800)/2
    assert_eq!(result.y(), 0.0);
}

#[test]
fn test_calculate_dest_bounds_cover_portrait_image() {
    let mut image = create_test_image().cover();
    image.intrinsic_size = Some(Size::new(100.0, 200.0));

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(100.0, 200.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Should scale by 4x (400/100), resulting in 400x800, centered vertically
    assert_eq!(result.width(), 400.0);
    assert_eq!(result.height(), 800.0);
    assert_eq!(result.x(), 0.0);
    assert_eq!(result.y(), -200.0); // (400-800)/2
}

#[test]
fn test_calculate_dest_bounds_none() {
    let image = create_test_image().fit(ImageFit::None);

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(100.0, 100.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Should not scale, just center
    assert_eq!(result.width(), 100.0);
    assert_eq!(result.height(), 100.0);
    assert_eq!(result.x(), 150.0); // (400-100)/2
    assert_eq!(result.y(), 150.0); // (400-100)/2
}

#[test]
fn test_calculate_dest_bounds_scale_down_fits() {
    let image = create_test_image().fit(ImageFit::ScaleDown);

    let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
    let image_size = Size::new(100.0, 100.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Image fits in container, should not scale
    assert_eq!(result.width(), 100.0);
    assert_eq!(result.height(), 100.0);
    assert_eq!(result.x(), 150.0); // (400-100)/2
    assert_eq!(result.y(), 150.0); // (400-100)/2
}

#[test]
fn test_calculate_dest_bounds_scale_down_too_large() {
    let image = create_test_image().fit(ImageFit::ScaleDown);

    let container = Bounds::from_xywh(0.0, 0.0, 100.0, 100.0);
    let image_size = Size::new(200.0, 200.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Image is too large, should scale down like contain
    assert_eq!(result.width(), 100.0);
    assert_eq!(result.height(), 100.0);
    assert_eq!(result.x(), 0.0);
    assert_eq!(result.y(), 0.0);
}

#[test]
fn test_calculate_dest_bounds_with_offset_container() {
    let image = create_test_image().contain();

    let container = Bounds::from_xywh(50.0, 100.0, 200.0, 200.0);
    let image_size = Size::new(100.0, 200.0);

    let result = image.calculate_dest_bounds(container, image_size);

    // Image should be centered within the offset container
    assert_eq!(result.width(), 100.0);
    assert_eq!(result.height(), 200.0);
    assert_eq!(result.x(), 100.0); // 50 + (200-100)/2
    assert_eq!(result.y(), 100.0);
}
