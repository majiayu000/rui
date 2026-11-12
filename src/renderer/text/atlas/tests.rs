use super::*;

#[test]
fn atlas_packs_glyphs_into_one_page_without_overlap() {
    let mut atlas = GlyphAtlas::new(64, usize::MAX);
    let slots = (0..20)
        .map(|_| atlas.allocate(10, 12).expect("allocate"))
        .collect::<Vec<_>>();

    assert_eq!(atlas.page_count(), 1);
    for (index, a) in slots.iter().enumerate() {
        assert!(a.rect.x + a.rect.width <= 64 && a.rect.y + a.rect.height <= 64);
        for b in &slots[index + 1..] {
            let apart = a.rect.x + a.rect.width + ATLAS_PADDING <= b.rect.x
                || b.rect.x + b.rect.width + ATLAS_PADDING <= a.rect.x
                || a.rect.y + a.rect.height + ATLAS_PADDING <= b.rect.y
                || b.rect.y + b.rect.height + ATLAS_PADDING <= a.rect.y;
            assert!(apart, "{a:?} and {b:?} overlap");
        }
    }
}

#[test]
fn atlas_adds_a_page_when_full_and_sizes_oversized_glyphs_to_fit() {
    let mut atlas = GlyphAtlas::new(32, usize::MAX);
    for _ in 0..4 {
        assert_eq!(atlas.allocate(15, 15).expect("allocate").page, 0);
    }
    assert_eq!(atlas.allocate(15, 15).expect("allocate").page, 1);

    let big = atlas.allocate(100, 40).expect("allocate");
    assert_eq!(big.page, 2);
    assert_eq!(atlas.page_extent(2), Some((101, 41)));
}

#[test]
fn atlas_reports_pressure_instead_of_dropping_a_glyph() {
    let mut atlas = GlyphAtlas::new(32, 32 * 32);
    for _ in 0..4 {
        atlas.allocate(15, 15).expect("allocate");
    }
    let err = atlas
        .allocate(15, 15)
        .expect_err("second page is over budget");
    assert!(err.is_pressure());
    assert_eq!(atlas.page_count(), 1);
}

fn request(content: &str) -> TextRequest<'_> {
    TextRequest::new(content, 14.0, 400, None, 1.2)
}

#[test]
fn shaped_text_and_glyphs_are_reused_across_frames() {
    let mut cache = TextGlyphCache::new();
    let draw_frame = |cache: &mut TextGlyphCache| {
        cache.begin_frame();
        let shaped = cache
            .shape(request("Hello atlas"))
            .expect("shape")
            .expect("ink");
        for glyph in &shaped.glyphs {
            cache.glyph(glyph, 28.0, 1).expect("glyph");
        }
        shaped
    };

    let first = draw_frame(&mut cache);
    let uploads = cache.take_uploads();
    assert!(!uploads.is_empty());
    let rasterized = cache.resource_stats().cache_misses;
    assert_eq!(cache.shape_misses(), 1);

    let second = draw_frame(&mut cache);
    assert!(Arc::ptr_eq(&first, &second), "shaping is kept");
    assert_eq!(cache.shape_misses(), 1);
    assert_eq!(cache.resource_stats().cache_misses, rasterized);
    assert!(
        cache.take_uploads().is_empty(),
        "nothing is rasterized again"
    );
    assert!(
        uploads
            .iter()
            .all(|upload| upload.pixels.iter().any(|&p| p > 0))
    );
}

#[test]
fn glyph_variants_are_rasterized_separately() {
    let mut cache = TextGlyphCache::new();
    let shaped = cache.shape(request("l")).expect("shape").expect("ink");
    let glyph = shaped.glyphs[0];
    let a = cache.glyph(&glyph, 28.0, 0).expect("glyph").expect("ink");
    let b = cache.glyph(&glyph, 28.0, 2).expect("glyph").expect("ink");
    let c = cache.glyph(&glyph, 14.0, 0).expect("glyph").expect("ink");
    assert_ne!(a.slot, b.slot);
    assert_ne!(a.slot, c.slot);
    assert!(
        c.slot.rect.height < a.slot.rect.height,
        "device size is honoured"
    );
    assert_eq!(cache.glyph(&glyph, 28.0, 0).expect("glyph"), Some(a));
}

