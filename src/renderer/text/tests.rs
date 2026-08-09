use super::*;

fn request(content: &str) -> TextRequest<'_> {
    TextRequest::new(content, 20.0, 400, None, 1.2)
}

fn measure(cache: &mut TextMeasureCache, request: TextRequest<'_>) -> TextMetrics {
    match cache.measure_single_line(request) {
        Ok(metrics) => metrics,
        Err(err) => panic!("text measurement failed: {:?}", err),
    }
}

fn shape(cache: &mut TextMeasureCache, request: TextRequest<'_>) -> TextShapePlan {
    match cache.shape_single_line(request) {
        Ok(plan) => plan,
        Err(err) => panic!("text shaping failed: {:?}", err),
    }
}

fn assert_close(left: f32, right: f32) {
    assert!(
        (left - right).abs() <= TEXT_BOUNDS_TOLERANCE,
        "{left} should be within {TEXT_BOUNDS_TOLERANCE} of {right}"
    );
}

#[test]
fn empty_text_measures_zero_without_font() {
    let mut cache = TextMeasureCache::without_font();
    let metrics = measure(&mut cache, request(""));
    assert_eq!(metrics, TextMetrics::empty());
}

#[test]
fn missing_font_is_an_explicit_error() {
    let mut cache = TextMeasureCache::without_font();
    assert_eq!(
        cache.measure_single_line(request("Hello")),
        Err(TextError::MissingFont)
    );
}

#[test]
fn unsupported_font_family_is_an_explicit_error() {
    let mut cache = TextMeasureCache::new();
    let result = cache.measure_single_line(TextRequest::new(
        "Hello",
        20.0,
        400,
        Some("Unknown Family"),
        1.2,
    ));
    let err = match result {
        Ok(metrics) => panic!("expected font family error, got {:?}", metrics),
        Err(err) => err,
    };
    assert_eq!(
        err,
        TextError::UnsupportedFontFamily("Unknown Family".to_string())
    );
}

#[test]
fn same_count_wide_and_narrow_text_measure_differently() {
    let mut cache = TextMeasureCache::new();
    let narrow = measure(&mut cache, request("iiii"));
    let wide = measure(&mut cache, request("WWWW"));
    assert!(wide.size.width > narrow.size.width);
}

#[test]
fn line_height_changes_height_not_width() {
    let mut cache = TextMeasureCache::new();
    let compact = measure(&mut cache, TextRequest::new("Hello", 18.0, 400, None, 1.0));
    let loose = measure(&mut cache, TextRequest::new("Hello", 18.0, 400, None, 1.8));

    assert_eq!(compact.size.width, loose.size.width);
    assert!((loose.size.height - 32.4).abs() < 0.01);
}

#[test]
fn shaping_splits_mixed_script_runs_and_keeps_grapheme_clusters() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("A界e\u{301}"));

    assert_eq!(plan.clusters().len(), 3);
    assert_eq!(plan.clusters()[0].script, TextScript::Latin);
    assert_eq!(plan.clusters()[1].script, TextScript::Cjk);
    assert_eq!(plan.clusters()[2].text, "e\u{301}");
    assert_eq!(plan.clusters()[2].script, TextScript::Latin);
    assert!(plan.runs().iter().any(|run| run.script == TextScript::Cjk));
    assert_eq!(measure(&mut cache, request("A界e\u{301}")), plan.metrics());
}

#[test]
fn shaping_records_positioned_glyphs_that_sum_to_the_advance() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("office"));

    assert!(!plan.glyphs().is_empty());
    assert!(
        plan.glyphs()
            .iter()
            .all(|glyph| glyph.byte_end > glyph.byte_start)
    );
    let glyph_advance = plan
        .glyphs()
        .iter()
        .map(|glyph| glyph.advance_width)
        .sum::<f32>();
    assert_close(glyph_advance, plan.metrics().advance_width);
}

#[test]
fn shaping_cluster_offsets_follow_positioned_glyphs() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("AV"));

    if plan.glyphs().len() != plan.clusters().len() {
        return;
    }

    for cluster in plan.clusters() {
        let glyph = plan
            .glyphs()
            .iter()
            .find(|glyph| glyph.byte_start == cluster.byte_start)
            .expect("cluster should map to a positioned glyph");
        assert_close(cluster.x_offset, glyph.x_offset);
        assert_close(cluster.advance_width, glyph.advance_width);
    }
}

