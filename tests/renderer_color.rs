#![cfg(target_os = "macos")]

//! Golden-style captures for color handling: the Metal renderer blends in linear
//! light, interpolates gradients in oklab and draws Gaussian shadows.

use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Edges, Size};
use rui::core::style::Corners;
use rui::elements::text::TextAlign;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::{Primitive, RendererError, Scene};

const WIDTH: usize = 100;
const HEIGHT: usize = 60;

fn capture(primitives: Vec<Primitive>) -> Option<Vec<u8>> {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let mut scene = Scene::new();
    for primitive in primitives {
        scene.insert(primitive);
    }
    scene.finish();
    Some(
        renderer
            .capture_frame_pixels(&scene, Size::new(WIDTH as f32, HEIGHT as f32))
            .expect("capture"),
    )
}

fn quad(bounds: Bounds, background: Rgba) -> Primitive {
    Primitive::Quad {
        bounds,
        background,
        border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(0.0),
    }
}

fn full(background: Rgba) -> Primitive {
    quad(
        Bounds::from_xywh(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        background,
    )
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 3] {
    let at = (y * WIDTH + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn to_byte(c: f32) -> f32 {
    linear_to_srgb(c.clamp(0.0, 1.0)) * 255.0
}

fn assert_near(actual: [u8; 3], expected: [f32; 3], tolerance: f32, what: &str) {
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (f32::from(*a) - e).abs() <= tolerance,
            "{what}: got {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn translucent_overlap_blends_in_linear_light() {
    let Some(pixels) = capture(vec![
        full(Rgba::new(0.0, 0.0, 0.0, 1.0)),
        quad(
            Bounds::from_xywh(0.0, 0.0, 60.0, 60.0),
            Rgba::new(1.0, 0.0, 0.0, 1.0),
        ),
        quad(
            Bounds::from_xywh(40.0, 0.0, 60.0, 60.0),
            Rgba::new(0.0, 1.0, 0.0, 0.5),
        ),
    ]) else {
        return;
    };

    // Half green over red is half the light of each: 0.5 linear, 188 in sRGB.
    // Blending the encoded values instead would give 128.
    let half = to_byte(0.5);
    assert_near(pixel(&pixels, 50, 30), [half, half, 0.0], 1.5, "overlap");
    assert_near(pixel(&pixels, 80, 30), [0.0, half, 0.0], 1.5, "over black");
    assert_near(pixel(&pixels, 20, 30), [255.0, 0.0, 0.0], 0.5, "opaque red");
}

fn oklab(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn oklab_to_srgb_bytes(lab: [f32; 3]) -> [f32; 3] {
    let [ll, a, b] = lab;
    let l = (ll + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (ll - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (ll - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
    .map(to_byte)
}

#[test]
fn gradient_midpoint_is_interpolated_in_oklab() {
    let (start, end) = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let Some(pixels) = capture(vec![Primitive::LinearGradient {
        bounds: Bounds::from_xywh(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        start: Rgba::new(start[0], start[1], start[2], 1.0),
        end: Rgba::new(end[0], end[1], end[2], 1.0),
        angle: 0.0,
        border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(0.0),
    }]) else {
        return;
    };

    let (a, b) = (oklab(start), oklab(end));
    for x in [10, 50, 90] {
        // Pixel centers sit half a pixel into the gradient.
        let t = (x as f32 + 0.5) / WIDTH as f32;
        let lab = [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
        assert_near(
            pixel(&pixels, x, 30),
            oklab_to_srgb_bytes(lab),
            2.0,
            &format!("gradient at x={x}"),
        );
    }
    // Mixing the encoded colors would give a dark (128, 0, 128).
    let [r, g, b] = pixel(&pixels, 50, 30);
    assert!(r > 128 && g > 60 && b > 128, "midpoint {r} {g} {b}");
}

/// Abramowitz and Stegun 7.1.26, accurate to 1.5e-7.
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_74 + t * (1.421_413_8 + t * (-1.453_152_1 + t * 1.061_405_4))));
    (1.0 - poly * (-x * x).exp()).copysign(x)
}

/// Coverage at `d` pixels outside one edge of a large box blurred by `sigma`.
fn blurred_edge(d: f32, sigma: f32) -> f32 {
    0.5 * (1.0 - erf(d / (sigma * std::f32::consts::SQRT_2)))
}

fn shadow(corner_radii: Corners, blur_radius: f32) -> Option<Vec<u8>> {
    capture(vec![
        full(Rgba::new(1.0, 1.0, 1.0, 1.0)),
        Primitive::Shadow {
            bounds: Bounds::from_xywh(30.0, 15.0, 40.0, 30.0),
            corner_radii,
            blur_radius,
            color: Rgba::new(0.0, 0.0, 0.0, 1.0),
        },
    ])
}

#[test]
fn shadow_falloff_follows_a_gaussian() {
    let blur = 8.0;
    let Some(pixels) = shadow(Corners::all(0.0), blur) else {
        return;
    };

    // Across the right edge at mid height the shadow is a Gaussian blurred step,
    // with sigma = blur / 2, times the blur of the 30 px tall box along y.
    let sigma = blur / 2.0;
    let rows = 1.0 - 2.0 * blurred_edge(15.0, sigma);
    // The last few columns hold the background quad's antialiased edge.
    for x in 60..WIDTH - 4 {
        let d = x as f32 + 0.5 - 70.0;
        let expected = to_byte(1.0 - blurred_edge(d, sigma) * rows);
        assert_near(
            pixel(&pixels, x, 30),
            [expected; 3],
            2.0,
            &format!("falloff at x={x}"),
        );
    }
}

#[test]
fn shadow_rounds_each_corner_with_its_own_radius() {
    let corners = Corners {
        top_left: 12.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };
    let blur = 4.0;
    let Some(pixels) = shadow(corners, blur) else {
        return;
    };

    // 1.5 px inside each corner of the box. The square corner is the product of
    // two blurred edges; the rounded one is outside its curve and stays lighter.
    let inside = blurred_edge(-1.5, blur / 2.0);
    let square = to_byte(1.0 - inside * inside);
    assert_near(pixel(&pixels, 68, 43), [square; 3], 2.0, "square corner");
    let top_left = pixel(&pixels, 31, 16)[0];
    assert!(
        f32::from(top_left) > square + 40.0,
        "rounded corner {top_left} vs square {square}"
    );
    // Away from the corners both edges are alike.
    assert_eq!(pixel(&pixels, 30, 30), pixel(&pixels, 69, 30));
}

fn ink(text: Rgba, background: Rgba) -> Option<f32> {
    let pixels = capture(vec![
        full(background),
        Primitive::Text {
            bounds: Bounds::from_xywh(5.0, 5.0, 90.0, 50.0),
            content: "Hamburgefonts".into(),
            color: text,
            font_size: 13.0,
            font_weight: 400,
            font_family: None,
            line_height: 1.2,
            align: TextAlign::Left,
        },
    ])?;
    let back = (background.g * 255.0).round();
    // Only the text box: the background quad's own antialiased edge is not ink.
    Some(
        (5..55)
            .flat_map(|y| (5..95).map(move |x| (x, y)))
            .map(|(x, y)| (f32::from(pixel(&pixels, x, y)[1]) - back).abs())
            .sum(),
    )
}

#[test]
fn text_weight_does_not_depend_on_its_color() {
    let (black, white) = (Rgba::new(0.0, 0.0, 0.0, 1.0), Rgba::new(1.0, 1.0, 1.0, 1.0));
    let (Some(dark_on_light), Some(light_on_dark)) = (ink(black, white), ink(white, black)) else {
        return;
    };

    // Blending raw coverage in linear light would make dark text thin and light
    // text bold. Coverage is corrected so both keep the ink they had when blending
    // encoded values.
    let ratio = dark_on_light / light_on_dark;
    assert!(
        (ratio - 1.0).abs() < 0.03,
        "dark on light {dark_on_light}, light on dark {light_on_dark}"
    );
}