#[test]
fn strings_drawn_last_frame_survive_shape_eviction() {
    let mut cache = TextGlyphCache::new();
    cache.begin_frame();
    for index in 0..=MAX_RETAINED_SHAPES as u64 {
        let entry = ShapedEntry {
            content: format!("old {index}"),
            font_size: 14.0,
            font_weight: 400,
            font_family: None,
            line_height: 1.2,
            text: None,
            last_used: cache.frame,
        };
        cache.shapes.insert(index, entry);
    }
    cache.begin_frame();
    let kept = cache.shape(request("kept")).expect("shape");
    let misses = cache.shape_misses();

    cache.begin_frame();
    cache.begin_frame();
    assert!(cache.shapes.len() <= MAX_RETAINED_SHAPES);
    let again = cache.shape(request("kept")).expect("shape");
    assert_eq!(
        cache.shape_misses(),
        misses,
        "string from the last frame is kept"
    );
    assert_eq!(kept, again);
}

/// A cache over an atlas of `pages` pages of 32 x 32, each holding four
/// 15 x 15 sprites.
fn small_cache(pages: usize) -> TextGlyphCache {
    TextGlyphCache::with_measurer(
        TextMeasureCache::new(),
        GlyphAtlas::new(32, pages * 32 * 32),
        usize::MAX,
    )
}

fn sprite(cache: &mut TextGlyphCache) -> Result<(AtlasSlot, u64), RendererResourceError> {
    cache.place_sprite(15, 15, vec![255; 15 * 15])
}

#[test]
fn pages_unfinished_frames_drew_from_are_kept_until_they_finish() {
    let mut cache = small_cache(1);
    cache.begin_frame();
    let first = sprite(&mut cache).expect("room");
    for _ in 0..3 {
        sprite(&mut cache).expect("room");
    }
    cache.take_cleared_pages();
    assert!(cache.use_sprite(first.0, first.1));

    // The next frame needs room while the GPU may still sample the page.
    cache.begin_frame();
    let err = sprite(&mut cache).expect_err("the only page is in flight");
    assert!(err.is_pressure());

    // Once that frame is done, its page is emptied and reused.
    cache.begin_frame();
    cache.set_completed_frame(cache.frame() - 1);
    let reused = sprite(&mut cache).expect("an idle page");
    assert_eq!(reused.0.page, 0);
    assert_ne!(reused.1, first.1, "the page's generation moved");
    assert_eq!(cache.take_cleared_pages(), [0]);
    assert!(!cache.use_sprite(first.0, first.1), "the old slot is gone");
    assert!(cache.use_sprite(reused.0, reused.1));
}

#[test]
fn the_frame_being_drawn_never_loses_a_page() {
    let mut cache = small_cache(1);
    cache.begin_frame();
    cache.begin_frame();
    cache.set_completed_frame(cache.frame() - 1);
    for _ in 0..4 {
        sprite(&mut cache).expect("room");
    }
    let err = sprite(&mut cache).expect_err("the page holds this frame's sprites");
    assert!(err.is_pressure());
    assert!(cache.take_cleared_pages().is_empty());
}

#[test]
fn glyphs_on_an_evicted_page_are_rasterized_again() {
    let mut cache = TextGlyphCache::with_measurer(
        TextMeasureCache::new(),
        GlyphAtlas::new(64, 64 * 64),
        usize::MAX,
    );
    let shaped = cache.shape(request("ab")).expect("shape").expect("ink");
    // One frame draws the glyph and fills the rest of the only page.
    cache.begin_frame();
    let a = cache
        .glyph(&shaped.glyphs[0], 28.0, 0)
        .expect("glyph")
        .expect("ink");
    while cache.place(false, 15, 15).is_ok() {}
    cache.take_uploads();
    cache.take_cleared_pages();

    // Once it is done, the next frame's sprite empties the page.
    cache.begin_frame();
    cache.set_completed_frame(cache.frame() - 1);
    sprite(&mut cache).expect("an idle page");
    assert_eq!(cache.take_cleared_pages(), [0]);
    assert_eq!(
        cache.resource_stats().evicted_entries,
        1,
        "the glyph was evicted"
    );

    cache.take_uploads();
    let again = cache
        .glyph(&shaped.glyphs[0], 28.0, 0)
        .expect("glyph")
        .expect("ink");
    assert_eq!((again.left, again.top), (a.left, a.top));
    assert_eq!(cache.take_uploads().len(), 1, "rasterized again");
}