#[test]
fn shaping_reports_ligature_substitution_when_the_font_applies_one() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("office"));

    if let Some(TextShapeDiagnostic::LigatureSubstitution {
        glyph_count,
        grapheme_count,
        ..
    }) = plan
        .diagnostics()
        .iter()
        .find(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::LigatureSubstitution { .. }))
    {
        assert!(*glyph_count < *grapheme_count);
    } else {
        assert_eq!(plan.glyphs().len(), plan.clusters().len());
    }
}

#[test]
fn shaped_ligature_glyph_ranges_cover_collapsed_clusters() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("office"));

    if !plan
        .diagnostics()
        .iter()
        .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::LigatureSubstitution { .. }))
    {
        return;
    }

    let collapsed = plan.glyphs().iter().any(|glyph| {
        plan.clusters()
            .iter()
            .filter(|cluster| {
                cluster.byte_start >= glyph.byte_start && cluster.byte_end <= glyph.byte_end
            })
            .count()
            > 1
    });
    assert!(
        collapsed,
        "expected one glyph range to cover every cluster collapsed into a ligature"
    );
}

#[test]
fn rtl_cluster_offsets_follow_visual_order() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("שלום"));
    let rtl_clusters = plan
        .clusters()
        .iter()
        .filter(|cluster| cluster.direction == TextDirection::RightToLeft)
        .collect::<Vec<_>>();

    assert!(rtl_clusters.len() > 1);
    assert!(
        rtl_clusters
            .windows(2)
            .all(|pair| { pair[0].x_offset > pair[1].x_offset })
    );
}

#[test]
fn shaping_marks_emoji_clusters_without_splitting_zwj_sequences() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("build 🧑‍💻"));

    assert!(
        plan.clusters().iter().any(|cluster| {
            cluster.text == "🧑‍💻" && cluster.script == TextScript::Emoji
        })
    );
}

#[test]
fn shaping_reports_mixed_direction_as_observable_diagnostic() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("abc שלום"));

    assert_eq!(plan.direction(), TextDirection::Mixed);
    assert!(plan.runs().iter().any(|run| {
        run.direction == TextDirection::RightToLeft && run.script == TextScript::Rtl
    }));
    assert!(plan.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        TextShapeDiagnostic::MixedDirection {
            direction: TextDirection::Mixed
        }
    )));
}

#[test]
fn shaping_recognizes_non_latin_ltr_scripts() {
    let mut cache = TextMeasureCache::new();

    let cyrillic = shape(&mut cache, request("Привет"));
    assert_eq!(cyrillic.direction(), TextDirection::LeftToRight);

    let mixed = shape(&mut cache, request("Привет שלום"));
    assert_eq!(mixed.direction(), TextDirection::Mixed);
}

#[test]
fn shaping_treats_emoji_as_neutral_for_bidi_diagnostics() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("שלום 🙂"));

    assert_eq!(plan.direction(), TextDirection::RightToLeft);
    assert!(
        plan.clusters().iter().any(|cluster| {
            cluster.text == "🙂" && cluster.direction == TextDirection::Neutral
        })
    );
    assert!(
        !plan
            .diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MixedDirection { .. }))
    );
}

#[test]
fn shaping_treats_arabic_indic_digits_as_numeric_neutral() {
    let mut cache = TextMeasureCache::new();

    let digits = shape(&mut cache, request("١٢٣"));
    assert_eq!(digits.direction(), TextDirection::Neutral);
    assert!(
        digits
            .clusters()
            .iter()
            .all(|cluster| cluster.script == TextScript::Number)
    );

    let latin_with_digits = shape(&mut cache, request("abc ١٢٣"));
    assert_eq!(latin_with_digits.direction(), TextDirection::LeftToRight);
    assert!(
        !latin_with_digits
            .diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MixedDirection { .. }))
    );
}

#[test]
fn shaping_recognizes_rtl_scripts_from_bidi_properties() {
    let mut cache = TextMeasureCache::new();

    let adlam = shape(&mut cache, request("\u{1e900}\u{1e901}"));
    assert_eq!(adlam.direction(), TextDirection::RightToLeft);
    assert!(
        adlam
            .clusters()
            .iter()
            .all(|cluster| cluster.script == TextScript::Rtl)
    );

    let old_hungarian = shape(&mut cache, request("\u{10c80}\u{10c81}"));
    assert_eq!(old_hungarian.direction(), TextDirection::RightToLeft);

    let mixed = shape(&mut cache, request("abc \u{1e900}"));
    assert_eq!(mixed.direction(), TextDirection::Mixed);
    assert!(mixed.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        TextShapeDiagnostic::MixedDirection {
            direction: TextDirection::Mixed
        }
    )));
}

