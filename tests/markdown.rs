#![cfg(feature = "markdown")]

use rui::advanced_ui::Theme;
use rui::core::event::Cursor;
use rui::core::{Point, Size};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::elements::markdown::markdown_parse_counts;
use rui::elements::{div, markdown};
use rui::renderer::Primitive;
use rui::testing::mount;
use std::cell::RefCell;
use std::rc::Rc;

const SAMPLE: &str = r#"# Release notes

Rui draws **bold**, *italic*, ~~struck~~ and `inline code` in one wrapped paragraph, with a [link](https://example.com/docs) that calls a handler, and ***bold italic*** too.

## Lists

- First item
- Second item with **strong** text
  1. Nested ordered
  2. And another

* [x] Done task
* [ ] Open task

> A quoted paragraph, drawn in a muted color beside a bar.

```rust
fn main() {
    println!("hello");

    let total = 1 + 2;
}
```

---

| Feature | Status |
|:--------|-------:|
| Tables  | done   |
| Images  | done   |

![a screenshot](tests/linux_window/hello_world.png)
"#;

fn texts(primitives: &[Primitive]) -> Vec<String> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.to_string()),
            _ => None,
        })
        .collect()
}

fn joined(primitives: &[Primitive]) -> String {
    texts(primitives).join("")
}

#[test]
fn draws_every_block_kind() {
    let session = mount(Size::new(640.0, 900.0), |_| {
        div().w(640.0).p(16.0).child(markdown(SAMPLE))
    })
    .expect("mount");
    let all = joined(session.primitives());
    for expected in [
        "Release notes",
        "bold",
        "italic",
        "struck",
        "inline code",
        "link",
        "First item",
        "Nested ordered",
        "Done task",
        "A quoted paragraph",
        "println!",
        "Feature",
        "Status",
        "Tables",
    ] {
        assert!(all.contains(expected), "{expected:?} is drawn in {all:?}");
    }
    // Markdown syntax is not drawn.
    for syntax in ["**", "~~", "](", "```", "|"] {
        assert!(!all.contains(syntax), "{syntax:?} is not drawn");
    }
    let markers = texts(session.primitives());
    assert!(markers.iter().any(|text| text == "\u{2022}"));
    assert!(markers.iter().any(|text| text == "1."));
    assert!(
        markers.iter().any(|text| text == "\u{2713}"),
        "the done task is checked"
    );
}

