use rui::ImageSource;
use rui::renderer::{
    GlyphResourceKey, Primitive, PrimitiveKind, RecordingRenderer, Renderer,
    RendererBatchDiagnostics, RendererDeviceDiagnostics, RendererDiagnostics,
    RendererFramePhaseDurations, RendererImageCache, RendererResourceCache, RendererResourceError,
    RendererResourceKind, RendererResourceLimits, RendererResourceStats, RendererTelemetryRecorder,
    RendererUnsupportedPrimitive,
};
use rui::{Bounds, Color, Corners, Edges, Size};

#[test]
fn renderer_resource_reuses_live_texture_allocations() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Texture, 2, 16);

    let first = match cache.resolve(7_u32, 8) {
        Ok(allocation) => allocation,
        Err(err) => panic!("texture allocation should succeed: {err}"),
    };
    let second = match cache.resolve(7_u32, 8) {
        Ok(allocation) => allocation,
        Err(err) => panic!("texture reuse should succeed: {err}"),
    };

    assert_eq!(first.handle, second.handle);
    assert!(!first.reused);
    assert!(second.reused);
    assert!(second.evicted.is_empty());
    assert_eq!(cache.stats().live_entries, 1);
    assert_eq!(cache.stats().live_bytes, 8);
    assert_eq!(cache.stats().cache_hits, 1);
    assert_eq!(cache.stats().cache_misses, 1);
}

#[test]
fn renderer_resource_pressure_does_not_evict_active_content() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Texture, 1, 8);

    match cache.resolve(1_u32, 8) {
        Ok(_) => {}
        Err(err) => panic!("first texture allocation should succeed: {err}"),
    }

    let err = match cache.resolve(2_u32, 8) {
        Ok(_) => panic!("active texture should not be evicted silently"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        RendererResourceError::ResourcePressure {
            kind: RendererResourceKind::Texture,
            requested_bytes: 8,
            max_bytes: 8,
            active_bytes: 8,
        }
    ));
    assert_eq!(cache.stats().pressure_events, 1);
    assert_eq!(cache.stats().cache_misses, 2);
}

#[test]
fn renderer_resource_evicts_inactive_entries_under_pressure() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Texture, 1, 8);

    match cache.resolve(1_u32, 8) {
        Ok(_) => {}
        Err(err) => panic!("first texture allocation should succeed: {err}"),
    }
    cache.begin_frame();

    let allocation = match cache.resolve(2_u32, 8) {
        Ok(allocation) => allocation,
        Err(err) => panic!("inactive texture should be evictable: {err}"),
    };

    assert_eq!(allocation.evicted, vec![1_u32]);
    assert!(!cache.contains(&1_u32));
    assert!(cache.contains(&2_u32));
    assert_eq!(cache.stats().disposed_entries, 1);
    assert_eq!(cache.stats().evicted_entries, 1);
}

#[test]
fn renderer_resource_image_cache_reuses_and_evicts_data_images() {
    let mut cache = RendererImageCache::with_limits(1, 4);
    let red = ImageSource::Data {
        data: vec![255, 0, 0, 255],
        width: 1,
        height: 1,
    };
    let blue = ImageSource::Data {
        data: vec![0, 0, 255, 255],
        width: 1,
        height: 1,
    };

    let first = match cache.resolve(&red) {
        Ok(entry) => entry,
        Err(err) => panic!("data image should resolve: {err}"),
    };
    let second = match cache.resolve(&red) {
        Ok(entry) => entry,
        Err(err) => panic!("data image should reuse: {err}"),
    };
    assert_eq!(first.handle, second.handle);

    cache.begin_frame();
    let third = match cache.resolve(&blue) {
        Ok(entry) => entry,
        Err(err) => panic!("inactive image should be evictable: {err}"),
    };

    assert_ne!(first.handle, third.handle);
    assert_eq!(cache.stats().live_entries, 1);
    assert_eq!(cache.stats().disposed_entries, 1);
}