#[test]
fn rasterization_filters_control_characters_like_measurement() {
    let mut cache = TextRasterCache::new();
    let with_control = match cache.resolve(request("A\tW")) {
        Ok(Some(entry)) => entry,
        Ok(None) => panic!("expected rasterized control-character text entry"),
        Err(err) => panic!("text rasterization failed: {:?}", err),
    };
    let without_control = match cache.resolve(request("AW")) {
        Ok(Some(entry)) => entry,
        Ok(None) => panic!("expected rasterized filtered text entry"),
        Err(err) => panic!("text rasterization failed: {:?}", err),
    };

    assert_eq!(with_control.metrics, without_control.metrics);
    assert_eq!(with_control.pixels, without_control.pixels);
}

#[test]
fn empty_raster_requests_do_not_require_fonts() {
    let mut cache = TextRasterCache {
        measurer: TextMeasureCache::without_font(),
        resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
        entries: HashMap::new(),
    };

    assert!(matches!(cache.resolve(request("")), Ok(None)));
    assert!(matches!(
        cache.resolve(TextRequest::new("Hello", 0.0, 400, None, 1.2)),
        Ok(None)
    ));
    assert!(matches!(
        cache.resolve(TextRequest::new("Hello", 20.0, 400, None, 0.0)),
        Ok(None)
    ));
}

#[test]
fn raster_cache_hits_do_not_require_reshaping() {
    let mut cache = TextRasterCache {
        measurer: TextMeasureCache::without_font(),
        resources: RendererResourceCache::unbounded(RendererResourceKind::Glyph),
        entries: HashMap::new(),
    };
    let key = GlyphResourceKey::new("Cached", 20.0, 400, None, 1.2);
    let entry = Arc::new(TextRasterEntry {
        id: 7,
        metrics: TextMetrics {
            size: Size::new(1.0, 1.0),
            ink_bounds: Bounds::from_xywh(0.0, 0.0, 1.0, 1.0),
            advance_width: 1.0,
        },
        pixels: vec![255, 255, 255, 255],
    });
    cache.entries.insert(key, entry.clone());

    let resolved = match cache.resolve(request("Cached")) {
        Ok(Some(entry)) => entry,
        Ok(None) => panic!("expected cached raster entry"),
        Err(err) => panic!("cache hit should not shape text: {:?}", err),
    };
    assert!(Arc::ptr_eq(&resolved, &entry));
}

#[test]
fn shaping_reports_missing_glyph_and_required_fallback() {
    let mut cache = TextMeasureCache::new();
    let plan = shape(&mut cache, request("\u{10ffff}"));

    assert!(
        plan.diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::MissingGlyph { .. }))
    );
    assert!(
        plan.diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::FallbackRequired { .. }))
    );
    assert!(
        plan.diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::FallbackFailed { .. }))
    );
}

#[test]
fn shaping_surfaces_font_fallback_when_primary_lacks_a_cluster() {
    let mut cache = TextMeasureCache::new();
    let families = cache
        .fonts
        .iter()
        .map(|font| font.family.clone())
        .collect::<Vec<_>>();

    let fallback_plan = families.into_iter().find_map(|family| {
        let plan = shape(
            &mut cache,
            TextRequest::new("A界", 20.0, 400, Some(&family), 1.2),
        );
        plan.diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic, TextShapeDiagnostic::FallbackApplied { .. }))
            .then_some(plan)
    });

    let plan = match fallback_plan {
        Some(plan) => plan,
        None => panic!("expected an installed primary font to need deterministic CJK fallback"),
    };
    assert!(
        plan.clusters().iter().any(|cluster| {
            cluster.script == TextScript::Cjk && !cluster.font_family.is_empty()
        })
    );
}

#[test]
fn raster_bounds_match_measured_ink_bounds_within_tolerance() {
    let mut cache = TextRasterCache::new();
    let entry = match cache.resolve(request("Bounds")) {
        Ok(Some(entry)) => entry,
        Ok(None) => panic!("expected rasterized text entry"),
        Err(err) => panic!("text rasterization failed: {:?}", err),
    };
    let raster_height = entry.metrics.ink_bounds.height().ceil().max(1.0);
    let raster_width = (entry.pixels.len() / 4) as f32 / raster_height;

    assert!((raster_width - entry.metrics.ink_bounds.width()).abs() <= TEXT_BOUNDS_TOLERANCE);
}