#[test]
fn styles_runs_within_one_paragraph() {
    let session = mount(Size::new(640.0, 400.0), |_| {
        div()
            .w(640.0)
            .child(markdown("plain **bold** *slanted* `code`"))
    })
    .expect("mount");
    let family = |needle: &str| {
        session
            .primitives()
            .iter()
            .find_map(|primitive| match primitive {
                Primitive::Text {
                    content,
                    font_family,
                    font_weight,
                    ..
                } if content.contains(needle) => Some((font_family.clone(), *font_weight)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{needle} is drawn"))
    };
    assert_eq!(family("bold").1, 700);
    assert_eq!(family("slanted").0.as_deref(), Some("Italic"));
    assert_ne!(family("code").0, family("plain").0, "code is monospace");
    // Inline code has a background behind it.
    assert!(
        session
            .primitives()
            .iter()
            .any(|primitive| matches!(primitive, Primitive::Quad { .. }))
    );
}

#[test]
fn a_link_click_calls_the_handler_and_shows_the_pointer() {
    let clicked = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = clicked.clone();
    let mut session = mount(Size::new(400.0, 200.0), move |_| {
        let sink = sink.clone();
        div().w(400.0).child(
            markdown("[open docs](https://example.com) please")
                .on_link(move |url| sink.borrow_mut().push(url.to_string())),
        )
    })
    .expect("mount");
    let bounds = session
        .primitives()
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Text {
                content, bounds, ..
            } if content.contains("open docs") => Some(*bounds),
            _ => None,
        })
        .expect("the link is drawn");
    let on_link = Point::new(bounds.x() + 10.0, bounds.y() + bounds.height() / 2.0);
    let event = |kind| PointerEvent {
        kind,
        position: on_link,
        button: None,
    };
    session.dispatch_pointer_event(event(PointerEventKind::Move));
    assert_eq!(session.cursor(), Cursor::Pointer);
    session.dispatch_pointer_event(event(PointerEventKind::Down));
    session.dispatch_pointer_event(event(PointerEventKind::Up));
    assert_eq!(*clicked.borrow(), ["https://example.com"]);

    // Past the link's text, a click calls nothing.
    session.pointer_down(Point::new(390.0, on_link.y));
    session.pointer_up(Point::new(390.0, on_link.y));
    assert_eq!(clicked.borrow().len(), 1);
}

#[test]
fn a_source_is_parsed_once_and_a_growing_one_from_its_last_block() {
    let source = Rc::new(RefCell::new(String::from(
        "# Title\n\nFirst paragraph.\n\nSecond",
    )));
    let shown = source.clone();
    let before = markdown_parse_counts();
    let mut session = mount(Size::new(400.0, 400.0), move |_| {
        div().w(400.0).child(markdown(shown.borrow().clone()))
    })
    .expect("mount");
    let first = markdown_parse_counts();
    assert_eq!(first.full, before.full + 1);

    // Rebuilding with the same source parses nothing.
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame");
    assert_eq!(markdown_parse_counts(), first);

    // Streamed tokens extend the last block.
    for token in [" paragraph", " grows", ".\n\n- a list"] {
        source.borrow_mut().push_str(token);
        session.app_context_mut().request_rebuild();
        session.frame().expect("frame");
    }
    let streamed = markdown_parse_counts();
    assert_eq!(streamed.full, first.full);
    assert_eq!(streamed.extended, first.extended + 3);
    let all = joined(session.primitives());
    assert!(all.contains("Second paragraph grows."), "{all}");
    assert!(all.contains("a list"), "{all}");
    assert!(all.contains("Title"), "{all}");
}

#[test]
fn draws_in_the_dark_theme() {
    let dark = Theme::dark();
    let session = mount(Size::new(400.0, 200.0), move |_| {
        div().w(400.0).child(markdown("Some *text*").theme(dark))
    })
    .expect("mount");
    let colors: Vec<_> = session
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(!colors.is_empty());
    assert!(
        colors
            .iter()
            .all(|color| *color == dark.colors.text.to_rgba())
    );
}

fn text_bounds(primitives: &[Primitive], needle: &str) -> rui::core::geometry::Bounds {
    primitives
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Text {
                content, bounds, ..
            } if content.contains(needle) => Some(*bounds),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{needle} is drawn"))
}

#[test]
fn spaces_around_inline_code_and_links_stay_visible() {
    let session = mount(Size::new(640.0, 200.0), |_| {
        div()
            .w(640.0)
            .child(markdown("and `code` in, a [link](u) and **bold** end"))
    })
    .expect("mount");
    let primitives = session.primitives();
    let chip = primitives
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Quad { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .expect("inline code has a background");
    let before = text_bounds(primitives, "and ");
    let code = text_bounds(primitives, "code");
    let after = text_bounds(primitives, " in");
    // A text primitive starts at its ink and is as wide as its advance, so
    // `end` is within a pixel of where the next span's pen starts.
    let end = |bounds: rui::core::geometry::Bounds| bounds.x() + bounds.width();
    // "and " ends in a space: the chip starts after it.
    assert!(chip.x() >= end(before) - 1.5, "{before:?} then {chip:?}");
    // " in" starts with a space, about 4 px at 14 px: the chip stops before it.
    assert!(
        after.x() - (chip.x() + chip.width()) >= 3.0,
        "{chip:?} then {after:?}"
    );
    assert!(chip.x() <= code.x() && chip.x() + chip.width() >= end(code) - 1.5);
    // The spaces after a link and bold text stay as wide.
    for (left, right) in [("link", " and "), ("bold", " end")] {
        let (left, right) = (
            text_bounds(primitives, left),
            text_bounds(primitives, right),
        );
        assert!(right.x() - end(left) >= 2.5, "{left:?} then {right:?}");
    }
}

#[test]
fn a_local_image_is_drawn_at_its_own_size_scaled_down_to_fit() {
    // 800 x 600.
    let source = "![shot](tests/linux_window/hello_world.png)";
    let image_bounds = |width: f32| {
        let session = mount(Size::new(1000.0, 900.0), move |_| {
            div().w(width).child(markdown(source))
        })
        .expect("mount");
        session
            .primitives()
            .iter()
            .find_map(|primitive| match primitive {
                Primitive::Image { bounds, .. } => Some(*bounds),
                _ => None,
            })
            .expect("the image is drawn")
    };
    let narrow = image_bounds(400.0);
    assert_eq!((narrow.width(), narrow.height()), (400.0, 300.0));
    let wide = image_bounds(1000.0);
    assert_eq!(
        (wide.width(), wide.height()),
        (800.0, 600.0),
        "never scaled up"
    );
}

#[test]
fn quotes_and_lists_nested_thousands_deep_are_drawn() {
    // A line this short can come from whoever is being replied to.
    for marker in [">", "- ", "1. ", "> - "] {
        let source = format!("{}deep inside", marker.repeat(2000));
        let session = mount(Size::new(640.0, 480.0), move |_| {
            div().w(640.0).child(markdown(source.clone()))
        })
        .expect("mount");
        let all = joined(session.primitives());
        assert!(all.contains("deep inside"), "{marker:?}: {all}");
    }
}

/// `RUI_MARKDOWN_PNG=<path>` writes the sample, drawn by Metal, to `<path>`.
#[cfg(all(target_os = "macos", feature = "metal"))]
#[test]
fn renders_through_metal() {
    use rui::renderer::RendererError;
    use rui::renderer::metal::MetalRenderer;

    let size = Size::new(640.0, 1100.0);
    let theme = Theme::default();
    let session = mount(size, move |_| {
        div()
            .w(640.0)
            .h(1100.0)
            .bg(theme.colors.surface)
            .p(20.0)
            .child(markdown(SAMPLE).theme(theme))
    })
    .expect("mount");
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer: {err}"),
    };
    let pixels = renderer
        .capture_frame_pixels_at_scale(session.scene(), size, 2.0)
        .expect("capture");
    let background = &pixels[0..4];
    assert!(
        pixels.chunks(4).any(|pixel| pixel != background),
        "the document draws something"
    );
    if let Ok(path) = std::env::var("RUI_MARKDOWN_PNG") {
        image::RgbaImage::from_raw(1280, 2200, pixels)
            .expect("rgba")
            .save(path)
            .expect("png");
    }
}
