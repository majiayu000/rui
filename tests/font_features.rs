//! OpenType features on text: JetBrains Mono's `calt` turns `->` into an
//! arrow unless it is off (#329).

use rui::renderer::text::{
    FontFeature, FontFeatures, TextGlyphCache, TextMeasureCache, TextRequest, register_font,
};

/// `-`, `>` and `0` of JetBrains Mono Regular (OFL-1.1, see fonts/OFL.txt),
/// with its `calt` and `liga` lookups.
const JETBRAINS_MONO: &[u8] = include_bytes!("fonts/JetBrainsMono-subset.ttf");

fn request<'a>(content: &'a str, features: &'a [FontFeature]) -> TextRequest<'a> {
    TextRequest::new(content, 18.0, 400, Some("JetBrains Mono"), 1.2).with_font_features(features)
}

fn glyph_ids(cache: &mut TextMeasureCache, content: &str, features: &[FontFeature]) -> Vec<u16> {
    let plan = cache
        .shape_single_line(request(content, features))
        .expect("shape");
    plan.glyphs().iter().map(|glyph| glyph.glyph_id).collect()
}

#[test]
fn calt_off_keeps_the_arrow_as_two_glyphs() {
    register_font(JETBRAINS_MONO).expect("register");
    let mut cache = TextMeasureCache::new();
    let separate = [
        glyph_ids(&mut cache, "-", &[])[0],
        glyph_ids(&mut cache, ">", &[])[0],
    ];

    let ligated = glyph_ids(&mut cache, "->", &[]);
    assert_ne!(ligated, separate, "calt is on by default");
    let off = FontFeatures::no_ligatures();
    assert_eq!(glyph_ids(&mut cache, "->", off.as_slice()), separate);
    assert_eq!(
        glyph_ids(&mut cache, "->", &[FontFeature::new(*b"calt", 0)]),
        separate
    );
    // The shaping plan built with features did not replace the default one.
    assert_eq!(glyph_ids(&mut cache, "->", &[]), ligated);

    // The measure cache keeps both.
    cache
        .measure_single_line(request("->", &[]))
        .expect("measure");
    cache
        .measure_single_line(request("->", off.as_slice()))
        .expect("measure");
    assert_eq!(cache.stats().measure_misses, 2);

    // So does the glyph cache the renderer draws from.
    let mut glyphs = TextGlyphCache::new();
    let mut drawn = |features: &[FontFeature]| -> Vec<u16> {
        let text = glyphs.shape(request("->", features)).expect("shape");
        let text = text.expect("ink");
        text.glyphs.iter().map(|glyph| glyph.glyph_id).collect()
    };
    assert_eq!(drawn(&[]), ligated);
    assert_eq!(drawn(off.as_slice()), separate);
}

#[cfg(target_os = "macos")]
#[test]
fn a_text_primitive_draws_with_its_features() {
    use rui::core::Bounds;
    use rui::core::color::Rgba;
    use rui::elements::text::TextAlign;
    use rui::renderer::metal::MetalRenderer;
    use rui::renderer::text::BaseDirection;
    use rui::renderer::{Primitive, RendererError, Scene};

    register_font(JETBRAINS_MONO).expect("register");
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let viewport = rui::core::Size::new(120.0, 40.0);
    let draw = |renderer: &mut MetalRenderer, font_features: FontFeatures| {
        let mut scene = Scene::new();
        scene.insert(Primitive::Text {
            bounds: Bounds::from_xywh(10.0, 5.0, 100.0, 30.0),
            content: "->".into(),
            color: Rgba::new(0.0, 1.0, 0.0, 1.0),
            font_size: 24.0,
            font_weight: 400,
            font_family: Some("JetBrains Mono".into()),
            font_features,
            line_height: 1.2,
            align: TextAlign::Left,
            direction: BaseDirection::Auto,
        });
        scene.finish();
        renderer
            .capture_frame_pixels_at_scale(&scene, viewport, 1.0)
            .expect("capture")
    };
    let ligated = draw(&mut renderer, FontFeatures::NONE);
    let separate = draw(&mut renderer, FontFeatures::no_ligatures());
    assert!(separate.iter().skip(1).step_by(4).any(|&g| g > 128), "drew");
    assert!(ligated != separate, "calt off drew different pixels");
}