#[test]
fn measure_caches_share_one_parsed_font_set() {
    let first = TextMeasureCache::new();
    let second = TextMeasureCache::new();

    if !first.has_fonts() {
        assert!(
            !second.has_fonts(),
            "font availability must not silently differ between adjacent cache constructions"
        );
        return;
    }
    assert!(first.shares_fonts_with(&second));
}

#[test]
fn repeated_measurement_reuses_the_cached_metrics_entry() {
    let mut cache = TextMeasureCache::new();
    if !cache.has_fonts() {
        assert_eq!(
            cache.measure_single_line(request("cached once")),
            Err(TextError::MissingFont)
        );
        return;
    }

    let first = measure(&mut cache, request("cached once"));
    let after_first = cache.cached_metrics_len();
    let second = measure(&mut cache, request("cached once"));

    assert_eq!(first, second);
    assert_eq!(after_first, 1);
    assert_eq!(cache.cached_metrics_len(), after_first);
}

#[test]
fn a_long_lived_empty_cache_retries_font_loading() {
    let mut cache = TextMeasureCache::retryable_without_font();
    let result = cache.measure_single_line(request("retry font loading"));

    if cache.has_fonts() {
        assert!(result.is_ok());
        assert!(!cache.fonts_retryable);
    } else {
        assert_eq!(result, Err(TextError::MissingFont));
        assert!(cache.fonts_retryable);
    }
}

#[test]
fn non_not_found_font_io_is_exposed_as_text_error() {
    let mut cache = TextMeasureCache::with_font_error_for_test(
        "/denied/font.ttf",
        std::io::ErrorKind::PermissionDenied,
        "permission denied by test",
    );

    assert_eq!(
        cache.measure_single_line(request("fail closed")),
        Err(TextError::FontIo {
            path: "/denied/font.ttf".to_string(),
            kind: std::io::ErrorKind::PermissionDenied,
            message: "permission denied by test".to_string(),
        })
    );
}

#[test]
fn raster_entries_are_invalidated_when_font_generation_changes() {
    let mut cache = TextRasterCache::new();
    let entry = match cache.resolve(request("generation")) {
        Ok(Some(entry)) => entry,
        Ok(None) => panic!("expected a raster entry"),
        Err(TextError::MissingFont) => return,
        Err(error) => panic!("unexpected raster error: {error:?}"),
    };
    assert!(entry.pixels.len() > 0);
    assert_eq!(cache.resource_stats().live_entries, 1);

    let previous_generation = cache.measurer.font_generation;
    cache.measurer.font_generation = previous_generation.saturating_add(1);
    cache.invalidate_for_font_generation_change(previous_generation);

    assert!(cache.entries.is_empty());
    assert_eq!(cache.resource_stats().live_entries, 0);
    assert_eq!(cache.resource_stats().live_bytes, 0);
}

#[test]
fn retained_metrics_stay_bounded_for_ever_changing_text() {
    let mut cache = TextMeasureCache::without_font();

    for index in 0..(MAX_RETAINED_METRICS + 64) {
        cache.retain_metrics(
            TextMeasureKey {
                content: format!("frame {index}"),
                size_bits: 12.0f32.to_bits(),
                line_height_bits: 1.2f32.to_bits(),
                font_weight: 400,
                font_family: "test".to_string(),
            },
            TextMetrics::empty(),
        );
        assert!(
            cache.cached_metrics_len() <= MAX_RETAINED_METRICS,
            "retained metrics must stay bounded across frames"
        );
    }
    assert!(cache.cached_metrics_len() > 0);
}

#[test]
fn retained_metrics_stay_within_the_byte_budget() {
    let mut cache = TextMeasureCache::without_font();
    let chunk = "x".repeat(MAX_RETAINED_METRIC_BYTES / 3);

    for index in 0..6 {
        cache.retain_metrics(
            TextMeasureKey {
                content: format!("{index}{chunk}"),
                size_bits: 12.0f32.to_bits(),
                line_height_bits: 1.2f32.to_bits(),
                font_weight: 400,
                font_family: "test".to_string(),
            },
            TextMetrics::empty(),
        );
        assert!(cache.cached_metric_bytes() <= MAX_RETAINED_METRIC_BYTES);
    }

    let entries_before_oversized_key = cache.cached_metrics_len();
    cache.retain_metrics(
        TextMeasureKey {
            content: "y".repeat(MAX_RETAINED_METRIC_BYTES + 1),
            size_bits: 12.0f32.to_bits(),
            line_height_bits: 1.2f32.to_bits(),
            font_weight: 400,
            font_family: "test".to_string(),
        },
        TextMetrics::empty(),
    );
    assert_eq!(cache.cached_metrics_len(), entries_before_oversized_key);
}