#[test]
fn renderer_resource_image_cache_reports_invalid_data_without_placeholder_content() {
    let mut cache = RendererImageCache::new();
    let invalid = ImageSource::Data {
        data: vec![0, 0, 0],
        width: 1,
        height: 1,
    };

    let err = match cache.resolve(&invalid) {
        Ok(_) => panic!("invalid image data should fail"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        RendererResourceError::InvalidResource {
            kind: RendererResourceKind::Image,
            ..
        }
    ));
}

#[test]
fn renderer_resource_errors_expose_structured_context() {
    let invalid = RendererResourceError::invalid(RendererResourceKind::Image, "bad pixels");
    assert_eq!(invalid.kind(), RendererResourceKind::Image);
    assert_eq!(invalid.resource_id(), None);
    assert!(!invalid.is_pressure());

    let missing = RendererResourceError::missing(RendererResourceKind::Texture, 42);
    assert_eq!(missing.kind(), RendererResourceKind::Texture);
    assert_eq!(missing.resource_id(), Some(42));
    assert!(!missing.is_pressure());

    let synthetic_missing = RendererResourceError::missing(RendererResourceKind::Texture, 0);
    assert_eq!(synthetic_missing.resource_id(), None);

    let mut cache = RendererResourceCache::new(RendererResourceKind::Glyph, 0, 8);
    let pressure = match cache.resolve(GlyphResourceKey::new("A", 12.0, 400, None, 1.0), 4) {
        Ok(_) => panic!("zero-entry cache should report resource pressure"),
        Err(err) => err,
    };
    assert_eq!(pressure.kind(), RendererResourceKind::Glyph);
    assert_eq!(pressure.resource_id(), None);
    assert!(pressure.is_pressure());
}

#[test]
fn renderer_diagnostics_reports_resource_totals_by_kind() {
    let diagnostics = RendererDiagnostics::new(
        RendererDeviceDiagnostics::headless("diagnostic-test"),
        vec![
            RendererResourceStats {
                kind: RendererResourceKind::Texture,
                live_entries: 2,
                live_bytes: 96,
                disposed_entries: 1,
                evicted_entries: 1,
                cache_hits: 4,
                cache_misses: 2,
                pressure_events: 0,
            },
            RendererResourceStats {
                kind: RendererResourceKind::Image,
                live_entries: 1,
                live_bytes: 32,
                disposed_entries: 0,
                evicted_entries: 0,
                cache_hits: 3,
                cache_misses: 1,
                pressure_events: 2,
            },
        ],
    );

    let texture = match diagnostics.resource(RendererResourceKind::Texture) {
        Some(stats) => stats,
        None => panic!("texture diagnostics should be available"),
    };
    assert_eq!(texture.live_entries, 2);
    assert_eq!(diagnostics.resource(RendererResourceKind::Glyph), None);
    assert_eq!(diagnostics.total_live_entries(), 3);
    assert_eq!(diagnostics.total_live_bytes(), 128);
    assert_eq!(diagnostics.total_pressure_events(), 2);
    assert_eq!(diagnostics.total_evicted_entries(), 1);
    assert_eq!(diagnostics.total_cache_hits(), 7);
    assert_eq!(diagnostics.total_cache_misses(), 3);
}

#[test]
fn renderer_diagnostics_reports_unsupported_primitive_reasons() {
    let diagnostics = RendererDiagnostics::new(
        RendererDeviceDiagnostics::headless("metal-test"),
        Vec::new(),
    )
    .with_unsupported_primitives(vec![RendererUnsupportedPrimitive::new(
        "metal-test",
        PrimitiveKind::Path,
        "path primitives are not implemented by this backend",
    )]);

    let unsupported = match diagnostics.unsupported_primitive(PrimitiveKind::Path) {
        Some(unsupported) => unsupported,
        None => panic!("path unsupported reason should be reported"),
    };
    assert_eq!(unsupported.backend, "metal-test");
    assert_eq!(
        unsupported.reason,
        "path primitives are not implemented by this backend"
    );
    assert_eq!(diagnostics.unsupported_primitive(PrimitiveKind::Text), None);
}

