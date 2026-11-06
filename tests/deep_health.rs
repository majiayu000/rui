use rui::ImageSource;
use rui::advanced_ui::{Button, Hoverable, Tooltip};
use rui::core::accessibility::AccessibilityContext;
use rui::elements::Element;
use rui::renderer::{ImageResourceKey, RendererResourceCache, RendererResourceKind};

#[test]
fn failed_allocation_keeps_inactive_resources() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Texture, 2, 8);
    let original = cache.resolve(1, 4).unwrap().handle;
    cache.resolve(2, 4).unwrap();
    cache.begin_frame();
    cache.resolve(2, 4).unwrap();
    assert!(cache.resolve(3, 8).is_err());
    assert!(
        cache.contains(&1),
        "failed allocation must not silently evict content"
    );
    assert_eq!(cache.stats().live_bytes, 8);
    assert_eq!(cache.stats().evicted_entries, 0);
    assert_eq!(cache.resolve(1, 4).unwrap().handle, original);
}

#[test]
fn oversized_rgba_dimensions_return_an_error() {
    let source = ImageSource::Data {
        data: vec![],
        width: 1 << 31,
        height: 1 << 31,
    };
    assert!(ImageResourceKey::from_source(&source).is_err());
}

#[test]
fn zero_rgba_dimensions_return_an_error() {
    let source = ImageSource::Data {
        data: vec![],
        width: 0,
        height: 1,
    };
    assert!(ImageResourceKey::from_source(&source).is_err());
}

#[test]
fn wrappers_preserve_child_accessibility() {
    let wrappers: Vec<Box<dyn Element>> = vec![
        Box::new(Tooltip::new(Button::new("Action"), "Help")),
        Box::new(Hoverable::new(Button::new("Action"))),
    ];
    for wrapper in wrappers {
        let nodes = wrapper
            .accessibility_nodes(&AccessibilityContext::default())
            .unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_label(), Some("Action"));
    }
}

#[test]
fn cache_pressure_near_usize_limit_returns_error_without_overflow() {
    let mut cache = RendererResourceCache::unbounded(RendererResourceKind::Texture);
    cache.resolve(1, usize::MAX - 1).unwrap();
    assert!(cache.resolve(2, 2).is_err());
    assert!(cache.contains(&1));
    assert_eq!(cache.stats().live_bytes, usize::MAX - 1);
}

#[test]
fn successful_allocation_reports_all_evictions_in_lru_order() {
    let mut cache = RendererResourceCache::new(RendererResourceKind::Texture, 3, 12);
    cache.resolve(1, 4).unwrap();
    cache.resolve(2, 4).unwrap();
    cache.resolve(3, 4).unwrap();
    cache.begin_frame();
    cache.resolve(3, 4).unwrap();
    let replacement = cache.resolve(4, 8).unwrap();
    assert_eq!(replacement.evicted, vec![1, 2]);
    assert!(cache.contains(&3));
    assert!(cache.contains(&4));
    assert_eq!(cache.stats().live_bytes, 12);
}