#[test]
fn past_the_soft_page_count_idle_pages_are_reused_before_growing() {
    let mut cache = small_cache(usize::MAX / (32 * 32));
    cache.begin_frame();
    for _ in 0..SOFT_ATLAS_PAGES * 4 {
        sprite(&mut cache).expect("room");
    }
    assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES);
    cache.take_cleared_pages();

    // The next frame reuses every idle page before adding one.
    cache.begin_frame();
    cache.set_completed_frame(cache.frame() - 1);
    for _ in 0..SOFT_ATLAS_PAGES * 4 {
        sprite(&mut cache).expect("an idle page");
    }
    assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES, "no new page");
    assert_eq!(cache.take_cleared_pages().len(), SOFT_ATLAS_PAGES);
    // With none idle, the atlas grows.
    sprite(&mut cache).expect("a new page");
    assert_eq!(cache.atlas().page_count(), SOFT_ATLAS_PAGES + 1);
}

#[test]
fn bitmap_emoji_become_color_glyphs() {
    let mut cache = TextGlyphCache::new();
    let Some(shaped) = cache.shape(request("\u{1F600}")).expect("shape") else {
        return;
    };
    let glyph = shaped.glyphs[0];
    let family = &cache.measurer.fonts[glyph.font_index as usize].family;
    if !family.contains("Emoji") {
        // No color emoji font on this machine.
        return;
    }
    let emoji = cache
        .glyph(&glyph, 28.0, 0)
        .expect("glyph")
        .expect("an emoji has pixels");
    assert!(emoji.color, "{emoji:?}");
    let size = (emoji.slot.rect.width, emoji.slot.rect.height);
    // A 28 px font, descent to ascent, is about 21 px per em in Apple Color
    // Emoji, the size of its square images and of its advance.
    let advance = cache.measurer.fonts[glyph.font_index as usize]
        .font()
        .glyph(GlyphId(glyph.glyph_id))
        .scaled(Scale::uniform(28.0))
        .h_metrics()
        .advance_width;
    assert!(
        (size.0 as f32 - advance).abs() <= 1.0 && size.0 == size.1,
        "{size:?} against an advance of {advance}"
    );
    assert!(emoji.top < 0, "drawn above the baseline: {emoji:?}");
    let uploads = cache.take_color_uploads();
    assert_eq!(uploads.len(), 1);
    let pixels = &uploads[0].pixels;
    assert_eq!(pixels.len() as u32, size.0 * size.1 * 4);
    // Not gray: an emoji has color.
    assert!(
        pixels
            .chunks_exact(4)
            .any(|p| p[3] > 200 && p[0].abs_diff(p[2]) > 60)
    );
    assert!(
        cache.take_uploads().is_empty(),
        "nothing in the coverage atlas"
    );
    assert_eq!(cache.glyph(&glyph, 28.0, 0).expect("glyph"), Some(emoji));
}

#[test]
fn glyph_limit_is_a_pressure_error() {
    let mut cache =
        TextGlyphCache::with_measurer(TextMeasureCache::new(), GlyphAtlas::new(64, usize::MAX), 1);
    let shaped = cache.shape(request("ab")).expect("shape").expect("ink");
    cache
        .glyph(&shaped.glyphs[0], 14.0, 0)
        .expect("first glyph fits");
    let err = cache
        .glyph(&shaped.glyphs[1], 14.0, 0)
        .expect_err("second glyph is over the limit");
    assert!(matches!(err, TextError::Resource(err) if err.is_pressure()));
}