#[test]
fn renderer_resource_glyphs_have_deterministic_lifetime() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Glyph, 1, 64);
    let title = GlyphResourceKey::new("Title", 14.0, 400, None, 1.2);
    let body = GlyphResourceKey::new("Body", 14.0, 400, None, 1.2);

    match cache.resolve(title.clone(), 32) {
        Ok(_) => {}
        Err(err) => panic!("glyph allocation should succeed: {err}"),
    }
    cache.begin_frame();

    let allocation = match cache.resolve(body.clone(), 32) {
        Ok(allocation) => allocation,
        Err(err) => panic!("inactive glyph should be evictable: {err}"),
    };

    assert_eq!(allocation.evicted, vec![title]);
    assert!(cache.contains(&body));
    assert_eq!(cache.stats().kind, RendererResourceKind::Glyph);
}

#[test]
fn renderer_resource_recording_renderer_reports_headless_diagnostics() {
    let renderer = RecordingRenderer::new();
    let diagnostics = renderer.diagnostics();

    assert_eq!(diagnostics.device.backend, "recording");
    assert!(diagnostics.device.is_headless);
    assert!(diagnostics.resources.is_empty());
    assert!(diagnostics.unsupported_primitives.is_empty());
}

#[test]
fn renderer_resource_limits_default_to_unbounded_cache_capacity() {
    let limits = RendererResourceLimits::default();

    assert_eq!(limits.texture_max_entries, usize::MAX);
    assert_eq!(limits.image_max_bytes, usize::MAX);
    assert_eq!(limits.glyph_max_entries, usize::MAX);
}

#[test]
fn renderer_resource_profile_json_exposes_frame_and_cache_fields() {
    let mut recorder = RendererTelemetryRecorder::new();
    let diagnostics = RendererDiagnostics::new(
        RendererDeviceDiagnostics::headless("profile-test"),
        vec![RendererResourceStats {
            kind: RendererResourceKind::Texture,
            live_entries: 1,
            live_bytes: 64,
            disposed_entries: 0,
            evicted_entries: 1,
            cache_hits: 2,
            cache_misses: 3,
            pressure_events: 1,
        }],
    );
    let mut scene = SceneBuilder::new();
    scene.push_quad();
    scene.push_quad();

    let telemetry = recorder.capture_telemetry(
        "profile-test",
        Size::new(320.0, 200.0),
        RendererFramePhaseDurations {
            render_ns: 20_000_000,
            paint_ns: 700,
            ..RendererFramePhaseDurations::default()
        },
        &diagnostics,
        RendererBatchDiagnostics::from_scene(scene.scene()),
    );
    let json = match serde_json::from_str::<serde_json::Value>(&telemetry.to_json_line()) {
        Ok(value) => value,
        Err(err) => panic!("profile JSON should parse: {err}"),
    };

    assert_eq!(json["schema"], "rui.renderer.profile.v1");
    assert_eq!(json["backend"], "profile-test");
    assert_eq!(json["primitive_count"], 2);
    assert_eq!(json["draw_count"], 2);
    assert_eq!(json["resource_cache_hits"], 2);
    assert_eq!(json["resource_cache_misses"], 3);
    assert_eq!(json["resource_evictions"], 1);
    assert_eq!(json["jank_count"], 1);
}

#[test]
fn renderer_resource_batch_diagnostics_detect_per_primitive_buffer_uploads() {
    let mut scene = SceneBuilder::new();
    scene.push_quad();
    scene.push_quad();
    scene.push_quad();

    let batch = RendererBatchDiagnostics::for_metal_scene(scene.scene());

    assert_eq!(batch.primitive_count, 3);
    assert_eq!(batch.draw_count, 3);
    assert_eq!(batch.batch_count, 1);
    assert_eq!(batch.buffer_allocations, 6);
}

struct SceneBuilder {
    scene: rui::renderer::Scene,
}

impl SceneBuilder {
    fn new() -> Self {
        Self {
            scene: rui::renderer::Scene::new(),
        }
    }

    fn scene(&self) -> &rui::renderer::Scene {
        &self.scene
    }

    fn push_quad(&mut self) {
        self.scene.insert(Primitive::Quad {
            bounds: Bounds::from_xywh(0.0, 0.0, 10.0, 10.0),
            background: Color::rgb(0.1, 0.2, 0.3).to_rgba(),
            border_color: Color::TRANSPARENT.to_rgba(),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        });
    }
}
